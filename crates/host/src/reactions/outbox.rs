//! Persistent preparation and acknowledgement of change deliveries.

use super::{Observation, Prepared, observation_limit, snapshot};
use crate::{internal, maintenance::Slot, space::Space};
use calternal_types::{ErrorCode, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use std::collections::BTreeSet;
use uuid::Uuid;

pub(super) const OUTBOX_BYTES: usize = 8 * 1024 * 1024;
const OUTBOX_EVENTS: usize = 128;
pub(super) const OUTBOX_REQUESTS: i64 = 32;

pub(super) struct Claim {
    slot: Slot,
    pub(super) id: [u8; 32],
}

impl Claim {
    fn acquire(slot: &Slot, id: [u8; 32]) -> Result<Option<Self>> {
        let mut observing = slot.observing.lock().map_err(internal)?;
        Ok(observing.insert(id).then(|| Self {
            slot: slot.clone(),
            id,
        }))
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        if let Ok(mut observing) = self.slot.observing.lock() {
            observing.remove(&self.id);
        }
    }
}

pub(super) fn has_outbox(db: &Connection) -> Result<bool> {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='pending_observations')",
        [],
        |row| row.get(0),
    )
    .map_err(internal)
}

pub(crate) fn expire_pending(db: &Connection, cutoff: i64) -> Result<()> {
    if has_outbox(db)? {
        db.execute(
            "DELETE FROM pending_observations WHERE rowid IN (
                SELECT rowid FROM pending_observations WHERE at < ?1 ORDER BY at LIMIT 128
            )",
            [cutoff],
        )
        .map_err(internal)?;
        db.execute(
            "DELETE FROM prepared_observations WHERE rowid IN (
                SELECT rowid FROM prepared_observations WHERE at < ?1 ORDER BY at LIMIT 32
            )",
            [cutoff],
        )
        .map_err(internal)?;
    }
    Ok(())
}

impl Space {
    pub(super) fn initialize_outbox(&mut self) -> Result<()> {
        let tx = self.db.transaction().map_err(internal)?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS pending_observations (
                gesture BLOB NOT NULL, event TEXT NOT NULL, at INTEGER NOT NULL,
                data BLOB NOT NULL, PRIMARY KEY(gesture,event)
             );
             CREATE INDEX IF NOT EXISTS pending_observations_age ON pending_observations(at);
             CREATE TABLE IF NOT EXISTS prepared_observations (
                gesture BLOB PRIMARY KEY, writer TEXT NOT NULL, write_key BLOB NOT NULL,
                at INTEGER NOT NULL, data BLOB NOT NULL
             );
             CREATE INDEX IF NOT EXISTS prepared_observations_age ON prepared_observations(at);",
        )
        .map_err(internal)?;
        tx.commit().map_err(internal)
    }

    pub(super) fn recover_observations(&mut self, cutoff: i64) -> Result<()> {
        for _ in 0..OUTBOX_REQUESTS {
            // A receipt proves the effect committed. Preparations without one
            // can still be retried or expire without delivering a change.
            let committed = self
                .db
                .query_row(
                    "SELECT p.gesture,p.data,r.result,r.at FROM prepared_observations p
                 JOIN receipts r ON r.actor=p.writer AND r.gesture=p.write_key
                 WHERE p.at>=?1 AND r.at>=p.at ORDER BY p.at LIMIT 1",
                    [cutoff],
                    |row| {
                        Ok((
                            row.get::<_, Vec<u8>>(0)?,
                            row.get::<_, Vec<u8>>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                        ))
                    },
                )
                .optional()
                .map_err(internal)?;
            let Some((key, prepared, receipt, at)) = committed else {
                break;
            };
            let key = key
                .try_into()
                .map_err(|_| internal("invalid observation key"))?;
            let prepared = serde_json::from_slice(&prepared).map_err(internal)?;
            let receipt = serde_json::from_str(&receipt).map_err(internal)?;
            self.finish_prepared(&prepared, &receipt, key, at)?;
        }
        Ok(())
    }

    pub(super) fn remove_prepared(&self, key: &[u8; 32]) -> Result<()> {
        self.db
            .execute(
                "DELETE FROM prepared_observations WHERE gesture=?1",
                [&key[..]],
            )
            .map_err(internal)?;
        Ok(())
    }

    pub(super) fn finish_prepared(
        &mut self,
        prepared: &Prepared,
        result: &Value,
        key: [u8; 32],
        at: i64,
    ) -> Result<()> {
        match self.enqueue_observations(prepared, result, key, at) {
            Err(error) if error.code == ErrorCode::TooLarge => {
                self.remove_prepared(&key)?;
                eprintln!("calternal-host: committed change exceeded the observation budget");
                Ok(())
            }
            result => result,
        }
    }

    fn enqueue_observations(
        &mut self,
        prepared: &Prepared,
        result: &Value,
        outbox_key: [u8; 32],
        now: i64,
    ) -> Result<()> {
        let Prepared {
            actor,
            key,
            before,
            listeners,
        } = prepared;
        let ids = result["changes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|change| Uuid::parse_str(change.get("id")?.as_str()?).ok())
            .collect::<BTreeSet<_>>();
        let mut events = Vec::new();
        let mut event_bytes = 0usize;
        for id in ids {
            let previous = before.get(&id);
            let current = self.rows.get(&id);
            if current.is_some_and(|row| {
                !listeners
                    .iter()
                    .any(|listener| listener.kind == row.head.kind)
            }) {
                continue;
            }
            if current.is_some_and(|row| row.head.prefix as usize > OUTBOX_BYTES - event_bytes) {
                return Err(observation_limit());
            }
            let after = current.map(snapshot).unwrap_or_default();
            if previous.map(|before| &before.row) == Some(&after) {
                continue;
            }
            for (index, listener) in listeners.iter().enumerate() {
                let before_member = previous.is_some_and(|before| before.matches[index]);
                let after_member = current.is_some_and(|row| self.matches(actor, listener, row));
                if before_member || after_member {
                    let after_bytes = current.map_or(0, |row| row.head.prefix as usize);
                    let before_bytes = previous.map_or(0, |before| before.bytes);
                    event_bytes = event_bytes
                        .saturating_add(after_bytes)
                        .saturating_add(before_bytes);
                    if events.len() == OUTBOX_EVENTS || event_bytes > OUTBOX_BYTES {
                        return Err(observation_limit());
                    }
                    let key = blake3::Hash::from_bytes(
                        self.hash(
                            &serde_json::to_vec(&(
                                actor,
                                &key,
                                id,
                                &listener.plugin,
                                &listener.selector,
                            ))
                            .map_err(internal)?,
                        ),
                    )
                    .to_hex()
                    .to_string();
                    events.push(Observation {
                        plugin: listener.plugin.clone(),
                        selector: listener.selector.clone(),
                        item: id,
                        key,
                        before: previous.map(|before| before.row.clone()),
                        after: after.clone(),
                    });
                }
            }
        }
        let tx = self.db.transaction().map_err(internal)?;
        tx.execute(
            "DELETE FROM pending_observations WHERE gesture=?1",
            [&outbox_key[..]],
        )
        .map_err(internal)?;
        let mut remaining = OUTBOX_BYTES;
        for event in events {
            let data = calternal_plugin_host::bounded_json(&event, remaining)
                .map_err(|_| observation_limit())?;
            remaining -= data.len();
            tx.execute(
                "INSERT INTO pending_observations VALUES (?1,?2,?3,?4)",
                params![&outbox_key[..], event.key, now, data],
            )
            .map_err(internal)?;
        }
        tx.execute(
            "DELETE FROM prepared_observations WHERE gesture=?1",
            [&outbox_key[..]],
        )
        .map_err(internal)?;
        tx.commit().map_err(internal)
    }

    pub(super) fn claim_observations(
        &self,
        slot: &Slot,
        key: [u8; 32],
        cutoff: i64,
    ) -> Result<Option<(Claim, Vec<Observation>)>> {
        let Some(claim) = Claim::acquire(slot, key)? else {
            return Ok(None);
        };
        let mut query = self
            .db
            .prepare_cached(
                "SELECT data FROM pending_observations WHERE gesture=?1 AND at>=?2 ORDER BY rowid",
            )
            .map_err(internal)?;
        let mut rows = query.query(params![&key[..], cutoff]).map_err(internal)?;
        let mut events = Vec::new();
        while let Some(row) = rows.next().map_err(internal)? {
            let data = row
                .get_ref(0)
                .map_err(internal)?
                .as_blob()
                .map_err(internal)?;
            events.push(serde_json::from_slice(data).map_err(internal)?);
        }
        Ok((!events.is_empty()).then_some((claim, events)))
    }

    pub(super) fn ack_observation(&self, gesture: &[u8; 32], key: &str) -> Result<()> {
        self.db
            .execute(
                "DELETE FROM pending_observations WHERE gesture=?1 AND event=?2",
                params![&gesture[..], key],
            )
            .map_err(internal)?;
        Ok(())
    }
}
