//! Exact attempt retirement and recovery on the execution writer transaction.
use super::*;

/// Stable cleanup identity for one admitted attempt. The machine service must
/// authenticate runtime proof for this exact identity before settlement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attempt {
    /// Opaque execution reference.
    pub execution: ExecutionRef,
    /// Admission generation scoped to that execution.
    pub generation: i64,
}

/// Reserve the exact attempt for a claimed job under a current live check.
/// Repeating the start returns the same live attempt. No carrier dispatch
/// occurs here. Cost: one indexed job/attempt lookup, one bounded pin decode
/// and one insert for the first start.
pub fn start(
    tx: &Transaction<'_>,
    job: &JobId,
    harness: &HarnessRef,
    check: impl FnOnce(&PinnedExecution) -> Result<bool>,
) -> Result<Attempt> {
    let (intent, encoded, execution, generation, retired) = tx
        .prepare_cached(
            "SELECT i.intent,i.pin,a.execution,a.generation,a.end_reason IS NOT NULL
             FROM execution_intents i LEFT JOIN execution_attempts a ON a.intent=i.intent
             WHERE i.job=?1 AND i.slot=0 AND i.claimed=1",
        )
        .map_err(internal)?
        .query_row([&job.0], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, bool>(4)?,
            ))
        })
        .optional()
        .map_err(internal)?
        .ok_or_else(unavailable)?;
    let pin: PinnedExecution = serde_json::from_str(&encoded).map_err(internal)?;
    if retired || &pin.harness != harness || !live(&pin, check)? {
        return Err(unavailable());
    }
    if let Some(execution) = execution {
        return Ok(Attempt {
            execution: ExecutionRef(execution),
            generation: generation.ok_or_else(|| internal("missing attempt generation"))?,
        });
    }
    let attempt = Attempt {
        execution: ExecutionRef(Uuid::now_v7().to_string()),
        generation: 1,
    };
    tx.prepare_cached(
        "INSERT INTO execution_attempts(execution,intent,generation,transition) VALUES(?1,?2,?3,?4)",
    )
    .map_err(internal)?
    .execute(params![
        attempt.execution.0,
        intent,
        attempt.generation,
        Uuid::now_v7().to_string()
    ])
    .map_err(internal)?;
    Ok(attempt)
}

/// Check current exact-attempt authority before a local effect. The supplied
/// check verifies live grants and runtime binding from the retained pin, and
/// the caller keeps the transaction through the local commit. Cost: one
/// indexed attempt/intent lookup and at most one bounded pin decode.
pub fn is_current(
    tx: &Transaction<'_>,
    attempt: &Attempt,
    check: impl FnOnce(&PinnedExecution) -> Result<bool>,
) -> Result<bool> {
    let encoded = tx
        .prepare_cached(
            "SELECT i.pin FROM execution_attempts a JOIN execution_intents i ON i.intent=a.intent
             WHERE a.execution=?1 AND a.generation=?2 AND a.end_reason IS NULL",
        )
        .map_err(internal)?
        .query_row(params![attempt.execution.0, attempt.generation], |r| {
            r.get::<_, String>(0)
        })
        .optional()
        .map_err(internal)?;
    let Some(encoded) = encoded else {
        return Ok(false);
    };
    live(&serde_json::from_str(&encoded).map_err(internal)?, check)
}

/// Fence the exact attempt selected by the confirmed Stop context. The host
/// checks the live target and starter or management authority in `authorize`
/// and records the Stop receipt in this transaction. Returns the durable
/// transition cursor; a stale or retired target is unavailable. The
/// successor is outside the effect. Cost: one indexed lookup and one update.
pub fn stop(
    tx: &Transaction<'_>,
    execution: &ExecutionRef,
    authorize: impl FnOnce(&ItemRef, AccountId) -> Result<()>,
) -> Result<String> {
    let (item, actor) = tx
        .prepare_cached(
            "SELECT i.item,i.actor FROM execution_attempts a JOIN execution_intents i ON i.intent=a.intent
             WHERE a.execution=?1 AND a.end_reason IS NULL",
        )
        .map_err(internal)?
        .query_row([&execution.0], |r| {
            Ok((ItemRef(r.get(0)?), r.get::<_, String>(1)?))
        })
        .optional()
        .map_err(internal)?
        .ok_or_else(unavailable)?;
    authorize(&item, AccountId::parse(&actor).map_err(internal)?)?;
    mark_stopped(tx, execution)
}

pub(super) fn mark_stopped(tx: &Transaction<'_>, execution: &ExecutionRef) -> Result<String> {
    let transition = Uuid::now_v7().to_string();
    tx.prepare_cached(
        "UPDATE execution_attempts SET end_reason='stopped',transition=?2 WHERE execution=?1",
    )
    .map_err(internal)?
    .execute(params![execution.0, transition])
    .map_err(internal)?;
    Ok(transition)
}

/// Retire only the exact pending intent from a protected Stop context.
/// Refuse if an attempt has since been admitted for it. The caller writes
/// the receipt in the same transaction after the live retirement check.
/// Cost: one indexed intent/attempt lookup and one delete.
pub fn stop_pending(
    tx: &Transaction<'_>,
    intent: &ExecutionIntentRef,
    authorize: impl FnOnce(&ItemRef, AccountId) -> Result<()>,
) -> Result<()> {
    let (item, actor) = tx
        .prepare_cached(
            "SELECT item,actor FROM execution_intents
             WHERE intent=?1 AND NOT EXISTS(SELECT 1 FROM execution_attempts WHERE intent=?1)",
        )
        .map_err(internal)?
        .query_row([&intent.0], |r| {
            Ok((ItemRef(r.get(0)?), r.get::<_, String>(1)?))
        })
        .optional()
        .map_err(internal)?
        .ok_or_else(unavailable)?;
    authorize(&item, AccountId::parse(&actor).map_err(internal)?)?;
    discard_pending(tx, intent)
}

pub(super) fn discard_pending(tx: &Transaction<'_>, intent: &ExecutionIntentRef) -> Result<()> {
    tx.prepare_cached("DELETE FROM execution_intents WHERE intent=?1")
        .map_err(internal)?
        .execute([&intent.0])
        .map_err(internal)?;
    Ok(())
}

/// Retire all work pinned to one revoked installation of this actor. The host
/// invokes this before publishing revocation under the cross-store guard.
/// Affected attempts become cleanup records and retain quota. Pending intents
/// lose dispatch authority immediately. Cost: two indexed mutations, bounded
/// by the number of admissions in the space.
pub fn fence_installation(
    tx: &Transaction<'_>,
    actor: AccountId,
    installation: InstallationId,
) -> Result<usize> {
    let actor = actor.to_string();
    let installation = installation.to_string();
    let fenced = tx
        .prepare_cached(
            "UPDATE execution_attempts SET end_reason='interrupted',transition=?3
             WHERE end_reason IS NULL AND intent IN
                (SELECT intent FROM execution_intents WHERE actor=?1 AND installation=?2)",
        )
        .map_err(internal)?
        .execute(params![actor, installation, Uuid::now_v7().to_string()])
        .map_err(internal)?;
    let discarded = tx
        .prepare_cached(
            "DELETE FROM execution_intents WHERE actor=?1 AND installation=?2 AND NOT EXISTS
                (SELECT 1 FROM execution_attempts WHERE intent=execution_intents.intent)",
        )
        .map_err(internal)?
        .execute(params![actor, installation])
        .map_err(internal)?;
    Ok(fenced + discarded)
}

/// Read one bounded cleanup batch. Dispatch exact Stops only after releasing
/// the writer. The same outstanding identity is recoverable on every restart.
/// Cost: scan at most one space's bounded admissions, allocate up to `limit`.
pub fn cleanup(tx: &Transaction<'_>, limit: u32) -> Result<Vec<Attempt>> {
    tx.prepare_cached(
        "SELECT execution,generation FROM execution_attempts
         WHERE end_reason IS NOT NULL ORDER BY execution LIMIT ?1",
    )
    .map_err(internal)?
    .query_map([limit], |r| {
        Ok(Attempt {
            execution: ExecutionRef(r.get(0)?),
            generation: r.get(1)?,
        })
    })
    .map_err(internal)?
    .collect::<rusqlite::Result<_>>()
    .map_err(internal)
}

/// Release a retired admission after authenticated teardown proof. Duplicate,
/// unrelated or stale proof leaves the state unchanged. Cost: one indexed
/// conditional delete with an exact attempt lookup and cascading retirement.
pub fn settle(tx: &Transaction<'_>, exact: &Attempt) -> Result<bool> {
    tx.prepare_cached(
        "DELETE FROM execution_intents WHERE intent=
            (SELECT intent FROM execution_attempts
             WHERE execution=?1 AND generation=?2 AND end_reason IS NOT NULL)",
    )
    .map_err(internal)?
    .execute(params![exact.execution.0, exact.generation])
    .map(|deleted| deleted == 1)
    .map_err(internal)
}
