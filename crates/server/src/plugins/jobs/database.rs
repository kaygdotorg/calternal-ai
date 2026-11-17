//! One space's jobs connection and its journal publication scheduling state.
use rusqlite::Connection;
use std::sync::{LockResult, Mutex, MutexGuard};

/// Serializes SQLite work for one cached space. Publication scheduling holds
/// a separate short lock that is never held across SQLite or journal work.
pub(crate) struct JobDatabase {
    connection: Mutex<Connection>,
    publishing: Mutex<Option<bool>>,
}

impl JobDatabase {
    /// Wrap an open connection. Cost: no I/O or allocation.
    pub(super) fn new(connection: Connection) -> Self {
        Self {
            connection: Mutex::new(connection),
            publishing: Mutex::new(None),
        }
    }

    /// Lock this space's connection for a short blocking query or transaction.
    pub(crate) fn lock(&self) -> LockResult<MutexGuard<'_, Connection>> {
        self.connection.lock()
    }

    /// Reserve a drain task, or signal the existing one to check for new work
    /// before exiting. Cost: one uncontended per-space lock in the usual case.
    pub(crate) fn wake_publication(&self) -> bool {
        let mut publishing = self.publishing.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(dirty) = publishing.as_mut() {
            *dirty = true;
            false
        } else {
            *publishing = Some(false);
            true
        }
    }

    /// Finish a drain pass and report whether another is needed. Cost: one
    /// short per-space lock. An idle connection holds no task.
    pub(crate) fn continue_publication(&self) -> bool {
        let mut publishing = self.publishing.lock().unwrap_or_else(|e| e.into_inner());
        let dirty = publishing.take() == Some(true);
        if dirty {
            *publishing = Some(false);
        }
        dirty
    }
}
