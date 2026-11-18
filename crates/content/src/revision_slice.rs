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
        if range.start > range.end || range.end > crate::MAX_THREAD_BYTES {
            return Err(Error::invalid_input("invalid revision byte range"));
        }
        self.with_space(space, |s| {
            let row = s
                .conn
                .prepare_cached(
                    "SELECT v.metadata,v.rowid,v.seq,coalesce(p.base_seq,v.seq),coalesce(p.offset,0)+length(v.bytes)
                     FROM versions v
                     JOIN items i ON i.id = v.item
                     JOIN versions b ON b.item = v.item AND b.seq = v.seq - 1
                     LEFT JOIN thread_revision_patches p ON p.item=v.item AND p.seq=v.seq
                     WHERE v.revision = ?1 AND v.item = ?2 AND i.kind = ?3
                       AND i.mat_state NOT IN ('purged','purging')
                       AND substr(i.path, 1, 7) <> '.trash/'
                       AND b.revision = ?4 AND coalesce(p.offset,0)+length(v.bytes) >= ?5",
                )
                .map_err(internal)?
                .query_row(
                    params![revision, item.to_string(), kind, base, range.end as i64],
                    |row| Ok((
                        row.get::<_, String>(0)?,
                        crate::thread_append::revision::Revision {
                            item: item.to_string(), rowid: row.get(1)?, seq: row.get(2)?,
                            base_seq: row.get(3)?, size: row.get::<_, u32>(4)? as usize,
                        },
                    )),
                )
                .optional()
                .map_err(internal)?;
            let Some((meta, revision)) = row else {
                return Ok(None);
            };
            let bytes = revision.read(&s.conn, range)?;
            Ok(Some((
                serde_json::from_str(&meta).map_err(internal)?,
                bytes,
            )))
        })
    }
}
