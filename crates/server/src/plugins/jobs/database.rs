//! One space's jobs connection and its journal publication scheduling state.
use calternal_types::{OwnerLifecycle, OwnerScoped, SpaceId};
use rusqlite::{Connection, OpenFlags, config::DbConfig};
use std::{path::PathBuf, sync::{Mutex, MutexGuard}};
use tokio::sync::Notify;

/// Serializes SQLite work for one cached space. Publication scheduling holds
/// a separate short lock that is never held across SQLite or journal work.
pub(crate) struct JobDatabase {
    connection: Mutex<Connection>,
    path: PathBuf,
    lifecycle: OwnerLifecycle,
    space: SpaceId,
    webhook_messages: Mutex<(i64, u8)>,
    publishing: Mutex<Option<bool>>,
    publication_wake: Notify,
}

impl JobDatabase {
    /// Wrap an open connection. Cost: no I/O or allocation.
    pub(super) fn new(connection: Connection, path: PathBuf, lifecycle: OwnerLifecycle, space: SpaceId) -> Self {
        Self {
            connection: Mutex::new(connection),
            path,
            lifecycle,
            space,
            webhook_messages: Mutex::new((0, 0)),
            publishing: Mutex::new(None),
            publication_wake: Notify::new(),
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
            self.publication_wake.notify_one();
            false
        } else {
            *publishing = Some(false);
            true
        }
    }

    /// Consume a publication wake, releasing this worker's reservation when
    /// no writes arrived and every published row has retired.
    pub(crate) fn continue_publication(&self, finish: bool) -> bool {
        let mut publishing = self.publishing.lock().unwrap_or_else(|e| e.into_inner());
        let dirty = publishing.take() == Some(true);
        if dirty || !finish {
            *publishing = Some(false);
        }
        dirty
    }

    /// Wait for a foreground job commit without holding a connection or lock.
    pub(crate) async fn wait_for_publication(&self) {
        self.publication_wake.notified().await;
    }

    /// Retire at most one bounded batch of already-published events with a
    /// FULL commit. A separate writer avoids the foreground connection mutex.
    /// Busy or locked means retry later. A removed space needs no retry.
    pub(crate) fn retire_turn_events(&self, through: i64, limit: i64) -> rusqlite::Result<bool> {
        let Ok(_owner) = self.lifecycle.step(self.space.as_uuid()) else {
            return Ok(false);
        };
        let retired = (|| {
            let conn = Connection::open_with_flags(
                &self.path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
            )?;
            conn.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)?;
            conn.busy_timeout(std::time::Duration::ZERO)?;
            conn.pragma_update(None, "synchronous", "FULL")?;
            conn.pragma_update(None, "wal_autocheckpoint", 0)?;
            let rows = conn.execute(
                "DELETE FROM saved_turn_events WHERE seq IN (
                   SELECT seq FROM saved_turn_events WHERE seq<=?1 ORDER BY seq LIMIT ?2
                 )",
                [through, limit],
            )?;
            if (rows as i64) < limit {
                return Ok(false);
            }
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM saved_turn_events WHERE seq<=?1)",
                [through],
                |row| row.get(0),
            )
        })();
        match retired {
            Err(rusqlite::Error::SqliteFailure(code, _))
                if matches!(
                    code.code, rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                ) => Ok(true),
            result => result,
        }
    }
}
