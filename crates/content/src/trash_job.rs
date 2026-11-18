//! Fixed-revision Empty Trash selection and bounded durable progress.
use crate::{
    Content, Request, Result, SpaceId,
    types::{internal, missing},
};
use calternal_types::{AccountId, Error, ErrorCode};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};
use uuid::Uuid;

pub(crate) const SCHEMA: &str = "
    CREATE TABLE notes_trash_jobs(
      id TEXT PRIMARY KEY, actor TEXT NOT NULL, count INTEGER NOT NULL,
      bytes INTEGER NOT NULL, purged INTEGER NOT NULL DEFAULT 0,
      skipped INTEGER NOT NULL DEFAULT 0, finished TEXT
    ) STRICT;
    CREATE UNIQUE INDEX notes_trash_active ON notes_trash_jobs((1)) WHERE finished IS NULL;
    CREATE TABLE notes_trash_targets(
      job TEXT NOT NULL REFERENCES notes_trash_jobs(id) ON DELETE CASCADE,
      item TEXT NOT NULL, revision TEXT NOT NULL, PRIMARY KEY(job,item)
    ) WITHOUT ROWID, STRICT;";

// The candidate snapshot contains only Notes. Other kinds share the same
// physical Trash folder and retain their own lifecycle.
const CANDIDATES: &str = "FROM items i WHERE i.kind='notes'
    AND substr(i.path,1,7)='.trash/' AND i.mat_state NOT IN ('purged','purging','blocked')";
pub(crate) const PURGE_STEP: &str = "trash.empty.item";
const PAGE: i64 = 16;

// The same query inside the admission transaction binds exactly the previewed
// revisions. SQLite length(BLOB) reads lengths without copying note bodies.
fn preview(db: &Connection, space: SpaceId) -> Result<(i64, i64, String)> {
    let mut q = db
        .prepare(&format!(
            "SELECT id,revision,(SELECT sum(length(bytes)) FROM versions WHERE item=i.id)
         {CANDIDATES} ORDER BY id"
        ))
        .map_err(internal)?;
    let mut rows = q.query([]).map_err(internal)?;
    let mut hash = blake3::Hasher::new();
    hash.update(space.as_uuid().as_bytes());
    let mut count = 0;
    let mut bytes = 0i64;
    while let Some(row) = rows.next().map_err(internal)? {
        count += 1;
        for col in 0..2 {
            let key = row
                .get_ref(col)
                .map_err(internal)?
                .as_str()
                .map_err(internal)?;
            hash.update(&(key.len() as u64).to_le_bytes());
            hash.update(key.as_bytes());
        }
        bytes = bytes
            .checked_add(row.get::<_, i64>(2).map_err(internal)?)
            .ok_or_else(|| internal("Trash byte count overflow"))?;
    }
    if bytes > (1i64 << 53) - 1 {
        return Err(Error::unavailable(
            "Trash byte count exceeds the exact JSON integer range",
        ));
    }
    Ok((count, bytes, hash.finalize().to_hex().to_string()))
}

fn freeze(db: &Connection, id: &str) -> Result<usize> {
    db.execute(
        &format!("INSERT INTO notes_trash_targets SELECT ?1,id,revision {CANDIDATES}"),
        [id],
    )
    .map_err(internal)
}

fn status(db: &Connection, id: &str) -> Result<Value> {
    db.query_row(
        "SELECT count,bytes,purged,skipped,finished IS NOT NULL FROM notes_trash_jobs WHERE id=?1",
        [id],
        |r| {
            Ok(
                json!({"job_id":id,"count":r.get::<_,i64>(0)?,"bytes":r.get::<_,i64>(1)?,
                      "purged":r.get::<_,i64>(2)?,"skipped":r.get::<_,i64>(3)?,
                      "state":if r.get::<_,bool>(4)? {"done"} else {"pending"}}),
            )
        },
    )
    .optional()
    .map_err(internal)?
    .ok_or_else(missing)
}

impl Content {
    /// Count the space's current trashed Notes and retained revision bytes.
    /// Returns a digest of the exact selection and reads no Markdown files.
    /// Cost is proportional to selected Notes and their retained revisions.
    pub fn empty_trash_preview(&self, space: SpaceId) -> Result<Value> {
        self.with_space(space, |s| {
            let (count, bytes, confirmation) = preview(&s.conn, space)?;
            Ok(json!({"count":count,"bytes":bytes,"confirmation":confirmation}))
        })
    }

    /// Admit one irreversible job per space after rechecking its preview.
    /// Commits the frozen selection and receipt intent together. A changed
    /// selection returns conflict. The caller must authorize permanence.
    pub fn empty_trash(
        &self,
        space: SpaceId,
        confirmation: &str,
        request: Request<'_>,
    ) -> Result<Value> {
        let _actor = self.lifecycle.step(request.account.as_uuid())?;
        self.with_space(space, |s| {
            self.publish_pending(s)?;
            if let Some(saved) = self.journals.receipt(&s.journal, request.account, request.id)? {
                return if saved.operation == request.operation && saved.fingerprint == request.fingerprint {
                    Ok(saved.result)
                } else {
                    Err(Error::request_reused())
                };
            }
            let tx = s.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(internal)?;
            let (count, bytes, current) = preview(&tx, space)?;
            if current != confirmation {
                return Err(Error::conflict("Trash changed; preview the current selection again"));
            }
            if tx.query_row("SELECT EXISTS(SELECT 1 FROM notes_trash_jobs WHERE finished IS NULL)", [], |r| r.get::<_,bool>(0)).map_err(internal)? {
                return Err(Error::conflict("an Empty Trash job is already running"));
            }
            let retention = time::Duration::try_from(self.receipt_retention()).map_err(internal)?;
            let cutoff = crate::format_rfc3339(time::OffsetDateTime::now_utc().saturating_sub(retention));
            tx.execute("DELETE FROM notes_trash_jobs WHERE finished < ?1", [&cutoff]).map_err(internal)?;
            let id = Uuid::now_v7().to_string();
            let finished = (count == 0).then(crate::now_rfc3339);
            tx.execute(
                "INSERT INTO notes_trash_jobs(id,actor,count,bytes,finished) VALUES(?1,?2,?3,?4,?5)",
                params![id,request.account.to_string(),count,bytes,finished],
            ).map_err(internal)?;
            if count != 0 {
                if freeze(&tx, &id)? as i64 != count {
                    return Err(internal("confirmed Trash selection changed during admission"));
                }
            }
            let response = status(&tx, &id)?;
            tx.execute(
                "INSERT INTO outbox VALUES(?1,?2,?3,?4,NULL,'control',?5)",
                params![request.account.to_string(),request.id,request.operation,request.fingerprint,
                        request.outbox_result(&response)],
            ).map_err(internal)?;
            tx.commit().map_err(internal)?;
            (self.fault)(crate::FaultPoint::AfterCommit);
            self.publish_pending(s)?;
            Ok(response)
        })
    }

    /// Read aggregate progress for an authorized space and job UUID.
    /// One indexed lookup. Expired or foreign job UUIDs return not found.
    pub fn empty_trash_status(&self, space: SpaceId, id: Uuid) -> Result<Value> {
        self.with_space(space, |s| {
            self.publish_pending(s)?;
            status(&s.conn, &id.to_string())
        })
    }

    /// Process at most one page of a space's job, releasing the writer between
    /// items. Changed revisions are skipped. After crashes, purge progress
    /// retires with the same durable transaction that clears the last revision.
    /// Returns true when queued work remains for a later turn.
    pub fn empty_trash_batch(&self, space: SpaceId) -> Result<bool> {
        let page = self.with_space(space, |s| {
            self.publish_pending(s)?;
            let job: Option<(String, String)> = s
                .conn
                .query_row(
                    "SELECT id,actor FROM notes_trash_jobs WHERE finished IS NULL",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(internal)?;
            let Some((job, account)) = job else {
                return Ok(None);
            };
            let mut q = s.conn.prepare_cached(
                "SELECT item,revision FROM notes_trash_targets WHERE job=?1 ORDER BY item LIMIT ?2",
            ).map_err(internal)?;
            let rows = q
                .query_map(params![job, PAGE], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(internal)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(internal)?;
            Ok(Some((
                job,
                AccountId::parse(&account).map_err(internal)?,
                rows,
            )))
        })?;
        let Some((job, account, rows)) = page else {
            return Ok(false);
        };
        for (item, revision) in rows {
            let uuid = Uuid::parse_str(&item).map_err(internal)?;
            let rid = format!("{job}/{item}");
            self.with_space(space, |s| {
                let result = self.purge_selected(
                    s,
                    "notes",
                    uuid,
                    None,
                    Some(&revision),
                    Request {
                        account,
                        via: None,
                        id: &rid,
                        operation: PURGE_STEP,
                        fingerprint: &rid,
                    },
                );
                if let Err(e) = result {
                    let blocked: bool = s.conn.query_row(
                        "SELECT EXISTS(SELECT 1 FROM items WHERE id=?1 AND mat_state='blocked')",
                        [&item], |r| r.get(0),
                    ).map_err(internal)?;
                    if !blocked && !matches!(e.code, ErrorCode::NotFound | ErrorCode::Conflict) {
                        return Err(e);
                    }
                    let tx = s
                        .conn
                        .transaction_with_behavior(TransactionBehavior::Immediate)
                        .map_err(internal)?;
                    retire_target(&tx, &job, &item, false)?;
                    tx.commit().map_err(internal)?;
                }
                Ok(())
            })?;
        }
        self.with_space(space, |s| {
            Ok(s.conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM notes_trash_targets WHERE job=?1)",
                    [&job],
                    |r| r.get(0),
                )
                .map_err(internal)?)
        })
    }
}

pub(crate) fn retire_purge(
    db: &Connection,
    operation: &str,
    request: &str,
    id: &str,
) -> Result<()> {
    if operation == PURGE_STEP
        && let Some((job, item)) = request.split_once('/')
        && item == id
    {
        retire_target(db, job, id, true)?;
    }
    Ok(())
}

fn retire_target(db: &Connection, job: &str, id: &str, purged: bool) -> Result<()> {
    if db
        .execute(
            "DELETE FROM notes_trash_targets WHERE job=?1 AND item=?2",
            params![job, id],
        )
        .map_err(internal)?
        != 0
    {
        db.execute(
            "UPDATE notes_trash_jobs SET purged=purged+?2,skipped=skipped+?3,
                finished=CASE WHEN purged+skipped+1=count THEN ?4 ELSE NULL END WHERE id=?1",
            params![job, purged, !purged, crate::now_rfc3339()],
        )
        .map_err(internal)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_and_selection_hold_one_hundred_thousand_revisions() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA).unwrap();
        db.execute_batch(
            "CREATE TABLE items(id TEXT PRIMARY KEY,revision TEXT,kind TEXT,path TEXT,mat_state TEXT);
             CREATE TABLE versions(item TEXT PRIMARY KEY,bytes BLOB);
             WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<100000)
             INSERT INTO items SELECT printf('%06d',i),'first','notes','.trash/'||i,'materialized' FROM n;
             INSERT INTO versions SELECT id,x'616263' FROM items;
             INSERT INTO notes_trash_jobs(id,actor,count,bytes) VALUES('fixture','actor',100000,300000);"
        ).unwrap();
        let space = SpaceId::parse("01990000-0000-7000-8000-000000000001").unwrap();
        let (count, bytes, confirmation) = preview(&db, space).unwrap();
        assert_eq!((count, bytes), (100_000, 300_000));
        assert_eq!(freeze(&db, "fixture").unwrap(), 100_000);
        db.execute("UPDATE items SET revision='second' WHERE id='000001'", [])
            .unwrap();
        assert_ne!(preview(&db, space).unwrap().2, confirmation);
        let saved: String = db
            .query_row(
                "SELECT revision FROM notes_trash_targets WHERE job='fixture' AND item='000001'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(saved, "first");
    }
}
