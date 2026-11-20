//! Execution effects that share the item's archive transaction.
use super::*;

/// The host-checked identity and retirement principal of one unresolved job.
/// The archive confirmation pins the ordered list of these targets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveTarget {
    /// Exact admitted intent, including a queued successor.
    pub intent: ExecutionIntentRef,
    /// Attempt already admitted for the intent, if any.
    pub attempt: Option<Attempt>,
    /// Account whose admission would be retired.
    pub starter: AccountId,
}

/// Resolve the set whose retirement the archive would authorize. Already
/// fenced attempts retain cleanup storage but need no new retirement.
/// Cost: one indexed item lookup returning at most the head and successor.
pub fn archive_targets(tx: &Transaction<'_>, item: &ItemRef) -> Result<Vec<ArchiveTarget>> {
    let mut select = tx
        .prepare_cached(
            "SELECT i.intent,i.actor,a.execution,a.generation
             FROM execution_intents i LEFT JOIN execution_attempts a ON a.intent=i.intent
             WHERE i.item=?1 AND a.end_reason IS NULL ORDER BY i.slot",
        )
        .map_err(internal)?;
    let rows = select
        .query_map([&item.0], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?
                    .zip(r.get::<_, Option<i64>>(3)?),
            ))
        })
        .map_err(internal)?;
    rows.map(|row| {
        let (intent, actor, attempt) = row.map_err(internal)?;
        Ok(ArchiveTarget {
            intent: ExecutionIntentRef(intent),
            starter: AccountId::parse(&actor).map_err(internal)?,
            attempt: attempt.map(|(execution, generation)| Attempt {
                execution: ExecutionRef(execution),
                generation,
            }),
        })
    })
    .collect()
}

/// Apply the execution effects inside the transaction that archives the item.
/// A changed affected set requires fresh confirmation. The live authorization
/// callback must permit retirement of every affected starter before any writes.
/// Passing an empty confirmed set enforces the idle-only archive precondition.
/// Cost: one target lookup and at most two updates or deletes.
pub fn fence_archive(
    tx: &Transaction<'_>,
    item: &ItemRef,
    confirmed: &[ArchiveTarget],
    authorize: impl FnOnce(&[ArchiveTarget]) -> Result<()>,
) -> Result<()> {
    let targets = archive_targets(tx, item)?;
    if targets != confirmed {
        return Err(Error::conflict("Execution changed. Resolve archive again."));
    }
    authorize(&targets)?;
    for target in targets {
        if let Some(attempt) = target.attempt {
            fence::mark_stopped(tx, &attempt.execution)?;
        } else {
            fence::discard_pending(tx, &target.intent)?;
        }
    }
    Ok(())
}
