//! Per-slot projection handles. Recovery drains the attached readers before
//! replacing the derived database and never holds the cache mutex during I/O.
use super::replay;
use crate::{Result, journal::lock, types::internal};
use calternal_store::{ProjectionJournal, SyncToken};
use rusqlite::Connection;
use std::{
    io::Read,
    path::Path,
    sync::{
        Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

const KEEP: usize = 2;

#[derive(Default)]
pub(crate) struct Runtime {
    pub(super) gate: RwLock<()>,
    state: Mutex<State>,
    revision: AtomicU64,
}

#[derive(Default)]
struct State {
    dormant: bool,
    writer: Option<Writer>,
    readers: Vec<Reader>,
    failure: Option<Failure>,
    checked_failure: bool,
}

pub(super) struct Writer {
    pub(super) conn: Connection,
    pub(super) journal: ProjectionJournal,
    pub(super) published: Option<SyncToken>,
    pub(super) revision: u64,
}

pub(super) struct Reader {
    pub(super) conn: Connection,
    pub(super) journal: ProjectionJournal,
}

#[derive(Clone, Copy)]
pub(super) struct Failure {
    pub(super) attempts: u8,
    pub(super) retry_at_ms: i64,
    pub(super) replace: bool,
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
        state.failure.is_none()
            && (state.dormant
                || state
                    .writer
                    .as_ref()
                    .filter(|w| w.revision == self.revision())
                    .and_then(|w| w.published.as_ref())
                    .is_some_and(|published| Some(published) == head))
    }

    pub(super) fn dormant(&self) {
        lock(&self.state).dormant = true;
    }

    pub(super) fn take_writer(&self) -> Option<Writer> {
        lock(&self.state).writer.take()
    }

    pub(super) fn put_writer(&self, writer: Writer) {
        let mut state = lock(&self.state);
        state.dormant = false;
        state.writer = Some(writer);
    }

    pub(super) fn take_reader(&self) -> Option<Reader> {
        lock(&self.state).readers.pop()
    }

    pub(super) fn put_reader(&self, reader: Reader) {
        let mut state = lock(&self.state);
        if state.failure.is_none() && state.readers.len() < KEEP {
            state.readers.push(reader);
        }
    }

    pub(super) fn drain(&self) {
        let (writer, readers) = {
            let mut state = lock(&self.state);
            (state.writer.take(), std::mem::take(&mut state.readers))
        };
        drop((writer, readers));
    }

    pub(super) fn failure(&self) -> Option<Failure> {
        lock(&self.state).failure
    }

    pub(super) fn initialize_failure(&self, source: &Path) -> Result<()> {
        {
            let state = lock(&self.state);
            if state.checked_failure {
                return Ok(());
            }
        }
        let saved = match std::fs::File::open(failure_path(source)) {
            Ok(file) => {
                let mut text = String::new();
                file.take(256).read_to_string(&mut text).map_err(internal)?;
                let mut fields = text.split_whitespace();
                let latest_retry = calternal_types::now_ms() + 3200;
                Some(Failure {
                    attempts: fields.nth(1).and_then(|v| v.parse().ok()).unwrap_or(1),
                    retry_at_ms: fields
                        .next()
                        .and_then(|v| v.parse::<i64>().ok())
                        .unwrap_or(0)
                        .min(latest_retry),
                    replace: fields.next() == Some("replace"),
                })
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(internal(error)),
        };
        let mut state = lock(&self.state);
        state.checked_failure = true;
        if state.failure.is_none() {
            state.failure = saved;
        }
        Ok(())
    }

    pub(super) fn report_reader_error(&self) {
        let mut state = lock(&self.state);
        if state.failure.is_none() {
            state.failure = Some(Failure {
                attempts: 0,
                retry_at_ms: 0,
                replace: true,
            });
        }
    }

    pub(super) fn record_failure(
        &self,
        source: &Path,
        reason: &'static str,
        replace: bool,
    ) -> Result<()> {
        let failure = {
            let mut state = lock(&self.state);
            let attempts = state
                .failure
                .map_or(1, |old| old.attempts.saturating_add(1));
            let retry_at_ms = calternal_types::now_ms() + (100 << attempts.min(5));
            let failure = Failure {
                attempts,
                retry_at_ms,
                replace,
            };
            state.failure = Some(failure);
            failure
        };
        persist_failure(source, reason, failure)
    }

    pub(super) fn replaced(&self, source: &Path) -> Result<()> {
        let failure = {
            let mut state = lock(&self.state);
            let failure = state
                .failure
                .as_mut()
                .expect("replacement requires a failure");
            failure.replace = false;
            *failure
        };
        persist_failure(source, "rebuilding", failure)
    }

    pub(super) fn recovered(&self, source: &Path) -> Result<()> {
        if self.failure().is_some() {
            remove(&failure_path(source))?;
            lock(&self.state).failure = None;
        }
        Ok(())
    }
}

fn failure_path(source: &Path) -> std::path::PathBuf {
    source.with_file_name("windows-v1.failure")
}

fn persist_failure(source: &Path, reason: &'static str, failure: Failure) -> Result<()> {
    let action = if failure.replace { "replace" } else { "replay" };
    let body = format!(
        "{reason} {} {} {action}\n",
        failure.attempts, failure.retry_at_ms
    );
    calternal_fsio::write_atomic_mode(&failure_path(source), body.as_bytes(), 0o600)
        .map_err(internal)
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
