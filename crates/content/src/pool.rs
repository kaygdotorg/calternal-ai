//! A few reusable read connections to one SQLite file, so point reads never queue
//! behind the file's single writer connection or behind each other.
use rusqlite::{Connection, OpenFlags};
use std::{path::PathBuf, sync::Mutex};

/// Idle connections kept for reuse; more may be open while in use.
const KEEP: usize = 8;

/// Read-only connections to one WAL database. The internal mutex is held only to
/// take or return a connection, never across a query.
pub struct ReadPool {
    path: PathBuf,
    idle: Mutex<Vec<Connection>>,
}

impl ReadPool {
    /// A pool for `path`; opens nothing until first use. Cost: one allocation.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            idle: Mutex::new(Vec::new()),
        }
    }

    /// Runs `read` on an idle connection, opening one (read-only, 5 s busy timeout)
    /// if none is idle. Cost: a vector pop and push under the mutex; a cold call adds
    /// one SQLite open. At most [`KEEP`] idle connections are retained.
    ///
    /// # Errors
    /// The open or the read's own SQLite error.
    pub fn read<T>(
        &self,
        read: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        let taken = self.idle.lock().unwrap_or_else(|e| e.into_inner()).pop();
        let conn = match taken {
            Some(conn) => conn,
            None => {
                let conn = Connection::open_with_flags(
                    &self.path,
                    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                )?;
                conn.busy_timeout(std::time::Duration::from_secs(5))?;
                conn
            }
        };
        let result = read(&conn);
        let mut idle = self.idle.lock().unwrap_or_else(|e| e.into_inner());
        if idle.len() < KEEP {
            idle.push(conn);
        }
        result
    }
}
