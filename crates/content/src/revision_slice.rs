//! Indexed reads of byte ranges in a live item's immutable versions.
use crate::{Content, Error, Result, SpaceId, types::internal};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;
use std::ops::Range;
use uuid::Uuid;

impl Content {
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
            let mut bytes = vec![0; range.len()];
            if !bytes.is_empty() {
                s.conn
                    .blob_open("main", "versions", "bytes", rowid, true)
                    .map_err(internal)?
                    .read_at_exact(&mut bytes, range.start)
                    .map_err(internal)?;
            }
            Ok(Some((
                serde_json::from_str(&meta).map_err(internal)?,
                bytes,
            )))
        })
    }
}
