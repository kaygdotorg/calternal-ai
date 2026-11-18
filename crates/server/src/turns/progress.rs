//! Persist provider output and its fenced durable checkpoint.
use super::blocking;
use crate::{
    api::Shared,
    auth::Directory,
    plugins::{jobs::Job, live::Live},
    threads::provider_session::ProviderSession,
};
use crate::{
    auth::Identity,
    machines::{LeaseState, MachineService},
    plugins::jobs::RunEnd,
};
use calternal_types::{Capability, Error};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Save scrubbed output through content and the existing jobs checkpoint.
/// A reason-only save has no content I/O and one fenced FULL jobs update.
pub(super) async fn save<D: Directory>(
    state: &Shared<D>,
    job: &Job,
    live: &Arc<Live>,
    checkpoint: &mut Value,
    thread_session: Option<ProviderSession>,
) -> Result<(), Error> {
    let (st_output, output_job, output_checkpoint) =
        (state.clone(), job.clone(), checkpoint.clone());
    blocking(move || {
        crate::threads::persistence::checkpoint(
            &st_output,
            &output_job,
            &output_checkpoint,
            thread_session,
        )
    })
    .await?;
    checkpoint["thread_text"] = json!("");
    checkpoint
        .as_object_mut()
        .expect("prepared checkpoint object")
        .remove("message_timing");
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

/// Identify a rejected thread checkpoint that would exceed the document limit.
pub(super) fn thread_full(error: &Error) -> bool {
    error
        .details
        .as_ref()
        .is_some_and(|details| details["reason"] == crate::threads::persistence::THREAD_FULL)
}

/// Preserve the storage failure in the jobs checkpoint and publish it to
/// active subscribers. Do not send the rejected bytes back through content.
pub(super) async fn finish_full<D: Directory>(
    state: &Shared<D>,
    job: &Job,
    live: &Arc<Live>,
    checkpoint: &mut Value,
    published_seq: u64,
    error: &Error,
) -> Result<RunEnd, Error> {
    let code = crate::threads::persistence::THREAD_FULL;
    checkpoint["thread_text"] = json!("");
    checkpoint["text_tail"] = Value::Null;
    checkpoint
        .as_object_mut()
        .expect("prepared checkpoint object")
        .remove("message_timing");
    checkpoint["error"] = json!({
        "code":code,"class":"permanent","message":error.message,"origin":"thread_storage"
    });
    save(state, job, live, checkpoint, None).await?;
    live.publish(job.runs, &json!({
        "version":1,"seq":published_seq + 1,
        "event":{"type":"error","class":"permanent","billing":"unknown","code":code,"message":error.message}
    }));
    Ok(RunEnd::Failed)
}

pub(super) enum Placement {
    Ready(Uuid, bool),
    Ended(RunEnd),
}

/// Wait for the fenced placement while checking account access. The queue
/// writes one checkpoint only when its displayed reason or position changes.
pub(super) async fn place<D: Directory>(
    state: &Shared<D>,
    machines: &Arc<MachineService>,
    job: &Job,
    live: &Arc<Live>,
    lease_id: Uuid,
    checkpoint: &mut Value,
) -> Result<Placement, Error> {
    let account = job.account;
    let who = Identity { id: account };
    let placement_deadline = Instant::now()
        + if job.input["verify"] == true {
            super::verify::PHASE_TIMEOUT
        } else {
            Duration::from_secs(600)
        };
    let mut placement = Value::Null;
    loop {
        if !live.is_current(job.runs) || !super::session_live(state, job).await? {
            return Ok(Placement::Ended(RunEnd::Failed));
        }
        if job.runs == 1
            && job.input["thread_request"]["automation_run"].is_object()
            && !crate::automations::scheduling::within_scheduled_day(job)
        {
            checkpoint["interruption"] = json!({"why":"automation_day_ended"});
            save_placement(state, job, live, checkpoint).await?;
            return Ok(Placement::Ended(RunEnd::Failed));
        }
        super::authorize(state, &who, &job.space, Capability::PluginsUse).await?;
        let m = machines.clone();
        let view = blocking(move || m.run(account, lease_id)).await?;
        if view.state == LeaseState::Interrupted {
            let m = machines.clone();
            let since = blocking(move || m.interrupted_at(account, lease_id)).await?;
            checkpoint["interruption"] = json!({"why":"machine_offline","since_ms":since});
            save_placement(state, job, live, checkpoint).await?;
            return Ok(Placement::Ended(RunEnd::Interrupted));
        }
        if view.state == LeaseState::Ended {
            return Ok(Placement::Ended(RunEnd::OutcomeUnknown));
        }
        if let Some(machine) = view.machine_id {
            return Ok(Placement::Ready(machine, view.personal_machine));
        }
        let memory_wait = view.reason.as_deref() == Some(crate::machines::MEMORY_WAIT);
        if !memory_wait && Instant::now() >= placement_deadline {
            checkpoint["interruption"] = json!({"why":"placement_timeout"});
            save_placement(state, job, live, checkpoint).await?;
            return Ok(Placement::Ended(RunEnd::Interrupted));
        }
        let (m, pin, position) = (
            machines.clone(),
            super::readiness::pin(&checkpoint, &job.input),
            view.queue_position,
        );
        let reason = blocking(move || m.placement_reason(account, pin, position, memory_wait)).await?;
        if placement != reason {
            placement = reason;
            checkpoint["placement"] = placement.clone();
            save_placement(state, job, live, checkpoint).await?;
            live.publish(
                job.runs,
                &json!({"type":"placement","queue_position":position,"reason":placement}),
            );
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn save_placement<D: Directory>(
    state: &Shared<D>,
    job: &Job,
    live: &Arc<Live>,
    checkpoint: &Value,
) -> Result<(), Error> {
    let (state, job, live, checkpoint) =
        (state.clone(), job.clone(), live.clone(), checkpoint.clone());
    blocking(move || state.jobs.checkpoint_placing(&job, &live, &checkpoint)).await
}
