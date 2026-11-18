//! Native lease admission and checkpoint share the per-space cancellation fence.
use super::*;

/// Prepare a lease only while the job is current; cleanup sees its durable ID.
/// One job current-run read, machine admission, and FULL checkpoint transaction.
/// On publication failure, release the admitted lease before returning the error.
pub(super) async fn prepare<D: Directory>(
    state: &Shared<D>,
    machines: &Arc<MachineService>,
    job: &Job,
    live: &Arc<crate::plugins::live::Live>,
    space: SpaceId,
    thread: Uuid,
    shared: Option<crate::machines::space_execution::SpaceExecution>,
) -> Result<Value, Error> {
    let account = job.account;
    let mut initial = job.checkpoint.clone();
    if !initial.is_object() {
        initial = json!({});
    }
    let (s, m, sp, id, l) = (
        state.clone(),
        machines.clone(),
        job.space.clone(),
        job.id.clone(),
        live.clone(),
    );
    let run = job.runs;
    let check = job.input["check"] == true;
    let pin = super::readiness::pin(&initial, &job.input);
    let pool_diagnostic = job.input["pool_diagnostic"] == true;
    if pool_diagnostic {
        super::verify::target(
            state,
            machines,
            account,
            job.input["check_target"].as_str().unwrap_or(""),
        )
        .await?;
    }
    if let Some(fields) = initial.as_object_mut() {
        fields.remove("error");
        fields.remove("interruption");
        fields.remove("placement");
        fields.remove("message_outcome");
        fields.remove("message_timing");
    }
    let checkpoint = blocking(move || {
        let mut lease = None;
        let result = s.jobs.checkpoint_prepared(&sp, &id, run, &l, || {
            let admitted = if pool_diagnostic {
                let machine =
                    pin.ok_or_else(|| Error::internal("pool diagnostic has no target"))?;
                m.request_pool_diagnostic(space, account, thread, machine)?
            } else if let Some(target) = shared {
                m.request_shared(space, account, thread, target, false)?
            } else {
                m.request_run_on(space, account, thread, None, pin)?
            };
            lease = Some(admitted.lease_id);
            initial["lease_id"] = json!(admitted.lease_id);
            initial["shared_sponsor"] = json!(shared.map(|target| target.sponsor));
            initial["shared_machine"] = json!(shared.map(|target| target.machine));
            Ok(initial)
        });
        if result.is_err() {
            if let Some(lease) = lease {
                let _ = if check {
                    m.release_probe(account, lease)
                } else {
                    m.release_run(account, lease)
                };
            }
        }
        result
    })
    .await?;
    Ok(checkpoint)
}

/// Publish the selected machine with the running state before the provider can
/// obtain its token or start. One per-space job commit on the blocking pool.
pub(super) async fn publish_placement<D: Directory>(
    state: &Shared<D>,
    job: &Job,
    live: &Arc<crate::plugins::live::Live>,
    checkpoint: &mut Value,
    personal: Option<(Uuid, Uuid)>,
) -> Result<(), Error> {
    checkpoint["thread_text"] = json!("");
    let (s, space, id, run, live, mut placed) = (
        state.clone(),
        job.space.clone(),
        job.id.clone(),
        job.runs,
        live.clone(),
        checkpoint.clone(),
    );
    let account = job.account;
    *checkpoint = blocking(move || {
        if let Some((thread, machine)) = personal {
            placed["message_machine"] = json!(crate::threads::provider_session::binding(
                account, thread, machine
            ));
        }
        s.jobs.start_placed_turn(&space, &id, run, &live, &placed)?;
        Ok(placed)
    })
    .await?;
    Ok(())
}
