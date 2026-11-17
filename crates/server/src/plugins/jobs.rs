//! Durable plugin jobs and their live, non-journaled streams.
//!
//! A job is admitted by a plugin's `start_job` host call and recorded durably before
//! its ID is returned. Each space keeps its jobs in its own SQLite file,
//! `<appdata>/spaces/<space-uuid>/jobs.db` (next to the content crate's `content.db`),
//! opened on first use; there is no installation-wide jobs connection. Records hold
//! the plugin, the starting account and its sign-in ([`Session`], an installation ID,
//! never a token), the run operation and input, the state, the last checkpoint and
//! the final result. Child results are recorded once per (parent, child) pair.
//!
//! Stream chunks are not stored: each running job has a bounded in-memory buffer
//! ([`MAX_BUFFERED_CHUNKS`] chunks and [`MAX_BUFFERED_BYTES`] bytes; the oldest are
//! dropped first) and a per-run `epoch`. Sequence numbers continue from the last
//! checkpoint, so after a restart a subscriber gets the checkpoint and the chunks
//! that follow it, or a reset signal when the chunks it asked for are gone.
//!
//! Locks: one map lock to find a space's connection or a job's route (never held
//! across I/O), one connection lock per space held for each short transaction
//! including its fsync, and one lock per live stream held only for buffer updates.
//! Every method that touches SQLite blocks: call it from a blocking task or a
//! plugin worker thread, never from an async runtime thread.
//!
//! Bounds and retention: [`MAX_ACTIVE_JOBS_PER_SPACE`] active jobs per space,
//! [`MAX_CHILDREN`] children per parent, [`MAX_CHECKPOINT_BYTES`] per checkpoint and
//! result, [`MAX_OPEN_SPACES`] open job files, [`MAX_SUBSCRIBERS_PER_JOB`] and
//! [`MAX_SUBSCRIBERS`] stream subscribers. Final jobs are deleted [`RETAIN_FINAL`]
//! after they finish, and beyond the newest [`MAX_FINAL_PER_SPACE`] per space.

mod checkpoint;
mod claim;
mod database;
pub(crate) use database::JobDatabase;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use calternal_core::{JobState, JobView};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::Value;
use tokio::sync::Semaphore;
use uuid::Uuid;

use super::Session;
use super::job_sql::{
    ACTIVE, COLUMNS, SCHEMA, all_reported, db_err, parse_state, report, retain, row, set_state,
};
use super::live::{Live, LiveMap};
use super::worker::CancelFlag;
use calternal_types::{AccountId, Error};

/// Active (admitted, running or waiting) jobs allowed per space.
pub const MAX_ACTIVE_JOBS_PER_SPACE: usize = 64;
/// Child jobs allowed per parent job.
pub const MAX_CHILDREN: usize = 32;
/// Largest serialized checkpoint, run input or result, in bytes.
pub const MAX_CHECKPOINT_BYTES: usize = 64 * 1024;
/// Largest serialized chunk payload, in bytes.
pub const MAX_CHUNK_BYTES: usize = 16 * 1024;
/// Chunks kept per live job for reconnecting subscribers.
pub const MAX_BUFFERED_CHUNKS: usize = 256;
/// Chunk bytes kept per live job.
pub const MAX_BUFFERED_BYTES: usize = 1024 * 1024;
/// Subscribers allowed per job.
pub const MAX_SUBSCRIBERS_PER_JOB: usize = 8;
/// Subscribers allowed across the server.
pub const MAX_SUBSCRIBERS: usize = 256;
/// Space job files kept open at once; the least recently used is closed beyond this.
pub const MAX_OPEN_SPACES: usize = 64;
/// Runs allowed per job (restarts and wake-ups by children); beyond it the job fails.
pub const MAX_RUNS: u32 = 64;
/// How long a final job's record is kept.
pub const RETAIN_FINAL: Duration = Duration::from_secs(7 * 24 * 3600);
/// Final job records kept per space, newest first.
pub const MAX_FINAL_PER_SPACE: usize = 1024;

/// Serialize `value` if it fits `max` bytes. Cost: one serialization.
pub fn bounded_json(value: &Value, max: usize) -> Result<String, Error> {
    let text = value.to_string();
    if text.len() > max {
        return Err(Error::invalid_input("job data is too large"));
    }
    Ok(text)
}

/// One durable job record.
#[derive(Debug, Clone)]
pub struct Job {
    /// Host-issued UUIDv7.
    pub id: String,
    /// Space UUID owning the job.
    pub space: String,
    /// Plugin whose run operation executes the job.
    pub plugin: String,
    /// The starting account.
    pub account: AccountId,
    /// The starting account's sign-in, rechecked at every boundary.
    pub session: Session,
    /// Declared operation invoked for each run.
    pub operation: String,
    /// Plugin-supplied run input.
    pub input: Value,
    /// Parent job for a child job.
    pub parent: Option<String>,
    /// Current state.
    pub state: JobState,
    /// Last checkpoint, or null.
    pub checkpoint: Value,
    /// Sequence number of the first chunk after the checkpoint.
    pub checkpoint_seq: u64,
    /// Checkpoints written so far; scopes the write receipts of the following run.
    pub checkpoints: u32,
    /// Final result, or null.
    pub result: Value,
    /// Runs started so far.
    pub runs: u32,
    /// Plugin activation generation the job was admitted under.
    pub generation: u32,
}

impl Job {
    /// Client view. Cost: clones the checkpoint and result.
    pub fn view(&self) -> JobView {
        JobView {
            id: self.id.clone(),
            plugin_id: self.plugin.clone(),
            space_id: self.space.clone(),
            state: self.state,
            parent_id: self.parent.clone(),
            checkpoint: self.checkpoint.clone(),
            checkpoint_seq: self.checkpoint_seq,
            result: self.result.clone(),
        }
    }
}

/// A job just moved to running, with its fresh stream and cancellation handle.
pub struct Claimed {
    /// The record after the claim (its `runs` is this run's number).
    pub job: Job,
    /// The job's live stream, reset to a new epoch.
    pub live: Arc<Live>,
    /// Cancels this run's worker invocation.
    pub cancel: Arc<CancelFlag>,
}

/// What a plugin asks for when it starts a job.
pub struct Admission {
    /// Space UUID.
    pub space: String,
    /// Plugin ID.
    pub plugin: String,
    /// The starting account.
    pub account: AccountId,
    /// Starting sign-in.
    pub session: Session,
    /// Declared run operation of the same plugin.
    pub operation: String,
    /// Run input, at most [`MAX_CHECKPOINT_BYTES`] serialized.
    pub input: Value,
    /// Parent job in the same space and plugin.
    pub parent: Option<String>,
    /// Activation generation of the admitting invocation.
    pub generation: u32,
    /// Idempotency key: the same key returns the job it admitted before.
    pub request_key: String,
}

/// How a run ended, as reported to [`Jobs::finish`].
pub enum RunEnd {
    /// The plugin finished the job with a result.
    Done(Value),
    /// The plugin parked the job until its children report.
    Waiting,
    /// Turn lost its machine; explicit resume is required.
    Interrupted,
    /// Provider error is durably waiting for its scheduled retry.
    RetryWaiting,
    /// Provider exited without a definite outcome.
    OutcomeUnknown,
    /// The run failed, was refused or broke protocol.
    Failed,
}

/// One child's final report, as passed to the parent's next run.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChildResult {
    /// Child job UUID.
    pub job_id: String,
    /// Its final state.
    pub state: JobState,
    /// Its result, or null.
    pub result: Value,
}

/// One open space job file and when it was last used (for closing the oldest).
type OpenSpace = (Arc<JobDatabase>, u64);

/// Durable job records plus live streams. Cheap to share behind the app state.
pub struct Jobs {
    pub(super) root: Option<PathBuf>,
    pub(super) security: Option<calternal_auth::agents::Agents>,
    pub(super) receipt_retention: std::time::Duration,
    spaces: Mutex<HashMap<String, OpenSpace>>,
    tick: AtomicUsize,
    pub(super) routes: RwLock<HashMap<String, String>>,
    pub(super) live: LiveMap,
    pub(super) subscribers: Arc<AtomicUsize>,
    permits: Arc<Semaphore>,
    thread_events: Option<Arc<crate::threads::turn_events::Publisher>>,
}

impl Jobs {
    /// Latest turn for one UUID thread inside an already authorized space. Cost:
    /// one indexed space-local query, one bounded job decode; no cross-space scan.
    pub(crate) fn thread_latest(&self, space: &str, thread: &str) -> Result<Option<Job>, Error> {
        let conn = self.conn(space)?;
        let c = conn.lock().unwrap_or_else(|p| p.into_inner());
        c.query_row(&format!("SELECT {COLUMNS} FROM jobs WHERE operation='agent.turn' AND json_extract(input,'$.request.thread_id')=?1 ORDER BY rowid DESC LIMIT 1"),[thread],|r|row(space,r)).optional().map_err(db_err)
    }

    /// Permits for concurrent runs ([`super::runner::MAX_RUNNING`]). Cost: one clone.
    pub fn permits(&self) -> Arc<Semaphore> {
        self.permits.clone()
    }

    /// Jobs stored under `appdata`. Every existing space job file is opened once to
    /// apply retention and load job routes (job ID to space); nothing else runs.
    /// Cost at startup: one directory listing plus, per space with jobs, one open,
    /// one retention delete and one ID scan.
    pub fn open(appdata: &Path) -> Result<Self, Error> {
        let jobs = Self {
            root: Some(appdata.join("spaces")),
            ..Self::disabled()
        };
        let Ok(dir) = std::fs::read_dir(appdata.join("spaces")) else {
            return Ok(jobs);
        };
        for entry in dir.flatten() {
            let space = entry.file_name().to_string_lossy().into_owned();
            if Uuid::parse_str(&space).is_err() || !entry.path().join("jobs.db").is_file() {
                continue;
            }
            let conn = jobs.conn(&space)?;
            let c = conn.lock().unwrap_or_else(|p| p.into_inner());
            let mut st = c.prepare("SELECT id FROM jobs").map_err(db_err)?;
            let ids = st
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(db_err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(db_err)?;
            let mut routes = jobs.routes.write().unwrap_or_else(|p| p.into_inner());
            routes.extend(ids.into_iter().map(|id| (id, space.clone())));
        }
        Ok(jobs)
    }

    /// Attach agent authority from the already guarded directory handle. No new
    /// connection or worker. Without it agent credential operations fail closed.
    pub fn with_security(mut self, security: calternal_auth::agents::Agents) -> Self {
        self.security = Some(security);
        self
    }

    /// Reuse the installation's receipt retention for completed person requests.
    /// Cost: one copy at startup, no I/O.
    pub fn with_receipt_retention(mut self, retention: std::time::Duration) -> Self {
        self.receipt_retention = retention;
        self
    }

    /// Publish durable saved-turn changes on the existing per-space journal.
    /// Install at server startup before recovery. Cost: one shared handle, no I/O.
    pub fn with_thread_events(mut self, content: Arc<calternal_content::Content>) -> Self {
        self.thread_events = Some(Arc::new(crate::threads::turn_events::Publisher::new(
            content,
        )));
        self
    }

    /// A job service that refuses every admission (for hosts without job storage).
    pub fn disabled() -> Self {
        Self {
            root: None,
            security: None,
            receipt_retention: calternal_content::DEFAULT_RECEIPT_RETENTION,
            spaces: Mutex::new(HashMap::new()),
            tick: AtomicUsize::new(0),
            routes: RwLock::new(HashMap::new()),
            live: Arc::new(Mutex::new(HashMap::new())),
            subscribers: Arc::new(AtomicUsize::new(0)),
            permits: Arc::new(Semaphore::new(super::runner::MAX_RUNNING)),
            thread_events: None,
        }
    }

    /// The space's job connection, opening (and applying retention to) it on first
    /// use and closing the least recently used beyond [`MAX_OPEN_SPACES`].
    /// Scoped to trusted host callers. Cost: cache lock and, if cold, opening the
    /// space job database, preparing its schema and applying retention.
    pub(crate) fn conn(&self, space: &str) -> Result<Arc<JobDatabase>, Error> {
        let root = self
            .root
            .as_ref()
            .ok_or_else(|| Error::unavailable("jobs are not available on this host"))?;
        if Uuid::parse_str(space).is_err() {
            return Err(Error::unavailable("jobs need a space UUID"));
        }
        let tick = self.tick.fetch_add(1, Ordering::Relaxed) as u64;
        if let Some((c, used)) = self
            .spaces
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get_mut(space)
        {
            *used = tick;
            return Ok(c.clone());
        }
        let dir = root.join(space);
        calternal_fsio::private::directory(&dir)
            .map_err(|_| Error::internal("job storage directory"))?;
        let path = dir.join("jobs.db");
        calternal_fsio::private::file(&path).map_err(|_| Error::internal("job storage file"))?;
        let c = Connection::open(path).map_err(db_err)?;
        c.pragma_update(None, "journal_mode", "WAL")
            .map_err(db_err)?;
        c.pragma_update(None, "synchronous", "FULL")
            .map_err(db_err)?;
        c.busy_timeout(Duration::from_secs(5)).map_err(db_err)?;
        c.execute_batch(SCHEMA).map_err(db_err)?;
        c.execute_batch(crate::threads::turn_events::SCHEMA)
            .map_err(db_err)?;
        retain(&c)?;
        let c = Arc::new(JobDatabase::new(c));
        let mut spaces = self.spaces.lock().unwrap_or_else(|p| p.into_inner());
        if spaces.len() >= MAX_OPEN_SPACES
            && let Some(oldest) = spaces
                .iter()
                .min_by_key(|(_, (_, u))| *u)
                .map(|(k, _)| k.clone())
        {
            spaces.remove(&oldest);
        }
        spaces.insert(space.to_owned(), (c.clone(), tick));
        Ok(c)
    }

    pub(super) fn tx<T>(
        &self,
        space: &str,
        f: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let conn = self.conn(space)?;
        let mut c = conn.lock().unwrap_or_else(|p| p.into_inner());
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_err)?;
        let value = f(&tx)?;
        tx.commit().map_err(db_err)?;
        if let Some(publisher) = &self.thread_events {
            let pending = c
                .prepare_cached("SELECT EXISTS(SELECT 1 FROM saved_turn_events)")
                .and_then(|mut query| query.query_row([], |row| row.get::<_, bool>(0)));
            drop(c);
            match pending {
                Ok(true) => publisher.wake(space, conn),
                Ok(false) => {}
                Err(error) => tracing::warn!(%error, %space, "saved turn journal wake deferred"),
            }
        }
        Ok(value)
    }

    /// The space a job lives in, if it exists. Cost: one shared map read.
    pub fn route(&self, id: &str) -> Option<String> {
        self.routes
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .get(id)
            .cloned()
    }

    /// Read one job. Cost: one indexed read.
    pub fn get(&self, space: &str, id: &str) -> Result<Option<Job>, Error> {
        let conn = self.conn(space)?;
        let c = conn.lock().unwrap_or_else(|p| p.into_inner());
        c.query_row(
            &format!("SELECT {COLUMNS} FROM jobs WHERE id=?1"),
            [id],
            |r| row(space, r),
        )
        .optional()
        .map_err(db_err)
    }

    /// Durably admit a job, or return the one already admitted under the same key.
    /// Refused: an over-full space, an oversized input, or a parent that is missing,
    /// final, of another plugin, or already has [`MAX_CHILDREN`] children.
    /// Cost: one FULL transaction with a few indexed reads and one insert.
    pub fn admit(&self, a: Admission) -> Result<String, Error> {
        let input = bounded_json(&a.input, MAX_CHECKPOINT_BYTES)?;
        let session = serde_json::to_string(&a.session).map_err(|_| Error::internal("encode"))?;
        let space = a.space.clone();
        let id = self.tx(&space, |tx| {
            if let Some(id) = tx
                .query_row("SELECT id FROM jobs WHERE request_key=?1", [&a.request_key], |r| {
                    r.get::<_, String>(0)
                })
                .optional()
                .map_err(db_err)?
            {
                return Ok(id);
            }
            if a.operation=="agent.turn" {
                let thread=a.input["request"]["thread_id"].as_str().ok_or_else(||Error::invalid_input("turn needs a thread UUID"))?;
                let active:bool=tx.query_row(&format!("SELECT EXISTS(SELECT 1 FROM jobs WHERE operation='agent.turn' AND json_extract(input,'$.request.thread_id')=?1 AND state IN {ACTIVE})"),[thread],|r|r.get(0)).map_err(db_err)?;
                if active{return Err(Error::conflict("this thread already has active or interrupted work"));}
            }
            let active: i64 = tx
                .query_row(&format!("SELECT count(*) FROM jobs WHERE state IN {ACTIVE}"), [], |r| r.get(0))
                .map_err(db_err)?;
            if active as usize >= MAX_ACTIVE_JOBS_PER_SPACE {
                return Err(Error::unavailable("too many active jobs in this space")
                    .with_details(serde_json::json!({"reason": "job_capacity"})));
            }
            if let Some(parent) = &a.parent {
                let ok: Option<(String, String)> = tx
                    .query_row("SELECT plugin,state FROM jobs WHERE id=?1", [parent], |r| {
                        Ok((r.get(0)?, r.get(1)?))
                    })
                    .optional()
                    .map_err(db_err)?;
                let children: i64 = tx
                    .query_row("SELECT count(*) FROM jobs WHERE parent=?1", [parent], |r| r.get(0))
                    .map_err(db_err)?;
                if !ok.is_some_and(|(p, s)| p == a.plugin && !parse_state(&s).is_final())
                    || children as usize >= MAX_CHILDREN
                {
                    return Err(Error::invalid_input("parent job cannot take a child"));
                }
            }
            let id = Uuid::now_v7().to_string();
            tx.execute(
                "INSERT INTO jobs VALUES(?1,?2,?3,?4,?5,?6,?7,'admitted','null',0,0,'null',0,?8,?9,NULL)",
                params![id, a.plugin, a.account.to_string(), session, a.operation, input, a.parent, a.generation, a.request_key],
            )
            .map_err(db_err)?;
            Ok(id)
        })?;
        self.routes
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id.clone(), space);
        Ok(id)
    }

    /// Durably record a checkpoint from the current run; the next run resumes from it
    /// and its sequence numbers continue after it. Cost: one FULL update.
    pub fn checkpoint(
        &self,
        space: &str,
        id: &str,
        run: u32,
        live: &Live,
        checkpoint: &Value,
    ) -> Result<u32, Error> {
        let text = bounded_json(checkpoint, MAX_CHECKPOINT_BYTES)?;
        let seq = live.next_seq() as i64;
        self.tx(space, |tx| {
            let n = tx
                .execute(
                    "UPDATE jobs SET checkpoint=?1,checkpoint_seq=?2,checkpoints=checkpoints+1 WHERE id=?3 AND state='running' AND runs=?4",
                    params![text, seq, id, run],
                )
                .map_err(db_err)?;
            if n == 0 {
                return Err(Error::conflict("job is no longer running"));
            }
            tx.query_row("SELECT checkpoints FROM jobs WHERE id=?1", [id], |r| r.get(0)).map_err(db_err)
        })
    }

    /// Finish run `run`. Late reports (cancelled job, superseded run) change nothing.
    /// A final job reports exactly once to its parent; a parent whose children have all
    /// reported is re-admitted. Returns the jobs to schedule.
    /// Cost: one FULL transaction with a few indexed reads and writes.
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
            RunEnd::Failed => (JobState::Failed, "null".into()),
        };
        let wake = self.tx(space, |tx| {
            let current: Option<(String, u32, Option<String>)> = tx
                .query_row(
                    "SELECT state,runs,parent FROM jobs WHERE id=?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()
                .map_err(db_err)?;
            let Some((s, runs, parent)) = current else {
                return Ok(vec![]);
            };
            if s != "running" || runs != run {
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

    /// Cancel a job and every active descendant. Their runs are told to stop, their
    /// later output is discarded, and the job reports `cancelled` to its parent unless
    /// that parent was cancelled too. Returns the jobs to schedule (a woken parent).
    /// Cost: one FULL transaction walking the descendant tree (at most
    /// [`MAX_CHILDREN`] per level within the space's active-job bound).
    pub fn cancel(&self, space: &str, id: &str) -> Result<Vec<String>, Error> {
        let (cancelled, wake) = self.tx(space, |tx| {
            let mut st = tx
                .prepare(&format!(
                    "WITH RECURSIVE tree(id) AS (SELECT ?1 UNION SELECT j.id FROM jobs j JOIN tree t ON j.parent=t.id) \
                     SELECT j.id FROM jobs j JOIN tree t ON j.id=t.id WHERE j.state IN {ACTIVE}"
                ))
                .map_err(db_err)?;
            let ids = st
                .query_map([id], |r| r.get::<_, String>(0))
                .map_err(db_err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(db_err)?;
            drop(st);
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
        for job in cancelled {
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
        Ok(wake)
    }
}
