//! Signal waits share the owning job's transaction and run fencing.
use super::*;
use crate::plugins::job_sql::now_ms;
use calternal_core::{PersonRequest, RequestDetail};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub(super) const SCHEMA: &str = "
CREATE TABLE plugin_waits(
    id TEXT PRIMARY KEY, job TEXT NOT NULL, checkpoint INTEGER NOT NULL,
    deadline_ms INTEGER NOT NULL, detail TEXT NOT NULL, signal TEXT NOT NULL,
    outcome TEXT, decision_request TEXT, decision_fingerprint TEXT, resolved_ms INTEGER,
    UNIQUE(job,checkpoint)
) STRICT;
CREATE UNIQUE INDEX plugin_waits_pending ON plugin_waits(job) WHERE outcome IS NULL;
CREATE INDEX plugin_waits_deadline ON plugin_waits(deadline_ms) WHERE outcome IS NULL;
CREATE UNIQUE INDEX plugin_waits_webhook ON plugin_waits(
    job,json_extract(signal,'$.route'),json_extract(signal,'$.correlation')
) WHERE json_extract(signal,'$.kind')='webhook';
CREATE TRIGGER plugin_waits_retain AFTER DELETE ON jobs BEGIN
    DELETE FROM plugin_waits WHERE job=OLD.id;
END;
CREATE TRIGGER plugin_waits_cancel AFTER UPDATE OF state ON jobs
WHEN OLD.state='signal_waiting' AND NEW.state='cancelled' BEGIN
    DELETE FROM plugin_waits WHERE job=NEW.id AND outcome IS NULL;
END;
";

const SPACE_LIMIT: u32 = 32;
const PACKAGE_LIMIT: u32 = 8;
const MAX_DEADLINE_MS: i64 = 7 * 24 * 60 * 60 * 1000;
const SIGNAL_LIMIT: usize = 16 * 1024;

/// Typed source allowed to resolve an individual wait.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum WaitSource {
    /// Let the deadline win without accepting a signal.
    Deadline,
    /// Accept a form answer matching a declared operation's cached input schema.
    Question {
        /// Inbox form rendered for the job's submitting account.
        form: calternal_core::GenUiView,
        /// Operation declaring the shape of the form result.
        schema_operation: String,
    },
    /// Ask for an affirmative or negative decision about this job.
    Approval,
    /// Accept a signed delivery on a manifest-declared webhook.
    Webhook {
        /// Manifest webhook identifier.
        route: String,
        /// Plugin-generated nonce supplied to the webhook sender.
        correlation: Uuid,
    },
}

/// Validated worker output, committed as one checkpoint and pending wait.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Park {
    /// Checkpoint for the next run.
    pub checkpoint: Value,
    /// User-visible reason, bounded to 256 UTF-8 bytes.
    pub reason: String,
    /// Deadline in Unix milliseconds, at most seven days from admission.
    pub deadline_ms: i64,
    /// Signal contract declared by this wait.
    pub source: WaitSource,
}

/// Read-only coordinates of a pending wait for the host deadline timer.
pub(crate) struct Timer {
    /// Owning space.
    pub space: String,
    /// Owning job.
    pub job: String,
    /// Account receiving Inbox invalidations.
    pub account: AccountId,
    /// Wait instance, distinct on each park.
    pub id: String,
    /// Unix milliseconds when the wait accepts a deadline outcome.
    pub deadline_ms: i64,
}

/// A signal with an optional receipt, written with the wake transition.
pub(crate) struct Decision<'a> {
    /// Outcome delivered to the resumed worker.
    pub outcome: Value,
    /// Request key and fingerprint, when the caller supports retries.
    pub receipt: Option<(&'a str, &'a str)>,
}

/// Result of resolving a wait inside the job transaction.
pub(crate) enum Resolution {
    /// The first winning outcome admitted the job.
    Woken,
    /// A previous successful decision had the same receipt.
    Replay,
    /// A timer raced another outcome or ran before its deadline.
    Unchanged,
}

impl Jobs {
    /// Commit the run's checkpoint and park it without a worker. A late result
    /// cannot park a superseded run. One FULL transaction with bounded reads.
    pub(crate) fn park_signal(
        &self,
        job: &Job,
        live: &Live,
        park: Park,
        detail: Option<RequestDetail>,
    ) -> Result<Option<Timer>, Error> {
        if park.reason.trim().is_empty() || park.reason.len() > 256 {
            return Err(Error::invalid_input(
                "wait needs a reason of at most 256 bytes",
            ));
        }
        if job.runs >= MAX_RUNS {
            return Err(Error::unavailable("job run limit reached"));
        }
        let checkpoint = bounded_json(&park.checkpoint, MAX_CHECKPOINT_BYTES)?;
        let detail = bounded_json(
            &json!({"reason": park.reason, "request": detail}),
            SIGNAL_LIMIT,
        )?;
        let source = bounded_json(&json!(park.source), SIGNAL_LIMIT)?;
        let id = Uuid::now_v7().to_string();
        let checkpoint_seq = live.next_seq() as i64;
        let _actor = self.lifecycle.step(job.account.as_uuid())?;
        let committed = self.tx(&job.space, |tx| {
            let now = now_ms();
            if !((now + 1)..=(now + MAX_DEADLINE_MS)).contains(&park.deadline_ms) {
                return Err(Error::invalid_input("wait deadline must be within the next seven days"));
            }
            let (space_count, package_count): (u32, u32) = tx.query_row(
                "SELECT count(*),count(CASE WHEN j.plugin=?1 THEN 1 END)
                 FROM plugin_waits w JOIN jobs j ON j.id=w.job
                 WHERE w.outcome IS NULL AND j.state='signal_waiting'",
                [&job.plugin], |r| Ok((r.get(0)?, r.get(1)?)),
            ).map_err(db_err)?;
            if space_count >= SPACE_LIMIT || package_count >= PACKAGE_LIMIT {
                return Err(Error::unavailable("durable wait capacity reached"));
            }
            if let WaitSource::Webhook { route, correlation } = &park.source {
                let prior: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM plugin_waits WHERE job=?1
                      AND json_extract(signal,'$.kind')='webhook'
                      AND json_extract(signal,'$.route')=?2 AND json_extract(signal,'$.correlation')=?3)",
                    params![job.id, route, correlation.to_string()], |r| r.get(0),
                ).map_err(db_err)?;
                if prior {
                    return Err(Error::invalid_input("webhook correlation must be fresh for this job"));
                }
            }
            let number: Option<u32> = tx.query_row(
                "UPDATE jobs SET state='signal_waiting',checkpoint=?3,checkpoint_seq=?4,checkpoints=checkpoints+1
                 WHERE id=?1 AND runs=?2 AND state='running' RETURNING checkpoints",
                params![job.id, job.runs, checkpoint, checkpoint_seq],
                |r| r.get(0),
            ).optional().map_err(db_err)?;
            let Some(number) = number else { return Ok(false) };
            tx.execute(
                "INSERT INTO plugin_waits(id,job,checkpoint,deadline_ms,detail,signal) VALUES(?1,?2,?3,?4,?5,?6)",
                params![id, job.id, number, park.deadline_ms, detail, source],
            ).map_err(db_err)?;
            Ok(true)
        })?;
        if committed {
            self.settle_run(&job.id, job.runs);
        }
        Ok(committed.then(|| Timer {
            space: job.space.clone(),
            job: job.id.clone(),
            account: job.account,
            id,
            deadline_ms: park.deadline_ms,
        }))
    }

    /// Read the first committed result of the wait belonging to this checkpoint.
    /// One indexed read, keeping the outcome stable across restart and retry.
    pub(crate) fn continuation(&self, job: &Job) -> Result<Option<Value>, Error> {
        if job.checkpoints == 0 {
            return Ok(None);
        }
        let db = self.conn(&job.space)?;
        let conn = db.lock().map_err(db_err)?;
        let outcome: Option<String> = conn
            .query_row(
                "SELECT outcome FROM plugin_waits WHERE job=?1 AND checkpoint=?2",
                params![job.id, job.checkpoints],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_err)?
            .flatten();
        outcome
            .map(|text| {
                serde_json::from_str(&text).map_err(|_| Error::internal("stored wait outcome"))
            })
            .transpose()
    }

    /// Read pending waits at startup for the existing durable job stores.
    /// One indexed walk of pending waits per space.
    pub(crate) fn wait_timers(&self) -> Result<Vec<Timer>, Error> {
        let spaces: std::collections::BTreeSet<_> = self
            .routes
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect();
        let mut timers = Vec::new();
        for space in spaces {
            let db = self.conn(&space)?;
            let conn = db.lock().map_err(db_err)?;
            let mut query = conn
                .prepare(
                    "SELECT w.job,w.id,w.deadline_ms,j.account FROM plugin_waits w
                 JOIN jobs j ON j.id=w.job WHERE w.outcome IS NULL AND j.state='signal_waiting'",
                )
                .map_err(db_err)?;
            for timer in query
                .query_map([], |r| {
                    Ok(Timer {
                        space: space.clone(),
                        job: r.get(0)?,
                        id: r.get(1)?,
                        deadline_ms: r.get(2)?,
                        account: r.get::<_, String>(3)?.parse().map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                3,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?,
                    })
                })
                .map_err(db_err)?
            {
                timers.push(timer.map_err(db_err)?);
            }
        }
        Ok(timers)
    }

    /// Project account-owned pending prompts into the existing Inbox.
    /// One bounded query in the authorized space with no writes.
    pub(crate) fn wait_requests(
        &self,
        space: &str,
        account: AccountId,
    ) -> Result<Vec<PersonRequest>, Error> {
        if !self.has_job_store(space) {
            return Ok(Vec::new());
        }
        let db = self.conn(space)?;
        let conn = db.lock().map_err(db_err)?;
        let mut query = conn.prepare_cached(
            "SELECT w.id,w.job,json_extract(w.detail,'$.request') FROM plugin_waits w JOIN jobs j ON j.id=w.job
             WHERE w.outcome IS NULL AND j.state='signal_waiting' AND j.account=?1 AND w.deadline_ms>?2
               AND json_type(w.detail,'$.request')='object'",
        ).map_err(db_err)?;
        query
            .query_map(params![account.to_string(), now_ms()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(db_err)?
            .map(|row| {
                let (id, job, detail) = row.map_err(db_err)?;
                Ok(PersonRequest {
                    id,
                    space_id: Some(space.to_owned()),
                    thread_id: None,
                    job_id: Some(job),
                    detail: serde_json::from_str(&detail)
                        .map_err(|_| Error::internal("stored wait request"))?,
                })
            })
            .collect()
    }

    fn has_job_store(&self, space: &str) -> bool {
        self.root
            .as_ref()
            .is_some_and(|root| root.join(space).join("jobs.db").is_file())
    }

    /// Find a wait in an authorized space without opening new stores. Reads one
    /// row and hides request IDs owned by other accounts.
    pub(crate) fn wait_source(
        &self,
        space: &str,
        id: &str,
        account: Option<AccountId>,
    ) -> Result<Option<(Job, WaitSource)>, Error> {
        if !self.has_job_store(space) {
            return Ok(None);
        }
        let db = self.conn(space)?;
        let conn = db.lock().map_err(db_err)?;
        let found: Option<(String, String)> = conn
            .query_row(
                "SELECT w.job,w.signal FROM plugin_waits w JOIN jobs j ON j.id=w.job
             WHERE w.id=?1 AND (?2 IS NULL OR j.account=?2)",
                params![id, account.map(|id| id.to_string())],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_err)?;
        let Some((id, source)) = found else {
            return Ok(None);
        };
        let job = conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM jobs WHERE id=?1"),
                [&id],
                |r| row(space, r),
            )
            .map_err(db_err)?;
        let source =
            serde_json::from_str(&source).map_err(|_| Error::internal("stored wait source"))?;
        Ok(Some((job, source)))
    }

    /// Publish exactly one outcome and admit its saved checkpoint. A duplicate
    /// with the same receipt can acknowledge without scheduling another run.
    /// One FULL transaction; validation happens before this transition.
    pub(crate) fn resolve_wait(
        &self,
        space: &str,
        id: &str,
        job: &str,
        decision: Option<Decision<'_>>,
    ) -> Result<Resolution, Error> {
        let timed = decision.is_none();
        let (outcome, receipt) = match decision {
            Some(decision) => (
                bounded_json(&decision.outcome, SIGNAL_LIMIT)?,
                decision.receipt,
            ),
            None => (String::from(r#"{"kind":"deadline"}"#), None),
        };
        let cutoff = i64::try_from(self.receipt_retention.as_millis()).unwrap_or(i64::MAX);
        let resolution = self.tx(space, |tx| {
            let now = now_ms();
            if let Some((key, fp)) = receipt {
                let prior: Option<(String, i64)> = tx.query_row(
                    "SELECT decision_fingerprint,resolved_ms FROM plugin_waits
                     WHERE id=?1 AND job=?2 AND decision_request=?3",
                    params![id, job, key], |r| Ok((r.get(0)?, r.get(1)?)),
                ).optional().map_err(db_err)?;
                if let Some((prior_fp, _)) = prior.filter(|(_, at)| *at > now.saturating_sub(cutoff)) {
                    return if prior_fp == fp { Ok(Resolution::Replay) } else { Err(Error::request_reused()) };
                }
            }
            let changed = tx.execute(
                "UPDATE plugin_waits SET outcome=?3,decision_request=?4,decision_fingerprint=?5,resolved_ms=?6
                 WHERE id=?1 AND job=?2 AND outcome IS NULL AND (deadline_ms<=?6)=?7
                 AND EXISTS(SELECT 1 FROM jobs WHERE id=?2 AND state='signal_waiting'
                            AND checkpoints=plugin_waits.checkpoint)",
                params![id, job, outcome, receipt.map(|r| r.0), receipt.map(|r| r.1), now, timed],
            ).map_err(db_err)?;
            if changed == 0 {
                return if timed { Ok(Resolution::Unchanged) } else { Err(Error::conflict("wait no longer accepts a decision")) };
            }
            tx.execute("UPDATE jobs SET state='admitted' WHERE id=?1", [job]).map_err(db_err)?;
            Ok(Resolution::Woken)
        })?;
        if matches!(resolution, Resolution::Woken) {
            self.stop_wait_timer(job, id);
        }
        Ok(resolution)
    }
}

impl Jobs {
    /// Register a deadline notification. The scheduler rechecks durable state
    /// after subscribing so decisions that preceded registration are observed.
    /// Cost: one short map lock.
    pub(crate) fn register_wait_timer(&self, timer: &Timer) -> Arc<tokio::sync::Notify> {
        let notify = Arc::new(tokio::sync::Notify::new());
        let prior = self
            .wait_signals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(timer.job.clone())
            .or_default()
            .insert(timer.id.clone(), notify.clone());
        if let Some(prior) = prior {
            prior.notify_one();
        }
        notify
    }

    /// Release a deadline notification if it still belongs to this task.
    /// Cost: one short map lock.
    pub(crate) fn forget_wait_timer(&self, timer: &Timer, notify: &Arc<tokio::sync::Notify>) {
        let mut signals = self.wait_signals.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(waits) = signals.get_mut(&timer.job) {
            if waits
                .get(&timer.id)
                .is_some_and(|current| Arc::ptr_eq(current, notify))
            {
                waits.remove(&timer.id);
            }
            if waits.is_empty() {
                signals.remove(&timer.job);
            }
        }
    }

    fn stop_wait_timer(&self, job: &str, wait: &str) {
        let signals = self.wait_signals.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(notify) = signals.get(job).and_then(|waits| waits.get(wait)) {
            notify.notify_one();
        }
    }

    /// Stop deadline tasks after a job's cancellation or retirement.
    /// Cost: one short map lock and a notification per registered task.
    pub(super) fn stop_job_wait_timers(&self, job: &str) {
        if let Some(waits) = self
            .wait_signals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(job)
        {
            for notify in waits.into_values() {
                notify.notify_one();
            }
        }
    }

    /// Check whether the registered deadline still belongs to a parked job.
    /// One indexed read, used once to close the timer registration race.
    pub(crate) fn wait_is_pending(&self, timer: &Timer) -> Result<bool, Error> {
        let db = self.conn(&timer.space)?;
        let conn = db.lock().map_err(db_err)?;
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM plugin_waits w JOIN jobs j ON j.id=w.job
                 WHERE w.id=?1 AND w.job=?2 AND w.outcome IS NULL AND j.state='signal_waiting')",
            params![timer.id, timer.job],
            |r| r.get(0),
        )
        .map_err(db_err)
    }
}

impl Jobs {
    /// Look up an admitted job's exact webhook correlation. Unrelated routes
    /// and spaces do not open stores. One throttle check and two indexed reads.
    pub(crate) fn webhook_wait(
        &self,
        space: &str,
        job: &str,
        route: &str,
        correlation: Uuid,
    ) -> Result<Option<(String, Job)>, Error> {
        if self.route(job).as_deref() != Some(space) {
            return Ok(None);
        }
        let db = self.conn(space)?;
        if !db.admit_webhook() {
            return Err(Error::unavailable("webhook verification capacity reached"));
        }
        let conn = db.lock().map_err(db_err)?;
        let wait: Option<String> = conn.query_row(
            "SELECT id FROM plugin_waits WHERE job=?1 AND json_extract(signal,'$.kind')='webhook'
             AND json_extract(signal,'$.route')=?2 AND json_extract(signal,'$.correlation')=?3",
            params![job, route, correlation.to_string()], |r| r.get(0),
        ).optional().map_err(db_err)?;
        let Some(wait) = wait else { return Ok(None) };
        conn.query_row(
            &format!("SELECT {COLUMNS} FROM jobs WHERE id=?1"),
            [job],
            |r| row(space, r),
        )
        .optional()
        .map(|record| record.map(|record| (wait, record)))
        .map_err(db_err)
    }
}

#[cfg(test)]
mod tests;
