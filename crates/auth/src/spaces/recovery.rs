//! Protected commit and recovery publication for the authority worker.
use super::*;

/// Host-injected durable recovery barrier for every protected auth transaction.
/// Implementations reconcile earlier proposals through committed visibility, stage
/// only affected protected records, insert their witness in `transaction`, and
/// refuse ambiguous recovery. They must never replay a transaction's own witness.
/// The host serializes the prepare/commit/complete interval; direct trait callers
/// must do the same. Every durability failure prevents acknowledgement.
/// With no configured sink, transaction behavior is unchanged.
pub trait RecoverySink: Send + Sync {
    /// Reconciles pending work against the still-authoritative database.
    fn reconcile(&self, connection: &Connection) -> Result<()>;
    /// Durably appends a proposed protected delta, adding its chain witness
    /// to this transaction. An empty token denotes no protected-state change.
    fn prepare(&self, transaction: &Connection) -> Result<String>;
    /// Publishes only a witnessed committed delta before acknowledgement; an
    /// empty token from prepare requires no filesystem publication.
    fn complete(&self, connection: &Connection, intent: &str) -> Result<()>;
    /// Drop superseded generations after an erasure's authority is published.
    /// The caller is outside a SQLite transaction and holds the same recovery
    /// serialization guard. Stateless test sinks have no file history.
    fn compact_erasure(&self, _connection: &Connection) -> Result<()> {
        Ok(())
    }
    /// Held from prepare through complete, so no other commit stages an intent while
    /// this one decides which witnesses are committed. It belongs to the sink, so
    /// database handles sharing a sink share it too.
    fn serial(&self) -> &std::sync::Mutex<()>;
}

/// Commits authority with its configured protected-publication barrier; no-op
/// deltas retain normal SQLite durability without writing recovery versions.
pub(crate) fn commit(transaction: rusqlite::Transaction<'_>) -> Result<()> {
    commit_with(transaction, || {})
}
/// Runs the in-memory revoke fence immediately after durable SQLite commit,
/// including when later recovery publication fails and no acknowledgement occurs.
/// A refused preparation rolls back authority before reconciling an aborted proposal.
pub(crate) fn commit_with(
    mut transaction: rusqlite::Transaction<'_>,
    committed: impl FnOnce(),
) -> Result<()> {
    let Some(sink) = crate::db::active_sink() else {
        transaction.commit().map_err(sql)?;
        committed();
        return Ok(());
    };
    // Retain the sink's barrier across commit: another worker must not stage an
    // intent while completion is deciding which witnesses are committed.
    let _guard = sink.serial().lock().map_err(Error::internal)?;
    let intent = match sink.prepare(&transaction) {
        Ok(intent) => intent,
        Err(error) => {
            // An aborted proposal requires a writable autocommit connection to
            // pin its abort marker. Reconcile only after rolling back authority.
            transaction.execute_batch("ROLLBACK").map_err(sql)?;
            transaction.set_drop_behavior(rusqlite::DropBehavior::Ignore);
            sink.reconcile(&transaction)?;
            return Err(error);
        }
    };
    // Keep the original pinned SQLite connection available after COMMIT. On a
    // failed COMMIT, the transaction still rolls back using its normal Drop.
    transaction.execute_batch("COMMIT").map_err(sql)?;
    transaction.set_drop_behavior(rusqlite::DropBehavior::Ignore);
    committed();
    sink.complete(&transaction, &intent)
}
