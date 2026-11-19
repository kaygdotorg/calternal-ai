//! Per-space recovery performed under the existing cache slot lock before admission.
use crate::{Content, ErrorCode, Result, db::Space, journal::block_on, types::internal};
use calternal_types::{AccountId, SpaceId, now_ms};
use rusqlite::Connection;
use std::time::{Duration, Instant};
use uuid::Uuid;

const RECEIPT_PRUNE_INTERVAL: Duration = Duration::from_secs(3600);

/// An operator-visible obstacle to normal receipt expiry in one space.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum ReceiptPruningIssue {
    /// An existing journal receipt is older than the configured retention.
    ExpiredPin {
        /// Elapsed milliseconds since the oldest pinned receipt was committed.
        age_ms: i64,
    },
    /// The outbox could not be read or validated, so the receipt age is unknown.
    OutboxUnreadable,
    /// The journal could not be read or validated, so the receipt age is unknown.
    JournalUnreadable,
}

fn outbox_pins(conn: &Connection) -> Result<Vec<(AccountId, String)>> {
    let mut stmt = conn
        .prepare("SELECT account,request FROM outbox")
        .map_err(internal)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(internal)?;
    rows.map(|row| {
        let (account, request) = row.map_err(internal)?;
        Ok((AccountId::parse(&account).map_err(internal)?, request))
    })
    .collect()
}

impl Content {
    /// Inspect one space for receipt-pruning problems using read-only snapshots.
    /// Blocks for one outbox scan and, when there are pins, indexed journal
    /// lookups. Does not recover the space or activate its writer. Observation
    /// may race retirement. Errors indicate that the space lifecycle is closed.
    pub fn receipt_pruning_issue(&self, id: SpaceId) -> Result<Option<ReceiptPruningIssue>> {
        let _step = self.lifecycle.step(id.as_uuid())?;
        let path = crate::db::space_path(&self.roots.appdata, id);
        match path.metadata() {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Ok(Some(ReceiptPruningIssue::OutboxUnreadable)),
            Ok(_) => {}
        }
        let pins = crate::pool::open_reader(&path)
            .map_err(internal)
            .and_then(|conn| {
                let binding: String = conn
                    .query_row("SELECT value FROM binding WHERE id='space'", [], |row| {
                        row.get(0)
                    })
                    .map_err(internal)?;
                if binding != id.to_string() {
                    return Err(crate::Error::internal("outbox belongs to another space"));
                }
                outbox_pins(&conn)
            });
        let Ok(pins) = pins else {
            return Ok(Some(ReceiptPruningIssue::OutboxUnreadable));
        };
        if pins.is_empty() {
            return Ok(None);
        }
        let Ok(Some(journal)) = calternal_store::ProjectionJournal::open(&self.roots.appdata, id)
        else {
            return Ok(Some(ReceiptPruningIssue::JournalUnreadable));
        };
        let oldest = match journal.oldest_receipt_ms(&pins) {
            Ok(oldest) => oldest,
            Err(_) => return Ok(Some(ReceiptPruningIssue::JournalUnreadable)),
        };
        let retention = i64::try_from(self.receipt_retention().as_millis()).unwrap_or(i64::MAX);
        Ok(oldest
            .map(|at| now_ms().saturating_sub(at))
            .filter(|age| *age > retention)
            .map(|age_ms| ReceiptPruningIssue::ExpiredPin { age_ms }))
    }

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
        let result = outbox_pins(&self.conn)
            .and_then(|pins| block_on(self.journal.prune_receipts(at_ms, pins)).map_err(internal));
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
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM outbox WHERE rowid>?1)",
                [self.published_append_cutoff()],
                |r| r.get(0),
            )
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
