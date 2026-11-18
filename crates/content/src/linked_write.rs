//! Shared revision staging and indexed same-kind edits for a path change.
use crate::{Content, FaultPoint, Item, Request, Result, db::Space, now_rfc3339, types::internal};
use calternal_types::Error;
use rusqlite::{Transaction, params};
use serde_json::{Value, json};
use uuid::Uuid;

pub(crate) type Retarget<'a> = dyn Fn(&Item, &str, &str) -> Result<Option<(Value, Vec<u8>)>> + 'a;

const MAX_RELATED: usize = 1000;
const MAX_RELATED_BYTES: usize = 32 * 1024 * 1024;

pub(crate) struct Staged {
    pub(crate) item: Item,
    pub(crate) seq: i64,
    pub(crate) path: String,
    pub(crate) expected: Option<String>,
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
               folded_path=excluded.folded_path,old_path=excluded.old_path,mat_state='pending'",
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
}

pub(crate) const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS outbox_placement(
            account TEXT NOT NULL, request TEXT NOT NULL, item TEXT NOT NULL REFERENCES items(id),
            PRIMARY KEY(account,request,item),
            FOREIGN KEY(account,request) REFERENCES outbox(account,request) ON DELETE CASCADE
        ) WITHOUT ROWID, STRICT;";

impl Space {
    pub(crate) fn stage_related(
        &mut self,
        root: &mut Staged,
        old: &str,
        retarget: &Retarget<'_>,
        fault: fn(FaultPoint),
    ) -> Result<Vec<Staged>> {
        let ids = {
            let mut stmt = self
                .conn
                .prepare_cached(
                    "SELECT item FROM item_terms WHERE kind=?1 AND category='link' AND name=?2
                 AND item<>?3 ORDER BY item LIMIT ?4",
                )
                .map_err(internal)?;
            let rows = stmt
                .query_map(
                    params![
                        root.item.kind,
                        calternal_fsio::names::fold(old),
                        root.item.id.to_string(),
                        (MAX_RELATED + 1) as i64,
                    ],
                    |r| r.get::<_, String>(0),
                )
                .map_err(internal)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(internal)?
        };
        if ids.len() > MAX_RELATED {
            return Err(Error::unavailable(
                "rename has too many links for one durable write",
            ));
        }
        if let Some((metadata, bytes)) = retarget(&root.item, old, &root.path)? {
            Content::validate_write_size(&metadata, &bytes)?;
            root.item.metadata = metadata;
            root.item.bytes = bytes;
        }
        let mut staged = Vec::with_capacity(ids.len());
        let mut bytes_total = 0;
        for id in ids {
            let (seq, path): (i64, String) = self
                .conn
                .query_row("SELECT seq,path FROM items WHERE id=?1", [&id], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .map_err(internal)?;
            let id = Uuid::parse_str(&id).map_err(internal)?;
            let mut source = self.get_selected(id, true)?;
            let Some((metadata, bytes)) = retarget(&source, old, &root.path)? else {
                continue;
            };
            Content::validate_write_size(&metadata, &bytes)?;
            bytes_total += bytes.len() + metadata.to_string().len();
            if bytes_total > MAX_RELATED_BYTES {
                return Err(Error::unavailable(
                    "rename link edits exceed the durable write budget",
                ));
            }
            self.place(id, fault)?;
            let expected = self.check_file(id)?;
            source.metadata = metadata;
            source.bytes = bytes;
            source.revision = Staged::version(seq + 1);
            source.updated_at = now_rfc3339();
            staged.push(Staged {
                item: source,
                seq: seq + 1,
                path,
                expected,
            });
        }
        Ok(staged)
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
                .prepare_cached("SELECT item FROM outbox_placement WHERE account=?1 AND request=?2")
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

pub(crate) fn stage_related_outbox(
    tx: &Transaction<'_>,
    root: &Staged,
    sources: &[Staged],
    request: Request<'_>,
) -> Result<()> {
    if sources.is_empty() {
        return Ok(());
    }
    let account = request.account.to_string();
    for item in sources.iter().chain(std::iter::once(root)) {
        tx.execute(
            "INSERT INTO outbox_placement VALUES(?1,?2,?3)",
            params![account, request.id, item.item.id.to_string()],
        )
        .map_err(internal)?;
    }
    for source in sources {
        source.write(tx, None, &account)?;
        let rid = Uuid::now_v7().to_string();
        source.outbox(
            tx,
            Request {
                id: &rid,
                fingerprint: &rid,
                ..request
            },
            &json!({"id":source.item.id}),
        )?;
    }
    Ok(())
}
