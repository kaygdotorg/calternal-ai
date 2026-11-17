//! The `index.db` schema (one current definition) and its opening rules.

use crate::token::entry_check;
use crate::{Error, Result};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::Path;

/// The whole `index.db` schema at [`VERSION`]: the change journal and its
/// expiring request receipts (nothing outlives a receipt). No migration chain:
/// preview data is disposable, and a database at another version is refused.
const SCHEMA: &str = "CREATE TABLE meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    ) STRICT;
    CREATE TABLE journal (
        seq          INTEGER PRIMARY KEY AUTOINCREMENT,
        at_ms        INTEGER NOT NULL,
        kind         TEXT NOT NULL,
        item         TEXT NOT NULL,
        data         TEXT NOT NULL,
        installation TEXT,
        request_id   TEXT,
        author       TEXT NOT NULL
    ) STRICT;
    CREATE TABLE idempotency (
        key          TEXT PRIMARY KEY,
        action       TEXT NOT NULL,
        payload_hash TEXT NOT NULL,
        result       TEXT NOT NULL,
        seq          INTEGER NOT NULL,
        at_ms        INTEGER NOT NULL
    ) STRICT;
    CREATE INDEX idempotency_at ON idempotency (at_ms);";
/// The schema version [`SCHEMA`] creates.
const VERSION: i64 = 2;

/// Validates the explicit space binding before exposing an opened journal.
pub(crate) fn open_bound_writer(
    path: &Path,
    with_check: bool,
    space: crate::SpaceId,
) -> Result<OpenState> {
    let value = space.to_string();
    // Refuse a journal bound to another space before touching its schema or epoch.
    if path.exists() {
        let conn = open_reader(path)?;
        let has_meta: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='meta')",
                [],
                |r| r.get(0),
            )
            .map_err(Error::internal)?;
        if has_meta {
            let existing: Option<String> = conn
                .query_row("SELECT value FROM meta WHERE key='space_id'", [], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(Error::internal)?;
            if existing.as_ref().is_some_and(|id| id != &value) {
                return Err(Error::new(
                    crate::ErrorCode::InvalidInput,
                    "journal belongs to another space",
                ));
            }
        }
    }
    let opened = open_writer(path, with_check)?;
    opened
        .conn
        .execute(
            "INSERT INTO meta (key,value) VALUES ('space_id',?1) ON CONFLICT(key) DO NOTHING",
            [&value],
        )
        .map_err(Error::internal)?;
    let existing: String = opened
        .conn
        .query_row("SELECT value FROM meta WHERE key='space_id'", [], |r| {
            r.get(0)
        })
        .map_err(Error::internal)?;
    if existing != value {
        return Err(Error::new(
            crate::ErrorCode::InvalidInput,
            "journal belongs to another space",
        ));
    }
    Ok(opened)
}

/// Values needed to start one per-space store without reading its journal twice.
pub(crate) struct OpenState {
    /// Writer connection used by the store's single writer thread.
    pub conn: Connection,
    /// The journal epoch, created when the store is first opened.
    pub epoch: String,
    /// The highest sequence in the indexed journal.
    pub head: i64,
    /// The check for the head row when signed tokens are enabled.
    pub head_check: Option<u32>,
    /// The sequence at which the current epoch began.
    pub epoch_head: i64,
    /// Whether restore detection was initialized on an earlier open.
    pub highwater_initialized: bool,
}

/// Ordinary journal writes retain NORMAL sync; a [`crate::Commit::durable`]
/// commit switches to FULL for its one transaction.
fn tune(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )
    .map_err(Error::internal)
}

/// Opens the writer connection and reads its indexed head and restore metadata.
pub(crate) fn open_writer(path: &Path, with_check: bool) -> Result<OpenState> {
    let mut conn = Connection::open(path).map_err(Error::internal)?;
    tune(&conn)?;
    migrate(&mut conn)?;
    let (epoch, epoch_head, highwater_initialized) = ensure_epoch(&conn, with_check)?;
    let (head, head_check) = read_head(&conn, with_check)?;
    Ok(OpenState {
        conn,
        epoch,
        head,
        head_check,
        epoch_head,
        highwater_initialized,
    })
}

/// Reads the newest journal row through the sequence index, with its check when requested.
pub(crate) fn read_head(conn: &Connection, with_check: bool) -> Result<(i64, Option<u32>)> {
    if !with_check {
        let head = conn
            .query_row(
                "SELECT seq FROM journal ORDER BY seq DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(Error::internal)?
            .unwrap_or(0);
        return Ok((head, None));
    }

    let row = conn
        .query_row(
            "SELECT seq, at_ms, kind, item, request_id
             FROM journal ORDER BY seq DESC LIMIT 1",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()
        .map_err(Error::internal)?;
    match row {
        Some((seq, at_ms, kind, item, request_id)) => Ok((
            seq,
            Some(entry_check(seq, at_ms, &kind, &item, request_id.as_deref())),
        )),
        None => Ok((0, Some(0))),
    }
}

/// Opens a read-only connection for one query.
pub(crate) fn open_reader(path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(Error::internal)?;
    conn.execute_batch("PRAGMA busy_timeout = 5000;")
        .map_err(Error::internal)?;
    Ok(conn)
}

/// Creates [`SCHEMA`] in an empty database; refuses any other version.
fn migrate(conn: &mut Connection) -> Result<()> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(Error::internal)?;
    match version {
        0 => {
            let tx = conn.transaction().map_err(Error::internal)?;
            tx.execute_batch(SCHEMA).map_err(Error::internal)?;
            tx.pragma_update(None, "user_version", VERSION)
                .map_err(Error::internal)?;
            tx.commit().map_err(Error::internal)
        }
        VERSION => Ok(()),
        _ => Err(Error::new(
            crate::ErrorCode::InvalidInput,
            "index.db schema version is not this build's; use fresh disposable data",
        )),
    }
}

/// The journal's generation id and, for signed stores, its restore metadata.
fn ensure_epoch(conn: &Connection, read_restore_state: bool) -> Result<(String, i64, bool)> {
    let (existing, epoch_head, highwater): (Option<String>, Option<String>, Option<String>) =
        if read_restore_state {
            conn.query_row(
                "SELECT
                    (SELECT value FROM meta WHERE key = 'epoch'),
                    (SELECT value FROM meta WHERE key = 'epoch_head'),
                    (SELECT value FROM meta WHERE key = 'highwater')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(Error::internal)?
        } else {
            let existing = conn
                .query_row("SELECT value FROM meta WHERE key = 'epoch'", [], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(Error::internal)?;
            (existing, None, None)
        };
    let epoch_head = epoch_head.and_then(|value| value.parse().ok()).unwrap_or(0);
    let highwater_initialized = highwater.as_deref() == Some("1");
    if let Some(epoch) = existing {
        return Ok((epoch, epoch_head, highwater_initialized));
    }
    let epoch = uuid::Uuid::new_v4().simple().to_string()[..12].to_owned();
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('epoch', ?1)",
        [&epoch],
    )
    .map_err(Error::internal)?;
    Ok((epoch, epoch_head, highwater_initialized))
}

#[cfg(test)]
mod tests {
    //! Checks the indexed journal-head read used while opening a signed store.

    use super::*;
    use crate::token::entry_check;

    /// A signed open gets the last row and its check from the same head query.
    #[test]
    fn signed_head_read_returns_the_last_row_check() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE journal (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                at_ms INTEGER NOT NULL,
                kind TEXT NOT NULL,
                item TEXT NOT NULL,
                data TEXT NOT NULL,
                request_id TEXT
            );
            INSERT INTO journal (seq, at_ms, kind, item, data, request_id)
                VALUES (1, 10, 'notes.edited', 'first', '{}', 'r1');
            INSERT INTO journal (seq, at_ms, kind, item, data, request_id)
                VALUES (2, 21, 'notes.edited', 'last', '{}', 'r2');",
        )
        .unwrap();

        assert_eq!(
            read_head(&conn, true).unwrap(),
            (
                2,
                Some(entry_check(2, 21, "notes.edited", "last", Some("r2")))
            )
        );
    }

    /// A signed open returns its epoch and restore metadata from one metadata query.
    #[test]
    fn secured_open_state_includes_restore_metadata() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO meta (key, value) VALUES
                ('epoch', 'e1'), ('epoch_head', '37'), ('highwater', '1');",
        )
        .unwrap();

        assert_eq!(
            ensure_epoch(&conn, true).unwrap(),
            ("e1".to_owned(), 37, true)
        );
    }
}
