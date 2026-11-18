//! Bounded Notes Trash maintenance over physical space directories.
use crate::store::Store;
use calternal_types::{Error, SpaceId};
use rusqlite::{Connection, OpenFlags};
use std::{fs, time::Duration};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const DEFAULT_DAYS: u32 = 30;
const EVERY: Duration = Duration::from_secs(60 * 60);

impl Store {
    /// Start Notes Trash expiration in the server runtime. Reads
    /// CALTERNAL_NOTES_TRASH_RETENTION_DAYS once (30 by default) and rejects
    /// invalid or zero values. The first sweep runs at startup and all
    /// filesystem and SQLite work is on the blocking pool.
    ///
    /// The caller keeps the task handle for the server lifetime. Cost: one
    /// task and config read, then discovery on startup, hourly idle ticks and work notifications.
    /// Bounded SQL pages advance jobs and apply with the normal purge costs for expired items. Closed
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
        let _ = self.notes_trash_days.set(days);
        let spaces = self.appdata().join("spaces");
        let content = self.shared_content();
        let wake = self.notes_maintenance_wake.clone();
        Ok(tokio::spawn(async move {
            loop {
                let spaces = spaces.clone();
                let content = content.clone();
                let result = crate::blocking::spawn(move || sweep(&spaces, &content, days)).await;
                let delay = match result {
                    Ok(Ok(true)) => Duration::from_millis(20),
                    Ok(Ok(false)) => EVERY,
                    Ok(Err(e)) => {
                        tracing::warn!(error=%e, "could not discover Notes Trash spaces");
                        Duration::from_secs(5)
                    }
                    Err(e) => {
                        tracing::warn!(error=%e, "Notes Trash worker failed");
                        Duration::from_secs(5)
                    }
                };
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = wake.notified() => {}
                }
            }
        }))
    }

    pub(crate) fn trash_days_remaining(&self, at: &str, now: OffsetDateTime) -> Result<u32, Error> {
        let days = *self.notes_trash_days.get().unwrap_or(&DEFAULT_DAYS);
        let deleted = OffsetDateTime::parse(at, &Rfc3339)
            .map_err(|_| Error::internal("invalid stored Notes deletion timestamp"))?;
        let expires = deleted.saturating_add(time::Duration::days(i64::from(days)));
        let nanos = (expires - now).whole_nanoseconds().max(0) as u128;
        let day = time::Duration::DAY.whole_nanoseconds() as u128;
        Ok(u32::try_from(nanos.div_ceil(day)).unwrap_or(days).min(days))
    }
}

fn sweep(
    spaces: &std::path::Path,
    content: &calternal_content::Content,
    days: u32,
) -> std::io::Result<bool> {
    let cutoff = calternal_content::format_rfc3339(
        OffsetDateTime::now_utc().saturating_sub(time::Duration::days(i64::from(days))),
    );
    let dirs = match fs::read_dir(spaces) {
        Ok(dirs) => dirs,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    let mut more = false;
    for entry in dirs {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let id = entry.file_name();
        let Some(space) = id.to_str().and_then(|id| SpaceId::parse(id).ok()) else {
            continue;
        };
        let db = entry.path().join("content.db");
        if !db.is_file() {
            continue;
        }
        let lifecycle = content.lifecycle();
        let Ok(_probe) = lifecycle.step(space.as_uuid()) else {
            continue;
        };
        let (job, expired, rename) = match eligible(&db, &cutoff) {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(%space, %error, "Notes maintenance eligibility probe failed");
                continue;
            }
        };
        if rename {
            match content.rename_job_batch(space, None, super::retarget) {
                Ok(backlog) => more |= backlog,
                Err(error) => tracing::warn!(%space, code=?error.code, "Notes rename turn failed"),
            }
        }
        // Explicit jobs get the first turn so expiry does not claim their
        // oldest selected revisions while they are making progress.
        let result = if job {
            content.empty_trash_batch(space)
        } else if expired {
            content.expire_trash(space, "notes", &cutoff, super::TRASH_EXPIRY_EVENT)
        } else {
            continue;
        };
        match result {
            Ok(backlog) => more |= backlog || (job && expired),
            Err(error) => tracing::warn!(%space, code=?error.code, "Notes maintenance turn failed"),
        }
    }
    Ok(more)
}

// An older content database may still await its lazy additive migration.
// The Trash expiry query works against the pinned baseline as well.
fn eligible(path: &std::path::Path, cutoff: &str) -> rusqlite::Result<(bool, bool, bool)> {
    if !fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file()) {
        return Err(rusqlite::Error::InvalidPath(path.to_owned()));
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let (jobs_table, rename_table, expired): (bool, bool, bool) = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='notes_trash_jobs'),
                EXISTS(SELECT 1 FROM sqlite_schema WHERE name='notes_rename_jobs'),
                EXISTS(SELECT 1 FROM items WHERE kind='notes' AND updated_at < ?1
                AND substr(path,1,7)='.trash/' AND mat_state NOT IN ('purged','blocked'))",
        [cutoff],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let job = jobs_table
        && conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM notes_trash_jobs WHERE finished IS NULL)",
            [],
            |row| row.get(0),
        )?;
    let rename = rename_table
        && conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM notes_rename_jobs WHERE finished IS NULL)",
            [],
            |row| row.get(0),
        )?;
    Ok((job, expired, rename))
}
