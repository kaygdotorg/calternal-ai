//! Private account completion marker, independent of protected sign-in authority.

use crate::store::Store;
use calternal_core::OnboardingState;
use calternal_types::{AccountId, Error};
use rusqlite::{Connection, OpenFlags};
use std::{io, time::Duration};

/// Initial account-local settings schema. The release inventory uses this descriptor.
pub(crate) const SCHEMA: calternal_store::migrations::Schema<'_> =
    calternal_store::migrations::Schema::new(
        "onboarding.db",
        1,
        &["CREATE TABLE finished(id INTEGER PRIMARY KEY CHECK(id=1)) STRICT;"],
        &[],
    );

/// Read or set completion for the admitted account on a blocking worker.
/// A missing store reads unfinished without creating files. Finishing performs
/// at most one durable marker insert after the version check. The owner fence
/// stops a request queued across erasure from recreating its database.
pub(crate) fn state(
    store: &Store,
    account: AccountId,
    finish: bool,
) -> Result<OnboardingState, Error> {
    let _step = store.content().lifecycle().step(account.as_uuid())?;
    let dir = store.appdata().join("accounts").join(account.to_string());
    let path = dir.join(SCHEMA.name());
    if finish {
        calternal_fsio::private::directory(&dir).map_err(storage_error)?;
        calternal_fsio::private::file(&path).map_err(storage_error)?;
    } else {
        match path.symlink_metadata() {
            Ok(metadata) if metadata.is_file() => (),
            Ok(_) => return Err(storage_error("onboarding store must be a regular file")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(OnboardingState { finished: false });
            }
            Err(error) => return Err(storage_error(error)),
        }
    }
    let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(storage_error)?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(storage_error)?;
    // If a concurrent finisher created the file and has yet to initialize it,
    // the shared runner serializes initialization before the state read.
    calternal_store::migrations::migrate(&conn, SCHEMA).map_err(storage_error)?;
    if finish {
        conn.pragma_update(None, "synchronous", "EXTRA")
            .map_err(storage_error)?;
        conn.execute("INSERT OR IGNORE INTO finished VALUES(1)", [])
            .map_err(storage_error)?;
        return Ok(OnboardingState { finished: true });
    }
    let finished = conn
        .query_row("SELECT EXISTS(SELECT 1 FROM finished)", [], |row| {
            row.get(0)
        })
        .map_err(storage_error)?;
    Ok(OnboardingState { finished })
}

fn storage_error(error: impl std::fmt::Display) -> Error {
    tracing::warn!(%error, "onboarding storage failed");
    Error::internal("onboarding state could not be accessed")
}
