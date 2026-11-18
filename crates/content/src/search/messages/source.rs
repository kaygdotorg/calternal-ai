//! Read only committed heads and exact revision byte ranges.
use super::{Result, internal};
use calternal_fsio::beneath::Folder;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

const LIVE: &str = "i.kind='threads' AND i.rowid<=250000 AND substr(i.path,1,7)<>'.trash/'
    AND i.mat_state NOT IN ('purging','purged') AND coalesce(i.metadata->>'deleted',0)<>1";

pub(super) struct Head {
    pub(super) slot: i64,
    pub(super) id: String,
    pub(super) revision: String,
    path: String,
    hash: Option<String>,
    version: i64,
}

fn head(row: &rusqlite::Row<'_>) -> rusqlite::Result<Head> {
    Ok(Head {
        slot: row.get(0)?,
        id: row.get(1)?,
        revision: row.get(2)?,
        path: row.get(3)?,
        hash: row.get(4)?,
        version: row.get(5)?,
    })
}

pub(super) fn exists(db: &Connection) -> Result<bool> {
    db.prepare_cached(&format!(
        "SELECT EXISTS(SELECT 1 FROM items i WHERE {LIVE})"
    ))
    .map_err(internal)?
    .query_row([], |r| r.get(0))
    .map_err(internal)
}

const FIELDS: &str = "i.rowid,i.id,i.revision,i.path,i.file_hash,v.rowid";
const JOIN: &str = "FROM items i JOIN versions v ON v.item=i.id AND v.seq=i.seq";

pub(super) fn page(db: &Connection, after: i64, limit: i64) -> Result<Vec<Head>> {
    db.prepare_cached(&format!(
        "SELECT {FIELDS} {JOIN} WHERE {LIVE} AND i.rowid>?1 ORDER BY i.rowid LIMIT ?2"
    ))
    .map_err(internal)?
    .query_map(params![after, limit], head)
    .map_err(internal)?
    .collect::<rusqlite::Result<_>>()
    .map_err(internal)
}

pub(super) fn get(db: &Connection, id: &str) -> Result<Option<Head>> {
    db.prepare_cached(&format!("SELECT {FIELDS} {JOIN} WHERE {LIVE} AND i.id=?1"))
        .map_err(internal)?
        .query_row([id], head)
        .optional()
        .map_err(internal)
}

pub(super) fn bytes(db: &Connection, folder: Option<&Folder>, head: &Head) -> Result<Vec<u8>> {
    // Materialization inserts one host identity line. Strip it before using
    // byte offsets, so both the guarded file and staged bytes map to the same BLOB.
    if let Some(mut bytes) = folder
        .zip(head.hash.as_deref())
        .and_then(|(folder, hash)| super::super::source::read_verified(folder, &head.path, hash))
    {
        let prefix = format!("---\ncalternal-id: {}\n", head.id);
        if bytes.starts_with(prefix.as_bytes()) {
            bytes.drain(4..prefix.len());
            return Ok(bytes);
        }
    }
    db.prepare_cached("SELECT bytes FROM versions WHERE rowid=?1")
        .map_err(internal)?
        .query_row([head.version], |r| r.get(0))
        .map_err(internal)
}

pub(super) struct Revision {
    pub(super) rowid: i64,
    pub(super) title: String,
    pub(super) size: usize,
}

pub(super) fn revision(db: &Connection, thread: Uuid, revision: &str) -> Result<Option<Revision>> {
    db.prepare_cached(&format!(
        "SELECT v.rowid, i.metadata->>'title', length(v.bytes) {JOIN}
         WHERE {LIVE} AND i.id=?1 AND i.revision=?2"
    ))
    .map_err(internal)?
    .query_row(params![thread.to_string(), revision], |r| {
        Ok(Revision {
            rowid: r.get(0)?,
            title: r.get(1)?,
            size: r.get::<_, u32>(2)? as usize,
        })
    })
    .optional()
    .map_err(internal)
}

pub(super) fn slice(
    db: &Connection,
    revision: &Revision,
    range: std::ops::Range<usize>,
) -> Result<Vec<u8>> {
    if range.end > revision.size || range.end > crate::MAX_DOCUMENT_BYTES || range.start > range.end
    {
        return Err(internal("invalid search message byte range"));
    }
    let mut bytes = vec![0; range.len()];
    db.blob_open("main", "versions", "bytes", revision.rowid, true)
        .map_err(internal)?
        .read_at_exact(&mut bytes, range.start)
        .map_err(internal)?;
    Ok(bytes)
}
