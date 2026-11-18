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

pub(crate) const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS trash_origins(
            item TEXT PRIMARY KEY REFERENCES items(id), path TEXT NOT NULL
        ) WITHOUT ROWID, STRICT;";

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
        let _actor = self.lifecycle.step(request.account.as_uuid())?;
        self.with_space(space, |s| {
            self.purge_selected(s, kind, item, None, None, request)
        })
    }

    /// Purge at most 16 expired items and release the space writer between
    /// items. Rechecks the deletion deadline at placement so a restored and
    /// re-deleted item keeps its own retention. True means to schedule another turn.
    pub fn expire_trash(
        &self,
        space: SpaceId,
        kind: &str,
        cutoff: &str,
        operation: &str,
    ) -> Result<bool> {
        const PAGE: usize = 16;
        let rows = self.with_space(space, |s| {
            let mut stmt = s
                .conn
                .prepare_cached(
                    "SELECT i.id,v.author
                 FROM items i JOIN versions v ON v.item=i.id AND v.seq=i.seq
                 WHERE i.kind=?1 AND substr(i.path,1,7) = '.trash/'
                 AND i.mat_state NOT IN ('purged','blocked') AND i.updated_at < ?2
                 ORDER BY i.updated_at,i.id LIMIT ?3",
                )
                .map_err(internal)?;
            stmt.query_map(params![kind, cutoff, PAGE as i64], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(internal)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(internal)
        })?;
        let more = rows.len() == PAGE;
        for (id, author) in rows {
            let item = Uuid::parse_str(&id).map_err(internal)?;
            let account = AccountId::parse(&author).map_err(internal)?;
            let rid = Uuid::now_v7().to_string();
            let result = self.with_space(space, |s| {
                self.purge_selected(
                    s,
                    kind,
                    item,
                    Some(cutoff),
                    None,
                    Request {
                        account,
                        via: None,
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
        }
        Ok(more)
    }

    pub(crate) fn purge_selected(
        &self,
        s: &mut Space,
        kind: &str,
        id: Uuid,
        cutoff: Option<&str>,
        expected_revision: Option<&str>,
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
             AND (?3 IS NULL OR updated_at < ?3) AND (?4 IS NULL OR revision=?4)",
                params![id.to_string(), kind, cutoff, expected_revision],
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
                request.outbox_result(&response),
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
        operation: &str,
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
        crate::trash_job::retire_purge(&tx, operation, request, id)?;
        tx.commit().map_err(internal)?;
        Ok(true)
    }
}
