//! Protected agent authority on the existing auth handle and recovery barrier.
//!
//! The host owns protocol validation; this port keeps its bounded access records,
//! approval decisions and discard authorizations in the closed security inventory.
//! It creates no worker, connection or lock: clones reuse the existing auth handle.
use crate::{db::Db, spaces::commit};
use calternal_types::Error;
use rusqlite::{Connection, Transaction, TransactionBehavior};

/// Agent authority sharing the account directory's worker and protected commits.
#[derive(Clone)]
pub struct Agents {
    db: Db,
}
impl Agents {
    pub(crate) fn new(db: Db) -> Self {
        Self { db }
    }

    /// Read current agent authority on the existing auth worker. Call only from a
    /// blocking task. The host callback must only read agent authority tables.
    /// Cost: one bounded worker round trip and the callback's indexed reads.
    pub fn read<T: Send + 'static>(
        &self,
        read: impl FnOnce(&Connection) -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        self.db
            .run_blocking(move |c| Ok(read(c)))
            .map_err(|_| Error::unavailable("agent authority unavailable"))?
    }

    /// Whether the admitting account sign-in is still live, using the same
    /// revocation and expiry predicates as account installations. One indexed
    /// read on the existing auth worker; lookup failures fail closed to callers.
    pub fn installation_live(
        &self,
        account: calternal_types::AccountId,
        installation: calternal_types::InstallationId,
    ) -> Result<bool, Error> {
        self.read(move|c|c.query_row("SELECT EXISTS(SELECT 1 FROM installations WHERE id=?1 AND user_id=?2 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>?3))",rusqlite::params![installation.to_string(),account.to_string(),calternal_types::now_ms()],|r|r.get(0)).map_err(|_|Error::unavailable("admitting sign-in unavailable")))
    }

    /// Commit an agent authority change through the same recovery barrier as
    /// memberships and machines. Call only from a blocking task; the trusted host
    /// callback may mutate only agent authority tables and must bound every row.
    /// Failure before acknowledgement leaves the barrier's ordinary recovery work;
    /// no authority is acknowledged without its protected commit. Cost: one FULL
    /// transaction and, when configured, intent/commit/witness publication.
    pub fn change<T: Send + 'static>(
        &self,
        change: impl FnOnce(&Transaction<'_>) -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        self.db
            .run_blocking(move |c| {
                let tx = c
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(crate::Error::internal)?;
                let value = match change(&tx) {
                    Ok(v) => v,
                    Err(e) => return Ok(Err(e)),
                };
                commit(tx)?;
                Ok(Ok(value))
            })
            .map_err(|_| Error::unavailable("agent authority recovery commit failed"))?
    }
}
