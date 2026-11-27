//! Indexed receipt and Trash maintenance for an admitted space.

use super::Space;
use crate::internal;
use calternal_types::{Error, ErrorCode, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Maximum live or recovered retry receipts retained by one space.
const SPACE_RECEIPTS: i64 = 16_384;
const MAINTENANCE_BATCH: usize = 64;

impl Space {
    /// Maintain indexed receipt and Trash expiry in bounded blocking batches.
    pub(crate) fn expire_trash(&mut self) -> Result<()> {
        self.maintain_at(calternal_types::now_ms())
    }

    fn maintain_at(&mut self, now: i64) -> Result<()> {
        if now < self.next_trash_check {
            return Ok(());
        }
        let tx = self.db.transaction().map_err(internal)?;
        let cutoff = now - self.retention_ms;
        let receipts = tx
            .execute(
                "DELETE FROM receipts WHERE rowid IN
              (SELECT rowid FROM receipts WHERE at<?1 ORDER BY at LIMIT ?2)",
                params![cutoff, MAINTENANCE_BATCH as i64],
            )
            .map_err(internal)?;
        let recovered = tx
            .execute(
                "DELETE FROM recovered_appends WHERE rowid IN
              (SELECT rowid FROM recovered_appends WHERE at<?1 ORDER BY at LIMIT ?2)",
                params![cutoff, MAINTENANCE_BATCH as i64],
            )
            .map_err(internal)?;

        let entries = {
            let mut stmt = tx
                .prepare_cached("SELECT item FROM trash WHERE until<=?1 ORDER BY until LIMIT ?2")
                .map_err(internal)?;
            stmt.query_map(params![now, MAINTENANCE_BATCH as i64], |row| {
                row.get::<_, String>(0)
            })
            .map_err(internal)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(internal)?
        };
        if !entries.is_empty() {
            let folder = self
                .files
                .child(".calternal", false)
                .and_then(|folder| folder.child("trash", false))
                .map_err(internal)?;
            for item in &entries {
                let path = format!("{item}.md");
                match folder.remove(&path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(internal(error)),
                }
                tx.execute("DELETE FROM trash WHERE item=?1", [item])
                    .map_err(internal)?;
            }
            folder.sync().map_err(internal)?;
        }
        tx.commit().map_err(internal)?;
        self.next_trash_check = if [entries.len(), receipts, recovered].contains(&MAINTENANCE_BATCH)
        {
            now
        } else {
            now + 60_000
        };
        Ok(())
    }

    pub(crate) fn receipt(
        &self,
        actor: &str,
        key: Option<&str>,
        input: &[u8],
    ) -> Result<Option<Receipt>> {
        let Some(key) = key else {
            return Ok(None);
        };
        if key.is_empty() || key.len() > 128 || key.chars().any(char::is_control) {
            return Err(Error::invalid_input("Invalid idempotency key."));
        }
        let proof = ReceiptProof {
            gesture: self.hash(key.as_bytes()),
            fingerprint: self.hash(input),
        };
        let cutoff = calternal_types::now_ms() - self.retention_ms;
        let existing: Option<(Vec<u8>, String)> = self
            .db
            .query_row(
                "SELECT fingerprint,result FROM receipts WHERE actor=?1 AND gesture=?2 AND at>=?3",
                params![actor, &proof.gesture[..], cutoff],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(internal)?;
        let replay = match existing {
            Some((existing, _)) if existing != proof.fingerprint => {
                return Err(Error::request_reused());
            }
            Some((_, result)) => Some(serde_json::from_str(&result).map_err(internal)?),
            None => None,
        };
        Ok(Some(Receipt {
            actor: actor.to_owned(),
            proof,
            cutoff,
            replay,
        }))
    }
}

#[derive(Deserialize, Serialize)]
pub(crate) struct Receipt {
    actor: String,
    pub(crate) proof: ReceiptProof,
    cutoff: i64,
    pub(crate) replay: Option<Value>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub(crate) struct ReceiptProof {
    pub(crate) gesture: [u8; 32],
    pub(crate) fingerprint: [u8; 32],
}

impl Receipt {
    /// Ensure a receipt can be inserted or replace its expired predecessor.
    pub(crate) fn check_capacity(&self, db: &Connection) -> Result<()> {
        if receipt_capacity(db)?
            || db
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM receipts WHERE actor=?1 AND gesture=?2 AND at<?3)",
                    params![self.actor, &self.proof.gesture[..], self.cutoff],
                    |row| row.get(0),
                )
                .map_err(internal)?
        {
            return Ok(());
        }
        Err(Error::new(
            ErrorCode::TooLarge,
            "The space has reached its receipt limit.",
        ))
    }

    pub(crate) fn store(&self, tx: &Connection, value: &Value, now: i64) -> Result<()> {
        self.check_capacity(tx)?;
        let written = tx
            .execute(
                "INSERT INTO receipts VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(actor,gesture) DO UPDATE SET fingerprint=excluded.fingerprint,
               result=excluded.result,at=excluded.at WHERE receipts.at<?6",
                params![
                    self.actor,
                    &self.proof.gesture[..],
                    &self.proof.fingerprint[..],
                    value.to_string(),
                    now,
                    self.cutoff
                ],
            )
            .map_err(internal)?;
        if written != 1 {
            return Err(Error::request_reused());
        }
        Ok(())
    }
}

/// Check the constant-time ledger counter before storing another retry receipt.
pub(crate) fn receipt_capacity(db: &Connection) -> Result<bool> {
    db.query_row(
        "SELECT total < ?1 FROM receipt_usage WHERE id=1",
        [SPACE_RECEIPTS],
        |row| row.get(0),
    )
    .map_err(internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use calternal_types::SpaceId;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn read_side_maintenance_expires_receipts_and_trash_without_an_insert() {
        let root = tempfile::tempdir().unwrap();
        let id = SpaceId::new();
        let mut space = Space::open(root.path(), id, 86_400_000).unwrap();
        let trash = space.open_folder(".calternal/trash", true).unwrap();
        let item = Uuid::new_v4().to_string();
        let name = format!("{item}.md");
        trash
            .create_temp("fixture")
            .unwrap()
            .persist(|_| Some(name.clone()))
            .unwrap();
        let now = calternal_types::now_ms();
        space
            .db
            .execute("INSERT INTO trash VALUES(?1,?2)", params![item, now])
            .unwrap();
        let fresh = space
            .receipt("owner", Some("fresh"), b"request")
            .unwrap()
            .unwrap();
        fresh.store(&space.db, &json!({}), now).unwrap();
        let expired = space
            .receipt("owner", Some("expired"), b"request")
            .unwrap()
            .unwrap();
        expired
            .store(&space.db, &json!({}), now - space.retention_ms - 1)
            .unwrap();
        space.maintain_at(now).unwrap();
        assert!(trash.open_file(&name).is_err());
        let usage: (i64, i64) = space
            .db
            .query_row(
                "SELECT (SELECT count(*) FROM receipts), total FROM receipt_usage",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(usage, (1, 1));
    }
}
