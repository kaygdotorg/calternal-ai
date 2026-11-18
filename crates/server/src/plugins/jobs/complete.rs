//! Durable terminal job transitions and cancellation of pending requests.
use super::{Jobs, MAX_CHECKPOINT_BYTES, MAX_RUNS, RunEnd, bounded_json};
use crate::plugins::{
    job_sql::{ACTIVE, all_reported, db_err, report, set_state},
    requests::cancel_pending_requests,
};
use calternal_core::JobState;
use calternal_types::Error;
use rusqlite::OptionalExtension;

impl Jobs {
    /// Finish run `run`. Late reports (cancelled job, superseded run) change nothing.
    /// A final job reports exactly once to its parent; a parent whose children have all
    /// reported is re-admitted. Returns the jobs to schedule.
    /// Cost: one FULL jobs transaction with a few indexed reads and writes.
    /// A native turn's final state also closes pending requests through the
    /// protected auth barrier while holding the space lock to order approvals.
    pub fn finish(
        &self,
        space: &str,
        id: &str,
        run: u32,
        end: RunEnd,
    ) -> Result<Vec<String>, Error> {
        let (state, result) = match end {
            RunEnd::Done(v) => match bounded_json(&v, MAX_CHECKPOINT_BYTES) {
                Ok(text) => (JobState::Done, text),
                Err(_) => (JobState::Failed, "null".into()),
            },
            RunEnd::Waiting => (JobState::Waiting, "null".into()),
            RunEnd::RetryWaiting => (JobState::RetryWaiting, "null".into()),
            RunEnd::Interrupted => (JobState::Interrupted, "null".into()),
            RunEnd::OutcomeUnknown => (JobState::OutcomeUnknown, "null".into()),
            RunEnd::WaitRejected(message) => (JobState::Failed, serde_json::json!({"wait_error": message}).to_string()),
            RunEnd::Failed => (JobState::Failed, "null".into()),
        };
        let wake = self.tx(space, |tx| {
            let current: Option<(String, u32, Option<String>, String, bool)> = tx
                .query_row(
                    "SELECT state,runs,parent,operation,json_extract(input,'$.verify') IS 1 FROM jobs WHERE id=?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()
                .map_err(db_err)?;
            let Some((s, runs, parent, operation, diagnostic)) = current else {
                return Ok(vec![]);
            };
            if !matches!(s.as_str(), "placing" | "running") || runs != run {
                return Ok(vec![]);
            }
            let mut state = state;
            if state == JobState::Waiting {
                let children: i64 = tx
                    .query_row("SELECT count(*) FROM jobs WHERE parent=?1", [id], |r| {
                        r.get(0)
                    })
                    .map_err(db_err)?;
                if children == 0 || runs >= MAX_RUNS {
                    state = JobState::Failed;
                }
            }
            if operation == "agent.turn" && !diagnostic && state.is_final() && self.security.is_some() {
                let (space, id) = (space.to_owned(), id.to_owned());
                self.security_change(move |auth| cancel_pending_requests(auth, &space, &id))?;
            }
            set_state(tx, id, state, &result)?;
            let mut wake = vec![];
            if state == JobState::Waiting && all_reported(tx, id)? {
                tx.execute("UPDATE jobs SET state='admitted' WHERE id=?1", [id])
                    .map_err(db_err)?;
                wake.push(id.to_owned());
            }
            if state.is_final()
                && let Some(parent) = parent
            {
                wake.extend(report(tx, &parent, id, state, &result)?);
            }
            Ok(wake)
        })?;
        self.settle_run(id, run);
        Ok(wake)
    }

    /// Readmit a signed-out turn after its machine reports a verified login.
    /// A later turn or another account cannot be resumed through this update.
    pub(crate) fn resume_signed_out(
        &self,
        space: &str,
        job: &str,
        account: calternal_types::AccountId,
    ) -> Result<bool, Error> {
        self.tx(space, |tx| {
            let affected = tx.execute(
                "UPDATE jobs AS j SET state='admitted',finished_ms=NULL,
                   checkpoint=json_set(checkpoint,'$.request_machine',json_extract(checkpoint,'$.machine_id'))
                 WHERE id=?1 AND account=?2 AND operation='agent.turn' AND state='failed'
                  AND json_extract(checkpoint,'$.error.class')='authentication'
                  AND NOT EXISTS(SELECT 1 FROM jobs AS newer WHERE newer.rowid>j.rowid
                   AND newer.operation='agent.turn'
                   AND json_extract(newer.input,'$.check') IS NOT 1
                   AND json_extract(newer.input,'$.request.thread_id')=json_extract(j.input,'$.request.thread_id'))",
                rusqlite::params![job, account.to_string()],
            ).map_err(db_err)?;
            Ok(affected == 1)
        })
    }

    /// Cancel a job and every active descendant. Their runs are told to stop, their
    /// later output is discarded, and the job reports `cancelled` to its parent unless
    /// that parent was cancelled too. Returns the jobs to schedule (a woken parent).
    /// Cost: one FULL transaction walking the descendant tree (at most
    /// [`super::MAX_CHILDREN`] per level within the space's active-job bound). Native
    /// turns also close pending requests in one protected transaction under
    /// the space lock, so a decision cannot race cancellation.
    pub fn cancel(&self, space: &str, id: &str) -> Result<Vec<String>, Error> {
        let (cancelled, wake) = self.tx(space, |tx| {
            let mut st = tx
                .prepare(&format!(
                    "WITH RECURSIVE tree(id) AS (SELECT ?1 UNION SELECT j.id FROM jobs j JOIN tree t ON j.parent=t.id) \
                     SELECT j.id, j.operation FROM jobs j JOIN tree t ON j.id=t.id WHERE j.state IN {ACTIVE}"
                ))
                .map_err(db_err)?;
            let ids = st
                .query_map([id], |r| Ok((r.get::<_, String>(0)?,r.get::<_, String>(1)?)))
                .map_err(db_err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(db_err)?;
            drop(st);
            if self.security.is_some() {
                let native: Vec<_> = ids.iter().filter(|(_,op)| op=="agent.turn")
                    .map(|(id,_)| id.clone()).collect();
                if !native.is_empty() {
                    let space = space.to_owned();
                    self.security_change(move |auth| {
                        for job in native {
                            cancel_pending_requests(auth,&space,&job)?;
                        }
                        Ok(())
                    })?;
                }
            }
            let ids: Vec<_> = ids.into_iter().map(|(id,_)| id).collect();
            for job in &ids {
                set_state(tx, job, JobState::Cancelled, "null")?;
            }
            let mut wake = vec![];
            if ids.iter().any(|j| j == id) {
                let parent: Option<String> = tx
                    .query_row("SELECT parent FROM jobs WHERE id=?1", [id], |r| r.get(0))
                    .map_err(db_err)?;
                if let Some(parent) = parent {
                    wake = report(tx, &parent, id, JobState::Cancelled, "null")?;
                }
            }
            Ok((ids, wake))
        })?;
        self.stop_live(cancelled);
        Ok(wake)
    }
    /// Settle routed jobs after durable cancellation or space retirement.
    /// One existing live-stream lookup per job, with no store I/O.
    pub(super) fn stop_live(&self, cancelled: Vec<String>) {
        for job in cancelled {
            self.stop_job_wait_timers(&job);
            let live = self
                .live
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .get(&job)
                .cloned();
            if let Some(live) = live {
                let cancel = live.lock().cancel.clone();
                if let Some(c) = cancel {
                    c.cancel();
                }
            }
            self.settle(&job);
        }
    }

    /// End an erased account's active work in retained spaces. The route map
    /// supplies only spaces with job stores. One indexed update per such space.
    pub(crate) fn retire_account(&self, account: calternal_types::AccountId) -> Result<(), Error> {
        let spaces: std::collections::BTreeSet<_> = self
            .routes
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .cloned()
            .collect();
        for space in spaces {
            let cancelled = match self.tx(&space, |tx| {
                tx.execute(
                    "DELETE FROM plugin_waits WHERE job IN (SELECT id FROM jobs WHERE account=?1)",
                    [account.to_string()],
                ).map_err(db_err)?;
                let mut update = tx.prepare_cached(&format!(
                    "UPDATE jobs SET state='cancelled',finished_ms=?2,
                        checkpoint=json_remove(checkpoint,'$.person_request','$.retry'),result='null'
                     WHERE account=?1 AND state IN {ACTIVE} RETURNING id"
                )).map_err(db_err)?;
                update.query_map(rusqlite::params![account.to_string(), crate::plugins::job_sql::now_ms()],
                    |row| row.get::<_,String>(0)).map_err(db_err)?
                    .collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
            }) {
                Ok(cancelled) => cancelled,
                Err(error) if error.code == calternal_types::ErrorCode::NotFound => continue,
                Err(error) => return Err(error),
            };
            self.stop_live(cancelled);
        }
        Ok(())
    }
}
