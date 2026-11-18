//! Per-space recovery performed under the existing cache slot lock before admission.
use crate::{Content, ErrorCode, Result, db::Space, journal::block_on, types::internal};
use calternal_types::{AccountId, now_ms};
use std::time::{Duration, Instant};
use uuid::Uuid;

const RECEIPT_PRUNE_INTERVAL: Duration = Duration::from_secs(3600);

impl Content {
    /// Prunes eligible receipts with the as-opened outbox pinned, then replays
    /// committed intents before materializing heads. Failure leaves the slot
    /// unopened. External file conflicts remain preserved and blocked.
    pub(crate) fn recover_space(&self, space: &mut Space) -> Result<()> {
        self.recover_space_at(space, now_ms())
    }

    fn recover_space_at(&self, space: &mut Space, at_ms: i64) -> Result<()> {
        space.cleanup()?;
        space.prune_receipts_at(at_ms);
        let result = self.publish_pending(space);
        #[cfg(target_os = "linux")]
        if result
            .as_ref()
            .is_err_and(crate::thread_append::proof::pending)
        {
            space.append_recovering = true;
            return Ok(());
        }
        result?;
        space.materialize_pending(self.fault)
    }
}

impl Space {
    /// Prune only after reading every pin under this same space's slot. Holding
    /// the slot until the command replies prevents publication, retirement and
    /// retries from changing the set during the indexed journal delete.
    fn prune_receipts_at(&mut self, at_ms: i64) {
        self.last_receipt_prune = Instant::now();
        let result = (|| -> Result<()> {
            let mut stmt = self
                .conn
                .prepare("SELECT account,request FROM outbox")
                .map_err(internal)?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(internal)?;
            let mut pins = Vec::new();
            for row in rows {
                let (account, request) = row.map_err(internal)?;
                pins.push((AccountId::parse(&account).map_err(internal)?, request));
            }
            block_on(self.journal.prune_receipts(at_ms, pins)).map_err(internal)
        })();
        if let Err(error) = result {
            tracing::warn!(space = %self.id, ?error, "receipt pruning skipped");
        }
    }

    /// Hourly maintenance on the existing space worker. Most passes check only
    /// the monotonic throttle; eligible work reads the outbox and waits for
    /// one bounded journal writer command. Returns whether it attempted work.
    pub(crate) fn prune_receipts_if_due(&mut self) -> bool {
        if self.last_receipt_prune.elapsed() < RECEIPT_PRUNE_INTERVAL {
            return false;
        }
        self.prune_receipts_at(now_ms());
        true
    }

    /// Complete durable pending heads in this space. Called under its slot lock
    /// by lazy recovery and the existing lifecycle worker, never an async worker.
    pub(crate) fn materialize_pending(&mut self, fault: fn(crate::FaultPoint)) -> Result<()> {
        // A later failed publication can coexist with an older acknowledged head.
        // Never let its old wake publish newer file bytes before journal recovery.
        let unpublished: bool = self
            .conn
            .query_row("SELECT EXISTS(SELECT 1 FROM outbox)", [], |r| r.get(0))
            .map_err(internal)?;
        if unpublished {
            return Err(crate::Error::unavailable(
                "pending journal publication; retry the request",
            ));
        }
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM items WHERE mat_state='pending'")
            .map_err(internal)?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(internal)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(internal)?;
        drop(stmt);
        for id in ids {
            if let Err(e) = self.place(Uuid::parse_str(&id).map_err(internal)?, fault)
                && e.code != ErrorCode::Conflict
            {
                return Err(e);
            }
        }
        self.pending_since = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
