//! Indexed reads of byte ranges in a live item's immutable versions.
use crate::{
    Content, Error, Result, SpaceId,
    types::{internal, missing},
};
use calternal_types::RevisionRef;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use std::ops::Range;
use uuid::Uuid;

const MAX_PAGE_BYTES: usize = 64 * 1024;

impl Content {
    /// Read a bounded page from one immutable revision of a live item.
    /// The broker checks the caller's exact revision pin and body authority
    /// before the read and before delivery. Wrong kind, Trash, purge and absent
    /// revisions are opaque not-found. An offset at or beyond EOF returns an
    /// empty page. Cost: one indexed lookup and at most one BLOB read on the
    /// selected space connection. Call on a blocking worker.
    pub fn read_revision_page(
        &self,
        space: SpaceId,
        item: Uuid,
        kind: &str,
        revision: &RevisionRef,
        offset: u64,
        length: usize,
    ) -> Result<(Vec<u8>, Option<usize>)> {
        if !(1..=MAX_PAGE_BYTES).contains(&length) {
            return Err(Error::invalid_input("invalid revision byte range"));
        }
        self.with_space(space, |s| {
            let (rowid, size) = s
                .conn
                .prepare_cached(
                    "SELECT v.rowid, length(v.bytes)
                 FROM versions v JOIN items i ON i.id = v.item
                 WHERE v.revision = ?1 AND v.item = ?2 AND i.kind = ?3
                   AND i.mat_state NOT IN ('purged','purging')
                   AND substr(i.path, 1, 7) <> '.trash/'",
                )
                .map_err(internal)?
                .query_row(params![revision.0, item.to_string(), kind], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
                })
                .optional()
                .map_err(internal)?
                .ok_or_else(missing)?;
            let size = usize::try_from(size).map_err(internal)?;
            let start = usize::try_from(offset).unwrap_or(usize::MAX).min(size);
            let end = start.saturating_add(length).min(size);
            let bytes = read_bytes(&s.conn, rowid, start..end)?;
            Ok((bytes, (end < size).then_some(end)))
        })
    }

    /// Return version metadata and exactly the requested byte range if this
    /// live item has the given adjacent revisions. Callers authorize the space
    /// and kind first. An unavailable pair returns None. Cost: one indexed
    /// lookup and at most one incremental SQLite BLOB read, with no userdata I/O.
    pub fn revision_slice(
        &self,
        space: SpaceId,
        item: Uuid,
        kind: &str,
        base: &str,
        revision: &str,
        range: Range<usize>,
    ) -> Result<Option<(Value, Vec<u8>)>> {
        if range.start > range.end || range.end > crate::MAX_DOCUMENT_BYTES {
            return Err(Error::invalid_input("invalid revision byte range"));
        }
        self.with_space(space, |s| {
            let row = s
                .conn
                .prepare_cached(
                    "SELECT v.metadata, v.rowid
                     FROM versions v
                     JOIN items i ON i.id = v.item
                     JOIN versions b ON b.item = v.item AND b.seq = v.seq - 1
                     WHERE v.revision = ?1 AND v.item = ?2 AND i.kind = ?3
                       AND i.mat_state NOT IN ('purged','purging')
                       AND substr(i.path, 1, 7) <> '.trash/'
                       AND b.revision = ?4 AND length(v.bytes) >= ?5",
                )
                .map_err(internal)?
                .query_row(
                    params![revision, item.to_string(), kind, base, range.end as i64],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                )
                .optional()
                .map_err(internal)?;
            let Some((meta, rowid)) = row else {
                return Ok(None);
            };
            Ok(Some((
                serde_json::from_str(&meta).map_err(internal)?,
                read_bytes(&s.conn, rowid, range)?,
            )))
        })
    }
}

fn read_bytes(connection: &Connection, rowid: i64, range: Range<usize>) -> Result<Vec<u8>> {
    let mut bytes = vec![0; range.len()];
    if !bytes.is_empty() {
        connection
            .blob_open("main", "versions", "bytes", rowid, true)
            .map_err(internal)?
            .read_at_exact(&mut bytes, range.start)
            .map_err(internal)?;
    }
    Ok(bytes)
}
