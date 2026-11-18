//! One space's jobs connection and its journal publication scheduling state.
use calternal_types::{OwnerLifecycle, OwnerScoped, SpaceId};
use rusqlite::Connection;
use std::sync::{Mutex, MutexGuard};

/// Serializes SQLite work for one cached space. Publication scheduling holds
/// a separate short lock that is never held across SQLite or journal work.
pub(crate) struct JobDatabase {
    connection: Mutex<Connection>,
    lifecycle: OwnerLifecycle,
    space: SpaceId,
    webhook_messages: Mutex<(i64, u8)>,
    publishing: Mutex<Option<bool>>,
}

impl JobDatabase {
    /// Wrap an open connection. Cost: no I/O or allocation.
    pub(super) fn new(connection: Connection, lifecycle: OwnerLifecycle, space: SpaceId) -> Self {
        Self {
            connection: Mutex::new(connection),
            lifecycle,
            space,
            webhook_messages: Mutex::new((0, 0)),
            publishing: Mutex::new(None),
        }
    }

    /// Limit webhook verification to 60 attempts per minute for this cached
    /// space. A window expires without work; one short lock and no SQLite.
    pub(crate) fn admit_webhook(&self) -> bool {
        let minute = crate::plugins::job_sql::now_ms() / 60_000;
        let mut window = self
            .webhook_messages
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if window.0 != minute {
            *window = (minute, 0);
        }
        if window.1 >= 60 {
            return false;
        }
        window.1 += 1;
        true
    }

    /// Lock this space's connection for a short blocking query or transaction.
    pub(crate) fn lock(&self) -> rusqlite::Result<OwnerScoped<MutexGuard<'_, Connection>>> {
        let step = self
            .lifecycle
            .step(self.space.as_uuid())
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        Ok(step.hold(self.connection.lock().unwrap_or_else(|p| p.into_inner())))
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
