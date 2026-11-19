//! Durable plugin jobs and their live, non-journaled streams.
//!
//! A job is admitted by a plugin's `start_job` host call and recorded durably before
//! its ID is returned. Each space keeps its jobs in its own SQLite file,
//! `<appdata>/spaces/<space-uuid>/content.db`,
//! opened on first use; there is no installation-wide jobs connection. Records hold
//! the plugin, the starting account and its sign-in ([`Session`], an installation ID,
//! never a token), the run operation and input, the state, the last checkpoint and
//! the final result. Child results are recorded once per (parent, child) pair.
//! A job can park on a typed signal with a deadline. The first outcome and
//! its replay receipt commit in the same space transaction as readmission.
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
mod complete;
mod database;
pub(crate) mod waits;
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
use super::job_sql::{ACTIVE, COLUMNS, SCHEMA, db_err, parse_state, retain, row};
use super::live::{Live, LiveMap};
use super::worker::CancelFlag;
use calternal_types::{AccountId, Error, now_ms};

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
    /// Public wait summary, when parked for a signal.
    pub(super) wait: Option<calternal_core::JobWait>,
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
    /// Space activation generation captured with the job.
    pub generation: u32,
    /// Installed worker revision captured with the job. Native jobs use zero.
    pub(super) package_revision: u32,
}

impl Job {
    /// Remove provider login, placement and person-request references from
    /// someone else's turn. Cost: existing view clone plus constant removals.
    pub(crate) fn view_for(&self, account: AccountId) -> JobView {
        let mut view = self.view();
        if self.operation == "agent.turn"
            && (self.account != account || self.input["check"] == true)
        {
            for value in [&mut view.checkpoint, &mut view.result] {
                if let Some(fields) = value.as_object_mut() {
                    for key in [
                        "machine_id",
                        "lease_id",
                        "request_machine",
                        "person_request",
                        "error",
                        "placement",
                        "interruption",
                        "session_id",
                        "session_notice",
                    ] {
                        fields.remove(key);
                    }
                }
            }
        }
        view
    }

    /// Client view. Cost: clones the checkpoint and result.
    pub fn view(&self) -> JobView {
        JobView {
            id: self.id.clone(),
            plugin_id: self.plugin.clone(),
            space_id: self.space.clone(),
            state: self.state,
            parent_id: self.parent.clone(),
            wait: self.wait.clone(),
            checkpoint: self.checkpoint.clone(),
            checkpoint_seq: self.checkpoint_seq,
            result: self.result.clone(),
        }
    }
}

/// A claimed job with its fresh stream and cancellation handle.
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
    /// The wait was rejected before parking, with a caller-safe explanation.
    WaitRejected(Error),
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
    wait_signals: Mutex<HashMap<String, HashMap<String, Arc<tokio::sync::Notify>>>>,
    spaces: Mutex<HashMap<String, OpenSpace>>,
    pub(super) lifecycle: calternal_types::OwnerLifecycle,
    tick: AtomicUsize,
    pub(super) routes: RwLock<HashMap<String, String>>,
    pub(super) live: LiveMap,
    pub(super) subscribers: Arc<AtomicUsize>,
    permits: Arc<Semaphore>,
    pub(crate) push: std::sync::OnceLock<crate::web_push::Service>,
    thread_events: Option<Arc<crate::threads::turn_events::Publisher>>,
}

impl Jobs {
    /// Latest turn for one UUID thread inside an already authorized space. Cost:
    /// one indexed space-local query, one bounded job decode; no cross-space scan.
    pub(crate) fn thread_latest(&self, space: &str, thread: &str) -> Result<Option<Job>, Error> {
        let conn = self.conn(space)?;
        let c = conn.lock().map_err(db_err)?;
        c.query_row(&format!("SELECT {COLUMNS} FROM jobs WHERE operation='agent.turn' AND json_extract(input,'$.request.thread_id')=?1 AND json_extract(input,'$.check') IS NOT 1 AND state!='awaiting_thread' ORDER BY rowid DESC LIMIT 1"),[thread],|r|row(space,r)).optional().map_err(db_err)
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
        Self::open_with_lifecycle(appdata, calternal_types::OwnerLifecycle::default())
    }

    /// Open jobs with the host's shared owner lifecycle, before admitting work.
    /// A marked space is skipped during the initial inventory.
    pub fn open_with_lifecycle(
        appdata: &Path,
        lifecycle: calternal_types::OwnerLifecycle,
    ) -> Result<Self, Error> {
        let jobs = Self {
            root: Some(appdata.join("spaces")),
            lifecycle,
            ..Self::disabled()
        };
        let Ok(dir) = std::fs::read_dir(appdata.join("spaces")) else {
            return Ok(jobs);
        };
        for entry in dir.flatten() {
            let space = entry.file_name().to_string_lossy().into_owned();
            if Uuid::parse_str(&space).is_err() || !entry.path().join("content.db").is_file() {
                continue;
            }
            let conn = match jobs.conn(&space) {
                Ok(conn) => conn,
                Err(err) if err.code == calternal_types::ErrorCode::NotFound => continue,
                Err(err) => return Err(err),
            };
            let c = conn.lock().map_err(db_err)?;
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
            wait_signals: Mutex::new(HashMap::new()),
            spaces: Mutex::new(HashMap::new()),
            lifecycle: calternal_types::OwnerLifecycle::default(),
            tick: AtomicUsize::new(0),
            routes: RwLock::new(HashMap::new()),
            live: Arc::new(Mutex::new(HashMap::new())),
            subscribers: Arc::new(AtomicUsize::new(0)),
            permits: Arc::new(Semaphore::new(super::runner::MAX_RUNNING)),
            thread_events: None,
            push: std::sync::OnceLock::new(),
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
        let space_id = calternal_types::SpaceId::parse(space)
            .map_err(|_| Error::unavailable("jobs need a space UUID"))?;
        let _step = self.lifecycle.step(space_id.as_uuid())?;
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
        let path = dir.join("content.db");
        let appdata = root.parent().ok_or_else(|| Error::internal("jobs root"))?;
        let c = calternal_content::open_space_database(appdata, space_id)?;
        calternal_store::migrations::initialize_component(&c, hosted_schema()).map_err(db_err)?;
        retain(&c)?;
        let c = Arc::new(JobDatabase::new(c, path, self.lifecycle.clone(), space_id));
        let mut spaces = self.spaces.lock().unwrap_or_else(|p| p.into_inner());
        if let Some((cached, used)) = spaces.get_mut(space) {
            *used = tick;
            return Ok(cached.clone());
        }
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

    /// Detach an already drained space's cached jobs and identifier routes.
    pub(crate) fn retire_space(&self, space: &str) {
        self.spaces
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(space);
        let retired = {
            let mut routes = self.routes.write().unwrap_or_else(|p| p.into_inner());
            let mut retired = Vec::new();
            routes.retain(|job, value| {
                if value == space {
                    retired.push(job.clone());
                    false
                } else {
                    true
                }
            });
            retired
        };
        self.stop_live(retired);
    }

    pub(super) fn tx<T>(
        &self,
        space: &str,
        f: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let conn = self.conn(space)?;
        let mut c = conn.lock().map_err(db_err)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_err)?;
        let value = f(&tx)?;
        #[cfg(feature = "perf-timings")]
        let commit = calternal_store::timings::SaveStage::JobCommit.start();
        tx.commit().map_err(db_err)?;
        #[cfg(feature = "perf-timings")]
        commit.finish();
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
        let c = conn.lock().map_err(db_err)?;
        c.query_row(
            &format!("SELECT {COLUMNS} FROM jobs WHERE id=?1 AND state!='awaiting_thread'"),
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
        self.admit_package(a, 0)
    }

    /// Find a retained saved-thread admission even when its completed job has
    /// already been evicted. One indexed lookup under the space job lock.
    pub(crate) fn thread_admission(
        &self,
        space: &str,
        request_key: &str,
    ) -> Result<Option<String>, Error> {
        let conn = self.conn(space)?;
        let c = conn.lock().map_err(db_err)?;
        let key = ring::digest::digest(&ring::digest::SHA256, request_key.as_bytes());
        thread_send_receipt(&c, key.as_ref(), now_ms())
    }

    /// Admit a saved turn and record its request key in the same FULL job
    /// transaction. The receipt survives the bounded completed-job history.
    #[cfg(any(test, not(target_os = "linux")))]
    pub(crate) fn admit_thread(&self, a: Admission) -> Result<String, Error> {
        self.admit_job(a, 0, true)
    }

    /// Admit a job with the immutable worker revision it must use on resume.
    /// Cost and error conditions match native job admission.
    pub(super) fn admit_package(&self, a: Admission, revision: u32) -> Result<String, Error> {
        self.admit_job(a, revision, false)
    }

    fn admit_job(&self, a: Admission, revision: u32, thread_send: bool) -> Result<String, Error> {
        let space = a.space.clone();
        let actor = a.account;
        let _actor = self.lifecycle.step(actor.as_uuid())?;
        let id = self.tx(&space, |tx| {
            self.insert_admission(tx, a, revision, thread_send, None)
        })?;
        self.register_admission(&space, &id);
        Ok(id)
    }

    /// Install the jobs component before taking the content WAL write lock.
    #[cfg(target_os = "linux")]
    pub(crate) fn prepare_thread_admission(&self, space: &str) -> Result<(), Error> {
        self.conn(space).map(|_| ())
    }

    /// Add the initial job and its receipt to the same FULL transaction as
    /// the Markdown append intent. The journal insertion releases the job
    /// only after the named file has synced.
    #[cfg(target_os = "linux")]
    pub(crate) fn stage_thread_admission(
        &self,
        tx: &rusqlite::Transaction<'_>,
        a: Admission,
        item: Uuid,
        seq: i64,
        request: &str,
    ) -> Result<(), Error> {
        self.insert_admission(tx, a, 0, true, Some((item, seq, request)))?;
        Ok(())
    }

    /// Populate the in-memory routing cache after the durable job is visible.
    pub(crate) fn register_admission(&self, space: &str, job: &str) {
        self.routes
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(job.into(), space.into());
    }

    /// Wake the existing journal drain after an append has activated its job.
    pub(crate) fn wake_saved_thread(&self, space: &str) {
        if let Some(publisher) = &self.thread_events
            && let Ok(conn) = self.conn(space)
        {
            publisher.wake(space, conn);
        }
    }

    fn insert_admission(
        &self,
        tx: &rusqlite::Transaction<'_>,
        a: Admission,
        revision: u32,
        thread_send: bool,
        staged: Option<(Uuid, i64, &str)>,
    ) -> Result<String, Error> {
        let generation = (i64::from(revision) << 32) | i64::from(a.generation);
        let input = bounded_json(&a.input, MAX_CHECKPOINT_BYTES)?;
        let session = serde_json::to_string(&a.session).map_err(|_| Error::internal("encode"))?;
        let receipt = thread_send
            .then(|| ring::digest::digest(&ring::digest::SHA256, a.request_key.as_bytes()));
        let now = now_ms();
        if let Some(key) = &receipt
            && let Some(id) = thread_send_receipt(tx, key.as_ref(), now)?
        {
            return Ok(id);
        }
        if let Some(id) = tx
            .query_row(
                "SELECT id FROM jobs WHERE request_key=?1",
                [&a.request_key],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(db_err)?
        {
            return Ok(id);
        }
        if a.operation == "agent.turn" {
            let thread = a.input["request"]["thread_id"]
                .as_str()
                .ok_or_else(|| Error::invalid_input("turn needs a thread UUID"))?;
            let active:bool=tx.query_row(&format!("SELECT EXISTS(SELECT 1 FROM jobs WHERE operation='agent.turn' AND json_extract(input,'$.request.thread_id')=?1 AND json_extract(input,'$.check') IS NOT 1 AND state IN {ACTIVE})"),[thread],|r|r.get(0)).map_err(db_err)?;
            if active {
                return Err(Error::conflict(
                    "this thread already has active or interrupted work",
                ));
            }
        }
        let active: i64 = tx
            .query_row(
                &format!("SELECT count(*) FROM jobs WHERE state IN {ACTIVE}"),
                [],
                |r| r.get(0),
            )
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
                .query_row("SELECT count(*) FROM jobs WHERE parent=?1", [parent], |r| {
                    r.get(0)
                })
                .map_err(db_err)?;
            if !ok.is_some_and(|(p, s)| p == a.plugin && !parse_state(&s).is_final())
                || children as usize >= MAX_CHILDREN
            {
                return Err(Error::invalid_input("parent job cannot take a child"));
            }
        }
        let id = Uuid::now_v7().to_string();
        tx.execute(
            "INSERT INTO jobs VALUES(?1,?2,?3,?4,?5,?6,?7,?10,'null',0,0,'null',0,?8,?9,NULL)",
            params![
                id,
                a.plugin,
                a.account.to_string(),
                session,
                a.operation,
                input,
                a.parent,
                generation,
                a.request_key,
                if staged.is_some() {
                    "awaiting_thread"
                } else {
                    "admitted"
                }
            ],
        )
        .map_err(db_err)?;
        if let Some(key) = &receipt {
            let expires = now.saturating_add(
                i64::try_from(self.receipt_retention.as_millis()).unwrap_or(i64::MAX),
            );
            tx.execute(
                "DELETE FROM thread_send_receipts WHERE request_key IN
                     (SELECT request_key FROM thread_send_receipts WHERE expires_ms<=?1
                      ORDER BY expires_ms LIMIT 128)",
                [now],
            )
            .map_err(db_err)?;
            tx.execute(
                    "INSERT INTO thread_send_receipts VALUES(?1,?2,?3)
                     ON CONFLICT(request_key) DO UPDATE SET job=excluded.job,expires_ms=excluded.expires_ms",
                    params![key.as_ref(), id, expires],
                ).map_err(db_err)?;
        }
        if let Some((item, seq, request)) = staged {
            let key = receipt
                .as_ref()
                .ok_or_else(|| Error::internal("missing admission receipt"))?;
            tx.execute(
                "INSERT INTO thread_job_placement VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    item.to_string(),
                    seq,
                    id,
                    key.as_ref(),
                    a.account.to_string(),
                    request
                ],
            )
            .map_err(db_err)?;
        }
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
        let (checkpoints, _actor) = self.tx(space, |tx| {
            let (checkpoints, id): (u32, String) = tx
                .query_row(
                    "UPDATE jobs SET checkpoint=?1,checkpoint_seq=?2,checkpoints=checkpoints+1
                      WHERE id=?3 AND state='running' AND runs=?4 RETURNING checkpoints,account",
                    params![text, seq, id, run],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(db_err)?
                .ok_or_else(|| Error::conflict("job is no longer running"))?;
            let account = calternal_types::AccountId::parse(&id)?;
            Ok((checkpoints, self.lifecycle.step(account.as_uuid())?))
        })?;
        Ok(checkpoints)
    }
}

fn hosted_schema() -> calternal_store::migrations::Schema<'static> {
    static BATCHES: std::sync::OnceLock<[&'static str; 5]> = std::sync::OnceLock::new();
    calternal_store::migrations::Schema::new(
        "jobs",
        4,
        BATCHES.get_or_init(|| {
            [
                SCHEMA,
                crate::threads::turn_events::schema(),
                waits::SCHEMA,
                THREAD_SEND_RECEIPTS,
                THREAD_JOB_PLACEMENT,
            ]
        }),
        &[],
    )
}

const THREAD_JOB_PLACEMENT: &str = "
    CREATE TABLE thread_job_placement(
      item TEXT NOT NULL, seq INTEGER NOT NULL, job TEXT NOT NULL,
      receipt BLOB NOT NULL, account TEXT NOT NULL, request TEXT NOT NULL,
      PRIMARY KEY(item,seq),
      FOREIGN KEY(item,seq) REFERENCES versions(item,seq) ON DELETE CASCADE
    ) STRICT, WITHOUT ROWID;
    CREATE UNIQUE INDEX thread_job_publication ON thread_job_placement(account,request);
    CREATE TRIGGER start_placed_thread_job AFTER INSERT ON journal
      WHEN NEW.kind='threads.send'
      BEGIN
        UPDATE jobs SET state='admitted' WHERE state='awaiting_thread'
          AND id=(SELECT job FROM thread_job_placement WHERE account=NEW.author AND request=NEW.request_id);
        DELETE FROM thread_job_placement WHERE account=NEW.author AND request=NEW.request_id;
      END;
    CREATE TRIGGER cancel_unplaced_thread_job BEFORE DELETE ON thread_job_placement
      WHEN EXISTS(SELECT 1 FROM jobs WHERE id=OLD.job AND state='awaiting_thread')
      BEGIN
        DELETE FROM thread_send_receipts WHERE request_key=OLD.receipt;
        DELETE FROM jobs WHERE id=OLD.job;
      END;
";

const THREAD_SEND_RECEIPTS: &str = "
    CREATE TABLE thread_send_receipts(
        request_key BLOB PRIMARY KEY CHECK(length(request_key)=32),
        job TEXT NOT NULL,
        expires_ms INTEGER NOT NULL
    ) STRICT, WITHOUT ROWID;
    CREATE INDEX thread_send_receipts_expiry ON thread_send_receipts(expires_ms);
";

/// Stable request key for one account's saved-thread admission.
pub(crate) fn thread_send_key(account: AccountId, request_id: &str) -> String {
    format!("threads/{account}/{request_id}")
}

fn thread_send_receipt(conn: &Connection, key: &[u8], now: i64) -> Result<Option<String>, Error> {
    conn.prepare_cached(
        "SELECT job FROM thread_send_receipts WHERE request_key=?1 AND expires_ms>?2",
    )
    .map_err(db_err)?
    .query_row(params![key, now], |row| row.get(0))
    .optional()
    .map_err(db_err)
}

#[cfg(test)]
mod admission_receipt_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn thread_send_receipt_survives_job_retention_and_restart() {
        let dir = tempfile::tempdir().unwrap();
        let space = calternal_types::SpaceId::new().to_string();
        let path = dir.path().join("spaces").join(&space).join("content.db");
        let account = AccountId::new();
        let thread = Uuid::now_v7();
        let key = thread_send_key(account, "saved-send");
        let admission = || Admission {
            space: space.clone(),
            account,
            session: Session::Static,
            plugin: "calternal-orchestrator".into(),
            operation: "agent.turn".into(),
            input: json!({"request":{"thread_id":thread}}),
            parent: None,
            generation: 1,
            request_key: key.clone(),
        };
        let jobs = Jobs::open(dir.path()).unwrap();
        let job = jobs.admit_thread(admission()).unwrap();
        assert_eq!(
            jobs.thread_admission(&space, &key).unwrap(),
            Some(job.clone())
        );
        drop(jobs);

        let conn = Connection::open(&path).unwrap();
        conn.execute("DELETE FROM jobs WHERE id=?1", [&job])
            .unwrap();
        drop(conn);

        let jobs = Jobs::open(dir.path()).unwrap();
        assert_eq!(
            jobs.thread_admission(&space, &key).unwrap(),
            Some(job.clone())
        );
        assert_eq!(jobs.admit_thread(admission()).unwrap(), job);
        assert!(jobs.get(&space, &job).unwrap().is_none());
    }
}
