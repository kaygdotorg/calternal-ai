//! Hourly bounded Notes retention over physical space directories.
use crate::store::Store;
use calternal_types::{Error, SpaceId};
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
    /// bounded SQL pages with the normal purge costs for expired items.
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
                let result = tokio::task::spawn_blocking(move || {
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
