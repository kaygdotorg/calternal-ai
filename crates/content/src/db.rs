//! One physically space-bound metadata/history database, with no space predicates.
use crate::types::{internal, missing};
use crate::{Error, Item, Result, SpaceId};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
};
use uuid::Uuid;

/// Shared eviction ceiling for per-space kind/name caches; eviction affects cost only.
pub(crate) const KIND_CACHE: usize = 32;

/// One current revision's header, loaded under the selected space lock. Reused
/// for admission, the outside-edit guard and placement; contains no body bytes.
pub(crate) struct Header {
    pub(crate) kind: String,
    pub(crate) revision: String,
    pub(crate) seq: i64,
    pub(crate) path: String,
    pub(crate) state: String,
    pub(crate) file_hash: Option<String>,
    pub(crate) old_path: Option<String>,
}

pub(crate) struct Space {
    pub(crate) id: SpaceId,
    /// This space's journal and receipts; dropped (and drained) with the space.
    pub(crate) journal: calternal_store::SpaceStore,
    pub(crate) conn: Connection,
    pub(crate) files: PathBuf,
    /// Oldest acknowledged head awaiting file durability; never reset by new writes.
    pub(crate) pending_since: Option<std::time::Instant>,
    /// Immutable kind definitions, shared by validation, naming and placement;
    /// bounded like the existing name cache and dropped with this space.
    pub(crate) registrations: RefCell<HashMap<String, (crate::Placement, crate::FileFormat)>>,
    pub(crate) names: HashMap<String, calternal_fsio::names::DirectoryNames>,
}
/// The existing per-space SQLite path under the guarded appdata root.
pub(crate) fn space_path(appdata: &Path, id: SpaceId) -> PathBuf {
    appdata
        .join("spaces")
        .join(id.to_string())
        .join("content.db")
}

pub(crate) fn database(path: &Path) -> Result<Connection> {
    calternal_fsio::private::directory(path.parent().ok_or_else(|| internal("parent"))?)
        .map_err(internal)?;
    calternal_fsio::private::file(path).map_err(internal)?;
    let conn = Connection::open(path).map_err(internal)?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(internal)?;
    conn.pragma_update(None, "synchronous", "FULL")
        .map_err(internal)?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(internal)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(internal)?;
    Ok(conn)
}
impl Space {
    pub(crate) fn open(
        appdata: &Path,
        userdata_ssd: &Path,
        id: SpaceId,
        journal: calternal_store::SpaceStore,
    ) -> Result<Self> {
        let conn = database(&space_path(appdata, id))?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(internal)?;
        if version != 0 && version != 3 {
            return Err(internal("unsupported content schema"));
        }
        conn.execute_batch("BEGIN IMMEDIATE;
            CREATE TABLE IF NOT EXISTS binding(id TEXT PRIMARY KEY CHECK(id='space'),value TEXT NOT NULL) STRICT;
            CREATE TABLE IF NOT EXISTS kinds(kind TEXT PRIMARY KEY,definition TEXT NOT NULL) STRICT;
            CREATE TABLE IF NOT EXISTS items(id TEXT PRIMARY KEY,kind TEXT NOT NULL,metadata TEXT NOT NULL,revision TEXT NOT NULL,seq INTEGER NOT NULL,updated_at TEXT NOT NULL,path TEXT NOT NULL,folded_path TEXT NOT NULL UNIQUE,old_path TEXT,mat_state TEXT NOT NULL,file_hash TEXT) STRICT;
            CREATE INDEX IF NOT EXISTS item_list ON items(kind,updated_at DESC,id);
            CREATE INDEX IF NOT EXISTS item_pending ON items(mat_state,id);
            CREATE TABLE IF NOT EXISTS versions(item TEXT NOT NULL REFERENCES items(id),seq INTEGER NOT NULL,revision TEXT NOT NULL UNIQUE,metadata TEXT NOT NULL,bytes BLOB NOT NULL,updated_at TEXT NOT NULL,author TEXT NOT NULL,PRIMARY KEY(item,seq)) STRICT;
            CREATE TABLE IF NOT EXISTS outbox(account TEXT NOT NULL,request TEXT NOT NULL,operation TEXT NOT NULL,fingerprint TEXT NOT NULL,item TEXT REFERENCES items(id),revision TEXT NOT NULL,result TEXT NOT NULL,PRIMARY KEY(account,request)) STRICT;
            CREATE TABLE IF NOT EXISTS controls(key TEXT PRIMARY KEY,value TEXT NOT NULL) STRICT;
            CREATE UNIQUE INDEX IF NOT EXISTS standard_collections ON items(json_extract(metadata,'$.item_kind'),json_extract(metadata,'$.standard_key')) WHERE kind='collections' AND json_extract(metadata,'$.standard_key') IS NOT NULL;
            PRAGMA user_version=3; COMMIT;").map_err(internal)?;
        conn.execute(
            "INSERT OR IGNORE INTO binding VALUES('space',?1)",
            [id.to_string()],
        )
        .map_err(internal)?;
        crate::discovery::install(&conn)?;
        crate::linked_write::install(&conn)?;
        crate::trash::install(&conn)?;
        let bound: String = conn
            .query_row("SELECT value FROM binding WHERE id='space'", [], |r| {
                r.get(0)
            })
            .map_err(internal)?;
        if bound != id.to_string() {
            return Err(internal("wrong space binding"));
        }
        Ok(Self {
            id,
            journal,
            conn,
            files: userdata_ssd.join("spaces").join(id.to_string()),
            names: HashMap::new(),
            registrations: RefCell::new(HashMap::new()),
            pending_since: None,
        })
    }
    /// Load one indexed header without body or file reads. Missing items fail;
    /// callers decide whether Trash/purge states are visible for their operation.
    pub(crate) fn header(&self, id: Uuid) -> Result<Header> {
        self.conn
            .query_row(
                "SELECT kind,revision,seq,path,mat_state,file_hash,old_path FROM items WHERE id=?1",
                [id.to_string()],
                |r| {
                    Ok(Header {
                        kind: r.get(0)?,
                        revision: r.get(1)?,
                        seq: r.get(2)?,
                        path: r.get(3)?,
                        state: r.get(4)?,
                        file_hash: r.get(5)?,
                        old_path: r.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(internal)?
            .ok_or_else(missing)
    }

    pub(crate) fn get(&self, id: Uuid) -> Result<Item> {
        self.get_selected(id, false)
    }
    pub(crate) fn get_selected(&self, id: Uuid, live_only: bool) -> Result<Item> {
        let row=self.conn.query_row("SELECT i.kind,v.metadata,v.bytes,v.revision,v.updated_at FROM items i JOIN versions v ON v.item=i.id AND v.seq=i.seq WHERE i.id=?1 AND i.mat_state NOT IN ('purged','purging') AND (NOT ?2 OR substr(i.path,1,7) <> '.trash/')",params![id.to_string(),live_only],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Vec<u8>>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?))).optional().map_err(internal)?.ok_or_else(missing)?;
        Ok(Item {
            id,
            space: self.id,
            kind: row.0,
            metadata: serde_json::from_str(&row.1).map_err(internal)?,
            bytes: row.2,
            revision: row.3,
            updated_at: row.4,
        })
    }
    pub(crate) fn history(
        &self,
        id: Uuid,
        limit: u32,
        cursor: Option<&str>,
        live_kind: Option<&str>,
    ) -> Result<(Vec<Item>, Option<String>)> {
        let item = self.get_selected(id, live_kind.is_some())?;
        if live_kind.is_some_and(|kind| kind != item.kind) {
            return Err(missing());
        }
        let before = if let Some(cursor) = cursor {
            let rev = cursor
                .strip_prefix('h')
                .ok_or_else(|| Error::invalid_input("invalid history cursor"))?;
            self.conn
                .query_row(
                    "SELECT seq FROM versions WHERE item=?1 AND revision=?2",
                    params![id.to_string(), rev],
                    |r| r.get::<_, i64>(0),
                )
                .optional()
                .map_err(internal)?
                .ok_or_else(|| Error::invalid_input("invalid history cursor"))?
        } else {
            i64::MAX
        };
        let limit = limit.clamp(1, 1000);
        let mut stmt=self.conn.prepare_cached("SELECT metadata,bytes,revision,updated_at FROM versions WHERE item=?1 AND seq<?2 ORDER BY seq DESC LIMIT ?3").map_err(internal)?;
        let rows = stmt
            .query_map(params![id.to_string(), before, limit + 1], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(internal)?;
        let versions: Vec<Item> = rows
            .map(|row| {
                let (meta, bytes, revision, updated_at) = row.map_err(internal)?;
                Ok(Item {
                    id,
                    space: self.id,
                    kind: item.kind.clone(),
                    metadata: serde_json::from_str::<Value>(&meta).map_err(internal)?,
                    bytes,
                    revision,
                    updated_at,
                })
            })
            .collect::<Result<_>>()?;
        let page = calternal_types::Page::from_lookahead(versions, limit as usize, |v| {
            format!("h{}", v.revision)
        });
        Ok((page.rows, page.next))
    }
}

/// Replayable bookkeeping never owns original bytes. NORMAL may lose these
/// changes after power loss; durable intents and file hashes make replay safe.
/// Always restore FULL before admitting another content/history transaction.
pub(crate) fn bookkeeping(conn: &Connection, run: impl FnOnce() -> Result<()>) -> Result<()> {
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(internal)?;
    let result = run();
    let restored = conn
        .pragma_update(None, "synchronous", "FULL")
        .map_err(internal);
    restored.and(result)
}
