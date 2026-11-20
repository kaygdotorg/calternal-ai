//! Per-slot projection handles. Recovery drains the attached readers before
//! replacing the derived database and never holds the cache mutex during I/O.
use super::replay;
use crate::{Error, Result, SpaceId, journal::lock, types::internal};
use calternal_store::{ProjectionJournal, SyncToken};
use rusqlite::Connection;
use std::{
    io::Read,
    path::Path,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const CACHE_IDLE: Duration = Duration::from_secs(5);
const KEPT_PAIRS: usize = 8;
const KEEP: usize = 2;
const FAILED_ATTEMPTS: u8 = 5;

/// Bound retained SQLite pairs across the service. Exhausting the cache
/// budget drops an idle handle; it never delays or refuses an operation.
pub(crate) struct HandleBudget {
    readers: Arc<Semaphore>,
    writers: Arc<Semaphore>,
}

impl Default for HandleBudget {
    fn default() -> Self {
        Self {
            readers: Arc::new(Semaphore::new(KEPT_PAIRS)),
            writers: Arc::new(Semaphore::new(KEPT_PAIRS)),
        }
    }
}

struct Cached<T> {
    value: T,
    retained_at: Instant,
    _permit: OwnedSemaphorePermit,
}

impl<T> Cached<T> {
    fn retain(value: T, budget: &Arc<Semaphore>) -> Option<Self> {
        Some(Self {
            value,
            retained_at: Instant::now(),
            _permit: budget.clone().try_acquire_owned().ok()?,
        })
    }

    fn expired(&self) -> bool {
        self.retained_at.elapsed() >= CACHE_IDLE
    }
}

#[derive(Default)]
pub(crate) struct Runtime {
    pub(super) gate: RwLock<()>,
    state: Mutex<State>,
    revision: AtomicU64,
}

#[derive(Default)]
struct State {
    dormant: bool,
    published: Option<(SyncToken, u64)>,
    writer: Option<Cached<Writer>>,
    readers: Vec<Cached<Reader>>,
    failure: Option<Failure>,
    checked_failure: bool,
    dirty_failure: bool,
}

pub(super) struct Writer {
    pub(super) conn: Connection,
    pub(super) journal: ProjectionJournal,
}

pub(super) struct Reader {
    pub(super) conn: Connection,
    pub(super) journal: ProjectionJournal,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Action {
    Reader,
    Replay,
    Replace,
    Rebuilding,
}

impl Action {
    fn name(self) -> &'static str {
        match self {
            Self::Reader => "reader",
            Self::Replay => "replay",
            Self::Replace => "replace",
            Self::Rebuilding => "rebuilding",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Failure {
    attempts: u8,
    retry_at_ms: i64,
    pub(super) action: Action,
    pub(super) reason: &'static str,
}

impl Failure {
    pub(super) fn terminal(self) -> bool {
        self.action != Action::Rebuilding && self.attempts >= FAILED_ATTEMPTS
    }

    pub(super) fn can_retry(self) -> bool {
        !self.terminal() && self.retry_at_ms <= calternal_types::now_ms()
    }
}

impl Runtime {
    pub(super) fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }

    pub(super) fn invalidate(&self) {
        self.revision.fetch_add(1, Ordering::AcqRel);
    }

    pub(super) fn idle_at(&self, head: Option<&SyncToken>) -> bool {
        let state = lock(&self.state);
        !state.dirty_failure
            && state.failure.is_none_or(|f| f.action == Action::Reader)
            && (state.dormant
                || state
                    .published
                    .as_ref()
                    .is_some_and(|(published, revision)| {
                        *revision == self.revision() && Some(published) == head
                    }))
    }

    pub(super) fn dormant(&self) {
        lock(&self.state).dormant = true;
    }

    pub(super) fn published(&self, head: SyncToken, revision: u64) {
        lock(&self.state).published = Some((head, revision));
    }

    pub(super) fn take_writer(&self) -> Option<Writer> {
        let mut state = lock(&self.state);
        state.published = None;
        state.writer.take().map(|cached| cached.value)
    }

    pub(super) fn put_writer(&self, writer: Writer, budget: &HandleBudget) {
        let cached = Cached::retain(writer, &budget.writers);
        let mut state = lock(&self.state);
        state.dormant = false;
        state.writer = cached;
    }

    pub(super) fn take_reader(&self) -> Option<Reader> {
        lock(&self.state).readers.pop().map(|cached| cached.value)
    }

    pub(super) fn put_reader(&self, reader: Reader, budget: &HandleBudget) {
        let Some(reader) = Cached::retain(reader, &budget.readers) else {
            return;
        };
        let mut state = lock(&self.state);
        if state.failure.is_none() && state.readers.len() < KEEP {
            state.readers.push(reader);
        }
    }

    pub(super) fn expire_handles(&self) {
        let (writer, readers) = {
            let mut state = lock(&self.state);
            let writer = state
                .writer
                .as_ref()
                .is_some_and(Cached::expired)
                .then(|| state.writer.take())
                .flatten();
            let readers = state
                .readers
                .extract_if(.., |reader| reader.expired())
                .collect::<Vec<_>>();
            (writer, readers)
        };
        drop((writer, readers));
    }

    pub(super) fn drain(&self) {
        let (writer, readers) = {
            let mut state = lock(&self.state);
            state.published = None;
            (state.writer.take(), std::mem::take(&mut state.readers))
        };
        drop((writer, readers));
    }

    pub(super) fn failure(&self) -> Option<Failure> {
        lock(&self.state).failure
    }

    pub(super) fn initialize_failure(&self, source: &Path) -> Result<()> {
        if lock(&self.state).checked_failure {
            return Ok(());
        }
        let saved = read_failure(source)?;
        let mut state = lock(&self.state);
        if !state.checked_failure {
            state.checked_failure = true;
            state.failure = saved;
        }
        Ok(())
    }

    pub(super) fn worker_deferred(&self) -> bool {
        let state = lock(&self.state);
        !state.dirty_failure
            && state.failure.is_some_and(|failure| {
                failure.terminal() || (failure.action != Action::Reader && !failure.can_retry())
            })
    }

    pub(super) fn reader_blocked(&self) -> bool {
        self.failure()
            .is_some_and(|failure| failure.action != Action::Reader || !failure.can_retry())
    }

    pub(super) fn report_failure(&self, space: SpaceId, error: &Error, fallback: Action) {
        let mut state = lock(&self.state);
        if state.failure.is_some_and(Failure::terminal) {
            return;
        }
        if fallback == Action::Reader
            && state
                .failure
                .is_some_and(|f| f.action != Action::Reader || !f.can_retry())
        {
            return;
        }
        let classification = error
            .details
            .as_ref()
            .and_then(|value| value["window_failure"].as_str());
        let (action, reason) = match classification {
            Some("sidecar_schema") => (Action::Replace, "sidecar_schema"),
            Some("sidecar_corrupt") => (Action::Replace, "sidecar_corrupt"),
            Some("malformed_source_metadata") => (fallback, "malformed_source_metadata"),
            _ if fallback == Action::Reader => (fallback, "reader_failed"),
            _ => (fallback, "replay_failed"),
        };
        let attempts = state.failure.map_or(1, |old| old.attempts + 1);
        let retry_at_ms = calternal_types::now_ms() + (100 << attempts.min(5));
        state.failure = Some(Failure {
            attempts,
            retry_at_ms,
            action,
            reason,
        });
        state.dirty_failure = true;
        drop(state);
        tracing::warn!(%space, reason, attempts, "window projection failed");
    }

    pub(super) fn replaced(&self) {
        let mut state = lock(&self.state);
        if let Some(failure) = &mut state.failure {
            failure.action = Action::Rebuilding;
            failure.reason = "rebuilding";
            state.dirty_failure = true;
        }
    }

    pub(super) fn recovered(&self, by_reader: bool) {
        let mut state = lock(&self.state);
        if state
            .failure
            .is_some_and(|f| (f.action == Action::Reader) == by_reader && !f.terminal())
        {
            state.failure = None;
            state.dirty_failure = true;
        }
    }

    // The worker is the only marker writer. Requests only update cache state.
    pub(super) fn flush_failure(&self, source: &Path) -> Result<()> {
        let saved = {
            let mut state = lock(&self.state);
            if !state.dirty_failure {
                return Ok(());
            }
            state.dirty_failure = false;
            state.failure
        };
        let result = match saved {
            Some(failure) => {
                let body = format!(
                    "{} {} {} {}\n",
                    failure.reason,
                    failure.attempts,
                    failure.retry_at_ms,
                    failure.action.name()
                );
                calternal_fsio::write_atomic_mode(&failure_path(source), body.as_bytes(), 0o600)
                    .map_err(internal)
            }
            None => remove(&failure_path(source)),
        };
        if result.is_err() {
            lock(&self.state).dirty_failure = true;
        }
        result
    }
}

fn failure_path(source: &Path) -> std::path::PathBuf {
    source.with_file_name("windows-v1.failure")
}

pub(super) fn read_failure(source: &Path) -> Result<Option<Failure>> {
    let file = match std::fs::File::open(failure_path(source)) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(internal(error)),
    };
    let mut text = String::new();
    file.take(256).read_to_string(&mut text).map_err(internal)?;
    let mut fields = text.split_whitespace();
    let reason = match fields.next() {
        Some("sidecar_schema") => "sidecar_schema",
        Some("sidecar_corrupt") => "sidecar_corrupt",
        Some("malformed_source_metadata") => "malformed_source_metadata",
        Some("reader_failed") => "reader_failed",
        Some("rebuilding") => "rebuilding",
        _ => "replay_failed",
    };
    let attempts = fields
        .next()
        .and_then(|v| v.parse::<u8>().ok())
        .unwrap_or(FAILED_ATTEMPTS)
        .min(FAILED_ATTEMPTS);
    let retry_at_ms = fields
        .next()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0)
        .min(calternal_types::now_ms() + 3200);
    let action = match fields.next() {
        Some("reader") => Action::Reader,
        Some("replace") if matches!(reason, "sidecar_schema" | "sidecar_corrupt") => {
            Action::Replace
        }
        Some("rebuilding") => Action::Rebuilding,
        _ => Action::Replay,
    };
    Ok(Some(Failure {
        attempts,
        retry_at_ms,
        action,
        reason,
    }))
}

fn remove(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(internal(error)),
    }
}

pub(super) fn replace_sidecar(source: &Path) -> Result<()> {
    let database = replay::path(source);
    // Drop the primary first so a crash cannot reopen old pages with a
    // missing WAL. Remove only the three derived files.
    remove(&database)?;
    for extension in ["db-wal", "db-shm"] {
        remove(&database.with_extension(extension))?;
    }
    Ok(())
}
