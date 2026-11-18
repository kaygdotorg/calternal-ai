//! Read the committed content snapshot without borrowing its writer.
use super::{Result, internal};
use rusqlite::{Connection, OptionalExtension, params};
use std::io::Read;
use std::path::Path;

pub(super) const SLOTS: i64 = 250_000;
pub(super) const BODY_BYTES: usize = 65_536;

/// Current durable head. Normal journal replay takes the existing title and
/// text from the version the save already staged.
pub(super) struct Head {
    pub(super) slot: i64,
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) title: String,
    pub(super) bytes: Vec<u8>,
    path: String,
    hash: Option<String>,
}

const HEAD: &str = "SELECT i.rowid,i.id,i.kind,
    replace(replace(substr(i.metadata->>'title',1,256),char(1),''),char(2),''),
    coalesce(substr(v.bytes,1,65536),x''),i.path,i.file_hash
    FROM items i JOIN versions v ON v.item=i.id AND v.seq=i.seq
    WHERE i.rowid<=250000 AND substr(i.path,1,7)<>'.trash/'
      AND i.mat_state NOT IN ('purging','purged')
      AND coalesce(i.metadata->>'deleted',0)<>1
      AND json_type(i.metadata,'$.title')='text'
      AND i.kind IN (SELECT value FROM json_each(?1))";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Head> {
    Ok(Head {
        slot: r.get(0)?,
        id: r.get(1)?,
        kind: r.get(2)?,
        title: r.get(3)?,
        bytes: r.get(4)?,
        path: r.get(5)?,
        hash: r.get(6)?,
    })
}

pub(super) fn scan(conn: &Connection, kinds: &str, after: i64, limit: usize) -> Result<Vec<Head>> {
    let mut stmt = conn
        .prepare_cached(&format!("{HEAD} AND i.rowid>?2 ORDER BY i.rowid LIMIT ?3"))
        .map_err(internal)?;
    let rows = stmt
        .query_map(params![kinds, after, limit as i64], row)
        .map_err(internal)?;
    rows.collect::<rusqlite::Result<_>>().map_err(internal)
}

pub(super) fn get(conn: &Connection, kinds: &str, id: &str) -> Result<Option<Head>> {
    conn.prepare_cached(&format!("{HEAD} AND i.id=?2"))
        .map_err(internal)?
        .query_row(params![kinds, id], row)
        .optional()
        .map_err(internal)
}

pub(super) fn pending(conn: &Connection) -> Result<bool> {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM outbox)", [], |row| row.get(0))
        .map_err(internal)
}

impl Head {
    /// Prefer the guarded materialized file when its bytes match the durable
    /// head. Concurrent placement, missing or externally edited files fall
    /// back to the durable version as they do for ordinary content reads.
    pub(super) fn recover_file(&mut self, root: &Path) {
        let Some(expected) = &self.hash else { return };
        let Ok(folder) = calternal_fsio::beneath::Folder::open(root) else {
            return;
        };
        let Ok(file) = folder.open_file(&self.path) else {
            return;
        };
        let mut bytes = Vec::new();
        let maximum = crate::MAX_DOCUMENT_BYTES + 128;
        if file
            .take(maximum as u64 + 1)
            .read_to_end(&mut bytes)
            .is_ok()
            && bytes.len() <= maximum
            && crate::file_hash(&bytes) == *expected
        {
            bytes.truncate(BODY_BYTES);
            self.bytes = bytes;
        }
    }
}

/// Uses live plugin controls after authorization. The same filter applies to
/// hits and coverage, so indexing disabled kinds cannot affect the response.
pub(super) fn allowed(conn: &Connection, filters: &str, kinds: &str) -> Result<(String, bool)> {
    conn.prepare_cached(
        "WITH allowed(kind) AS (
            SELECT j.value->>'kind' FROM json_each(?1) j
            LEFT JOIN controls c ON c.key=j.value->>'control'
            WHERE CASE WHEN c.key IS NULL THEN j.value->>'default_enabled'
                       ELSE c.value->>'enabled'=1
                            AND json_type(c.value,'$.generation')='integer'
                            AND (CAST(c.value->>'generation' AS INTEGER) >> 32)=j.value->>'approval'
                  END = 1
              AND (json_array_length(?2)=0 OR j.value->>'kind' IN (SELECT value FROM json_each(?2)))
         )
         SELECT (SELECT json_group_array(kind) FROM allowed),
                EXISTS(SELECT 1 FROM items WHERE rowid>?3 AND kind IN allowed)",
    )
    .map_err(internal)?
    .query_row(params![filters, kinds, SLOTS], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })
    .map_err(internal)
}
