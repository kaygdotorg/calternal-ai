//! Durable execution admission on the per-space content writer. Every operation
//! runs on a blocking worker inside the caller's FULL transaction. The hosting
//! layer authenticates callers and supplies current install and grant checks.
use crate::{Error, Result, types::internal};
use calternal_types::{
    AccountId, ContributionIdentity, ExecutionIntentRef, ExecutionRef, HarnessRef, InstallationId,
    ItemRef, JobId, MAX_FRAME_BYTES, PackageId, PlacementRef, now_ms,
};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

mod archive;
pub use archive::{ArchiveTarget, archive_targets, fence_archive};
mod fence;
pub use fence::{
    Attempt, cleanup, fence_installation, is_current, settle, start, stop, stop_pending,
};
mod writer;

/// Host-checked execution bindings retained between admission and dispatch.
/// Only trusted host code constructs this, after checking the kind, installed
/// job, contribution preflight and placement choice. The JSON inputs back the
/// installed worker invocation and are bounded by the host frame limit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PinnedExecution {
    /// Checked target in the selected space.
    pub item: ItemRef,
    /// Acting person who supplied the execution authority.
    pub actor: AccountId,
    /// Authenticated installation that initiated this interactive execution.
    pub installation: InstallationId,
    /// Live installation generation at admission.
    pub installation_generation: u64,
    /// Upper bound for the initiating installation's validity, in Unix milliseconds.
    pub installation_expiry_ms: i64,
    /// Installed worker operation resolved from the declared job.
    pub operation: String,
    /// Installed package-local job declaration selected at admission.
    pub declaration: String,
    /// Logical package that declared that job.
    pub package: PackageId,
    /// Exact build of the declaring package.
    pub package_digest: String,
    /// Checked binding to the exact installed harness contribution.
    pub harness: HarnessRef,
    /// Logical identity of the installed contribution in the job context.
    pub contribution: ContributionIdentity,
    /// Checked placement accepted for this execution.
    pub placement: PlacementRef,
    /// Exact profile identity for the selected placement.
    pub profile: String,
    /// Audience generation under which the item was admitted.
    pub audience_generation: u64,
    /// Host-resolved upper bound on effects available to the attempt.
    pub effect_ceiling: Value,
    /// Checked run input with its declared target injected by the host.
    pub input: Value,
    /// Exact input accepted at contribution-frame preflight.
    pub contribution_input: Value,
}

/// Durable identifiers allocated with the admission reservation.
#[derive(Debug, Eq, PartialEq)]
pub struct Admission {
    /// Durable job reference returned by `start_job`.
    pub job: JobId,
    /// Exact execution intent used for pending Stop.
    pub intent: ExecutionIntentRef,
}

/// Reserve one head or successor and return its identities. The caller checks
/// the live item and authority and writes the operation receipt atomically in
/// this same transaction. Refuses queue or space capacity without writing.
/// Cost: one bounded space count, one indexed item-slot read and one insert.
pub fn admit(tx: &Transaction<'_>, pin: &PinnedExecution, capacity: usize) -> Result<Admission> {
    if pin.installation_expiry_ms <= now_ms() {
        return Err(unavailable());
    }
    let encoded = serde_json::to_string(pin).map_err(internal)?;
    if encoded.len() > MAX_FRAME_BYTES {
        return Err(Error::new(
            calternal_types::ErrorCode::TooLarge,
            "Execution bindings exceed the host frame limit",
        ));
    }
    let slot = tx
        .prepare_cached("SELECT max(slot) FROM execution_intents WHERE item=?1")
        .map_err(internal)?
        .query_row([&pin.item.0], |r| r.get::<_, Option<i64>>(0))
        .map_err(internal)?;
    if slot == Some(1) {
        return Err(pending_limit());
    }
    let count = tx
        .prepare_cached("SELECT count(*) FROM execution_intents")
        .map_err(internal)?
        .query_row([], |r| r.get::<_, i64>(0))
        .map_err(internal)?;
    let count = usize::try_from(count).map_err(internal)?;
    if count >= capacity {
        return Err(pending_limit());
    }
    let admission = Admission {
        job: JobId(Uuid::now_v7().to_string()),
        intent: ExecutionIntentRef(Uuid::now_v7().to_string()),
    };
    tx.prepare_cached(
        "INSERT INTO execution_intents(intent,job,item,slot,actor,installation,pin)
         VALUES(?1,?2,?3,?4,?5,?6,?7)",
    )
    .map_err(internal)?
    .execute(params![
        admission.intent.0,
        admission.job.0,
        pin.item.0,
        slot.map_or(0, |_| 1),
        pin.actor.to_string(),
        pin.installation.to_string(),
        encoded
    ])
    .map_err(internal)?;
    Ok(admission)
}

/// Promote one queued intent only after its predecessor has settled. The live
/// check uses the original pin without substitutions. A permanent refusal
/// discards the queued authority. A storage or check error rolls back.
/// Cost: one indexed item query, one pin decode and at most one mutation.
pub fn promote(
    tx: &Transaction<'_>,
    item: &ItemRef,
    check: impl FnOnce(&PinnedExecution) -> Result<bool>,
) -> Result<Option<JobId>> {
    let queued = tx
        .prepare_cached(
            "SELECT job,pin FROM execution_intents WHERE item=?1 AND slot=1
             AND NOT EXISTS(SELECT 1 FROM execution_intents WHERE item=?1 AND slot=0)",
        )
        .map_err(internal)?
        .query_row([&item.0], |r| {
            Ok((JobId(r.get(0)?), r.get::<_, String>(1)?))
        })
        .optional()
        .map_err(internal)?;
    let Some((job, encoded)) = queued else {
        return Ok(None);
    };
    let pin = serde_json::from_str(&encoded).map_err(internal)?;
    if !live(&pin, check)? {
        discard(tx, &job)?;
        return Ok(None);
    }
    tx.prepare_cached("UPDATE execution_intents SET slot=0 WHERE job=?1")
        .map_err(internal)?
        .execute([&job.0])
        .map_err(internal)?;
    Ok(Some(job))
}

/// Claim an unclaimed head and return its checked invocation pin. Queued,
/// already claimed and retired jobs cannot claim. A permanent lack of live
/// authority discards the head before any dispatch. Check errors roll back.
/// Cost: one indexed job read, one pin decode and at most one mutation.
pub fn claim(
    tx: &Transaction<'_>,
    job: &JobId,
    check: impl FnOnce(&PinnedExecution) -> Result<bool>,
) -> Result<Option<PinnedExecution>> {
    let encoded = tx
        .prepare_cached("SELECT pin FROM execution_intents WHERE job=?1 AND slot=0 AND claimed=0")
        .map_err(internal)?
        .query_row([&job.0], |r| r.get::<_, String>(0))
        .optional()
        .map_err(internal)?;
    let Some(encoded) = encoded else {
        return Ok(None);
    };
    let pin = serde_json::from_str(&encoded).map_err(internal)?;
    if !live(&pin, check)? {
        discard(tx, job)?;
        return Ok(None);
    }
    tx.prepare_cached("UPDATE execution_intents SET claimed=1 WHERE job=?1")
        .map_err(internal)?
        .execute([&job.0])
        .map_err(internal)?;
    Ok(Some(pin))
}

fn discard(tx: &Transaction<'_>, job: &JobId) -> Result<()> {
    tx.prepare_cached("DELETE FROM execution_intents WHERE job=?1")
        .map_err(internal)?
        .execute([&job.0])
        .map_err(internal)?;
    Ok(())
}

fn live(
    pin: &PinnedExecution,
    check: impl FnOnce(&PinnedExecution) -> Result<bool>,
) -> Result<bool> {
    if pin.installation_expiry_ms <= now_ms() {
        return Ok(false);
    }
    check(pin)
}

fn pending_limit() -> Error {
    Error::conflict("Execution capacity is full").with_details(json!({"reason": "pending_limit"}))
}

fn unavailable() -> Error {
    Error::not_found("Execution is unavailable")
}

#[cfg(test)]
mod tests;
