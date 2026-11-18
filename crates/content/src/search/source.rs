//! Read the committed content snapshot without borrowing its writer.
use super::{Result, internal};
use calternal_fsio::beneath::Folder;
use rusqlite::{Connection, OptionalExtension, params};
use std::io::Read;

pub(super) const SLOTS: i64 = 250_000;
pub(super) const BODY_BYTES: usize = 65_536;

/// Current durable head. Its body is limited to the search projection's prefix.
pub(super) struct Head {
    pub(super) slot: i64,
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) title: String,
    pub(super) bytes: Vec<u8>,
}

const FIELDS: &str = "i.rowid,i.id,i.kind,
    replace(replace(substr(i.metadata->>'title',1,256),char(1),''),char(2),'')";
const VISIBLE: &str = "i.rowid<=250000 AND substr(i.path,1,7)<>'.trash/'
      AND i.mat_state NOT IN ('purging','purged')
      AND coalesce(i.metadata->>'deleted',0)<>1
      AND json_type(i.metadata,'$.title')='text'
      AND i.kind IN (SELECT value FROM json_each(?1))";

fn head(r: &rusqlite::Row<'_>, bytes: Vec<u8>) -> rusqlite::Result<Head> {
    Ok(Head {
        slot: r.get(0)?,
        id: r.get(1)?,
        kind: r.get(2)?,
        title: r.get(3)?,
        bytes,
    })
}

/// Read one bounded rowid page and use its exact staged revision on fallback.
/// The caller keeps a read snapshot through the page so foreground writes cannot
/// change which revision is selected between reading its metadata and bytes.
pub(super) fn scan(
    conn: &Connection,
    folder: Option<&Folder>,
    kinds: &str,
    after: i64,
    limit: usize,
) -> Result<Vec<Head>> {
    let mut stmt = conn
        .prepare_cached(&format!(
            "SELECT {FIELDS},i.path,i.file_hash,i.seq FROM items i NOT INDEXED
             WHERE {VISIBLE} AND i.rowid>?2
             AND EXISTS(SELECT 1 FROM versions v WHERE v.item=i.id AND v.seq=i.seq)
             ORDER BY i.rowid LIMIT ?3"
        ))
        .map_err(internal)?;
    let mut rows = stmt
        .query(params![kinds, after, limit as i64])
        .map_err(internal)?;
    let mut page = Vec::new();
    while let Some(row) = rows.next().map_err(internal)? {
        let mut document = head(row, Vec::new()).map_err(internal)?;
        if document.kind == "threads" {
            page.push(document);
            continue;
        }
        let path: String = row.get(4).map_err(internal)?;
        let hash: Option<String> = row.get(5).map_err(internal)?;
        document.bytes = match folder
            .zip(hash.as_deref())
            .and_then(|(folder, hash)| read_verified(folder, &path, hash))
        {
            Some(mut bytes) => {
                bytes.truncate(BODY_BYTES);
                bytes
            }
            None => conn
                .prepare_cached(
                    "SELECT coalesce(substr(bytes,1,65536),x'') FROM versions WHERE item=?1 AND seq=?2",
                )
                .map_err(internal)?
                .query_row(
                    params![document.id, row.get::<_, i64>(6).map_err(internal)?],
                    |r| r.get(0),
                )
                .map_err(internal)?,
        };
        page.push(document);
    }
    Ok(page)
}

pub(super) fn get(conn: &Connection, kinds: &str, id: &str) -> Result<Option<Head>> {
    conn.prepare_cached(&format!(
        "SELECT {FIELDS},CASE WHEN i.kind='threads' THEN x'' ELSE coalesce(substr(v.bytes,1,65536),x'') END
         FROM items i JOIN versions v ON v.item=i.id AND v.seq=i.seq WHERE {VISIBLE} AND i.id=?2"
    ))
    .map_err(internal)?
    .query_row(params![kinds, id], |row| head(row, row.get(4)?))
    .optional()
    .map_err(internal)
}

/// A regular file beneath the space with the full durable hash. Read a bounded
/// amount and hash before using the bytes in a derived projection.
pub(super) fn read_verified(folder: &Folder, path: &str, expected: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let maximum = crate::MAX_DOCUMENT_BYTES + 128;
    folder
        .open_file(path)
        .ok()?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > maximum || crate::file_hash(&bytes) != expected {
        return None;
    }
    Some(bytes)
}

pub(super) fn pending(conn: &Connection) -> Result<bool> {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM outbox)", [], |row| row.get(0))
        .map_err(internal)
}

/// Uses live plugin controls after authorization. The same filter applies to
/// hits and coverage, so indexing disabled kinds cannot affect the response.
pub(super) fn allowed(
    conn: &Connection,
    filters: &str,
    kinds: &str,
    space: calternal_types::SpaceId,
) -> Result<(String, bool)> {
    conn.prepare_cached(
        "WITH allowed(kind) AS (
            SELECT j.value->>'kind' FROM json_each(?1) j
            LEFT JOIN controls c ON c.key=j.value->>'control'
            WHERE CASE WHEN c.key IS NULL THEN j.value->>'default_enabled'
                       ELSE c.value->>'enabled'=1
                            AND json_type(c.value,'$.generation')='integer'
                            AND (CAST(c.value->>'generation' AS INTEGER) >> 32)>=j.value->>'approval'
                  END = 1
              AND (j.value->>'space' IS NULL OR j.value->>'space'=?4)
              AND (json_array_length(?2)=0 OR j.value->>'kind' IN (SELECT value FROM json_each(?2)))
         )
         SELECT (SELECT json_group_array(kind) FROM allowed),
                EXISTS(SELECT 1 FROM items WHERE rowid>?3 AND kind IN allowed)",
    )
    .map_err(internal)?
    .query_row(params![filters, kinds, SLOTS, space.to_string()], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })
    .map_err(internal)
}
