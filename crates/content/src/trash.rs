//! Per-space Trash queries and recoverable permanent removal.
use crate::{
    Content, Error, FaultPoint, Item, Request, Result,
    db::Space,
    types::{blocked, internal, missing},
};
use calternal_types::{AccountId, ErrorCode, SpaceId};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};
use std::fs;
use uuid::Uuid;

pub(crate) fn install(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS trash_origins(
            item TEXT PRIMARY KEY REFERENCES items(id), path TEXT NOT NULL
        ) WITHOUT ROWID, STRICT;",
    )
    .map_err(internal)
}

impl Content {
    /// Fetch a current live item after space authorization, hiding Trash and purged
    /// items without reading the file. Cost: one indexed head/version join.
    pub fn get_live(&self, space: SpaceId, item: Uuid) -> Result<Item> {
        self.with_space(space, |s| s.get_selected(item, true))
    }

    /// Read a bounded history page while the selected item is live in that kind.
    /// Existence and the page are read under one space lock. Cost: indexed head
    /// and cursor reads plus the requested version-page allocation.
    pub fn history_live(
        &self,
        space: SpaceId,
        kind: &str,
        item: Uuid,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<(Vec<Item>, Option<String>)> {
        self.with_space(space, |s| s.history(item, limit, cursor, Some(kind)))
    }

    /// Read a live item's metadata in an authorized space without loading its
    /// current body or reading the userdata file. Cost: one indexed head read.
    pub fn live_metadata(&self, space: SpaceId, kind: &str, item: Uuid) -> Result<Value> {
        self.with_space(space, |s| {
            let metadata: String = s.conn.query_row(
                "SELECT metadata FROM items WHERE id=?1 AND kind=?2 AND substr(path,1,7) <> '.trash/'",
                params![item.to_string(),kind], |r| r.get(0),
            ).optional().map_err(internal)?.ok_or_else(missing)?;
            serde_json::from_str(&metadata).map_err(internal)
        })
    }

    /// Read an item's state after authorization. The permanent identity remains
    /// routable after purge. Cost: one indexed head lookup without loading bytes.
    pub fn item_state(&self, space: SpaceId, item: Uuid) -> Result<&'static str> {
        self.with_space(space, |s| {
            s.conn
                .query_row(
                    "SELECT CASE WHEN mat_state IN ('purged','purging') THEN 'deleted'
                             WHEN substr(path,1,7) = '.trash/' THEN 'in_trash'
                             ELSE 'active' END FROM items WHERE id=?1",
                    [item.to_string()],
                    |r| match r.get::<_, String>(0)?.as_str() {
                        "deleted" => Ok("deleted"),
                        "in_trash" => Ok("in_trash"),
                        _ => Ok("active"),
                    },
                )
                .optional()
                .map_err(internal)?
                .ok_or_else(missing)
        })
    }

    /// Purge a trashed item and all its content versions. A durable intent precedes
    /// unlink, and replay finishes unlink before journal publication. The stable ID
    /// remains reserved. A live item or a mismatched on-disk file is preserved.
    /// Cost: indexed reads, one file hash, one directory fsync and durable
    /// intent, finalization, journal and retirement transactions in this space.
    pub fn purge(
        &self,
        space: SpaceId,
        kind: &str,
        item: Uuid,
        request: Request<'_>,
    ) -> Result<Value> {
        self.with_space(space, |s| self.purge_selected(s, kind, item, None, request))
    }

    /// Sweep one physical space in bounded pages, unblocking other requests
    /// between items. The cutoff is rechecked under the lock before each purge so
    /// a restore followed by another delete cannot inherit the old deadline.
    /// Cost: indexed pages plus the ordinary purge cost per expired item.
    pub fn expire_trash(
        &self,
        space: SpaceId,
        kind: &str,
        cutoff: &str,
        operation: &str,
    ) -> Result<()> {
        let mut cursor = (String::new(), String::new());
        loop {
            let rows = self.with_space(space, |s| {
                let mut stmt = s
                    .conn
                    .prepare_cached(
                        "SELECT i.updated_at,i.id,v.author
                     FROM items i JOIN versions v ON v.item=i.id AND v.seq=i.seq
                     WHERE i.kind=?1 AND substr(i.path,1,7) = '.trash/'
                     AND i.mat_state NOT IN ('purged','blocked') AND i.updated_at < ?2
                     AND (i.updated_at,i.id) > (?3,?4)
                     ORDER BY i.updated_at,i.id LIMIT 100",
                    )
                    .map_err(internal)?;
                let rows = stmt
                    .query_map(params![kind, cutoff, cursor.0, cursor.1], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                        ))
                    })
                    .map_err(internal)?;
                rows.collect::<rusqlite::Result<Vec<_>>>().map_err(internal)
            })?;
            if rows.is_empty() {
                return Ok(());
            }
            for (at, id, author) in rows {
                let item = Uuid::parse_str(&id).map_err(internal)?;
                let account = AccountId::parse(&author).map_err(internal)?;
                // Recovery retains the admitted ID in the outbox. A fresh sweep
                // can generate a fresh ID because a purged item is never eligible.
                let rid = Uuid::now_v7().to_string();
                let result = self.with_space(space, |s| {
                    self.purge_selected(
                        s,
                        kind,
                        item,
                        Some(cutoff),
                        Request {
                            account,
                            id: &rid,
                            operation,
                            fingerprint: &rid,
                        },
                    )
                });
                if let Err(e) = result
                    && !matches!(e.code, ErrorCode::NotFound | ErrorCode::Conflict)
                {
                    return Err(e);
                }
                cursor = (at, id);
            }
        }
    }

    fn purge_selected(
        &self,
        s: &mut Space,
        kind: &str,
        id: Uuid,
        cutoff: Option<&str>,
        request: Request<'_>,
    ) -> Result<Value> {
        self.publish_pending(s)?;
        if let Some(saved) = self
            .journals
            .receipt(&s.journal, request.account, request.id)?
        {
            if saved.operation == request.operation && saved.fingerprint == request.fingerprint {
                return Ok(saved.result);
            }
            return Err(Error::request_reused());
        }
        let (path, state, revision): (String, String, String) = s
            .conn
            .query_row(
                "SELECT path,mat_state,revision FROM items WHERE id=?1 AND kind=?2
             AND (?3 IS NULL OR updated_at < ?3)",
                params![id.to_string(), kind, cutoff],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(internal)?
            .ok_or_else(missing)?;
        if state == "purged" {
            return Err(missing());
        }
        if !path.starts_with(".trash/") {
            return Err(Error::conflict(
                "only items in Trash can be permanently deleted",
            ));
        }
        s.place(id, self.fault)?;
        let response = json!({"id":id, "state":"deleted"});
        let tx = s
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(internal)?;
        tx.execute(
            "UPDATE items SET mat_state='purging' WHERE id=?1",
            [id.to_string()],
        )
        .map_err(internal)?;
        tx.execute(
            "INSERT INTO outbox VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                request.account.to_string(),
                request.id,
                request.operation,
                request.fingerprint,
                id.to_string(),
                revision,
                response.to_string(),
            ],
        )
        .map_err(internal)?;
        tx.commit().map_err(internal)?;
        (self.fault)(FaultPoint::AfterCommit);
        self.publish_pending(s)?;
        if !s
            .conn
            .query_row(
                "SELECT mat_state='purged' FROM items WHERE id=?1",
                [id.to_string()],
                |r| r.get::<_, bool>(0),
            )
            .map_err(internal)?
        {
            return Err(blocked());
        }
        Ok(response)
    }
}

impl Space {
    pub(crate) fn trash_origin(&self, id: Uuid) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT path FROM trash_origins WHERE item=?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .optional()
            .map_err(internal)
    }

    // Called under the per-space writer lock before outbox publication; the lock
    // spans unlink and fsync so restore cannot race permanent removal.
    pub(crate) fn finish_purge(
        &mut self,
        id: &str,
        account: &str,
        request: &str,
        fault: fn(FaultPoint),
    ) -> Result<bool> {
        let intent: Option<(String, Option<String>)> = self
            .conn
            .query_row(
                "SELECT path,file_hash FROM items WHERE id=?1 AND mat_state='purging'",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(internal)?;
        let Some((path, expected)) = intent else {
            return Ok(true);
        };
        // The only admissible purge target is the same UUID's file in this
        // space's real Trash directory, even during recovery.
        if !path
            .strip_prefix(".trash/")
            .and_then(|name| name.split_once('.'))
            .is_some_and(|(stem, ext)| {
                stem == id && !ext.is_empty() && ext.bytes().all(|b| b.is_ascii_alphanumeric())
            })
        {
            return Err(internal("invalid Trash path in purge intent"));
        }
        let path = self.files.join(path);
        let dir = path.parent().expect("validated Trash path");
        if !fs::symlink_metadata(dir).map_err(internal)?.is_dir() {
            return Err(internal("Trash directory is not a real directory"));
        }
        fault(FaultPoint::BeforePlacement);
        let removed = calternal_fsio::remove_unless_changed(&path, |bytes| {
            expected.as_deref() == Some(crate::file_hash(bytes).as_str())
        })
        .map_err(internal)?;
        if !removed {
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(internal)?;
            tx.execute("UPDATE items SET mat_state='blocked' WHERE id=?1", [id])
                .map_err(internal)?;
            tx.execute(
                "DELETE FROM outbox WHERE account=?1 AND request=?2",
                [account, request],
            )
            .map_err(internal)?;
            tx.commit().map_err(internal)?;
            return Ok(false);
        }
        // Covers replay after an unlink followed by interruption before its sync.
        calternal_fsio::sync_dir(dir).map_err(internal)?;
        fault(FaultPoint::AfterPlacement);
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(internal)?;
        tx.execute("DELETE FROM trash_origins WHERE item=?1", [id])
            .map_err(internal)?;
        tx.execute("DELETE FROM versions WHERE item=?1", [id])
            .map_err(internal)?;
        tx.execute(
            "UPDATE items SET metadata='{}',file_hash=NULL,old_path=NULL,mat_state='purged' WHERE id=?1",
            [id],
        ).map_err(internal)?;
        tx.commit().map_err(internal)?;
        Ok(true)
    }
}
