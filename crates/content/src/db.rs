//! One physically space-bound metadata/history database, with no space predicates.
use crate::types::{internal, missing};
use crate::{Error, Item, Result, SpaceId, Summary};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub(crate) struct Space {
    pub(crate) id: SpaceId,
    pub(crate) _journal: calternal_store::SpaceStore,
    pub(crate) conn: Connection,
    pub(crate) files: PathBuf,
    pub(crate) names: HashMap<String, calternal_fsio::names::DirectoryNames>,
}
pub(crate) fn database(path: &Path) -> Result<Connection> {
    calternal_auth::private_fs::directory(path.parent().ok_or_else(|| internal("parent"))?)
        .map_err(internal)?;
    calternal_auth::private_fs::file(path).map_err(internal)?;
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
        let path = appdata
            .join("spaces")
            .join(id.to_string())
            .join("content.db");
        let conn = database(&path)?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(internal)?;
        if version != 0 && version != 1 {
            return Err(internal("unsupported content schema"));
        }
        conn.execute_batch("BEGIN IMMEDIATE;
            CREATE TABLE IF NOT EXISTS binding(id TEXT PRIMARY KEY CHECK(id='space'),value TEXT NOT NULL) STRICT;
            CREATE TABLE IF NOT EXISTS kinds(kind TEXT PRIMARY KEY,definition TEXT NOT NULL) STRICT;
            CREATE TABLE IF NOT EXISTS items(id TEXT PRIMARY KEY,kind TEXT NOT NULL,metadata TEXT NOT NULL,revision TEXT NOT NULL,seq INTEGER NOT NULL,updated_at TEXT NOT NULL,path TEXT NOT NULL,folded_path TEXT NOT NULL UNIQUE,old_path TEXT,mat_state TEXT NOT NULL,file_sha256 TEXT) STRICT;
            CREATE INDEX IF NOT EXISTS item_list ON items(kind,updated_at DESC,id);
            CREATE INDEX IF NOT EXISTS item_pending ON items(mat_state,id);
            CREATE TABLE IF NOT EXISTS versions(item TEXT NOT NULL REFERENCES items(id),seq INTEGER NOT NULL,revision TEXT NOT NULL UNIQUE,metadata TEXT NOT NULL,bytes BLOB NOT NULL,updated_at TEXT NOT NULL,author TEXT NOT NULL,PRIMARY KEY(item,seq)) STRICT;
            CREATE TABLE IF NOT EXISTS outbox(account TEXT NOT NULL,request TEXT NOT NULL,operation TEXT NOT NULL,fingerprint TEXT NOT NULL,item TEXT NOT NULL REFERENCES items(id),revision TEXT NOT NULL,result TEXT NOT NULL,PRIMARY KEY(account,request)) STRICT;
            CREATE UNIQUE INDEX IF NOT EXISTS standard_collections ON items(json_extract(metadata,'$.item_kind'),json_extract(metadata,'$.standard_key')) WHERE kind='collections' AND json_extract(metadata,'$.standard_key') IS NOT NULL;
            PRAGMA user_version=1; COMMIT;").map_err(internal)?;
        conn.execute(
            "INSERT OR IGNORE INTO binding VALUES('space',?1)",
            [id.to_string()],
        )
        .map_err(internal)?;
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
            _journal: journal,
            conn,
            files: userdata_ssd.join("spaces").join(id.to_string()),
            names: HashMap::new(),
        })
    }
    pub(crate) fn get(&self, id: Uuid) -> Result<Item> {
        let row=self.conn.query_row("SELECT i.kind,v.metadata,v.bytes,v.revision,v.updated_at FROM items i JOIN versions v ON v.item=i.id AND v.seq=i.seq WHERE i.id=?1",[id.to_string()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Vec<u8>>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?))).optional().map_err(internal)?.ok_or_else(missing)?;
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
    pub(crate) fn list(&self, kind: &str, limit: u32) -> Result<Vec<Summary>> {
        let mut stmt=self.conn.prepare_cached("SELECT id,metadata,revision,updated_at FROM items WHERE kind=?1 ORDER BY updated_at DESC,id LIMIT ?2").map_err(internal)?;
        let rows = stmt
            .query_map(params![kind, limit.min(1000)], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(internal)?;
        rows.map(|row| {
            let (id, metadata, revision, updated_at) = row.map_err(internal)?;
            Ok(Summary {
                id: Uuid::parse_str(&id).map_err(internal)?,
                space: self.id,
                kind: kind.into(),
                metadata: serde_json::from_str(&metadata).map_err(internal)?,
                revision,
                updated_at,
            })
        })
        .collect()
    }
    pub(crate) fn history(
        &self,
        id: Uuid,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<(Vec<Item>, Option<String>)> {
        let item = self.get(id)?;
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
        let mut versions: Vec<Item> = rows
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
        let more = versions.len() > limit as usize;
        versions.truncate(limit as usize);
        let cursor = if more {
            versions.last().map(|v| format!("h{}", v.revision))
        } else {
            None
        };
        Ok((versions, cursor))
    }
}
