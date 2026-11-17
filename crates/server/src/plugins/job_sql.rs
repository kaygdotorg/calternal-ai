//! SQL of the per-space job files: schema, row mapping, state names and the
//! transactional steps (final state, child reports, retention) shared by
//! [`super::jobs::Jobs`]. Every function runs inside the caller's transaction or
//! connection lock and performs only indexed statements.

use calternal_core::JobState;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use super::Session;
use super::jobs::{Job, Jobs, MAX_FINAL_PER_SPACE, MAX_RUNS, RETAIN_FINAL};
use calternal_types::Error;

pub(super) const SCHEMA: &str = "BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY,plugin TEXT NOT NULL,account TEXT NOT NULL,session TEXT NOT NULL,operation TEXT NOT NULL,input TEXT NOT NULL,parent TEXT,state TEXT NOT NULL,checkpoint TEXT NOT NULL,checkpoint_seq INTEGER NOT NULL,checkpoints INTEGER NOT NULL,result TEXT NOT NULL,runs INTEGER NOT NULL,generation INTEGER NOT NULL,request_key TEXT UNIQUE,finished_ms INTEGER) STRICT;
CREATE INDEX IF NOT EXISTS jobs_state ON jobs(state);
CREATE INDEX IF NOT EXISTS jobs_parent ON jobs(parent);
CREATE INDEX IF NOT EXISTS jobs_finished ON jobs(finished_ms) WHERE finished_ms IS NOT NULL;
CREATE TABLE IF NOT EXISTS child_results(parent TEXT NOT NULL,child TEXT NOT NULL,state TEXT NOT NULL,result TEXT NOT NULL,PRIMARY KEY(parent,child)) STRICT;
PRAGMA user_version=1;COMMIT;";

pub(super) fn db_err(e: rusqlite::Error) -> Error {
    tracing::error!(error = %e, "jobs sqlite error");
    Error::internal("job storage error")
}

pub(super) fn state_name(state: JobState) -> &'static str {
    match state {
        JobState::Admitted => "admitted",
        JobState::Running => "running",
        JobState::RetryWaiting => "retry_waiting",
        JobState::Interrupted => "interrupted",
        JobState::Waiting => "waiting",
        JobState::Done => "done",
        JobState::Failed => "failed",
        JobState::Cancelled => "cancelled",
        JobState::OutcomeUnknown => "outcome_unknown",
    }
}

pub(super) fn parse_state(name: &str) -> JobState {
    match name {
        "admitted" => JobState::Admitted,
        "running" => JobState::Running,
        "retry_waiting" => JobState::RetryWaiting,
        "interrupted" => JobState::Interrupted,
        "waiting" => JobState::Waiting,
        "done" => JobState::Done,
        "cancelled" => JobState::Cancelled,
        "outcome_unknown" => JobState::OutcomeUnknown,
        _ => JobState::Failed,
    }
}

pub(super) const ACTIVE: &str = "('admitted','running','waiting','interrupted','retry_waiting')";

pub(crate) use calternal_types::now_ms;

pub(super) const COLUMNS: &str = "id,plugin,account,session,operation,input,parent,state,checkpoint,checkpoint_seq,checkpoints,result,runs,generation";

pub(super) fn row(space: &str, r: &rusqlite::Row<'_>) -> rusqlite::Result<Job> {
    let json = |i: usize| -> rusqlite::Result<Value> {
        Ok(serde_json::from_str(&r.get::<_, String>(i)?).unwrap_or(Value::Null))
    };
    Ok(Job {
        id: r.get(0)?,
        space: space.to_owned(),
        plugin: r.get(1)?,
        account: r.get::<_, String>(2)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?,
        // An unreadable session fails closed: fresh IDs match no live installation.
        session: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or(Session::Installation {
            account: calternal_types::AccountId::new(),
            installation: calternal_types::InstallationId::new(),
        }),
        operation: r.get(4)?,
        input: json(5)?,
        parent: r.get(6)?,
        state: parse_state(&r.get::<_, String>(7)?),
        checkpoint: json(8)?,
        checkpoint_seq: r.get::<_, i64>(9)? as u64,
        checkpoints: r.get(10)?,
        result: json(11)?,
        runs: r.get(12)?,
        generation: r.get(13)?,
    })
}

pub(super) fn set_state(
    tx: &rusqlite::Transaction<'_>,
    id: &str,
    state: JobState,
    result: &str,
) -> Result<(), Error> {
    let finished = state.is_final().then(now_ms);
    tx.execute(
        "UPDATE jobs SET state=?1,result=?2,finished_ms=?3 WHERE id=?4",
        params![state_name(state), result, finished, id],
    )
    .map_err(db_err)?;
    Ok(())
}

/// Whether every child of `parent` has reported.
pub(super) fn all_reported(tx: &rusqlite::Transaction<'_>, parent: &str) -> Result<bool, Error> {
    let pending: i64 = tx
        .query_row(
            "SELECT count(*) FROM jobs j WHERE j.parent=?1 AND NOT EXISTS (SELECT 1 FROM child_results c WHERE c.parent=?1 AND c.child=j.id)",
            [parent],
            |r| r.get(0),
        )
        .map_err(db_err)?;
    Ok(pending == 0)
}

/// Record a child's final report once; wake the parent if it waits and is complete.
pub(super) fn report(
    tx: &rusqlite::Transaction<'_>,
    parent: &str,
    child: &str,
    state: JobState,
    result: &str,
) -> Result<Vec<String>, Error> {
    tx.execute(
        "INSERT INTO child_results VALUES(?1,?2,?3,?4) ON CONFLICT(parent,child) DO NOTHING",
        params![parent, child, state_name(state), result],
    )
    .map_err(db_err)?;
    let waiting = tx
        .query_row("SELECT state FROM jobs WHERE id=?1", [parent], |r| {
            r.get::<_, String>(0)
        })
        .optional()
        .map_err(db_err)?
        .is_some_and(|s| s == "waiting");
    if waiting && all_reported(tx, parent)? {
        tx.execute("UPDATE jobs SET state='admitted' WHERE id=?1", [parent])
            .map_err(db_err)?;
        return Ok(vec![parent.to_owned()]);
    }
    Ok(vec![])
}

/// Delete final jobs past [`RETAIN_FINAL`] or beyond [`MAX_FINAL_PER_SPACE`], and the
/// child reports of deleted parents. Cost: indexed deletes; runs when a file opens.
pub(super) fn retain(c: &Connection) -> Result<(), Error> {
    let cutoff = now_ms() - RETAIN_FINAL.as_millis() as i64;
    c.execute_batch(&format!(
        "BEGIN IMMEDIATE;
         DELETE FROM jobs WHERE finished_ms IS NOT NULL AND (finished_ms < {cutoff} OR id NOT IN
           (SELECT id FROM jobs WHERE finished_ms IS NOT NULL ORDER BY finished_ms DESC LIMIT {MAX_FINAL_PER_SPACE}));
         DELETE FROM child_results WHERE parent NOT IN (SELECT id FROM jobs);
         COMMIT;"
    ))
    .map_err(db_err)
}

/// One active job as seen by restart recovery.
struct Active {
    id: String,
    state: String,
    plugin: String,
    operation: String,
    runs: u32,
    parent: Option<String>,
}

impl Jobs {
    /// After a restart: every running job becomes admitted again (to resume from its
    /// checkpoint) unless `resumable` says its run cannot be repeated or it used up
    /// [`MAX_RUNS`], in which case it ends `outcome_unknown` and reports to its parent.
    /// Waiting jobs whose children all reported are re-admitted. Returns the
    /// `(space, job)` pairs to schedule. Cost: per space one FULL transaction and
    /// indexed scans of its active jobs.
    pub fn recover(
        &self,
        resumable: impl Fn(&str, &str) -> bool,
    ) -> Result<Vec<(String, String)>, Error> {
        let spaces: Vec<String> = {
            let routes = self.routes.read().unwrap_or_else(|p| p.into_inner());
            let mut s: Vec<String> = routes.values().cloned().collect();
            s.sort();
            s.dedup();
            s
        };
        let mut out = vec![];
        for space in spaces {
            let woken = self.tx(&space, |tx| {
                let mut st = tx
                    .prepare(&format!("SELECT id,state,plugin,operation,runs,parent FROM jobs WHERE state IN {ACTIVE}"))
                    .map_err(db_err)?;
                let rows = st
                    .query_map([], |r| {
                        Ok(Active {
                            id: r.get(0)?,
                            state: r.get(1)?,
                            plugin: r.get(2)?,
                            operation: r.get(3)?,
                            runs: r.get(4)?,
                            parent: r.get(5)?,
                        })
                    })
                    .map_err(db_err)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(db_err)?;
                drop(st);
                let mut wake = vec![];
                for Active { id, state, plugin, operation, runs, parent } in rows {
                    if operation == "agent.turn" {
                        if state!="retry_waiting" {set_state(tx, &id, JobState::Interrupted, "null")?;}
                        continue;
                    }
                    match state.as_str() {
                        "running" if resumable(&plugin, &operation) && runs < MAX_RUNS => {
                            tx.execute("UPDATE jobs SET state='admitted' WHERE id=?1", [&id]).map_err(db_err)?;
                            wake.push(id);
                        }
                        "running" => {
                            set_state(tx, &id, JobState::OutcomeUnknown, "null")?;
                            if let Some(parent) = parent {
                                wake.extend(report(tx, &parent, &id, JobState::OutcomeUnknown, "null")?);
                            }
                        }
                        "waiting" if all_reported(tx, &id)? => {
                            tx.execute("UPDATE jobs SET state='admitted' WHERE id=?1", [&id]).map_err(db_err)?;
                            wake.push(id);
                        }
                        "admitted" => wake.push(id),
                        _ => {}
                    }
                }
                Ok(wake)
            })?;
            out.extend(woken.into_iter().map(|id| (space.clone(), id)));
        }
        out.sort();
        out.dedup();
        Ok(out)
    }
}
