//! The one machine-output reader for each turn; fans out through the existing job live buffer.
use super::{blocking, progress::save};
use crate::{
    api::Shared,
    auth::{Directory, Identity},
    machines::{LeaseState, MachineService},
    plugins::jobs::{Job, RunEnd},
    threads::provider_session::ProviderSession,
};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use calternal_core::{
    AgentInput, PROVIDER_EVENT_BYTES, ProviderCommand, ProviderEvent, ProviderEventKind,
};
use calternal_machine::protocol::{ChunkEvent, Stream};
use calternal_types::SpaceId;
use calternal_types::{Capability, Error};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Queue a turn; claim prevents duplicate request retries from starting it twice.
/// Admission is bounded per space. Only placed output readers hold the shared
/// execution budget; offline-machine queues cannot occupy those slots.
pub(crate) fn schedule<D: Directory>(
    state: &Shared<D>,
    machines: &Arc<MachineService>,
    space: String,
    id: String,
) {
    schedule_checked(state, machines, space, id, None);
}

/// Account-bound scheduling for a thread send or an account pending submission.
/// The claim transaction checks native operation identity and account together;
/// result data from a plugin never selects a different job interpreter. Cost:
/// one route-map read and async task, then the existing FULL claim transaction;
/// unknown routes and operation/account mismatches do not start work.
pub(crate) fn schedule_for_account<D: Directory>(
    state: &Shared<D>,
    machines: &Arc<MachineService>,
    space: String,
    id: String,
    account: calternal_types::AccountId,
) {
    schedule_checked(state, machines, space, id, Some(account));
}
fn schedule_checked<D: Directory>(
    state: &Shared<D>,
    machines: &Arc<MachineService>,
    space: String,
    id: String,
    account: Option<calternal_types::AccountId>,
) {
    // Result data may come from a pending plugin operation. Route only known
    // jobs before touching a space file; never create state from a supplied UUID.
    if state.jobs.route(&id).as_deref() != Some(space.as_str()) {
        return;
    }
    let (st, svc) = (state.clone(), machines.clone());
    tokio::spawn(async move {
        let (s, sp, jid) = (st.clone(), space.clone(), id.clone());
        let Ok(Some(claimed)) = blocking(move || s.jobs.claim_turn(&sp, &jid, account)).await
        else {
            return;
        };
        claimed.live.retain_for_subscriber();
        let verify = claimed.job.input["verify"] == true;
        let end = execute(&st, &svc, &claimed.job, &claimed.live)
            .await
            .unwrap_or(RunEnd::OutcomeUnknown);
        let end = match end {
            RunEnd::Interrupted if verify => RunEnd::Failed,
            end => end,
        };
        let (s, sp, jid, run) = (st.clone(), space, id.clone(), claimed.job.runs);
        let waiting = matches!(end, RunEnd::RetryWaiting);
        let failed = matches!(end, RunEnd::Failed);
        let done = matches!(&end, RunEnd::Done(_));
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
            s.jobs.finish(&sp, &jid, run, end)?;
            if !verify {
                s.jobs.agent_revoke_run(&sp, &jid, run)?;
            }
            Ok(())
        })
        .await;
        if waiting && finished.is_ok() {
            super::retry::schedule(&st, &svc, wait_space, wait_id);
        } else if failed
            && !verify
            && finished.is_ok()
            && let Ok(job) = super::load(&st, &id).await
        {
            let _ = crate::provider_sign_in::offer_failed(st, svc, job.session.clone(), &job).await;
        } else if done
            && !verify
            && finished.is_ok()
            && claimed.job.input.get("thread_request").is_some()
        {
            crate::threads::title::schedule(st, svc, claimed.job);
        }
    });
}

async fn execute<D: Directory>(
    state: &Shared<D>,
    machines: &Arc<MachineService>,
    job: &Job,
    live: &Arc<crate::plugins::live::Live>,
) -> Result<RunEnd, Error> {
    let i: AgentInput = serde_json::from_value(job.input["request"].clone())
        .map_err(|_| Error::internal("invalid turn input"))?;
    let account = job.account;
    let space = SpaceId::parse(&job.space).map_err(|_| Error::not_found("turn not found"))?;
    if job.runs == 1 && job.input["thread_request"]["automation_run"].is_object() {
        let (state, job) = (state.clone(), job.clone());
        if !blocking(move || crate::automations::scheduling::check_launch(&state, &job, space))
            .await?
        {
            return Ok(RunEnd::Failed);
        }
    }
    let check = job.input["check"] == true;
    let verify = job.input["verify"] == true;
    let thread = Uuid::parse_str(if check { &job.id } else { &i.thread_id })
        .map_err(|_| Error::invalid_input("invalid thread id"))?;
    let who = Identity { id: job.account };
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
        .snapshot()
        .get(provider_id)
        .ok_or_else(|| Error::unavailable("provider plugin is not installed"))?
        .clone();
    let spec = provider
        .manifest
        .provider
        .as_ref()
        .ok_or_else(|| Error::unavailable("plugin does not provide an adapter"))?;
    let executable = spec.executable.clone();
    let model = if i.model.is_empty() {
        spec.default_model.clone()
    } else {
        i.model.clone()
    };
    let (st, key, p) = (
        state.clone(),
        SpaceId::parse(&job.space).map_err(|_| Error::not_found("turn not found"))?,
        provider.clone(),
    );
    let generation = blocking(move || {
        let store = &st.store;
        let row = crate::plugins::activation(store, key, &p)?;
        if !row.enabled {
            return Err(Error::forbidden("provider plugin is disabled"));
        }
        Ok(row.generation)
    })
    .await?;
    let issuing_run = job.runs;
    let scope = if verify {
        None
    } else {
        Some(super::launch::prepare_authority(state, job, &i, thread).await?)
    };
    let saved_thread = job.input.get("thread_request").is_some();
    let mut phases = saved_thread.then(|| super::phases::Phases::queued(job, live));
    let mut checkpoint = super::lease::prepare(state, machines, job, live, space, thread).await?;
    let lease_id = checkpoint["lease_id"]
        .as_str()
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or_else(|| Error::internal("invalid admitted lease"))?;
    let previous_machine = job
        .checkpoint
        .get("machine_id")
        .and_then(Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok());
    let (machine_id, personal_machine, _permit) = match super::progress::place(
        state,
        machines,
        job,
        live,
        lease_id,
        &mut checkpoint,
    )
    .await?
    {
        super::progress::Placement::Ready(machine, personal, permit) => (machine, personal, permit),
        super::progress::Placement::Ended(end) => return Ok(end),
    };
    checkpoint["placement"] = Value::Null;
    checkpoint["machine_id"] = json!(machine_id);
    if let Some(phases) = &mut phases {
        crate::threads::persistence::bind_provider(&mut checkpoint, &provider.manifest, &model);
        checkpoint["message_machine"] = Value::Null;
        checkpoint["message_id"] = if job.runs == 1 {
            json!(job.id)
        } else {
            json!(Uuid::now_v7())
        };
        phases.switch("starting_sandbox", job, live, &mut checkpoint);
    }
    let mut local_session = if previous_machine == Some(machine_id) {
        checkpoint
            .get("session_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
    } else {
        None
    };
    if job.runs == 1
        && let Some(session) = ProviderSession::owned(&job.input, account)
    {
        match session.resume(provider_id, thread, machine_id) {
            Ok(id) => local_session = Some(id),
            Err(message) => {
                checkpoint["session_notice"] = json!(message);
                live.publish(job.runs, &json!({"type":"notice","message":message}));
            }
        }
    }
    super::lease::publish_placement(
        state,
        job,
        live,
        &mut checkpoint,
        (saved_thread && personal_machine).then_some((thread, machine_id)),
    )
    .await?;
    let mut context = i.context.clone();
    if job.runs > 1 && local_session.is_none() {
        context.push(calternal_core::ProviderContext {
            source: "calternal/checkpoint".into(),
            text: json!({"text_tail":checkpoint["text_tail"],"last_tool":checkpoint["last_tool"]})
                .to_string(),
        });
    }
    if job.runs > 1 {
        let (st, id) = (state.clone(), job.id.clone());
        let outcomes = blocking(move || st.jobs.agent_request_outcomes(account, &id)).await?;
        if !outcomes.is_empty() {
            context.push(calternal_core::ProviderContext {
                source: "calternal/person-request".into(),
                text: json!(outcomes).to_string(),
            });
        }
    }
    let commands = if check {
        vec![ProviderCommand::Check]
    } else {
        let scope = scope
            .as_ref()
            .ok_or_else(|| Error::internal("missing turn authority"))?;
        let instructions = super::instructions::build(state, &i, &job.space, scope).await?;
        if instructions.contains("[Space instructions truncated at 16 KiB.]") {
            live.publish(
                job.runs,
                &json!({"type":"notice","message":"Space instructions truncated at 16 KiB."}),
            );
        }
        vec![
            ProviderCommand::Start {
                model,
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
    let stdin = super::launch::encode(commands)?;
    let (env, mut redactor) = if let Some(scope) = scope {
        let st = state.clone();
        let token = blocking(move || st.jobs.agent_token(&scope, issuing_run)).await?;
        (
            vec![("CALTERNAL_AGENT_TOKEN".into(), token.clone())],
            Some(super::redact::Channels::new(token)),
        )
    } else {
        (Vec::new(), None)
    };
    let mut published_seq = 0;
    let m = machines.clone();
    if !live.is_current(job.runs) || !super::session_live(state, job).await? {
        return Ok(RunEnd::Failed);
    }
    let exec = blocking(move || {
        m.exec_input(
            account,
            lease_id,
            super::launch::argv(executable),
            env,
            stdin,
        )
    })
    .await?;
    let mut cursor = 0;
    let mut pending = Vec::new();
    let mut provider_seq = 0;
    let mut outcome = None;
    let mut checked = false;
    let mut sign_in_command = None;
    let mut failure = None;
    let mut retry_forbidden = false;
    let mut pending_tools = std::collections::HashSet::new();
    let mut text_flush = Instant::now();
    let deadline = Instant::now()
        + if verify {
            super::verify::PHASE_TIMEOUT
        } else {
            Duration::from_secs(600)
        };
    let result = async { loop {
        if !live.is_current(job.runs) || !super::session_live(state, job).await? {
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
            SpaceId::parse(&job.space).map_err(|_| Error::not_found("turn not found"))?,
            provider.clone(),
        );
        let active = blocking(move || {
            let store = &st.store;
            let row = crate::plugins::activation(store, key, &p)?;
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
        if verify && view.state == LeaseState::Running {
            checkpoint["sandbox_started"] = json!(true);
        }
        if view.state == LeaseState::Interrupted {
            let m = machines.clone();
            let since = blocking(move || m.interrupted_at(account, lease_id)).await?;
            checkpoint["interruption"] = json!({"why":"machine_offline","since_ms":since});
            save(state, job, live, &mut checkpoint, None).await?;
            return Ok(RunEnd::Interrupted);
        }
        if Instant::now() >= deadline {
            checkpoint["interruption"] = json!({"why":"execution_timeout"});
            save(state, job, live, &mut checkpoint, None).await?;
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
                        .map_err(|_| Error::internal("invalid machine output"))?;
                    for byte in bytes {
                        pending.push(byte);
                        if pending.len() > PROVIDER_EVENT_BYTES {
                            return Err(Error::invalid_input("provider event exceeds budget"));
                        }
                        if byte != b'\n' {
                            continue;
                        }
                        let mut e: ProviderEvent = serde_json::from_slice(&pending)
                            .map_err(|_| Error::invalid_input("invalid provider event"))?;
                        e.validate()?;
                        pending.clear();
                        if e.version != 1 || e.seq != provider_seq + 1 || outcome.is_some() {
                            return Err(Error::invalid_input("provider event sequence failed"));
                        }
                        provider_seq = e.seq;
                        if verify && !matches!(e.event, ProviderEventKind::Check(_) | ProviderEventKind::Done { .. }) {
                            continue;
                        }
                        let flush = redactor.as_mut()
                            .and_then(|redactor| redactor.event(&mut e.event))
                            .map(|event| ProviderEvent {version: 1, seq: 0, event});
                        for mut e in flush.into_iter().chain(std::iter::once(e)) {
                            let mut thread_session = None;
                            match &e.event {
                                ProviderEventKind::Session { session_id } => {
                                    checkpoint["session_id"] = json!(session_id);
                                    if job.input.get("thread_request").is_some() {
                                        thread_session = Some(ProviderSession::new(
                                            account, provider_id, thread, machine_id, session_id,
                                        ));
                                    }
                                }
                                ProviderEventKind::TextDelta { text } => {
                                    if job.input.get("thread_request").is_some() {
                                        let saved =
                                            checkpoint["thread_text"].as_str().unwrap_or("");
                                        checkpoint["thread_text"] = json!(format!("{saved}{text}"));
                                    }
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
                                ProviderEventKind::ToolStarted { .. }
                                | ProviderEventKind::ToolOutput { .. }
                                | ProviderEventKind::ToolFinished { .. } => {
                                    super::tool_steps::observe(&mut checkpoint, &mut pending_tools, &e.event, saved_thread)?;
                                }
                                ProviderEventKind::Error {
                                    class,
                                    billing,
                                    reset_at_ms,
                                    code,
                                    message,
                                } => {
                                    checkpoint["error"] = json!({
                                        "code":code,"message":message,"class":class,"reset_at_ms":reset_at_ms,
                                        "provider":provider_id,"provider_name":provider.manifest.name,"sign_in_command":sign_in_command,
                                    });
                                    retry_forbidden |= !matches!(
                                        class,
                                        calternal_core::ProviderFailureClass::Transient
                                            | calternal_core::ProviderFailureClass::UsageLimit
                                    );
                                    failure = Some((*class, *billing, *reset_at_ms));
                                }
                                ProviderEventKind::Done { outcome: o } => {
                                    outcome = Some(o.clone());
                                    checkpoint["message_outcome"] = json!(o);
                                }
                                ProviderEventKind::Check(status) => {
                                    checked = true;
                                    if verify {
                                        checkpoint["sandbox_started"] = json!(true);
                                        checkpoint["provider_check"] = json!(status);
                                    }
                                    sign_in_command = Some(status.sign_in_command.clone());
                                    let needs_sign_in = check && status.installed && !status.signed_in;

                                    let (m, provider, status) = (machines.clone(), provider_id.to_owned(), status.clone());
                                    blocking(move || m.record_check(account, lease_id, machine_id, provider, status)).await?;
                                    if needs_sign_in {
                                        let (s, svc, job, provider) = (state.clone(), machines.clone(), job.clone(), provider_id.to_owned());
                                        tokio::spawn(async move {
                                            let _ = crate::provider_sign_in::offer_check(s, svc, &job, machine_id, provider).await;
                                        });
                                    }

                                    // Only the owning person can observe a manual Check job.
                                    // Normal turns record readiness privately without a public chunk.
                                    if !check {
                                        continue;
                                    }
                                }
                                _ => {}
                            }
                            if verify {
                                continue;
                            }
                            if let Some(phases) = &mut phases {
                                phases.observe(&e.event, !pending_tools.is_empty(), job, live, &mut checkpoint)?;
                                if matches!(e.event, ProviderEventKind::Done { .. }) {
                                    phases.finish(&mut checkpoint);
                                }
                            }
                            if matches!(
                                e.event,
                                ProviderEventKind::Session { .. }
                                    | ProviderEventKind::ToolStarted { .. }
                                    | ProviderEventKind::ToolFinished { .. }
                                    | ProviderEventKind::Done { .. }
                            ) || checkpoint["thread_text"]
                                .as_str()
                                .is_some_and(|s| s.len() >= 16 * 1024)
                                || text_flush.elapsed() >= Duration::from_secs(3)
                            {
                                checkpoint["provider_seq"] = json!(published_seq + 1);
                                if let Some(redactor) = &redactor {
                                    redactor.checkpoint(&mut checkpoint);
                                }
                                save(state, job, live, &mut checkpoint, thread_session).await?;
                                text_flush = Instant::now();
                            }
                            published_seq += 1;
                            e.seq = published_seq;
                            let value = serde_json::to_value(&e)
                                .map_err(|_| Error::internal("provider encoding"))?;
                            if !live.publish(job.runs, &value) {
                                return Err(Error::conflict("turn stopped accepting output"));
                            }
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
                            let m = machines.clone();
                            let since = blocking(move || m.interrupted_at(account, lease_id)).await?;
                            checkpoint["interruption"] = json!({"why":"machine_offline","since_ms":since});
                            save(state, job, live, &mut checkpoint, None).await?;
                            return Ok(RunEnd::Interrupted);
                        }
                    }
                    if !pending.is_empty() {
                        return Ok(RunEnd::OutcomeUnknown);
                    }
                    if !verify && outcome.as_deref() == Some("failed")
                        && pending_tools.is_empty()
                        && !retry_forbidden
                        && let Some((class, billing, reset)) = failure
                    {
                        let (st, sp, jid) = (state.clone(), job.space.clone(), job.id.clone());
                        let latest = blocking(move || {
                            st.jobs
                                .get(&sp, &jid)?
                                .ok_or_else(|| Error::not_found("turn not found"))
                        })
                        .await?;
                        let settings: calternal_core::AgentRetrySettings =
                            serde_json::from_value(latest.input["retry_settings"].clone())
                                .map_err(|_| Error::internal("invalid retry settings"))?;
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
                            if let Some(redactor) = &redactor {
                                redactor.checkpoint(&mut checkpoint);
                            }
                            save(state, job, live, &mut checkpoint, None).await?;
                            return Ok(RunEnd::RetryWaiting);
                        }
                    }
                    if let Some(phases) = &mut phases {
                        phases.finish(&mut checkpoint);
                    }
                    checkpoint["provider_seq"] = json!(published_seq);
                    if let Some(redactor) = &redactor {
                        redactor.checkpoint(&mut checkpoint);
                    }
                    save(state, job, live, &mut checkpoint, None).await?;
                    return Ok(if exit.code == Some(0) && check && checked {
                        RunEnd::Done(json!({"checked":true}))
                    } else {
                        match outcome.as_deref() {
                            Some("done") if exit.code == Some(0) => RunEnd::Done(checkpoint.clone()),
                            Some("failed") => RunEnd::Failed,
                            _ => RunEnd::OutcomeUnknown,
                        }
                    });
                }
            }
        }
    }}.await;
    // Preserve scrubbed output on an early exit. A rejected checkpoint must
    // not be retried by the exit path when the document is already full.
    let result = if checkpoint["thread_text"]
        .as_str()
        .is_some_and(|text| !text.is_empty())
        && !result.as_ref().is_err_and(super::progress::thread_full)
    {
        if let Some(phases) = &mut phases {
            phases.finish(&mut checkpoint);
        }
        checkpoint["provider_seq"] = json!(published_seq);
        if let Some(redactor) = &redactor {
            redactor.checkpoint(&mut checkpoint);
        }
        save(state, job, live, &mut checkpoint, None)
            .await
            .and(result)
    } else {
        result
    };
    if let Err(error) = &result
        && super::progress::thread_full(error)
    {
        return super::progress::finish_full(
            state,
            job,
            live,
            &mut checkpoint,
            published_seq,
            error,
        )
        .await;
    }
    result
}
