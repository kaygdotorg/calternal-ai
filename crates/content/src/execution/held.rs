//! Sends accepted while execution admission is closed by the updater fence or
//! by startup recovery. A held Send is committed with its receipt before the
//! ACK, reserves no execution capacity and launches nothing. Release moves
//! each row out exactly once, in original order, under live admission checks.
use super::*;

/// Outcome of releasing one held Send.
#[derive(Debug, PartialEq, Eq)]
pub enum Released {
    /// Admitted as a head or successor under its original identities.
    Admitted(Admission),
    /// Refused at release (lost authority or full queue); the job is gone.
    Refused(JobId),
}

/// Hold one checked Send while admission is closed. Allocates the job and
/// intent identities now, so the receipt written in this transaction is the
/// same one replay returns after release. Refuses when the space already
/// holds `capacity` Sends. Cost: one serialization, one count, one insert.
pub fn hold(tx: &Transaction<'_>, pin: &PinnedExecution, capacity: usize) -> Result<Admission> {
    let encoded = encode(pin)?;
    let held = tx
        .prepare_cached("SELECT count(*) FROM execution_held")
        .map_err(internal)?
        .query_row([], |r| r.get::<_, i64>(0))
        .map_err(internal)?;
    if usize::try_from(held).map_err(internal)? >= capacity {
        return Err(pending_limit());
    }
    let admission = Admission {
        job: JobId(Uuid::now_v7().to_string()),
        intent: ExecutionIntentRef(Uuid::now_v7().to_string()),
    };
    tx.prepare_cached(
        "INSERT INTO execution_held(intent,job,item,actor,installation,pin)
         VALUES(?1,?2,?3,?4,?5,?6)",
    )
    .map_err(internal)?
    .execute(params![
        admission.intent.0,
        admission.job.0,
        pin.item.0,
        pin.actor.to_string(),
        pin.installation.to_string(),
        encoded
    ])
    .map_err(internal)?;
    Ok(admission)
}

/// Release every held Send of this space in original order. Each row is
/// deleted in this transaction whatever its outcome, so a second release
/// finds nothing: delivery happens once. `check` re-runs the live
/// admission check on the original pin; a refusal or full queue drops the
/// job. Storage and check errors roll back the whole release. Cost: one
/// ordered scan bounded by the hold capacity, and per row one decode, the
/// admission insert and one delete.
pub fn release_held(
    tx: &Transaction<'_>,
    capacity: usize,
    mut check: impl FnMut(&PinnedExecution) -> Result<bool>,
) -> Result<Vec<Released>> {
    let rows: Vec<(i64, String, String, String)> = tx
        .prepare_cached("SELECT seq,intent,job,pin FROM execution_held ORDER BY seq")
        .map_err(internal)?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .map_err(internal)?
        .collect::<rusqlite::Result<_>>()
        .map_err(internal)?;
    let mut released = Vec::with_capacity(rows.len());
    for (seq, intent, job, encoded) in rows {
        tx.prepare_cached("DELETE FROM execution_held WHERE seq=?1")
            .map_err(internal)?
            .execute([seq])
            .map_err(internal)?;
        let pin: PinnedExecution = serde_json::from_str(&encoded).map_err(internal)?;
        let admission = Admission {
            job: JobId(job),
            intent: ExecutionIntentRef(intent),
        };
        if !live(&pin, &mut check)? {
            released.push(Released::Refused(admission.job));
            continue;
        }
        match insert_intent(tx, &pin, &encoded, &admission, capacity) {
            Ok(()) => released.push(Released::Admitted(admission)),
            Err(error) if error.code == calternal_types::ErrorCode::Conflict => {
                released.push(Released::Refused(admission.job));
            }
            Err(error) => return Err(error),
        }
    }
    Ok(released)
}

/// Count of execution work in this space that keeps the updater from
/// reporting idle: every unsettled intent (queued, claimed, running,
/// reconnecting, stopping or awaiting teardown proof) plus held Sends.
/// Cost: two bounded counts.
pub fn unsettled(tx: &Transaction<'_>) -> Result<usize> {
    let count = tx
        .prepare_cached(
            "SELECT (SELECT count(*) FROM execution_intents) + (SELECT count(*) FROM execution_held)",
        )
        .map_err(internal)?
        .query_row([], |r| r.get::<_, i64>(0))
        .map_err(internal)?;
    usize::try_from(count).map_err(internal)
}

/// Number of Sends held in this space, for the startup scan. Cost: one count.
pub fn held_count(tx: &Transaction<'_>) -> Result<usize> {
    let count = tx
        .prepare_cached("SELECT count(*) FROM execution_held")
        .map_err(internal)?
        .query_row([], |r| r.get::<_, i64>(0))
        .map_err(internal)?;
    usize::try_from(count).map_err(internal)
}
