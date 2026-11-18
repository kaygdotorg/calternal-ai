//! Outcome-only diagnostic over the existing durable Check job and readiness cache.
use super::*;
use calternal_core::MachineVerifyResult;

/// Bounds each phase of a diagnostic within the common client's 15-second request.
pub(super) const PHASE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(6);

pub(super) async fn target<D: Directory>(
    state: &Shared<D>,
    machines: &Arc<MachineService>,
    account: calternal_types::AccountId,
    machine: &str,
) -> Result<(Uuid, bool), Error> {
    let id = Uuid::parse_str(machine).map_err(|_| Error::not_found("machine not found"))?;
    let auth = state
        .directory
        .http_auth()
        .map(|service| service.auth.clone());
    let machines = machines.clone();
    blocking(move || {
        let pool_admin = auth
            .map(|auth| auth.has_server_capability(account, Capability::ServerSettings))
            .transpose()
            .map_err(|_| Error::unavailable("could not check pool authority"))?
            .unwrap_or(false);
        Ok((id, machines.verify_target(account, id, pool_admin)?))
    })
    .await
}

/// Wait for the existing job signal and project its final checkpoint. Placement,
/// execution and the caller wait are bounded independently. No output buffer or
/// provider diagnostics are delivered from this operation.
pub(super) async fn outcome<D: Directory>(
    state: &Shared<D>,
    machines: &Arc<MachineService>,
    who: &Identity,
    mut job: Job,
) -> Result<Value, Error> {
    if !job.state.is_final() {
        let subscription = state.jobs.subscribe(&job.id)?;
        let mut changed = subscription.live.watch();
        tokio::time::timeout(std::time::Duration::from_secs(14), async {
            // Cover completion before subscription, including live-buffer eviction.
            job = load(state, &job.id).await?;
            while !job.state.is_final() {
                changed.borrow_and_update();
                if subscription.live.run_settled() {
                    job = load(state, &job.id).await?;
                    if job.state.is_final() {
                        break;
                    }
                }
                changed
                    .changed()
                    .await
                    .map_err(|_| Error::unavailable("machine diagnostic ended"))?;
            }
            Ok::<_, Error>(())
        })
        .await
        .map_err(|_| {
            Error::unavailable(
                "Machine verification timed out. Check that the machine is connected and retry.",
            )
        })??;
    }
    authorize(state, who, &job.space, Capability::PluginsUse).await?;
    target(
        state,
        machines,
        who.id,
        job.input["check_target"].as_str().unwrap_or(""),
    )
    .await?;
    let provider = serde_json::from_value(job.checkpoint["provider_check"].clone())
        .map_err(|_| Error::internal("invalid diagnostic outcome"))?;
    Ok(json!(MachineVerifyResult {
        sandbox_started: job.checkpoint["sandbox_started"] == true,
        provider,
    }))
}
