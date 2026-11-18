//! Hourly bounded Notes retention over physical space directories.
use crate::store::Store;
use calternal_types::{Error, SpaceId};
use rusqlite::{Connection, OpenFlags};
use std::{fs, time::Duration};
use time::OffsetDateTime;

const DEFAULT_DAYS: u32 = 30;
const EVERY: Duration = Duration::from_secs(60 * 60);

impl Store {
    /// Start Notes Trash expiration in the server runtime. Reads
    /// CALTERNAL_NOTES_TRASH_RETENTION_DAYS once (30 by default) and rejects
    /// invalid or zero values. The first sweep runs at startup and all
    /// filesystem and SQLite work is on the blocking pool.
    ///
    /// The caller keeps the task handle for the server lifetime. Cost: one
    /// task and config read, then an hourly walk of space directories and
    /// bounded SQL pages with the normal purge costs for expired items. Closed
    /// spaces get one short read-only eligibility probe, not a journal/cache open;
    /// only spaces containing expired Trash activate the shared content lifecycle.
    pub fn start_notes_retention(&self) -> Result<tokio::task::JoinHandle<()>, Error> {
        let days = match std::env::var("CALTERNAL_NOTES_TRASH_RETENTION_DAYS") {
            Ok(value) => value
                .parse::<u32>()
                .ok()
                .filter(|days| *days > 0)
                .ok_or_else(|| {
                    Error::invalid_input(
                        "CALTERNAL_NOTES_TRASH_RETENTION_DAYS must be a positive number of days",
                    )
                })?,
            Err(std::env::VarError::NotPresent) => DEFAULT_DAYS,
            Err(_) => {
                return Err(Error::invalid_input(
                    "CALTERNAL_NOTES_TRASH_RETENTION_DAYS is not valid text",
                ));
            }
        };
        let spaces = self.appdata().join("spaces");
        let content = self.shared_content();
        Ok(tokio::spawn(async move {
            loop {
                let spaces = spaces.clone();
                let content = content.clone();
                let result = crate::blocking::spawn(move || {
                    let cutoff = calternal_content::format_rfc3339(
                        OffsetDateTime::now_utc()
                            .checked_sub(time::Duration::days(i64::from(days)))
                            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
                    );
                    let dirs = match fs::read_dir(&spaces) {
                        Ok(dirs) => dirs,
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                        Err(e) => return Err(e),
                    };
                    for entry in dirs {
                        let entry = entry?;
                        if !entry.file_type()?.is_dir() || !entry.path().join("content.db").is_file() {
                            continue;
                        }
                        let id = entry.file_name();
                        let Some(space) = id.to_str().and_then(|id| SpaceId::parse(id).ok()) else {
                            continue;
                        };
                        let lifecycle = content.lifecycle();
                        let Ok(_probe) = lifecycle.step(space.as_uuid()) else { continue };
                        match has_expired(&entry.path().join("content.db"), &cutoff) {
                            Ok(false) => continue,
                            Ok(true) => {},
                            Err(error) => {
                                tracing::warn!(%space, %error, "Notes Trash eligibility probe failed");
                                continue;
                            }
                        }
                        if let Err(e) = content.expire_trash(space, "notes", &cutoff, super::TRASH_EXPIRY_EVENT) {
                            tracing::warn!(%space, code=?e.code, "Notes Trash sweep could not finish");
                        }
                    }
                    Ok(())
                }).await;
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => tracing::warn!(error=%e, "could not discover Notes Trash spaces"),
                    Err(e) => tracing::warn!(error=%e, "Notes Trash worker failed"),
                }
                tokio::time::sleep(EVERY).await;
            }
        }))
    }
}

/// Probe one closed space without creating its database, updating its schema or
/// opening a content journal. SQLite may map its existing WAL sidecars. One read-only connection/statement at a time; the mutation rechecks
/// eligibility under the existing space lock, so concurrent restores stay safe.
fn has_expired(path: &std::path::Path, cutoff: &str) -> rusqlite::Result<bool> {
    if !fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file()) {
        return Err(rusqlite::Error::InvalidPath(path.to_owned()));
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM items WHERE kind='notes' AND updated_at < ?1
         AND substr(path,1,7)='.trash/' AND mat_state NOT IN ('purged','blocked'))",
        [cutoff],
        |row| row.get(0),
    )
}
