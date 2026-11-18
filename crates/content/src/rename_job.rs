//! Durable forward and inverse batches over the content revision outbox.
use crate::{
    Content, Item, Request, Result, SpaceId,
    db::Space,
    linked_write::{self, Staged},
    types::{internal, missing},
};
use calternal_types::{AccountId, Error, ErrorCode};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

pub(crate) const SCHEMA: &str = "
    CREATE TABLE notes_rename_jobs(
      id TEXT PRIMARY KEY, actor TEXT NOT NULL, root TEXT NOT NULL,
      from_path TEXT NOT NULL, to_path TEXT NOT NULL,
      before_rev TEXT NOT NULL, after_rev TEXT NOT NULL,
      total INTEGER NOT NULL, remaining INTEGER NOT NULL, skipped INTEGER NOT NULL,
      phase TEXT NOT NULL, finished TEXT
    ) STRICT;
    CREATE UNIQUE INDEX notes_rename_active ON notes_rename_jobs((1)) WHERE finished IS NULL;
    CREATE TABLE notes_rename_steps(
      job TEXT NOT NULL REFERENCES notes_rename_jobs(id) ON DELETE CASCADE,
      item TEXT NOT NULL, before_rev TEXT NOT NULL, after_rev TEXT, state TEXT NOT NULL,
      PRIMARY KEY(job,item)
    ) WITHOUT ROWID, STRICT;
    CREATE INDEX notes_rename_step_state ON notes_rename_steps(job,state,item);";

struct Job {
    id: String,
    actor: AccountId,
    root: Uuid,
    from: String,
    to: String,
    before: String,
    after: String,
    phase: String,
}

fn load(db: &Connection, id: Option<&str>) -> Result<Option<Job>> {
    let mut stmt = db.prepare_cached(if id.is_some() {
        "SELECT id,actor,root,from_path,to_path,before_rev,after_rev,phase FROM notes_rename_jobs WHERE id=?1"
    } else {
        "SELECT id,actor,root,from_path,to_path,before_rev,after_rev,phase FROM notes_rename_jobs WHERE finished IS NULL AND ?1 IS NULL"
    }).map_err(internal)?;
    let row = stmt
        .query_row([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
            ))
        })
        .optional()
        .map_err(internal)?;
    row.map(|(id, actor, root, from, to, before, after, phase)| {
        Ok(Job {
            id,
            actor: AccountId::parse(&actor).map_err(internal)?,
            root: Uuid::parse_str(&root).map_err(internal)?,
            from,
            to,
            before,
            after,
            phase,
        })
    })
    .transpose()
}

pub(crate) fn ensure_idle(db: &Connection) -> Result<()> {
    if db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM notes_rename_jobs WHERE finished IS NULL)",
            [],
            |r| r.get::<_, bool>(0),
        )
        .map_err(internal)?
    {
        return Err(Error::conflict(
            "a Notes rename job is already running in this space",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn record(
    db: &Connection,
    job: Uuid,
    root: &Staged,
    old: &str,
    base: &str,
    candidates: &[(String, String)],
    request: Request<'_>,
    retention: Duration,
) -> Result<()> {
    let retention = time::Duration::try_from(retention).map_err(internal)?;
    let cutoff = crate::format_rfc3339(time::OffsetDateTime::now_utc().saturating_sub(retention));
    db.execute(
        "DELETE FROM notes_rename_jobs WHERE finished < ?1",
        [&cutoff],
    )
    .map_err(internal)?;
    let id = job.to_string();
    let done = candidates.is_empty();
    db.execute(
        "INSERT INTO notes_rename_jobs VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,0,?10,?11)",
        params![
            id,
            request.account.to_string(),
            root.item.id.to_string(),
            old,
            root.path,
            base,
            root.item.revision,
            candidates.len() as i64,
            candidates.len() as i64,
            if done { "done" } else { "pending" },
            done.then(crate::now_rfc3339)
        ],
    )
    .map_err(internal)?;
    let mut stmt = db
        .prepare_cached("INSERT INTO notes_rename_steps VALUES(?1,?2,?3,?4,?5)")
        .map_err(internal)?;
    for (item, base) in candidates {
        stmt.execute(params![id, item, base, None::<String>, "pending"])
            .map_err(internal)?;
    }
    Ok(())
}

fn status(db: &Connection, id: &str) -> Result<Value> {
    db.query_row(
        "SELECT root,total,remaining,skipped,phase FROM notes_rename_jobs WHERE id=?1",
        [id],
        |r| {
            Ok(
                json!({"job_id":id,"note_id":r.get::<_,String>(0)?,"total":r.get::<_,i64>(1)?,
                      "remaining":r.get::<_,i64>(2)?,"skipped":r.get::<_,i64>(3)?,
                      "state":r.get::<_,String>(4)?}),
            )
        },
    )
    .optional()
    .map_err(internal)?
    .ok_or_else(missing)
}

pub(crate) fn owns_unpublished_root(db: &Connection, item: Uuid, revision: &str) -> Result<bool> {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM notes_rename_jobs WHERE root=?1 AND after_rev=?2)",
        params![item.to_string(), revision],
        |r| r.get(0),
    )
    .map_err(internal)
}

// An exclusive move rejected the destination without changing the old file.
// Cancel its unpublished root and plan in one FULL transaction so future
// writes can choose a fresh name and no dependent source can run.
pub(crate) fn cancel_occupied_root(
    space: &mut Space,
    item: &str,
    revision: &str,
    operation: &str,
    account: &str,
    request: &str,
) -> Result<()> {
    if !matches!(
        operation,
        "notes.rename" | "notes.update" | "notes.rename.undo"
    ) {
        return Ok(());
    }
    retire_occupied_root(
        &mut space.conn,
        item,
        revision,
        operation == "notes.rename.undo",
        account,
        request,
    )
}

fn retire_occupied_root(
    db: &mut Connection,
    item: &str,
    revision: &str,
    undo: bool,
    account: &str,
    request: &str,
) -> Result<()> {
    let _revision = calternal_types::begin_item_access_change();
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(internal)?;
    let row: Option<(String, String, i64, String, String, String)> = tx
        .query_row(
            "SELECT j.id,i.old_path,v.seq,v.revision,v.metadata,v.updated_at
         FROM notes_rename_jobs j JOIN items i ON i.id=j.root
         JOIN versions v ON v.item=i.id AND v.seq=i.seq-1
         WHERE i.id=?1 AND i.revision=?2 AND j.after_rev=?2 AND i.old_path IS NOT NULL",
            params![item, revision],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(internal)?;
    let Some((job, path, seq, previous, metadata, at)) = row else {
        return Ok(());
    };
    tx.execute(
        "UPDATE items SET path=?2,folded_path=?3,seq=?4,revision=?5,metadata=?6,updated_at=?7,
         old_path=NULL,mat_state='materialized' WHERE id=?1",
        params![
            item,
            path,
            calternal_fsio::names::fold(&path),
            seq,
            previous,
            metadata,
            at
        ],
    )
    .map_err(internal)?;
    tx.execute(
        "DELETE FROM versions WHERE item=?1 AND revision=?2",
        params![item, revision],
    )
    .map_err(internal)?;
    tx.execute(
        "UPDATE notes_rename_jobs SET phase=?2,after_rev=?3,remaining=0,finished=?4 WHERE id=?1",
        params![
            job,
            if undo { "done" } else { "blocked" },
            if undo { &previous } else { revision },
            crate::now_rfc3339()
        ],
    )
    .map_err(internal)?;
    tx.execute(
        "DELETE FROM outbox WHERE account=?1 AND request=?2",
        params![account, request],
    )
    .map_err(internal)?;
    tx.commit().map_err(internal)
}

fn revision_body(db: &Connection, id: Uuid, revision: &str) -> Result<(Value, Vec<u8>)> {
    let (metadata, bytes): (String, Vec<u8>) = db
        .query_row(
            "SELECT metadata,bytes FROM versions WHERE item=?1 AND revision=?2",
            params![id.to_string(), revision],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(internal)?
        .ok_or_else(missing)?;
    Ok((serde_json::from_str(&metadata).map_err(internal)?, bytes))
}

impl Content {
    /// Return current progress of one authorized space's rename or inverse.
    /// Replays unresolved placements first, then performs one indexed read.
    pub fn rename_job_status(&self, space: SpaceId, job: Uuid) -> Result<Value> {
        self.with_space(space, |s| {
            self.publish_pending(s)?;
            status(&s.conn, &job.to_string())
        })
    }

    /// Start the inverse of a completed rename. Every changed note must
    /// still have the revision written by the original job. Claim the old
    /// root path before advancing any dependent inverse and preserve an
    /// outside occupant. The inverse targets the stored preceding revisions.
    pub fn undo_rename(&self, space: SpaceId, id: Uuid, request: Request<'_>) -> Result<Value> {
        let _actor = self.lifecycle.step(request.account.as_uuid())?;
        self.with_space(space, |s| {
            self.publish_pending(s)?;
            if let Some(saved) = self
                .journals
                .receipt(&s.journal, request.account, request.id)?
            {
                return if saved.operation == request.operation
                    && saved.fingerprint == request.fingerprint
                {
                    Ok(saved.result)
                } else {
                    Err(Error::request_reused())
                };
            }
            ensure_idle(&s.conn)?;
            let job = load(&s.conn, Some(&id.to_string()))?.ok_or_else(missing)?;
            if job.phase != "done" {
                return Err(Error::conflict("only a completed rename can be undone"));
            }
            let header = s.header(job.root)?;
            if header.revision != job.after || header.path != job.to {
                return Err(Error::conflict("the renamed note has changed"));
            }
            let changed: bool = s.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM notes_rename_steps a LEFT JOIN items i ON i.id=a.item
                 WHERE a.job=?1 AND a.state='applied' AND i.revision IS NOT a.after_rev)",
                [&job.id], |r| r.get(0),
            ).map_err(internal)?;
            if changed {
                return Err(Error::conflict("a rewritten note has changed"));
            }
            let (metadata, bytes) = revision_body(&s.conn, job.root, &job.before)?;
            let same_fold =
                calternal_fsio::names::fold(&job.from) == calternal_fsio::names::fold(&job.to);
            let path = s.choose_path(
                "notes",
                metadata["title"].as_str().unwrap_or("Untitled"),
                same_fold.then_some(job.to.as_str()),
                Some(&job.from),
            )?;
            if path != job.from {
                return Err(Error::conflict("the original filename is occupied"));
            }
            let mut root = s
                .stage_link_edit(
                    job.root,
                    &job.after,
                    |_| Ok(Some((metadata, bytes))),
                    self.fault,
                )?
                .ok_or_else(|| internal("rename inverse lacks a root revision"))?;
            root.path = path;
            let _revision = calternal_types::begin_item_access_change();
            let tx = s
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(internal)?;
            root.write(&tx, Some(&job.to), &request.account.to_string())?;
            let count: i64 = tx
                .query_row(
                    "SELECT count(*) FROM notes_rename_steps WHERE job=?1 AND state='applied'",
                    [&job.id],
                    |r| r.get(0),
                )
                .map_err(internal)?;
            tx.execute(
                "UPDATE notes_rename_jobs SET phase=?2,finished=?3,remaining=?4,actor=?5,
                 after_rev=?6,skipped=0 WHERE id=?1",
                params![
                    job.id,
                    if count == 0 { "undone" } else { "undoing" },
                    (count == 0).then(crate::now_rfc3339),
                    count,
                    request.account.to_string(),
                    root.item.revision
                ],
            )
            .map_err(internal)?;
            let response = status(&tx, &job.id)?;
            root.outbox(&tx, request, &response)?;
            root.require_placement(&tx, request)?;
            tx.commit().map_err(internal)?;
            (self.fault)(crate::FaultPoint::AfterCommit);
            self.publish_pending(s)?;
            Ok(response)
        })
    }

    /// Advance at most eight link edits or inverses for one space. Each source
    /// is revision-fenced and uses the same placement and journal order as a
    /// direct edit. True schedules another bounded turn. Outside conflicts
    /// leave the source untouched and are counted in the job.
    pub fn rename_job_batch(
        &self,
        space: SpaceId,
        job: Option<Uuid>,
        retarget: impl Fn(&Item, &str, &str, &str, &str, Uuid) -> Result<Option<(Value, Vec<u8>)>>,
    ) -> Result<bool> {
        self.with_space(space, |s| {
            self.publish_pending(s)?;
            let selected = job.map(|id| id.to_string());
            let Some(job) = load(&s.conn, selected.as_deref())? else { return Ok(false); };
            if !matches!(job.phase.as_str(), "pending" | "undoing") {
                return Ok(false);
            }
            let undo = job.phase == "undoing";
            let root_path = if undo { &job.from } else { &job.to };
            if s.header(job.root)?.path != *root_path {
                s.conn.execute(
                    "UPDATE notes_rename_jobs SET phase='blocked',finished=?2 WHERE id=?1",
                    params![job.id,crate::now_rfc3339()],
                ).map_err(internal)?;
                return Ok(false);
            }
            let titles: Option<(String,String)> = if undo { None } else {
                let lookup = |revision: &str| s.conn.query_row(
                    "SELECT coalesce(metadata->>'title','') FROM versions WHERE item=?1 AND revision=?2",
                    params![job.root.to_string(),revision], |r| r.get::<_,String>(0),
                ).map_err(internal);
                Some((lookup(&job.before)?,lookup(&job.after)?))
            };
            let rows = {
                let mut stmt = s.conn.prepare_cached(
                    "SELECT item,before_rev,after_rev FROM notes_rename_steps
                     WHERE job=?1 AND state=?2 ORDER BY item LIMIT ?3",
                ).map_err(internal)?;
                stmt.query_map(params![job.id,if undo {"applied"} else {"pending"},linked_write::BATCH as i64], |r| {
                    Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?))
                }).map_err(internal)?.collect::<rusqlite::Result<Vec<_>>>().map_err(internal)?
            };
            for (item,before,after) in rows {
                let item = Uuid::parse_str(&item).map_err(internal)?;
                let step = if let Some((old_title,title)) = &titles {
                    s.stage_link_edit(item, &before, |note| retarget(
                        note,&job.from,&job.to,old_title,title,job.root,
                    ),self.fault)
                } else {
                    let base = after.as_deref().ok_or_else(|| internal("rename step has no applied revision"))?;
                    let previous = revision_body(&s.conn, item, &before);
                    match previous {
                        Ok(previous) => s.stage_link_edit(item,base,|_| Ok(Some(previous)),self.fault),
                        Err(error) => Err(error),
                    }
                };
                let (staged,conflict) = match step {
                    Ok(staged) => (staged,false),
                    Err(error) if matches!(error.code, ErrorCode::Conflict | ErrorCode::NotFound) => (None,true),
                    Err(error) => return Err(error),
                };
                apply_step(s,&job,item,undo,staged,conflict)?;
                (self.fault)(crate::FaultPoint::AfterCommit);
                self.publish_pending(s)?;
            }
            Ok(load(&s.conn,None)?.is_some())
        })
    }
}

fn apply_step(
    s: &mut Space,
    job: &Job,
    item: Uuid,
    undo: bool,
    step: Option<Staged>,
    conflict: bool,
) -> Result<()> {
    let tx = s
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(internal)?;
    let phase = if let Some(staged) = &step {
        let id = Uuid::now_v7().to_string();
        let request = Request {
            account: job.actor,
            via: None,
            id: &id,
            operation: if undo {
                "notes.rename.undo.item"
            } else {
                "notes.rename.item"
            },
            fingerprint: &id,
        };
        staged.write(&tx, None, &job.actor.to_string())?;
        staged.outbox(&tx, request, &json!({"job_id":job.id,"id":item}))?;
        staged.require_placement(&tx, request)?;
        if undo { "undone" } else { "applied" }
    } else {
        "skipped"
    };
    tx.execute(
        "UPDATE notes_rename_steps SET state=?3,after_rev=coalesce(?4,after_rev)
         WHERE job=?1 AND item=?2",
        params![
            job.id,
            item.to_string(),
            phase,
            step.map(|s| s.item.revision)
        ],
    )
    .map_err(internal)?;
    tx.execute(
        "UPDATE notes_rename_jobs SET remaining=remaining-1,skipped=skipped+?2,
         phase=CASE WHEN remaining=1 THEN ?3 ELSE phase END,
         finished=CASE WHEN remaining=1 THEN ?4 ELSE NULL END WHERE id=?1",
        params![
            job.id,
            conflict,
            if undo { "undone" } else { "done" },
            crate::now_rfc3339()
        ],
    )
    .map_err(internal)?;
    tx.commit().map_err(internal)
}
