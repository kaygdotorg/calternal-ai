//! Bounded replay on the existing per-space projection worker.
use super::runtime::{self, Writer};
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
    calternal_store::migrations::Schema::new("window sidecar", 1, &[SQL], &[]);

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

pub(super) fn reader(source: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        path(source),
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(sql_error)?;
    conn.busy_timeout(Duration::from_millis(50))
        .map_err(sql_error)?;
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
            "SELECT epoch,sequence,config,scan_after,ready,member_after FROM window_publication",
        )
        .map_err(sql_error)?
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
        .map_err(sql_error)
    }

    fn persist(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "INSERT INTO window_publication VALUES(1,?1,?2,?3,?4,?5,?6)
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
        .map_err(sql_error)?;
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

    pub(super) fn matches_source(&self, conn: &Connection) -> Result<bool> {
        Ok(self.ready && source_config(conn)? == Some(self.config))
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
            if let Some(failure) = cache.failure() {
                if failure.retry_at_ms > calternal_types::now_ms() {
                    return Ok(false);
                }
                if failure.replace {
                    let Ok(_exclusive) = cache.gate.try_write() else {
                        return Ok(false);
                    };
                    if failure.attempts == 0 {
                        tracing::warn!(%space, "rebuilding unreadable window projection");
                        cache.record_failure(&source, "reader_failed", true)?;
                    }
                    cache.drain();
                    runtime::replace_sidecar(&source)?;
                    cache.replaced(&source)?;
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
            let more = match advance(&writer.conn, &writer.journal, &head) {
                Ok(more) => more,
                Err(error) if error.code == ErrorCode::Conflict => {
                    cache.put_writer(writer, &self.window_handles);
                    return Ok(false);
                }
                Err(error) => return Err(error),
            };
            if !more {
                if let Some(publication) = Publication::read(&writer.conn)?
                    && publication.ready
                    && publication.head == head
                    && publication.matches_source(&writer.conn)?
                {
                    cache.recovered(&source)?;
                    cache.published(head, revision);
                }
            }
            cache.put_writer(writer, &self.window_handles);
            Ok(more)
        })();
        if let Err(error) = result {
            if error.code == ErrorCode::Conflict {
                return Ok(false);
            }
            tracing::warn!(%space, ?error, "window projection failed");
            if let Err(storage) = cache.record_failure(&source, "replay_failed", true) {
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
            .map_err(sql_error)?;
    conn.busy_timeout(Duration::from_millis(50))
        .map_err(sql_error)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
        .map_err(sql_error)?;
    calternal_store::migrations::migrate(&conn, SCHEMA).map_err(sql_error)?;
    attach(&conn, source)?;
    Ok(Writer { conn, journal })
}

pub(super) fn sql_error(error: rusqlite::Error) -> Error {
    if matches!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    ) {
        Error::conflict("window database is busy")
    } else {
        internal(error)
    }
}

fn advance(conn: &Connection, journal: &ProjectionJournal, head: &SyncToken) -> Result<bool> {
    let tx = conn.unchecked_transaction().map_err(sql_error)?;
    let Some(config) = source_config(&tx)? else {
        return Ok(false);
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
        return Ok(false);
    }

    if publication.scan_after.as_deref() == Some("!") {
        let removed = tx
            .execute(
                "DELETE FROM window_rows WHERE rowid IN (SELECT rowid FROM window_rows LIMIT ?1)",
                [PAGE as i64],
            )
            .map_err(sql_error)?;
        if removed == 0 {
            let skipped = tx.execute(
                "DELETE FROM window_skipped WHERE item IN (SELECT item FROM window_skipped LIMIT ?1)",
                [PAGE as i64],
            ).map_err(sql_error)?;
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
            item(&tx, id)?;
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
                    )?;
                    if publication.member_after.is_some() {
                        break;
                    }
                }
                item(&tx, &id)?;
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
        return Ok(true);
    }
    publication.ready = publication.scan_after.is_none() && publication.member_after.is_none();
    let caught_up = publication.ready && publication.head == current;
    publication.persist(&tx)?;
    tx.commit().map_err(sql_error)?;
    Ok(!caught_up)
}

const ITEM_ROWS: &str = "
    SELECT n.collection,n.audience,n.key,n.id,
           CASE WHEN n.audience='explicit' THEN CASE WHEN json_type(i.metadata,'$.title')='text' THEN json_extract(i.metadata,'$.title') ELSE '' END
                ELSE i.revision END AS visible
      FROM window_note_candidates n JOIN items i ON i.id=n.id WHERE n.id=?1
    UNION ALL
    SELECT 'notes','trash',t.key,t.id,i.revision
      FROM window_trash_notes t JOIN items i ON i.id=t.id WHERE t.id=?1
    UNION ALL
    SELECT g.collection,g.audience,g.key,g.id,i.revision
      FROM window_generic_candidates g JOIN items i ON i.id=g.id WHERE g.id=?1";

const UPSERT: &str = "ON CONFLICT(item,collection,audience) DO UPDATE
    SET sort_key=excluded.sort_key, visible=excluded.visible
    WHERE window_rows.sort_key IS NOT excluded.sort_key OR window_rows.visible IS NOT excluded.visible";

// A space from before scope admission can still contain excess memberships.
// Exclude only that item and retain a reason until its memberships are repaired.
const EXCESS_SCOPES: &str = "EXISTS(SELECT 1 FROM note_collection_members
    WHERE note_id=n.id LIMIT 1 OFFSET 31)";

pub(super) fn item(conn: &Connection, id: &str) -> Result<()> {
    static LIMIT: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        format!(
            "SELECT EXISTS(SELECT 1 FROM window_live_notes n WHERE n.id=?1 AND {EXCESS_SCOPES})"
        )
    });
    let excess: bool = conn
        .prepare_cached(&LIMIT)
        .map_err(sql_error)?
        .query_row([id], |row| row.get(0))
        .map_err(sql_error)?;
    if excess {
        conn.execute("DELETE FROM window_rows WHERE item=?1", [id])
            .map_err(sql_error)?;
        conn.execute(
            "INSERT OR IGNORE INTO window_skipped VALUES(?1,'effective_scope_limit_exceeded')",
            [id],
        )
        .map_err(sql_error)?;
        return Ok(());
    }
    conn.execute("DELETE FROM window_skipped WHERE item=?1", [id])
        .map_err(sql_error)?;

    static DELETE: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        format!(
            "DELETE FROM window_rows WHERE item=?1 AND (collection,audience) NOT IN
         (SELECT collection,audience FROM ({ITEM_ROWS}))"
        )
    });
    static PUT: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        format!("INSERT INTO window_rows SELECT * FROM ({ITEM_ROWS}) WHERE true {UPSERT}")
    });
    conn.prepare_cached(&DELETE)
        .map_err(sql_error)?
        .execute([id])
        .map_err(sql_error)?;
    conn.prepare_cached(&PUT)
        .map_err(sql_error)?
        .execute([id])
        .map_err(sql_error)?;
    Ok(())
}

fn is_collection(conn: &Connection, id: &str) -> Result<bool> {
    conn.prepare_cached("SELECT EXISTS(SELECT 1 FROM items WHERE id=?1 AND kind='collections')")
        .map_err(sql_error)?
        .query_row([id], |row| row.get(0))
        .map_err(sql_error)
}

fn collection_page(conn: &Connection, id: &str, after: &str) -> Result<Option<String>> {
    // The source range admits new edges and the sidecar range locates old
    // edges for deletion. Each branch seeks by collection and item first.
    let mut stmt = conn
        .prepare_cached(
            "SELECT item FROM (
           SELECT item FROM (SELECT note_id AS item FROM note_collection_members
             WHERE collection_id=?1 AND note_id>?2 ORDER BY note_id LIMIT ?3)
           UNION
           SELECT item FROM (SELECT item FROM window_rows INDEXED BY window_members
             WHERE collection=?1 AND item>?2 AND audience IN ('explicit','explicit_newest')
             ORDER BY item LIMIT ?3)
           UNION
           SELECT item FROM (SELECT item FROM window_skipped WHERE item>?2 ORDER BY item LIMIT ?3)
         ) ORDER BY item LIMIT ?3",
        )
        .map_err(sql_error)?;
    let ids = stmt
        .query_map(params![id, after, PAGE as i64], |row| {
            row.get::<_, String>(0)
        })
        .map_err(sql_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(sql_error)?;
    for note in &ids {
        item(conn, note)?;
    }
    Ok((ids.len() == PAGE).then(|| ids.last().unwrap().clone()))
}
