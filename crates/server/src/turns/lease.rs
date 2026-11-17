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
    let pin = initial["request_machine"]
        .as_str()
        .and_then(|id| Uuid::parse_str(id).ok());
    let checkpoint = blocking(move || {
        let mut lease = None;
        let result = s.jobs.checkpoint_prepared(&sp, &id, run, &l, || {
            let admitted = m.request_run_on(space, account, thread, None, pin)?;
            lease = Some(admitted.lease_id);
            initial["lease_id"] = json!(admitted.lease_id);
            Ok(initial)
        });
        if result.is_err() {
            if let Some(lease) = lease {
                let _ = m.release_run(account, lease);
            }
        }
        result
    })
    .await?;
    Ok(checkpoint)
}
