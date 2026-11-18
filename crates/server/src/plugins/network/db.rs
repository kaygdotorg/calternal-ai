//! Durable per-space network preferences. The shared runner owns all DDL.

use calternal_core::ContentFetchPreference;
use calternal_store::migrations::{Schema, migrate};
use calternal_types::{AccountId, Error, Owner, OwnerLifecycle, SpaceId};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::{io::ErrorKind, path::Path, time::Duration};

const MAX_PREFERENCES: u32 = 10_000;
const INITIAL_SQL: &str = "
    CREATE TABLE content_preference(
        account TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL CHECK (value IN ('ask', 'always', 'never'))
    ) STRICT, WITHOUT ROWID;
    CREATE TABLE activity(
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        started INTEGER NOT NULL,
        actor TEXT NOT NULL,
        package TEXT NOT NULL,
        package_revision INTEGER NOT NULL,
        approval INTEGER NOT NULL,
        class TEXT NOT NULL,
        origin TEXT NOT NULL,
        host TEXT NOT NULL,
        port INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
        protocol TEXT NOT NULL,
        outcome TEXT NOT NULL,
        elapsed_ms INTEGER NOT NULL DEFAULT 0,
        sent_bytes INTEGER NOT NULL DEFAULT 0,
        received_bytes INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE INDEX activity_time ON activity(started, id);
    CREATE INDEX activity_actor ON activity(actor);
";

/// The migration descriptor used by both the network writer and upgrades.
pub(crate) const SCHEMA: Schema<'static> = Schema::new("plugin network", 1, &[INITIAL_SQL], &[]);

pub(super) fn open(path: &Path, create: bool) -> Result<Option<Connection>, Error> {
    let dir = path
        .parent()
        .ok_or_else(|| Error::internal("network storage directory is missing"))?;
    match path.symlink_metadata() {
        Ok(metadata) if metadata.is_file() => (),
        Ok(_) => return Err(Error::internal("network storage is not a regular file")),
        Err(error) if error.kind() == ErrorKind::NotFound && create => {
            calternal_fsio::private::directory(dir).map_err(storage)?;
            calternal_fsio::private::file(path).map_err(storage)?;
        }
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(storage(error)),
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW;
    let connection = Connection::open_with_flags(path, flags).map_err(storage)?;
    connection
        .busy_timeout(Duration::from_secs(3))
        .map_err(storage)?;
    migrate(&connection, SCHEMA).map_err(storage)?;
    Ok(Some(connection))
}

pub(super) fn storage(error: impl std::fmt::Display) -> Error {
    tracing::warn!(%error, "plugin network storage failed");
    Error::unavailable("Plugin network preferences are unavailable.")
}

/// Read the viewer's preference. An absent store or row defaults to Always
/// without provisioning storage. Call on a blocking thread.
pub(crate) fn preference(
    appdata: &Path,
    space: SpaceId,
    account: AccountId,
) -> Result<ContentFetchPreference, Error> {
    let path = appdata
        .join(Owner::Space(space).directory())
        .join(space.to_string())
        .join("network.db");
    let Some(db) = open(&path, false)? else {
        return Ok(ContentFetchPreference::Always);
    };
    let value = db
        .query_row(
            "SELECT value FROM content_preference WHERE account = ?1",
            [account.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage)?;
    match value.as_deref() {
        None | Some("always") => Ok(ContentFetchPreference::Always),
        Some("ask") => Ok(ContentFetchPreference::Ask),
        Some("never") => Ok(ContentFetchPreference::Never),
        Some(_) => Err(Error::internal("stored network preference is invalid")),
    }
}

/// Set one viewer's policy without keeping rows for defaults. One immediate
/// SQLite transaction bounds quota and publication together. Call off the async executor.
pub(crate) fn set_preference(
    appdata: &Path,
    space: SpaceId,
    account: AccountId,
    value: ContentFetchPreference,
) -> Result<(), Error> {
    let path = appdata
        .join(Owner::Space(space).directory())
        .join(space.to_string())
        .join("network.db");
    let Some(mut db) = open(&path, value != ContentFetchPreference::Always)? else {
        return Ok(());
    };
    let account = account.to_string();
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(storage)?;
    match value {
        ContentFetchPreference::Always => {
            tx.execute(
                "DELETE FROM content_preference WHERE account = ?1",
                [account],
            )
            .map_err(storage)?;
        }
        ContentFetchPreference::Ask | ContentFetchPreference::Never => {
            let text = if value == ContentFetchPreference::Ask {
                "ask"
            } else {
                "never"
            };
            let changed = tx
                .execute(
                    "INSERT INTO content_preference(account, value)
                     SELECT ?1, ?2 WHERE EXISTS(SELECT 1 FROM content_preference WHERE account = ?1)
                                      OR (SELECT count(*) FROM content_preference) < ?3
                     ON CONFLICT(account) DO UPDATE SET value = excluded.value",
                    params![account, text, MAX_PREFERENCES],
                )
                .map_err(storage)?;
            if changed == 0 {
                return Err(Error::unavailable(
                    "Space network preference limit reached.",
                ));
            }
        }
    }
    tx.commit().map_err(storage)
}

/// Remove a closed account's preference rows before acknowledging erasure.
/// Scans only existing UUID space roots and opens only existing network stores.
pub(crate) fn erase_account(
    appdata: &Path,
    lifecycle: &OwnerLifecycle,
    account: AccountId,
) -> Result<(), Error> {
    let spaces = match std::fs::read_dir(appdata.join("spaces")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(storage(error)),
    };
    for entry in spaces {
        let entry = entry.map_err(storage)?;
        let name = entry.file_name();
        let Some(space) = name.to_str().and_then(|name| SpaceId::parse(name).ok()) else {
            continue;
        };
        let Ok(_space) = lifecycle.step(space.as_uuid()) else {
            continue;
        };
        if !entry.file_type().map_err(storage)?.is_dir() {
            return Err(Error::internal("network space storage is not a directory"));
        }
        if let Some(db) = open(&entry.path().join("network.db"), false)? {
            db.execute_batch("PRAGMA secure_delete=ON;")
                .map_err(storage)?;
            let tx =
                rusqlite::Transaction::new_unchecked(&db, rusqlite::TransactionBehavior::Immediate)
                    .map_err(storage)?;
            let account = account.to_string();
            tx.execute(
                "DELETE FROM content_preference WHERE account = ?1",
                [&account],
            )
            .map_err(storage)?;
            tx.execute("DELETE FROM activity WHERE actor = ?1", [&account])
                .map_err(storage)?;
            tx.commit().map_err(storage)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_space_gets_the_default_and_preferences_are_per_person() {
        let dir = tempfile::tempdir().unwrap();
        let space = SpaceId::parse("01932910-0000-7000-8000-000000000002").unwrap();
        let first = AccountId::parse("01932910-0000-7000-8000-000000000001").unwrap();
        let second = AccountId::parse("01932910-0000-7000-8000-000000000003").unwrap();
        assert_eq!(
            preference(dir.path(), space, first).unwrap(),
            ContentFetchPreference::Always
        );
        assert!(!dir.path().join("spaces").exists());
        set_preference(dir.path(), space, first, ContentFetchPreference::Never).unwrap();
        assert_eq!(
            preference(dir.path(), space, first).unwrap(),
            ContentFetchPreference::Never
        );
        assert_eq!(
            preference(dir.path(), space, second).unwrap(),
            ContentFetchPreference::Always
        );
        set_preference(dir.path(), space, first, ContentFetchPreference::Always).unwrap();
        assert_eq!(
            preference(dir.path(), space, first).unwrap(),
            ContentFetchPreference::Always
        );
    }

    #[test]
    fn account_erasure_removes_only_that_accounts_preference() {
        let dir = tempfile::tempdir().unwrap();
        let space = SpaceId::parse("01932910-0000-7000-8000-000000000002").unwrap();
        let a = AccountId::parse("01932910-0000-7000-8000-000000000001").unwrap();
        let b = AccountId::parse("01932910-0000-7000-8000-000000000003").unwrap();
        set_preference(dir.path(), space, a, ContentFetchPreference::Never).unwrap();
        set_preference(dir.path(), space, b, ContentFetchPreference::Ask).unwrap();
        let lifecycle = OwnerLifecycle::default();
        lifecycle.close(a.as_uuid());
        erase_account(dir.path(), &lifecycle, a).unwrap();
        assert_eq!(
            preference(dir.path(), space, a).unwrap(),
            ContentFetchPreference::Always
        );
        assert_eq!(
            preference(dir.path(), space, b).unwrap(),
            ContentFetchPreference::Ask
        );
    }
}
