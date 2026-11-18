//! Runs admitted plugin jobs on their (plugin, space) worker.
//!
//! A run invokes the job's declared run operation with
//! `{ job_id, input, checkpoint, children }`, where `checkpoint` is the last durable
//! checkpoint (null on the first run) and `children` the reports of finished child
//! jobs. The operation answers `{ "status": "done", "result": … }` or
//! `{ "status": "waiting" }` (parked until all its children have reported); anything
//! else fails the job. Every host call, chunk and the completion are checked against
//! the starting account's sign-in, its current capability in the space, the plugin's
//! activation at the admitted generation, and the run still being current.
//!
//! Bounds: at most [`MAX_RUNNING`] runs at once across the server (others wait for a
//! permit), each with a [`JOB_RUN_TIMEOUT`] deadline. A waiting task holds no
//! worker, only its place in the semaphore queue, which the per-space active-job
//! bound limits.

use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::time::Duration;

use calternal_core::{ManifestIdempotency, ManifestV1};
use serde_json::{Value, json};

use super::broker::{Run, SpaceHost};
use super::jobs::RunEnd;
use super::worker::Outcome;
use super::{Call, activation, host_capability};
use crate::api::Shared;
use crate::auth::{Directory, Identity};
use calternal_types::{Error, SpaceId};

/// Concurrent job runs across the server. Kept below the worker pool size so
/// interactive plugin calls always find a worker.
pub const MAX_RUNNING: usize = 4;
/// Deadline of one run invocation; a job that needs longer checkpoints and is resumed.
pub const JOB_RUN_TIMEOUT: Duration = Duration::from_secs(600);

/// Whether a job's run operation may be repeated after a restart: everything except
/// `idempotency: "never"`, which ends `outcome_unknown` instead. Cost: O(operations).
pub fn resumable(manifest: &ManifestV1, operation: &str) -> bool {
    manifest
        .operations
        .iter()
        .find(|op| op.id == operation)
        .is_some_and(|op| op.idempotency != ManifestIdempotency::Never)
}

/// Queue a run of an admitted job. Returns at once; the run waits for a permit.
/// Must be called inside the async runtime. Cost: one task spawn.
pub fn schedule<D: Directory>(state: &Shared<D>, space: String, job: String) {
    let state = state.clone();
    tokio::spawn(async move {
        let Ok(_permit) = state.jobs.permits().acquire_owned().await else {
            return;
        };
        if let Err(e) = run(&state, &space, &job).await {
            tracing::warn!(job = %job, error = %e.message, "job run failed");
        }
    });
}

/// After a restart: mark interrupted runs for resumption (or `outcome_unknown`) and
/// schedule every admitted job. Call once at startup inside the runtime.
/// Cost: see [`super::jobs::Jobs::recover`], on a blocking task.
pub async fn recover<D: Directory>(state: &Shared<D>) -> Result<usize, Error> {
    let st = state.clone();
    let woken = crate::blocking::spawn(move || {
        let catalog = st.registry.snapshot();
        st.jobs.recover(|plugin, operation, revision| {
            let entry = if revision == 0 {
                catalog.get(plugin)
            } else {
                catalog.revision(plugin, revision)
            };
            entry.is_some_and(|p| resumable(&p.manifest, operation))
        })
    })
    .await
    .map_err(|_| Error::internal("job recovery task failed"))??;
    let n = woken.len();
    for (space, job) in woken {
        schedule(state, space, job);
    }
    Ok(n)
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    crate::blocking::spawn(f)
        .await
        .map_err(|_| Error::internal("job task failed"))?
}

/// One run: claim, recheck authority, invoke, record the end, wake what it unblocks.
async fn run<D: Directory>(state: &Shared<D>, space: &str, id: &str) -> Result<(), Error> {
    let (st, sp, jid) = (state.clone(), space.to_owned(), id.to_owned());
    let Some(super::jobs::Claimed { job, live, cancel }) =
        blocking(move || st.jobs.claim(&sp, &jid)).await?
    else {
        return Ok(());
    };
    let end = match execute(state, &job, &live, &cancel).await {
        Ok(Some(end)) => Some(end),
        Ok(None) => None,
        Err(e) => {
            tracing::debug!(job = %job.id, error = %e.message, "job run refused");
            Some(RunEnd::Failed)
        }
    };
    let (st, sp, jid, run) = (state.clone(), space.to_owned(), id.to_owned(), job.runs);
    let wake = match end {
        Some(end) => blocking(move || st.jobs.finish(&sp, &jid, run, end)).await?,
        None => vec![],
    };
    for job in wake {
        schedule(state, space.to_owned(), job);
    }
    Ok(())
}

/// Invoke the run operation. `Ok(None)` means the run was cancelled: its end is
/// already recorded by the cancellation and its late output is discarded.
async fn execute<D: Directory>(
    state: &Shared<D>,
    job: &super::jobs::Job,
    live: &Arc<super::live::Live>,
    cancel: &Arc<super::worker::CancelFlag>,
) -> Result<Option<RunEnd>, Error> {
    let refused = || Error::forbidden("job authority no longer holds");
    let plugin = state
        .registry
        .snapshot()
        .revision(&job.plugin, job.package_revision)
        .ok_or_else(refused)?
        .clone();
    let required = plugin
        .operation(&job.operation)
        .and_then(|op| host_capability(op.required_capability))
        .ok_or_else(refused)?;
    let space = SpaceId::parse(&job.space).map_err(|_| refused())?;
    let identity = Identity { id: job.account };
    if !job.session.is_live_in(state).await {
        return Err(refused());
    }
    let (st, account, p2, jid, sp) = (
        state.clone(),
        job.account,
        plugin.clone(),
        job.id.clone(),
        job.space.clone(),
    );
    let (generation, children) = blocking(move || {
        if !st
            .directory
            .capabilities(account, space)
            .is_some_and(|c| c.contains(required))
        {
            return Err(refused());
        }
        let row = activation(&st.store, space, &p2)?;
        Ok((row, st.jobs.children(&sp, &jid)?))
    })
    .await?;
    if !generation.enabled
        || generation.generation != job.generation
        || plugin.job_revision() != job.package_revision
    {
        return Err(refused());
    }
    let host = SpaceHost::new(
        state.clone(),
        identity,
        job.session.clone(),
        space,
        plugin.clone(),
        generation.generation,
        format!("job/{}", job.id),
    )
    .for_run(Run {
        job: job.id.clone(),
        run: job.runs,
        live: live.clone(),
        checkpoints: AtomicU32::new(job.checkpoints),
    });
    let input = json!({
        "job_id": job.id,
        "input": job.input,
        "checkpoint": job.checkpoint,
        "children": children,
    });
    let outcome = state
        .workers
        .invoke(
            &plugin,
            space,
            Call {
                operation: job.operation.clone(),
                input,
                account: job.account,
                host: Arc::new(host),
                timeout: JOB_RUN_TIMEOUT,
                cancel: Some(cancel.clone()),
                job: Some(job.id.clone()),
            },
        )
        .await;
    Ok(match outcome {
        Outcome::Cancelled => None,
        Outcome::Complete(value) => Some(end_of(value)),
        _ => Some(RunEnd::Failed),
    })
}

/// Interpret a run operation's answer.
fn end_of(value: Value) -> RunEnd {
    match value.get("status").and_then(Value::as_str) {
        Some("done") => RunEnd::Done(value.get("result").cloned().unwrap_or(Value::Null)),
        Some("waiting") => RunEnd::Waiting,
        _ => RunEnd::Failed,
    }
}
