//! Bounded replay on the existing per-space projection worker.
use crate::{Content, Result, SpaceId, types::internal};
use calternal_store::{ProjectionJournal, SyncToken};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
    time::Duration,
};

const PAGE: usize = 128;
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
        .map_err(internal)?;
    Ok(())
}

pub(super) fn reader(source: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        path(source),
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(internal)?;
    conn.busy_timeout(Duration::from_millis(50))
        .map_err(internal)?;
    attach(&conn, source)?;
    Ok(conn)
}

#[derive(PartialEq, Eq)]
pub(super) struct Publication {
    pub(super) head: SyncToken,
    config: i64,
    scan_after: Option<String>,
    pub(super) ready: bool,
}

impl Publication {
    pub(super) fn read(conn: &Connection) -> Result<Option<Self>> {
        conn.prepare_cached("SELECT epoch,sequence,config,scan_after,ready FROM window_publication")
            .map_err(internal)?
            .query_row([], |row| {
                Ok(Self {
                    head: SyncToken {
                        epoch: row.get(0)?,
                        seq: row.get(1)?,
                    },
                    config: row.get(2)?,
                    scan_after: row.get(3)?,
                    ready: row.get(4)?,
                })
            })
            .optional()
            .map_err(internal)
    }

    fn persist(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "INSERT INTO window_publication VALUES(1,?1,?2,?3,?4,?5)
             ON CONFLICT(id) DO UPDATE SET epoch=excluded.epoch,sequence=excluded.sequence,
                 config=excluded.config,scan_after=excluded.scan_after,ready=excluded.ready",
            params![
                self.head.epoch,
                self.head.seq,
                self.config,
                self.scan_after,
                self.ready
            ],
        )
        .map_err(internal)?;
        Ok(())
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
    .map_err(internal)?
    .query_row([], |row| {
        let pending: bool = row.get(1)?;
        if pending {
            Ok(None)
        } else {
            row.get(0).map(Some)
        }
    })
    .map_err(internal)
}

impl Content {
    /// Wake the existing journal worker without waiting on its bounded queue.
    /// A full queue delays publication. The next read also requests a wake.
    pub(crate) fn wake_window(&self, space: SpaceId, create: bool) {
        if let Some(worker) = self.search_worker.get() {
            let _ = worker.try_send((space, create));
        }
    }

    /// Run one bounded window replay step on the existing projection worker.
    /// Returns true while recovery, bootstrap, or further journal pages remain.
    pub(crate) fn advance_window(&self, space: SpaceId, create: bool) -> Result<bool> {
        let _step = self.lifecycle.step(space.as_uuid())?;
        let source = crate::db::space_path(&self.roots.appdata, space);
        let sidecar = path(&source);
        // Lazily start a new projection on the first authorized window. An
        // ordinary Notes mutation still wakes a projection already on disk.
        if (!create && !sidecar.is_file()) || !source.exists() {
            return Ok(false);
        }
        let source_reader = crate::pool::open_reader(&source).map_err(internal)?;
        let (configured, pending): (bool, bool) = source_reader
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='window_configuration'),
                    EXISTS(SELECT 1 FROM outbox)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(internal)?;
        if !configured || (pending && !crate::lock(&self.spaces).contains_key(&space)) {
            self.live_journal(space)?;
            return Ok(true);
        }
        if pending {
            // Let the writer retire its outbox. Its wake or the worker poll
            // will retry without spinning against a durable save.
            return Ok(false);
        }
        let Some(journal) =
            ProjectionJournal::open(&self.roots.appdata, space).map_err(internal)?
        else {
            return Ok(false);
        };
        let head = journal.head().map_err(internal)?;
        calternal_fsio::private::file(&sidecar).map_err(internal)?;
        let conn =
            Connection::open_with_flags(sidecar, OpenFlags::default() | OpenFlags::SQLITE_OPEN_URI)
                .map_err(internal)?;
        conn.busy_timeout(Duration::from_millis(50))
            .map_err(internal)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(internal)?;
        calternal_store::migrations::migrate(&conn, SCHEMA).map_err(internal)?;
        attach(&conn, &source)?;
        advance(&conn, &journal, &head)
    }
}

fn advance(conn: &Connection, journal: &ProjectionJournal, head: &SyncToken) -> Result<bool> {
    let tx = conn.unchecked_transaction().map_err(internal)?;
    let Some(config) = source_config(&tx)? else {
        return Ok(false);
    };
    let mut publication = match Publication::read(&tx)? {
        Some(mut publication)
            if publication.head.epoch == head.epoch && publication.head.seq <= head.seq =>
        {
            if publication.config != config {
                publication.config = config;
                publication.scan_after = Some(String::new());
                publication.ready = false;
            }
            publication
        }
        _ => {
            tx.execute_batch("DELETE FROM window_rows;")
                .map_err(internal)?;
            Publication {
                head: head.clone(),
                config,
                scan_after: Some(String::new()),
                ready: false,
            }
        }
    };
    if publication.ready && publication.head == *head {
        return Ok(false);
    }

    if let Some(after) = &publication.scan_after {
        let ids = {
            let mut stmt = tx
                .prepare_cached("SELECT id FROM source.items WHERE id>?1 ORDER BY id LIMIT ?2")
                .map_err(internal)?;
            stmt.query_map(params![after, PAGE as i64], |row| row.get::<_, String>(0))
                .map_err(internal)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(internal)?
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
        let mut changed = std::collections::BTreeSet::new();
        for entry in entries
            .into_iter()
            .take_while(|entry| entry.sequence <= head.seq)
        {
            if entry.sequence != expected + 1 {
                break;
            }
            expected = entry.sequence;
            if let Some(id) = entry.item {
                changed.insert(id);
            }
        }
        for id in changed {
            item(&tx, &id)?;
            collection(&tx, &id)?;
        }
        if expected == publication.head.seq {
            // No continuous replay is available. Begin a new unpublished build.
            tx.execute_batch("DELETE FROM window_rows;")
                .map_err(internal)?;
            publication.head = head.clone();
            publication.scan_after = Some(String::new());
        } else {
            publication.head.seq = expected;
        }
    }
    let current = journal.head().map_err(internal)?;
    if current.epoch != publication.head.epoch || current.seq < publication.head.seq {
        return Ok(true);
    }
    publication.ready = publication.scan_after.is_none() && publication.head == current;
    publication.persist(&tx)?;
    tx.commit().map_err(internal)?;
    Ok(!publication.ready)
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
// Keep its generation unpublished until the authoritative lists are repaired.
const EXCESS_SCOPES: &str = "EXISTS(SELECT 1 FROM note_collection_members
    WHERE note_id=n.id LIMIT 1 OFFSET 31)";

fn enforce_scope_limit(conn: &Connection, sql: &str, id: &str) -> Result<()> {
    let excess: bool = conn
        .prepare_cached(sql)
        .map_err(internal)?
        .query_row([id], |row| row.get(0))
        .map_err(internal)?;
    if excess {
        return Err(crate::Error::new(
            crate::ErrorCode::EffectiveScopeLimitExceeded,
            "stored Notes memberships exceed the window scope limit",
        ));
    }
    Ok(())
}

pub(super) fn item(conn: &Connection, id: &str) -> Result<()> {
    static LIMIT: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        format!(
            "SELECT EXISTS(SELECT 1 FROM window_live_notes n WHERE n.id=?1 AND {EXCESS_SCOPES})"
        )
    });
    enforce_scope_limit(conn, &LIMIT, id)?;

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
        .map_err(internal)?
        .execute([id])
        .map_err(internal)?;
    conn.prepare_cached(&PUT)
        .map_err(internal)?
        .execute([id])
        .map_err(internal)?;
    Ok(())
}

fn collection(conn: &Connection, id: &str) -> Result<()> {
    // A journalled collection edit can add/remove members without editing their
    // heads. Derive the delta from the authoritative inverse membership index.
    static LIMIT: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        format!(
            "SELECT EXISTS(SELECT 1 FROM note_collection_members m
             JOIN window_live_notes n ON n.id=m.note_id
             WHERE m.collection_id=?1 AND {EXCESS_SCOPES})"
        )
    });
    enforce_scope_limit(conn, &LIMIT, id)?;

    static PUT: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        format!(
        "INSERT INTO window_rows
         SELECT ?1, CASE WHEN o.collection IS NULL THEN 'explicit' ELSE 'explicit_newest' END,
           CASE WHEN o.collection IS NULL THEN n.title ELSE n.newest END, n.id,
           CASE WHEN o.collection IS NULL THEN CASE WHEN json_type(i.metadata,'$.title')='text' THEN json_extract(i.metadata,'$.title') ELSE '' END
                ELSE i.revision END
         FROM note_collection_members m
         JOIN window_live_notes n ON n.id=m.note_id JOIN items i ON i.id=n.id
         LEFT JOIN window_collection_newest o ON o.collection=m.collection_id
         WHERE m.collection_id=?1 {UPSERT}"
    )
    });
    conn.prepare_cached(
        "DELETE FROM window_rows AS w WHERE w.collection=?1
         AND w.audience IN ('explicit','explicit_newest') AND NOT EXISTS (
             SELECT 1 FROM note_collection_members m
             JOIN window_live_notes n ON n.id=m.note_id
             LEFT JOIN window_collection_newest o ON o.collection=m.collection_id
             WHERE m.collection_id=?1 AND m.note_id=w.item
             AND w.audience=CASE WHEN o.collection IS NULL THEN 'explicit' ELSE 'explicit_newest' END
         )",
    ).map_err(internal)?.execute([id]).map_err(internal)?;
    conn.prepare_cached(&PUT)
        .map_err(internal)?
        .execute([id])
        .map_err(internal)?;
    Ok(())
}
