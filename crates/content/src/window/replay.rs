//! Bounded replay on the existing per-space projection worker.
use super::{
    replay_rows::{collection_page, is_collection, item},
    runtime::{self, Action, Writer},
};
use crate::{Content, Error, ErrorCode, Result, SpaceId, types::internal};
use calternal_store::{ProjectionJournal, SyncToken};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) const PAGE: usize = 128;
const SQL: &str = include_str!("sidecar.sql");
const SCHEMA: calternal_store::migrations::Schema<'_> =
    calternal_store::migrations::Schema::new("window sidecar", 2, &[SQL], &[]);

pub(super) fn path(source: &Path) -> PathBuf {
    source.with_file_name("windows-v1.db")
}

fn attach(conn: &Connection, source: &Path) -> Result<()> {
    let mut uri = String::from("file:");
    for byte in source.as_os_str().as_encoded_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'-' | b'_') {
            uri.push(char::from(*byte));
        } else {
            write!(uri, "%{byte:02X}").expect("write to String");
        }
    }
    uri.push_str("?mode=ro");
    conn.execute("ATTACH DATABASE ?1 AS source", [uri])
        .map_err(sql_error)?;
    Ok(())
}

// A derived file keeps a stable path across schema versions. v1 is obsolete
// and must rebuild from the journal rather than be read as the current layout.
fn check_version(conn: &Connection, allow_new: bool) -> Result<()> {
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(sidecar_error)?;
    if version == SCHEMA.version() || (version == 0 && allow_new) {
        return Ok(());
    }
    Err(classified("sidecar_schema"))
}

pub(super) fn open_sidecar_reader(sidecar: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        sidecar,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(sidecar_error)?;
    conn.busy_timeout(Duration::from_millis(50))
        .map_err(sidecar_error)?;
    check_version(&conn, false)?;
    Ok(conn)
}

pub(super) fn reader(source: &Path) -> Result<Connection> {
    let conn = open_sidecar_reader(&path(source))?;
    attach(&conn, source)?;
    Ok(conn)
}

#[derive(PartialEq, Eq)]
pub(super) struct Publication {
    pub(super) head: SyncToken,
    config: i64,
    scan_after: Option<String>,
    pub(super) ready: bool,
    member_after: Option<String>,
}

impl Publication {
    pub(super) fn read(conn: &Connection) -> Result<Option<Self>> {
        conn.prepare_cached(
            "SELECT epoch,sequence,config,scan_after,ready,member_after FROM main.window_publication",
        )
        .map_err(sidecar_error)?
        .query_row([], |row| {
            Ok(Self {
                head: SyncToken {
                    epoch: row.get(0)?,
                    seq: row.get(1)?,
                },
                config: row.get(2)?,
                scan_after: row.get(3)?,
                ready: row.get(4)?,
                member_after: row.get(5)?,
            })
        })
        .optional()
        .map_err(sidecar_error)
    }

    fn persist(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "INSERT INTO main.window_publication VALUES(1,?1,?2,?3,?4,?5,?6)
             ON CONFLICT(id) DO UPDATE SET epoch=excluded.epoch,sequence=excluded.sequence,
                 config=excluded.config,scan_after=excluded.scan_after,ready=excluded.ready,member_after=excluded.member_after",
            params![
                self.head.epoch,
                self.head.seq,
                self.config,
                self.scan_after,
                self.ready,
                self.member_after
            ],
        )
        .map_err(sidecar_error)?;
        Ok(())
    }

    pub(super) fn fresh_for(
        &self,
        conn: &Connection,
        journal: &ProjectionJournal,
        notes: bool,
    ) -> Result<bool> {
        if !self.ready {
            return Ok(false);
        }
        let config = if notes {
            conn.prepare_cached(
                "WITH pending AS MATERIALIZED (SELECT item FROM source.outbox LIMIT ?1)
                 SELECT revision, (SELECT count(*) FROM pending) = ?1
                   OR EXISTS(SELECT 1 FROM pending p LEFT JOIN source.items i ON i.id=p.item
                             WHERE i.kind IS NULL OR i.kind IN ('notes','collections'))
                 FROM source.window_configuration",
            )
            .map_err(sql_error)?
            .query_row([PAGE as i64 + 1], |row| {
                if row.get::<_, bool>(1)? {
                    Ok(None)
                } else {
                    row.get(0).map(Some)
                }
            })
            .map_err(sql_error)?
        } else {
            source_config(conn)?
        };
        if config != Some(self.config) {
            return Ok(false);
        }
        let head = journal.head().map_err(internal)?;
        if head == self.head {
            return Ok(true);
        }
        let lag = head.seq.saturating_sub(self.head.seq);
        if !notes || head.epoch != self.head.epoch || !(1..=PAGE as i64).contains(&lag) {
            return Ok(false);
        }
        let mut kind = conn
            .prepare_cached("SELECT kind FROM source.items WHERE id=?1")
            .map_err(sql_error)?;
        let mut sequenced = self.head.seq;
        for entry in journal.since(sequenced, lag as usize).map_err(internal)? {
            let Some(item) = entry.item else {
                return Ok(false);
            };
            if entry.sequence != sequenced + 1 || entry.sequence > head.seq {
                return Ok(false);
            }
            let Some(found) = kind
                .query_row([item], |row| row.get::<_, String>(0))
                .optional()
                .map_err(sql_error)?
            else {
                return Ok(false);
            };
            if matches!(found.as_str(), "notes" | "collections") {
                return Ok(false);
            }
            sequenced = entry.sequence;
        }
        Ok(sequenced == head.seq && journal.head().map_err(internal)?.epoch == head.epoch)
    }
}

fn source_config(conn: &Connection) -> Result<Option<i64>> {
    conn.prepare_cached(
        "SELECT revision, EXISTS(SELECT 1 FROM source.outbox)
         FROM source.window_configuration",
    )
    .map_err(sql_error)?
    .query_row([], |row| {
        let pending: bool = row.get(1)?;
        if pending {
            Ok(None)
        } else {
            row.get(0).map(Some)
        }
    })
    .map_err(sql_error)
}

impl Content {
    /// Wake the existing journal worker without waiting on its bounded queue.
    /// An order change invalidates the fast path even without a journal event.
    pub(crate) fn wake_window(&self, space: SpaceId, create: bool) {
        if !create && let Some(slot) = crate::lock(&self.spaces).get(&space) {
            slot.windows.invalidate();
        }
        if let Some(worker) = self.search_worker.get() {
            let _ = worker.try_send((space, create));
        }
    }

    /// Run a bounded replay step. Active idle slots reuse the cached head, and
    /// dormant slots require an authorized request before opening a sidecar.
    pub(crate) fn advance_window(&self, space: SpaceId, create: bool) -> Result<bool> {
        let slot = self.pin_slot(space)?;
        let cache = &slot.windows;
        cache.expire_handles();
        if cache.worker_deferred() {
            return Ok(false);
        }
        if !create && cache.idle_at(None) {
            return Ok(false);
        }
        let current = slot
            .space
            .try_lock()
            .ok()
            .and_then(|held| held.as_ref().map(|open| open.journal.head()));
        if !create && cache.idle_at(current.as_ref()) {
            return Ok(false);
        }
        let _step = self.lifecycle.step(space.as_uuid())?;
        let source = crate::db::space_path(&self.roots.appdata, space);
        let result = (|| {
            if !source.try_exists().map_err(internal)? {
                cache.dormant();
                return Ok(false);
            }
            cache.initialize_failure(&source)?;
            cache.flush_failure(&source)?;
            if let Some(failure) = cache.failure() {
                if failure.terminal() || (failure.action != Action::Reader && !failure.can_retry())
                {
                    return Ok(false);
                }
                if failure.action == Action::Replace {
                    let Ok(_exclusive) = cache.gate.try_write() else {
                        return Ok(false);
                    };
                    cache.drain();
                    runtime::replace_sidecar(&source)?;
                    cache.replaced();
                    cache.flush_failure(&source)?;
                }
            }
            let revision = cache.revision();
            let writer = match cache.take_writer() {
                Some(writer) => writer,
                None => {
                    if !create
                        && cache.failure().is_none()
                        && !path(&source).try_exists().map_err(internal)?
                    {
                        cache.dormant();
                        return Ok(false);
                    }
                    let Ok(_exclusive) = cache.gate.try_write() else {
                        return Ok(false);
                    };
                    // Cold initialization also runs the normal outbox recovery
                    // and additive content migration.
                    if slot.visibility.get().is_none() {
                        self.live_journal(space)?;
                    }
                    writer(&source, &self.roots.appdata, space)?
                }
            };
            let head = writer.journal.head().map_err(internal)?;
            let progress = match advance(&writer.conn, &writer.journal, &head, space) {
                Ok(progress) => progress,
                Err(error) if error.code == ErrorCode::Conflict => {
                    cache.put_writer(writer, &self.window_handles);
                    return Ok(false);
                }
                Err(error) => return Err(error),
            };
            if progress.ready {
                cache.recovered(false);
                cache.flush_failure(&source)?;
            }
            if let Some(head) = progress.caught_up {
                cache.published(head, revision);
            }
            cache.put_writer(writer, &self.window_handles);
            Ok(progress.more)
        })();
        if let Err(error) = result {
            if error.code == ErrorCode::Conflict {
                return Ok(false);
            }
            cache.report_failure(space, &error, Action::Replay);
            if let Err(storage) = cache.flush_failure(&source) {
                tracing::warn!(%space, ?storage, "could not persist window replay failure");
            }
            return Ok(false);
        }
        result
    }
}

fn writer(source: &Path, appdata: &Path, space: SpaceId) -> Result<Writer> {
    let journal = ProjectionJournal::open(appdata, space)
        .map_err(internal)?
        .ok_or_else(|| Error::unavailable("window journal is unavailable"))?;
    let sidecar = path(source);
    calternal_fsio::private::file(&sidecar).map_err(internal)?;
    let conn =
        Connection::open_with_flags(sidecar, OpenFlags::default() | OpenFlags::SQLITE_OPEN_URI)
            .map_err(sidecar_error)?;
    conn.busy_timeout(Duration::from_millis(50))
        .map_err(sidecar_error)?;
    check_version(&conn, true)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
        .map_err(sidecar_error)?;
    calternal_store::migrations::migrate(&conn, SCHEMA).map_err(sidecar_error)?;
    attach(&conn, source)?;
    Ok(Writer { conn, journal })
}

pub(super) fn malformed_metadata() -> Error {
    classified("malformed_source_metadata")
}

fn classified(reason: &'static str) -> Error {
    Error::internal("window projection storage error")
        .with_details(serde_json::json!({"window_failure": reason}))
}

// Only operations confined to main may authorize replacement. An error on
// the source, journal, or SQL spanning databases must keep the sidecar.
pub(super) fn sidecar_error(error: rusqlite::Error) -> Error {
    use rusqlite::ErrorCode::*;
    let reason = match error.sqlite_error_code() {
        Some(DatabaseCorrupt | NotADatabase) => Some("sidecar_corrupt"),
        Some(SchemaChanged) => Some("sidecar_schema"),
        _ => {
            let message = error.to_string();
            (message.starts_with("no such table:")
                || message.starts_with("no such column:")
                || message.starts_with("no such index:"))
            .then_some("sidecar_schema")
        }
    };
    if let Some(reason) = reason {
        tracing::debug!(%error, reason, "invalid window sidecar");
        return classified(reason);
    }
    sql_error(error)
}

pub(super) fn sql_error(error: rusqlite::Error) -> Error {
    if matches!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    ) {
        Error::conflict("window database is busy")
    } else if matches!(&error, rusqlite::Error::SqliteFailure(_, Some(message)) if message == "malformed JSON")
    {
        malformed_metadata()
    } else {
        internal(error)
    }
}

#[derive(Default)]
struct Progress {
    more: bool,
    ready: bool,
    caught_up: Option<SyncToken>,
}

fn advance(
    conn: &Connection,
    journal: &ProjectionJournal,
    head: &SyncToken,
    space: SpaceId,
) -> Result<Progress> {
    let tx = conn.unchecked_transaction().map_err(sql_error)?;
    let Some(config) = source_config(&tx)? else {
        return Ok(Progress::default());
    };
    let mut publication = match Publication::read(&tx)? {
        Some(mut publication)
            if publication.head.epoch == head.epoch && publication.head.seq <= head.seq =>
        {
            if publication.config != config {
                publication.config = config;
                if publication.scan_after.as_deref() != Some("!") {
                    publication.scan_after = Some(String::new());
                }
                publication.member_after = None;
                publication.ready = false;
            }
            publication
        }
        _ => Publication {
            head: head.clone(),
            config,
            scan_after: Some("!".into()),
            ready: false,
            member_after: None,
        },
    };
    if publication.ready && publication.head == *head {
        return Ok(Progress {
            ready: true,
            caught_up: Some(head.clone()),
            more: false,
        });
    }

    let mut skipped = Vec::new();
    if publication.scan_after.as_deref() == Some("!") {
        let removed = tx
            .execute(
                "DELETE FROM window_rows WHERE rowid IN (SELECT rowid FROM window_rows LIMIT ?1)",
                [PAGE as i64],
            )
            .map_err(sidecar_error)?;
        if removed == 0 {
            let skipped = tx.execute(
                "DELETE FROM window_skipped WHERE item IN (SELECT item FROM window_skipped LIMIT ?1)",
                [PAGE as i64],
            ).map_err(sidecar_error)?;
            if skipped < PAGE {
                publication.scan_after = Some(String::new());
            }
        }
    } else if let Some(after) = &publication.scan_after {
        let ids = {
            let mut stmt = tx
                .prepare_cached("SELECT id FROM source.items WHERE id>?1 ORDER BY id LIMIT ?2")
                .map_err(sql_error)?;
            stmt.query_map(params![after, PAGE as i64], |row| row.get::<_, String>(0))
                .map_err(sql_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(sql_error)?
        };
        for id in &ids {
            if item(&tx, id)? {
                skipped.push(id.clone());
            }
        }
        publication.scan_after = (ids.len() == PAGE).then(|| ids.last().unwrap().clone());
    } else if publication.head.seq < head.seq {
        let entries = journal
            .since(publication.head.seq, PAGE)
            .map_err(internal)?;
        let mut expected = publication.head.seq;
        let mut found = false;
        for entry in entries
            .into_iter()
            .take_while(|entry| entry.sequence <= head.seq)
        {
            if entry.sequence != expected + 1 {
                break;
            }
            found = true;
            if let Some(id) = entry.item {
                if is_collection(&tx, &id)? {
                    // A collection can be much larger than a journal page.
                    // Do one membership page per transaction.
                    if expected != publication.head.seq {
                        break;
                    }
                    publication.member_after = collection_page(
                        &tx,
                        &id,
                        publication.member_after.as_deref().unwrap_or(""),
                        &mut skipped,
                    )?;
                    if publication.member_after.is_some() {
                        break;
                    }
                }
                if item(&tx, &id)? {
                    skipped.push(id);
                }
            }
            expected = entry.sequence;
        }
        if !found {
            publication.head = head.clone();
            publication.scan_after = Some("!".into());
            publication.member_after = None;
        } else {
            publication.head.seq = expected;
        }
    }
    let current = journal.head().map_err(internal)?;
    if current.epoch != publication.head.epoch || current.seq < publication.head.seq {
        return Ok(Progress {
            more: true,
            ..Progress::default()
        });
    }
    publication.ready = publication.scan_after.is_none() && publication.member_after.is_none();
    let caught_up = publication.ready && publication.head == current;
    publication.persist(&tx)?;
    tx.commit().map_err(sidecar_error)?;
    for item in skipped {
        tracing::warn!(%space, %item, "window omitted explicit collections for an over-limit note");
    }
    Ok(Progress {
        more: !caught_up,
        ready: publication.ready,
        caught_up: caught_up.then_some(current),
    })
}
