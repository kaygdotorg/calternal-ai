//! Shared revision staging and indexed same-kind edits for a path change.
use crate::{Content, FaultPoint, Item, Request, Result, db::Space, now_rfc3339, types::internal};
use calternal_types::Error;
use rusqlite::{Transaction, params};
use serde_json::Value;
use uuid::Uuid;

pub(crate) type Retarget<'a> = dyn Fn(&Item, &str, &str) -> Result<Option<(Value, Vec<u8>)>> + 'a;

pub(crate) const BATCH: usize = 8;
const MAX_RELATED: usize = 250_000;

pub(crate) struct Staged {
    pub(crate) item: Item,
    pub(crate) seq: i64,
    pub(crate) path: String,
    pub(crate) expected: Option<String>,
    pub(crate) room: Option<crate::rooms::Checkpoint>,
}

impl Staged {
    pub(crate) fn version(seq: i64) -> String {
        format!("v{seq}.{}", Uuid::now_v7().simple())
    }

    pub(crate) fn write(
        &self,
        tx: &Transaction<'_>,
        old_path: Option<&str>,
        account: &str,
    ) -> Result<()> {
        let item = &self.item;
        let metadata = item.metadata.to_string();
        tx.execute(
            "INSERT INTO items(id,kind,metadata,revision,seq,updated_at,path,folded_path,old_path,mat_state,file_hash)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'pending',?10)
             ON CONFLICT(id) DO UPDATE SET metadata=excluded.metadata,revision=excluded.revision,
               seq=excluded.seq,updated_at=excluded.updated_at,path=excluded.path,
               folded_path=excluded.folded_path,old_path=excluded.old_path,mat_state='pending',file_hash=excluded.file_hash",
            params![
                item.id.to_string(),item.kind,metadata,item.revision,self.seq,item.updated_at,
                self.path,calternal_fsio::names::fold(&self.path),
                old_path.filter(|old| *old != self.path),self.expected,
            ],
        ).map_err(internal)?;
        tx.execute(
            "INSERT INTO versions VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                item.id.to_string(),
                self.seq,
                item.revision,
                metadata,
                item.bytes,
                item.updated_at,
                account
            ],
        )
        .map_err(internal)?;
        if let Some(room) = &self.room {
            room.write(tx, item.id, self.seq)?;
        }
        Ok(())
    }

    pub(crate) fn outbox(
        &self,
        tx: &Transaction<'_>,
        request: Request<'_>,
        response: &Value,
    ) -> Result<()> {
        tx.execute(
            "INSERT INTO outbox VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                request.account.to_string(),
                request.id,
                request.operation,
                request.fingerprint,
                self.item.id.to_string(),
                self.item.revision,
                request.outbox_result(response),
            ],
        )
        .map_err(internal)?;
        Ok(())
    }
    pub(crate) fn require_placement(
        &self,
        tx: &Transaction<'_>,
        request: Request<'_>,
    ) -> Result<()> {
        tx.execute(
            "INSERT INTO outbox_placement VALUES(?1,?2,?3)",
            params![
                request.account.to_string(),
                request.id,
                self.item.id.to_string()
            ],
        )
        .map_err(internal)?;
        Ok(())
    }
}

pub(crate) const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS outbox_placement(
            account TEXT NOT NULL, request TEXT NOT NULL, item TEXT NOT NULL REFERENCES items(id),
            PRIMARY KEY(account,request,item),
            FOREIGN KEY(account,request) REFERENCES outbox(account,request) ON DELETE CASCADE
        ) WITHOUT ROWID, STRICT;";

impl Space {
    pub(crate) fn related_revisions(
        &self,
        root: &Staged,
        old: &str,
    ) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT i.id,i.revision FROM item_terms t JOIN items i ON i.id=t.item
             WHERE t.kind=?1 AND t.category='link' AND t.name=?2 AND t.item<>?3
             ORDER BY t.item LIMIT ?4",
            )
            .map_err(internal)?;
        let rows = stmt
            .query_map(
                params![
                    root.item.kind,
                    calternal_fsio::names::fold(old),
                    root.item.id.to_string(),
                    (MAX_RELATED + 1) as i64
                ],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(internal)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(internal)?;
        if rows.len() > MAX_RELATED {
            return Err(Error::unavailable("rename exceeds the related-note limit"));
        }
        Ok(rows)
    }

    pub(crate) fn stage_link_edit(
        &mut self,
        id: Uuid,
        base: &str,
        edit: impl FnOnce(&Item) -> Result<Option<(Value, Vec<u8>)>>,
        fault: fn(FaultPoint),
    ) -> Result<Option<Staged>> {
        let mut head = self.header(id)?;
        if head.kind != "notes" || head.path.starts_with(".trash/") || head.revision != base {
            return Err(Error::conflict(
                "a note changed since the rename was planned",
            ));
        }
        let mut source = self.get_selected(id, true)?;
        let Some((mut metadata, mut bytes)) = edit(&source)? else {
            return Ok(None);
        };
        Content::validate_write_size(&source.kind, &metadata, &bytes)?;
        if head.state == "pending" {
            self.place_header(id, head, Some(&source), fault)?;
            head = self.header(id)?;
        }
        let expected = self.check_header(id, &head)?;
        let room = if self.live_editing(&source.kind)? {
            let (merged, checkpoint, _) = self.merge_room_write(
                &source,
                head.seq,
                base,
                &mut metadata,
                crate::rooms::Body::Replace(&bytes),
            )?;
            Content::validate_write_size(&source.kind, &metadata, &merged)?;
            bytes = merged;
            Some(checkpoint)
        } else {
            None
        };
        source.metadata = metadata;
        source.bytes = bytes;
        source.revision = Staged::version(head.seq + 1);
        source.updated_at = now_rfc3339();
        Ok(Some(Staged {
            item: source,
            seq: head.seq + 1,
            path: head.path,
            expected,
            room,
        }))
    }

    pub(crate) fn place_related(
        &mut self,
        account: &str,
        request: &str,
        fault: fn(FaultPoint),
    ) -> Result<()> {
        let ids = {
            let mut stmt = self
                .conn
                .prepare_cached(
                    "SELECT p.item FROM outbox_placement p JOIN outbox o USING(account,request)
                     WHERE p.account=?1 AND p.request=?2 ORDER BY p.item<>o.item,p.item",
                )
                .map_err(internal)?;
            let rows = stmt
                .query_map(params![account, request], |r| r.get::<_, String>(0))
                .map_err(internal)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(internal)?
        };
        for id in ids {
            self.place(Uuid::parse_str(&id).map_err(internal)?, fault)?;
        }
        Ok(())
    }
}
