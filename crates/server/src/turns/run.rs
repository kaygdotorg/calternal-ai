//! The one machine-output reader for each turn; fans out through the existing job live buffer.
use super::{access::Authority, blocking};
use crate::{
    api::Shared,
    auth::{Directory, Identity},
    authz::Capability,
    error::HostError,
    machines::{LeaseState, MachineService},
    plugins::jobs::{Job, RunEnd},
};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use calternal_auth::{AccountId, SpaceId};
use calternal_core::{
    AgentInput, PROVIDER_COMMAND_BYTES, PROVIDER_EVENT_BYTES, ProviderCommand, ProviderEvent,
    ProviderEventKind, ProviderRequest,
};
use calternal_machine::protocol::{ChunkEvent, Stream};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Queue a turn; claim prevents duplicate request retries from starting it twice.
/// Bounded by the per-space active-job limit and existing job execution permits.
pub(super) fn schedule<D: Directory>(
    state: &Shared<D>,
    machines: &Arc<MachineService>,
    space: String,
    id: String,
) {
    let (st, svc) = (state.clone(), machines.clone());
    tokio::spawn(async move {
        let Ok(_permit) = st.jobs.permits().acquire_owned().await else {
            return;
        };
        let (s, sp, jid) = (st.clone(), space.clone(), id.clone());
        let Ok(Some(claimed)) = blocking(move || s.jobs.claim(&sp, &jid)).await else {
            return;
        };
        claimed.live.retain_for_polling();
        let end = execute(&st, &svc, &claimed.job, &claimed.live)
            .await
            .unwrap_or(RunEnd::OutcomeUnknown);
        let (s, sp, jid, run) = (st.clone(), space, id.clone(), claimed.job.runs);
        let waiting = matches!(end, RunEnd::RetryWaiting);
        if !waiting {
            if let Ok(latest) = super::load(&st, &id).await
                && latest.runs == claimed.job.runs
            {
                let _ = super::stop_machine(&svc, &latest).await;
            }
        }
        let wait_space = sp.clone();
        let wait_id = jid.clone();
        let finished = blocking(move || {
            s.jobs.agent_revoke_run(&sp, &jid, run)?;
            s.jobs.finish(&sp, &jid, run, end).map(|_| ())
        })
        .await;
        if waiting && finished.is_ok() {
            super::retry::schedule(&st, &svc, wait_space, wait_id);
        }
    });
}

async fn execute<D: Directory>(
    state: &Shared<D>,
    machines: &Arc<MachineService>,
    job: &Job,
    live: &Arc<crate::plugins::live::Live>,
) -> Result<RunEnd, HostError> {
    let i: AgentInput = serde_json::from_value(job.input["request"].clone())
        .map_err(|_| HostError::internal("invalid turn input"))?;
    let account =
        AccountId::parse(&job.account).ok_or_else(|| HostError::not_found("turn not found"))?;
    let space = SpaceId::parse(&job.space).ok_or_else(|| HostError::not_found("turn not found"))?;
    let thread =
        Uuid::parse_str(&i.thread_id).map_err(|_| HostError::invalid_input("invalid thread id"))?;
    let who = Identity {
        id: Arc::from(job.account.as_str()),
    };
    super::authorize(state, &who, &job.space, Capability::PluginsUse).await?;
    if super::enabled_generation(state, &job.space, &job.plugin).await? != job.generation {
        return Ok(RunEnd::Failed);
    }
    let provider_id = if i.provider_id.is_empty() {
        "calternal-claude-code"
    } else {
        &i.provider_id
    };
    let provider = state
        .registry
        .get(provider_id)
        .ok_or_else(|| HostError::unavailable("provider plugin is not installed"))?
        .clone();
    let spec = provider
        .manifest
        .provider
        .as_ref()
        .ok_or_else(|| HostError::unavailable("plugin does not provide an adapter"))?;
    let executable = spec.executable.clone();
    let default_model = spec.default_model.clone();
    let (st, key, p) = (
        state.clone(),
        state.tokens.content_space(&job.space),
        provider.clone(),
    );
    let generation = blocking(move || {
        let store = st
            .store
            .lock()
            .map_err(|_| HostError::internal("store poisoned"))?;
        let row = crate::plugins::activation(&store, &key, &p)?;
        if !row.enabled {
            return Err(HostError::forbidden("provider plugin is disabled"));
        }
        Ok(row.generation)
    })
    .await?;
    let (s, sp, id, a) = (
        state.clone(),
        job.space.clone(),
        job.id.clone(),
        Authority {
            account: job.account.clone(),
            space: job.space.clone(),
            thread: i.thread_id.clone(),
            job: job.id.clone(),
            mode: i.mode,
            source_app: i.source_app.clone(),
            grants: vec![],
        },
    );
    let issuing_run = job.runs;
    let scope = blocking(move || {
        let scope = if s.jobs.get(&sp, &id)?.is_some_and(|j| j.runs > 1) {
            s.jobs.agent_scope(&sp, &id)?
        } else {
            a
        };
        s.jobs.agent_prepare_scope(&scope, issuing_run)
    })
    .await?;
    let (m, t) = (machines.clone(), thread);
    let lease = blocking(move || m.request_run(space, account, t, None)).await?;
    let lease_id = lease.lease_id;
    let mut checkpoint = job.checkpoint.clone();
    if !checkpoint.is_object() {
        checkpoint = json!({});
    }
    checkpoint["lease_id"] = json!(lease_id);
    if let Err(e) = save(state, job, live, &checkpoint).await {
        let m = machines.clone();
        let _ = blocking(move || m.release_run(account, lease_id)).await;
        return Err(e);
    }
    let previous_machine = job
        .checkpoint
        .get("machine_id")
        .and_then(Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok());
    let placement_deadline = Instant::now() + Duration::from_secs(600);
    let mut queued = None;
    let machine_id = loop {
        if !live.is_current(job.runs) {
            return Ok(RunEnd::Failed);
        }
        super::authorize(state, &who, &job.space, Capability::PluginsUse).await?;
        let m = machines.clone();
        let view = blocking(move || m.run(account, lease_id)).await?;
        if view.state == LeaseState::Interrupted {
            return Ok(RunEnd::Interrupted);
        }
        if view.state == LeaseState::Ended {
            return Ok(RunEnd::OutcomeUnknown);
        }
        if let Some(machine) = view.machine_id {
            break machine;
        }
        if queued != view.queue_position {
            queued = view.queue_position;
            live.publish(
                job.runs,
                &json!({"type":"placement","queue_position":queued}),
            );
        }
        if Instant::now() >= placement_deadline {
            return Ok(RunEnd::Interrupted);
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    };
    checkpoint["machine_id"] = json!(machine_id);
    save(state, job, live, &checkpoint).await?;
    let local_session = if previous_machine == Some(machine_id) {
        checkpoint
            .get("session_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
    } else {
        None
    };
    let mut context = i.context.clone();
    if job.runs > 1 && local_session.is_none() {
        context.push(calternal_core::ProviderContext {
            source: "calternal/checkpoint".into(),
            text: json!({"text_tail":checkpoint["text_tail"],"last_tool":checkpoint["last_tool"]})
                .to_string(),
        });
    }
    let check = job.input["check"] == true;
    let instructions = super::instructions::build(state, &i, &job.space, &scope).await?;
    if instructions.contains("[Space instructions truncated at 16 KiB.]") {
        live.publish(
            job.runs,
            &json!({"type":"notice","message":"Space instructions truncated at 16 KiB."}),
        );
    }
    let commands = if check {
        vec![ProviderCommand::Check]
    } else {
        vec![
            ProviderCommand::Start {
                model: if i.model.is_empty() {
                    default_model
                } else {
                    i.model
                },
                options: i.options,
                cwd: ".".into(),
                instructions,
                resume: local_session.clone(),
            },
            ProviderCommand::Send {
                prompt: if job.runs > 1 && local_session.is_some() {
                    "Continue the interrupted task from the recorded session. Inspect uncertain effects; do not repeat completed actions.".into()
                } else if job.runs > 1 {
                    format!(
                        "Original task:\n{}\nContinue from the attached checkpoint. Inspect uncertain effects before acting; do not repeat completed actions.",
                        i.prompt
                    )
                } else {
                    i.prompt
                },
                context,
            },
        ]
    };
    let mut stdin = String::new();
    for (seq, command) in commands.into_iter().enumerate() {
        let line = serde_json::to_string(&ProviderRequest {
            version: 1,
            seq: seq as u64 + 1,
            command,
        })
        .map_err(|_| HostError::internal("provider encoding"))?;
        if line.len() + 1 > PROVIDER_COMMAND_BYTES {
            return Err(HostError::invalid_input(
                "provider instructions exceed command budget",
            ));
        }
        stdin.push_str(&line);
        stdin.push('\n');
    }
    let (st, scope) = (state.clone(), scope.clone());
    let token = blocking(move || st.jobs.agent_token(&scope, issuing_run)).await?;
    let (m, env) = (
        machines.clone(),
        vec![("CALTERNAL_AGENT_TOKEN".into(), token)],
    );
    if !live.is_current(job.runs) {
        return Ok(RunEnd::Failed);
    }
    let exec =
        blocking(move || m.exec_input(account, lease_id, vec![executable], env, stdin)).await?;
    let mut cursor = 0;
    let mut pending = Vec::new();
    let mut provider_seq = 0;
    let mut outcome = None;
    let mut checked = false;
    let mut failure = None;
    let mut retry_forbidden = false;
    let mut pending_tools = std::collections::HashSet::new();
    let mut text_flush = Instant::now();
    let deadline = Instant::now() + Duration::from_secs(600);
    loop {
        if !live.is_current(job.runs) {
            return Ok(RunEnd::Failed);
        }
        if super::authorize(state, &who, &job.space, Capability::PluginsUse)
            .await
            .is_err()
        {
            let m = machines.clone();
            let _ = blocking(move || m.release_run(account, lease_id)).await;
            return Ok(RunEnd::Failed);
        }
        let (st, key, p) = (
            state.clone(),
            state.tokens.content_space(&job.space),
            provider.clone(),
        );
        let active = blocking(move || {
            let store = st
                .store
                .lock()
                .map_err(|_| HostError::internal("store poisoned"))?;
            let row = crate::plugins::activation(&store, &key, &p)?;
            Ok(row.enabled && row.generation == generation)
        })
        .await?;
        if !active
            || super::enabled_generation(state, &job.space, &job.plugin).await? != job.generation
        {
            return Ok(RunEnd::Failed);
        }
        let (m, l) = (machines.clone(), lease_id);
        let view = blocking(move || m.run(account, l)).await?;
        if view.state == LeaseState::Interrupted {
            return Ok(RunEnd::Interrupted);
        }
        if Instant::now() >= deadline {
            let m = machines.clone();
            let _ = blocking(move || m.release_run(account, lease_id)).await;
            return Ok(RunEnd::Interrupted);
        }
        let output = machines
            .read_output(account, exec, cursor, Duration::from_secs(1))
            .await?;
        for chunk in output.chunks {
            cursor = chunk.seq;
            match chunk.event {
                ChunkEvent::Data {
                    stream: Stream::Stdout,
                    data,
                } => {
                    let bytes = B64
                        .decode(data)
                        .map_err(|_| HostError::internal("invalid machine output"))?;
                    for byte in bytes {
                        pending.push(byte);
                        if pending.len() > PROVIDER_EVENT_BYTES {
                            return Err(HostError::invalid_input("provider event exceeds budget"));
                        }
                        if byte != b'\n' {
                            continue;
                        }
                        let e: ProviderEvent = serde_json::from_slice(&pending)
                            .map_err(|_| HostError::invalid_input("invalid provider event"))?;
                        e.validate()?;
                        pending.clear();
                        if e.version != 1 || e.seq != provider_seq + 1 || outcome.is_some() {
                            return Err(HostError::invalid_input("provider event sequence failed"));
                        }
                        provider_seq = e.seq;
                        match &e.event {
                            ProviderEventKind::Session { session_id } => {
                                checkpoint["session_id"] = json!(session_id)
                            }
                            ProviderEventKind::TextDelta { text } => {
                                let tail = checkpoint
                                    .get("text_tail")
                                    .and_then(Value::as_str)
                                    .unwrap_or("");
                                let merged = format!("{tail}{text}");
                                let mut start = merged.len().saturating_sub(16 * 1024);
                                while !merged.is_char_boundary(start) {
                                    start += 1;
                                }
                                checkpoint["text_tail"] = json!(&merged[start..]);
                            }
                            ProviderEventKind::ToolStarted { id, name, summary } => {
                                checkpoint["last_tool"] = json!({"id":id,"name":name,"summary":summary,"state":"started"});
                                pending_tools.insert(id.clone());
                            }
                            ProviderEventKind::ToolFinished { id, failed, .. } => {
                                checkpoint["last_tool"]["state"] = json!("finished");
                                checkpoint["last_tool"]["failed"] = json!(failed);
                                pending_tools.remove(id);
                            }
                            ProviderEventKind::Error {
                                class,
                                billing,
                                reset_at_ms,
                                ..
                            } => {
                                retry_forbidden |= !matches!(
                                    class,
                                    calternal_core::ProviderFailureClass::Transient
                                        | calternal_core::ProviderFailureClass::UsageLimit
                                );
                                failure = Some((*class, *billing, *reset_at_ms));
                            }
                            ProviderEventKind::Done { outcome: o } => outcome = Some(o.clone()),
                            ProviderEventKind::Check { .. } => checked = true,
                            _ => {}
                        }
                        let value = serde_json::to_value(&e)
                            .map_err(|_| HostError::internal("provider encoding"))?;
                        if !live.publish(job.runs, &value) {
                            return Err(HostError::conflict("turn stopped accepting output"));
                        }
                        if matches!(
                            e.event,
                            ProviderEventKind::Session { .. }
                                | ProviderEventKind::ToolFinished { .. }
                                | ProviderEventKind::Done { .. }
                        ) || text_flush.elapsed() >= Duration::from_secs(3)
                        {
                            checkpoint["provider_seq"] = json!(provider_seq);
                            save(state, job, live, &checkpoint).await?;
                            text_flush = Instant::now();
                        }
                    }
                }
                ChunkEvent::Data { .. } => {} // Never forward adapter stderr.
                ChunkEvent::Exit(exit) => {
                    if exit.error.is_some() {
                        let m = machines.clone();
                        if blocking(move || m.run(account, lease_id)).await?.state
                            == LeaseState::Interrupted
                        {
                            return Ok(RunEnd::Interrupted);
                        }
                    }
                    if !pending.is_empty() {
                        return Ok(RunEnd::OutcomeUnknown);
                    }
                    if outcome.as_deref() == Some("failed")
                        && pending_tools.is_empty()
                        && !retry_forbidden
                        && let Some((class, billing, reset)) = failure
                    {
                        let (st, sp, jid) = (state.clone(), job.space.clone(), job.id.clone());
                        let latest = blocking(move || {
                            st.jobs
                                .get(&sp, &jid)?
                                .ok_or_else(|| HostError::not_found("turn not found"))
                        })
                        .await?;
                        let settings: calternal_core::AgentRetrySettings =
                            serde_json::from_value(latest.input["retry_settings"].clone())
                                .map_err(|_| HostError::internal("invalid retry settings"))?;
                        let completed = job.checkpoint["retry"]["retry_count"]
                            .as_u64()
                            .unwrap_or(0)
                            .min(u32::MAX as u64) as u32;
                        if let Some(wait) = super::retry::next(
                            settings,
                            class,
                            billing,
                            reset,
                            completed,
                            crate::plugins::job_sql::now_ms(),
                        ) {
                            checkpoint["retry"] = wait;
                            save(state, job, live, &checkpoint).await?;
                            return Ok(RunEnd::RetryWaiting);
                        }
                    }
                    return Ok(if exit.code == Some(0) && check && checked {
                        RunEnd::Done(json!({"checked":true}))
                    } else {
                        match outcome.as_deref() {
                            Some("done") if exit.code == Some(0) => RunEnd::Done(checkpoint),
                            Some("failed") => RunEnd::Failed,
                            _ => RunEnd::OutcomeUnknown,
                        }
                    });
                }
            }
        }
    }
}

async fn save<D: Directory>(
    state: &Shared<D>,
    job: &Job,
    live: &Arc<crate::plugins::live::Live>,
    checkpoint: &Value,
) -> Result<(), HostError> {
    let (st, sp, id, run, live, checkpoint) = (
        state.clone(),
        job.space.clone(),
        job.id.clone(),
        job.runs,
        live.clone(),
        checkpoint.clone(),
    );
    blocking(move || {
        st.jobs
            .checkpoint(&sp, &id, run, &live, &checkpoint)
            .map(|_| ())
    })
    .await
}
