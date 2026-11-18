//! Private machine process, bounded challenge framing, and unconditional teardown.
use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_core::ProviderCheck;
use calternal_machine::protocol::{ChunkEvent, Stream};
use calternal_types::{AccountId, Error};
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::{
    authorize, blocking,
    inbox::{Prompt, TTL, Target},
};
use crate::{
    api::Shared,
    auth::Directory,
    machines::{LeaseState, MachineService},
    plugins::Session,
};

const PROMPT_BYTES: usize = 4096;
const POLL: Duration = Duration::from_secs(1);

#[allow(clippy::too_many_arguments)]
pub(super) async fn run<D: Directory>(
    state: Shared<D>,
    machines: Arc<MachineService>,
    account: AccountId,
    session: Session,
    target: Target,
    id: Uuid,
    executable: String,
    answer: mpsc::Receiver<String>,
) {
    let (space, machine) = (target.space, target.machine);
    let notice = if let Some(thread) = target.thread {
        let s = state.clone();
        let label = target.label.clone();
        blocking(move || {
            if !super::allowed(&s, account, space) {
                return Err(failed());
            }
            crate::threads::provider_notice::append(&s, space, thread, account, id, &label)
        })
        .await
        .ok()
    } else {
        None
    };
    let svc = machines.clone();
    let lease = blocking(move || svc.request_sign_in(space, account, machine)).await;
    let done = match lease {
        Ok(lease) => {
            let lease = lease.lease_id;
            let result = tokio::time::timeout(
                TTL,
                relay(
                    &state, &machines, account, &session, &target, id, lease, executable, answer,
                ),
            )
            .await;
            let done = matches!(result, Ok(Ok(())));
            let svc = machines.clone();
            let _ = blocking(move || svc.release_probe(account, lease)).await;
            done
        }
        Err(_) => false,
    };
    let (job, thread) = state
        .inputs
        .sign_ins
        .access(account, |entries| {
            Ok(entries
                .iter_mut()
                .find(|e| e.id == id)
                .map_or((None, target.thread), |entry| {
                    entry.prompt = None;
                    entry.outcome = Some(done);
                    (entry.job.take(), entry.target.thread)
                }))
        })
        .unwrap_or((None, target.thread));
    let s = state.clone();
    let job_id = job.clone();
    let label = target.label;
    let should_resume = blocking(move || {
        if !super::allowed(&s, account, space) {
            return Ok(false);
        }
        if let Some(thread) = thread {
            let notice = notice.or_else(|| {
                crate::threads::provider_notice::append(&s, space, thread, account, id, &label).ok()
            });
            if done && notice.is_some() {
                let _ = crate::threads::provider_notice::complete(&s, space, thread, account, id);
            }
        }
        if let Some(job) = job_id.filter(|_| done) {
            return s.jobs.resume_signed_out(&space.to_string(), &job, account);
        }
        Ok(false)
    })
    .await
    .unwrap_or(false);
    invalidate(&state, account).await;
    if should_resume && let Some(job) = job {
        crate::turns::schedule_thread(&state, &machines, space.to_string(), job, account);
    }
}

#[allow(clippy::too_many_arguments)]
async fn relay<D: Directory>(
    state: &Shared<D>,
    machines: &Arc<MachineService>,
    account: AccountId,
    session: &Session,
    target: &Target,
    id: Uuid,
    lease: Uuid,
    executable: String,
    mut answer: mpsc::Receiver<String>,
) -> Result<(), Error> {
    loop {
        current(state, account, session, target).await?;
        let svc = machines.clone();
        let run = blocking(move || svc.run(account, lease)).await?;
        match run.state {
            LeaseState::Running => break,
            LeaseState::Waiting | LeaseState::Assigned => tokio::time::sleep(POLL).await,
            _ => return Err(failed()),
        }
    }
    let svc = machines.clone();
    let exec =
        blocking(move || svc.sign_in_exec(account, lease, vec![executable, "--sign-in".into()]))
            .await?;
    let mut cursor = 0;
    let mut frame = Vec::new();
    let mut prompted = false;
    loop {
        current(state, account, session, target).await?;
        tokio::select! {
            code = answer.recv() => {
                let mut code = code.ok_or_else(failed)?.into_bytes();
                code.push(b'\n');
                let svc = machines.clone();
                blocking(move || svc.interactive_input(account, exec, code)).await?;
            }
            batch = machines.read_output(account, exec, cursor, POLL) => {
                let batch = batch?;
                if batch.lost_before.is_some() {
                    return Err(failed());
                }
                cursor = batch.next;
                for chunk in batch.chunks {
                    if let ChunkEvent::Data { stream: Stream::Stdout, data } = chunk.event {
                        if prompted { return Err(failed()); }
                        let bytes = STANDARD.decode(data).map_err(|_| failed())?;
                        if frame.len() + bytes.len() > PROMPT_BYTES { return Err(failed()); }
                        frame.extend(bytes);
                        if frame.ends_with(b"\n") {
                            let prompt = parse(&frame)?;
                            frame.clear();
                            prompted = true;
                            state.inputs.sign_ins.access(account, |entries| {
                                let entry = entries.iter_mut().find(|e| e.id == id).ok_or_else(failed)?;
                                entry.prompt = Some(prompt);
                                Ok(())
                            })?;
                            invalidate(state, account).await;
                        }
                    }
                }
                if let Some(exit) = batch.exit {
                    if exit.code != Some(0) || !frame.is_empty() { return Err(failed()); }
                    let svc = machines.clone();
                    let (machine, provider) = (target.machine, target.provider.clone());
                    return blocking(move || svc.record_check(account, lease, machine, provider, ProviderCheck{
                        installed:true, signed_in:true, sign_in_command: "claude auth login".into(),
                    })).await;
                }
            }
        }
    }
}

fn parse(bytes: &[u8]) -> Result<Prompt, Error> {
    let prompt: Prompt = serde_json::from_slice(bytes).map_err(|_| failed())?;
    let valid_url = |raw: &str| {
        raw.parse::<axum::http::Uri>().is_ok_and(|url| {
            url.scheme_str() == Some("https")
                && matches!(
                    url.authority().map(|a| a.as_str()),
                    Some(
                        "claude.ai"
                            | "claude.com"
                            | "platform.claude.com"
                            | "console.anthropic.com"
                    )
                )
                && url.path() == "/oauth/authorize"
        })
    };
    if !prompt.url.as_deref().is_some_and(valid_url)
        || prompt.display_code.as_deref().is_some_and(|code| {
            code.len() > 64 || !code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        return Err(failed());
    }
    Ok(prompt)
}

async fn current<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    session: &Session,
    target: &Target,
) -> Result<(), Error> {
    if !session.is_live_in(state).await {
        return Err(failed());
    }
    authorize(state, account, target.space).await
}

async fn invalidate<D: Directory>(state: &Shared<D>, account: AccountId) {
    let s = state.clone();
    let _ = blocking(move || s.inputs.invalidate(account)).await;
}

fn failed() -> Error {
    Error::unavailable("Provider sign-in did not finish.")
}
