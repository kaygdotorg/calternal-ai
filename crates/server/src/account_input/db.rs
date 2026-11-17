//! Owner-local SQL and bounded invalidation history. Events carry no content.
use super::{Inputs, internal};
use crate::auth::{Directory, Identity};
use calternal_core::AccountInputRecord;
use calternal_types::Error;
use calternal_types::{AccountId, Capability, SpaceId};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};
use uuid::Uuid;

impl Inputs {
    pub(super) fn open(&self, id: AccountId) -> Result<Connection, Error> {
        let dir = self.root.join("accounts").join(id.to_string());
        calternal_fsio::private::directory(&dir).map_err(internal)?;
        let path = dir.join("input.db");
        calternal_fsio::private::file(&path).map_err(internal)?;
        let c = Connection::open(&path).map_err(internal)?;
        c.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(internal)?;
        c.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA secure_delete=ON;",
        )
        .map_err(internal)?;
        let version: i64 = c
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(internal)?;
        if version == 0 {
            // The record body is the one authoritative copy of its ownership:
            // `space` and `item` are generated from it, so lookups and revocation
            // cleanup can never disagree with what is delivered. A draft's item has
            // one draft in one space; pending identities are (space, request ID).
            c.execute_batch("BEGIN IMMEDIATE;
CREATE TABLE binding(account TEXT PRIMARY KEY,epoch TEXT NOT NULL);
CREATE TABLE inputs(kind TEXT NOT NULL CHECK(kind IN ('draft','pending')),id TEXT NOT NULL,body TEXT NOT NULL,draft_revision TEXT,space TEXT GENERATED ALWAYS AS (json_extract(body,'$.space_id')) STORED NOT NULL,item TEXT GENERATED ALWAYS AS (json_extract(body,'$.item_id')) STORED NOT NULL,payload_bytes INTEGER GENERATED ALWAYS AS (length(CAST(coalesce(json_extract(body,'$.text'),json_extract(body,'$.request')) AS BLOB))) STORED);
CREATE UNIQUE INDEX inputs_key ON inputs(kind,space,id);
CREATE UNIQUE INDEX inputs_one_draft ON inputs(id) WHERE kind='draft';
CREATE INDEX inputs_space ON inputs(space);
CREATE INDEX inputs_budget ON inputs(kind,payload_bytes);
CREATE INDEX inputs_due ON inputs(kind,json_extract(body,'$.state'),json_extract(body,'$.created_at_ms'));
CREATE TABLE events(seq INTEGER PRIMARY KEY AUTOINCREMENT,token TEXT NOT NULL UNIQUE);
PRAGMA user_version=2;COMMIT;").map_err(internal)?;
            c.execute(
                "INSERT OR IGNORE INTO binding VALUES(?1,?2)",
                params![id.to_string(), Uuid::new_v4().to_string()],
            )
            .map_err(internal)?;
        } else if version != 2 {
            return Err(internal("schema"));
        }
        let bound: String = c
            .query_row("SELECT account FROM binding", [], |r| r.get(0))
            .map_err(internal)?;
        if bound != id.to_string() {
            return Err(internal("owner binding"));
        }
        Ok(c)
    }
    pub(super) fn change(&self, c: &Connection) -> Result<(), Error> {
        c.execute(
            "INSERT INTO events(token) VALUES(?1)",
            [Uuid::new_v4().to_string()],
        )
        .map_err(internal)?;
        c.execute(
            "DELETE FROM events WHERE seq <= (SELECT coalesce(max(seq),0)-?1 FROM events)",
            [self.limits.events as i64],
        )
        .map_err(internal)?;
        Ok(())
    }
    /// One record by its identity in `space`. Cost: one indexed read.
    pub(super) fn get(
        c: &Connection,
        kind: &str,
        space: &str,
        id: &str,
    ) -> Result<Option<AccountInputRecord>, Error> {
        c.query_row(
            "SELECT body FROM inputs WHERE kind=?1 AND space=?2 AND id=?3",
            params![kind, space, id],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(internal)?
        .map(|v| serde_json::from_str(&v).map_err(internal))
        .transpose()
    }
    /// The space holding the draft for `item`, if any: a draft's space never
    /// changes. Cost: one indexed read.
    pub(super) fn draft_space(c: &Connection, item: &str) -> Result<Option<String>, Error> {
        c.query_row(
            "SELECT space FROM inputs WHERE kind='draft' AND id=?1",
            [item],
            |r| r.get(0),
        )
        .optional()
        .map_err(internal)
    }
    pub(super) fn list(c: &Connection, kind: &str) -> Result<Vec<AccountInputRecord>, Error> {
        let mut q = c
            .prepare_cached("SELECT body FROM inputs WHERE kind=?1 ORDER BY id")
            .map_err(internal)?;
        let rows = q
            .query_map([kind], |r| r.get::<_, String>(0))
            .map_err(internal)?;
        rows.map(|r| serde_json::from_str(&r.map_err(internal)?).map_err(internal))
            .collect()
    }
    pub(super) fn put(
        &self,
        c: &Connection,
        kind: &str,
        record: &AccountInputRecord,
        draft: Option<&str>,
    ) -> Result<(), Error> {
        let body = serde_json::to_string(record).map_err(internal)?;
        let payload = record
            .text
            .as_ref()
            .map(|t| t.len())
            .unwrap_or_else(|| serde_json::to_vec(&record.request).expect("envelope").len());
        let (count, total): (i64, i64) = c
            .query_row(
                "SELECT count(*) FILTER(WHERE kind=?1),coalesce(sum(payload_bytes),0) FROM inputs",
                [kind],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(internal)?;
        let prior: Option<i64> = c
            .query_row(
                "SELECT payload_bytes FROM inputs WHERE kind=?1 AND space=?2 AND id=?3",
                params![kind, record.space_id, record.id],
                |r| r.get(0),
            )
            .optional()
            .map_err(internal)?;
        let cap = if kind == "draft" {
            self.limits.drafts
        } else {
            self.limits.pending
        };
        if (prior.is_none() && count >= cap as i64)
            || (total - prior.unwrap_or(0) + payload as i64 > self.limits.bytes as i64
                && payload as i64 > prior.unwrap_or(0))
        {
            return Err(Error::new(
                calternal_core::ErrorCode::TooLarge,
                "account input cap reached; send, erase or cancel input, or ask the operator to raise CALTERNAL_DRAFT_COUNT, CALTERNAL_PENDING_COUNT or CALTERNAL_INPUT_BYTES",
            ));
        }
        if prior.is_some() {
            c.execute(
                "UPDATE inputs SET body=?3 WHERE kind=?1 AND space=?4 AND id=?2",
                params![kind, record.id, body, record.space_id],
            )
        } else {
            c.execute(
                "INSERT INTO inputs(kind,id,body,draft_revision) VALUES(?1,?2,?3,?4)",
                params![kind, record.id, body, draft],
            )
        }
        .map_err(internal)?;
        self.change(c)
    }
    // State-only changes preserve payload bytes and bypass admission counters.
    // Each record is decoded/serialized once; age bursts do not rescan the queue.
    pub(super) fn set_state(
        &self,
        c: &Connection,
        row: &mut AccountInputRecord,
        state: &str,
    ) -> Result<(), Error> {
        row.state = state.into();
        row.revision = Uuid::now_v7().to_string();
        let body = serde_json::to_string(row).map_err(internal)?;
        c.execute(
            "UPDATE inputs SET body=?1 WHERE kind='pending' AND space=?2 AND id=?3",
            params![body, row.space_id, row.id],
        )
        .map_err(internal)?;
        self.change(c)
    }
    // Bounded by the account count ceilings. Never returns revoked content, including
    // a stale seven-day request. Revocation commits independently of this cleanup.
    pub(super) fn clean(
        &self,
        c: &mut Connection,
        d: &impl Directory,
        p: &Identity,
    ) -> Result<(), Error> {
        let before = c.total_changes();
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(internal)?;
        let spaces: Vec<String> = {
            let mut q = tx
                .prepare_cached("SELECT DISTINCT space FROM inputs")
                .map_err(internal)?;
            q.query_map([], |r| r.get(0))
                .map_err(internal)?
                .collect::<Result<_, _>>()
                .map_err(internal)?
        };
        for space in spaces {
            let readable = SpaceId::parse(&space)
                .ok()
                .and_then(|s| d.capabilities(p.id, s))
                .is_some_and(|c| c.contains(Capability::SpaceRead));
            if !readable {
                tx.execute("DELETE FROM inputs WHERE space=?1", [space])
                    .map_err(internal)?;
                self.change(&tx)?;
            }
        }
        let now = (self.clock)();
        let due: Vec<AccountInputRecord> = {
            let mut q=tx.prepare_cached("SELECT body FROM inputs WHERE kind='pending' AND json_extract(body,'$.state')='queued' AND json_extract(body,'$.created_at_ms')<?1").map_err(internal)?;
            let rows = q
                .query_map([now.saturating_sub(7 * 24 * 60 * 60 * 1000)], |r| {
                    r.get::<_, String>(0)
                })
                .map_err(internal)?;
            rows.map(|r| serde_json::from_str(&r.map_err(internal)?).map_err(internal))
                .collect::<Result<_, _>>()?
        };
        for mut row in due {
            if row.state == "queued"
                && now.saturating_sub(row.created_at_ms) > 7 * 24 * 60 * 60 * 1000
            {
                self.set_state(&tx, &mut row, "confirmation_required")?;
            }
        }
        let changed = tx.total_changes() > before;
        tx.commit().map_err(internal)?;
        if changed {
            self.wakes.committed(p.id);
        }
        Ok(())
    }
    pub(super) fn head(c: &Connection) -> Result<String, Error> {
        Ok(c.query_row(
            "SELECT token FROM events ORDER BY seq DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(internal)?
        .unwrap_or(
            c.query_row("SELECT epoch FROM binding", [], |r| r.get(0))
                .map_err(internal)?,
        ))
    }
    pub(super) fn page(c: &Connection, after: &str) -> Result<Option<Vec<String>>, Error> {
        let epoch: String = c
            .query_row("SELECT epoch FROM binding", [], |r| r.get(0))
            .map_err(internal)?;
        let min: i64 = c
            .query_row("SELECT coalesce(min(seq),1) FROM events", [], |r| r.get(0))
            .map_err(internal)?;
        let seq = if after == epoch && min == 1 {
            Some(0)
        } else {
            c.query_row("SELECT seq FROM events WHERE token=?1", [after], |r| {
                r.get::<_, i64>(0)
            })
            .optional()
            .map_err(internal)?
        };
        let Some(seq) = seq else { return Ok(None) };
        let mut q = c
            .prepare_cached("SELECT token FROM events WHERE seq>?1 ORDER BY seq LIMIT 64")
            .map_err(internal)?;
        Ok(Some(
            q.query_map([seq], |r| r.get(0))
                .map_err(internal)?
                .collect::<Result<_, _>>()
                .map_err(internal)?,
        ))
    }
    pub(super) fn result(record: Option<AccountInputRecord>, result: Option<Value>) -> Value {
        json!({"record":record,"result":result})
    }
}

impl Inputs {
    /// Erase one revoked account/space's unsent input after authority commits.
    /// One owner-local transaction and event; revocation itself never waits for
    /// this cleanup to stop access. No private content is retained in events.
    pub fn erase_space(&self, id: AccountId, space: SpaceId) -> Result<(), Error> {
        if !self
            .root
            .join("accounts")
            .join(id.to_string())
            .join("input.db")
            .exists()
        {
            return Ok(());
        }
        let mut c = self.open(id)?;
        let before = c.total_changes();
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(internal)?;
        if tx
            .execute("DELETE FROM inputs WHERE space=?1", [space.to_string()])
            .map_err(internal)?
            > 0
        {
            self.change(&tx)?;
        }
        let changed = tx.total_changes() > before;
        tx.commit().map_err(internal)?;
        if changed {
            self.wakes.committed(id);
        }
        Ok(())
    }
}

impl Inputs {
    // Owner-local advisory slot locks bound connection admission across processes.
    // Held for the HTTP body lifetime, never by mutations or publication. Files
    // are empty appdata state, bounded by the operator listener ceiling, and reused.
    pub(super) fn admit(&self, id: AccountId) -> Result<std::fs::File, Error> {
        let dir = self.root.join("accounts").join(id.to_string()).join("live");
        calternal_fsio::private::directory(&dir).map_err(internal)?;
        for slot in 0..self.limits.listeners {
            let file = calternal_fsio::private::file(&dir.join(format!("{slot}.lock")))
                .map_err(internal)?;
            match file.try_lock() {
                Ok(()) => return Ok(file),
                Err(std::fs::TryLockError::WouldBlock) => {}
                Err(e) => return Err(internal(e)),
            }
        }
        Err(Error::unavailable(
            "account live subscriber cap reached; close a connection or raise CALTERNAL_LIVE_LISTENERS",
        ))
    }
}

impl Inputs {
    // Completed input reuses the ordinary per-space receipt (request IDs are scoped
    // to account and space); no second result log or copy of private input exists.
    // Cost: one capability lookup, then one journal receipt read in that space.
    pub(super) fn receipt(
        &self,
        d: &impl Directory,
        p: &Identity,
        space: SpaceId,
        id: &str,
    ) -> Result<Option<calternal_content::Receipt>, Error> {
        let Some(content) = &self.content else {
            return Ok(None);
        };
        if !d
            .capabilities(p.id, space)
            .is_some_and(|c| c.contains(Capability::ContentWrite))
        {
            return Err(Error::not_found("input not found"));
        }
        content.receipt(space, p.id, id).map_err(internal)
    }
}
