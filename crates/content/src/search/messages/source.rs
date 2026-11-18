//! Read only committed heads and exact revision byte ranges.
use super::{Result, internal};
use crate::thread_append::revision::Revision as Bytes;
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
    seq: i64,
    patched: bool,
}

fn head(row: &rusqlite::Row<'_>) -> rusqlite::Result<Head> {
    Ok(Head {
        slot: row.get(0)?,
        id: row.get(1)?,
        revision: row.get(2)?,
        path: row.get(3)?,
        hash: row.get(4)?,
        version: row.get(5)?,
        seq: row.get(6)?,
        patched: row.get(7)?,
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

const FIELDS: &str = "i.rowid,i.id,i.revision,i.path,i.file_hash,v.rowid,v.seq,p.seq IS NOT NULL";
const JOIN: &str = "FROM items i JOIN versions v ON v.item=i.id AND v.seq=i.seq
    LEFT JOIN thread_revision_patches p ON p.item=v.item AND p.seq=v.seq";

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
    if head.patched {
        let bytes = Bytes::get(db, head.id.clone(), head.seq)?
            .ok_or_else(|| internal("saved revision missing"))?;
        return bytes.read(db, 0..bytes.size);
    }
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
    bytes: Bytes,
    pub(super) title: String,
}

pub(super) fn revision(db: &Connection, thread: Uuid, revision: &str) -> Result<Option<Revision>> {
    db.prepare_cached(&format!(
        "SELECT v.rowid,i.metadata->>'title',coalesce(p.offset,0)+length(v.bytes),
                v.seq,coalesce(p.base_seq,v.seq) {JOIN}
         WHERE {LIVE} AND i.id=?1 AND i.revision=?2"
    ))
    .map_err(internal)?
    .query_row(params![thread.to_string(), revision], |r| {
        Ok(Revision {
            bytes: Bytes {
                item: thread.to_string(),
                rowid: r.get(0)?,
                size: r.get::<_, u32>(2)? as usize,
                seq: r.get(3)?,
                base_seq: r.get(4)?,
            },
            title: r.get(1)?,
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
    if range.end > crate::MAX_THREAD_BYTES {
        return Err(internal("invalid search message byte range"));
    }
    revision.bytes.read(db, range)
}

/// A steady append advances a ready projection with only newly appended
/// frames. A replaced base, pruned version or unusually long backlog needs
/// the ordinary rebuild of that one thread.
pub(super) fn fragments(
    db: &Connection,
    head: &Head,
    prior: &str,
) -> Result<Option<Vec<(usize, Vec<u8>)>>> {
    const MAX_FRAGMENTS: usize = 64;
    if !head.patched {
        return Ok(None);
    }
    let Some(current) = Bytes::get(db, head.id.clone(), head.seq)? else {
        return Ok(None);
    };
    let old: Option<i64> = db
        .prepare_cached("SELECT seq FROM versions WHERE item=?1 AND revision=?2")
        .map_err(internal)?
        .query_row(params![head.id, prior], |r| r.get(0))
        .optional()
        .map_err(internal)?;
    let Some(old) = old.filter(|seq| {
        *seq >= current.base_seq && *seq < current.seq && current.seq - *seq <= MAX_FRAGMENTS as i64
    }) else {
        return Ok(None);
    };
    db.prepare_cached(
        "SELECT p.offset,v.bytes FROM thread_revision_patches p JOIN versions v USING(item,seq)
         WHERE p.item=?1 AND p.base_seq=?2 AND p.seq>?3 AND p.seq<=?4 ORDER BY p.seq",
    )
    .map_err(internal)?
    .query_map(params![head.id, current.base_seq, old, current.seq], |r| {
        Ok((r.get::<_, u32>(0)? as usize, r.get(1)?))
    })
    .map_err(internal)?
    .collect::<rusqlite::Result<_>>()
    .map(Some)
    .map_err(internal)
}
