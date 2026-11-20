//! Rebuild the keys of one item or one bounded page of collection members.

use super::replay::{PAGE, malformed_metadata, sidecar_error, sql_error};
use crate::Result;
use rusqlite::{Connection, OptionalExtension, params};

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

// Inherited over-limit members still have their ordinary Notes or Trash row.
const OMIT_COLLECTIONS: &str = "WHERE NOT ?2 OR audience NOT IN ('explicit','explicit_newest')";

pub(super) fn item(conn: &Connection, id: &str) -> Result<bool> {
    let (valid, excess): (bool, bool) = conn
        .prepare_cached(
            "SELECT json_valid(metadata), kind='notes' AND mat_state NOT IN ('purging','purged')
          AND EXISTS(SELECT 1 FROM note_collection_members WHERE note_id=i.id LIMIT 1 OFFSET 31)
         FROM items i WHERE i.id=?1",
        )
        .map_err(sql_error)?
        .query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()
        .map_err(sql_error)?
        .unwrap_or((true, false));
    if !valid {
        return Err(malformed_metadata());
    }
    let newly_skipped = if excess {
        conn.execute(
            "INSERT OR IGNORE INTO main.window_skipped VALUES(?1,'effective_scope_limit_exceeded')",
            [id],
        )
        .map_err(sidecar_error)?
            == 1
    } else {
        conn.execute("DELETE FROM main.window_skipped WHERE item=?1", [id])
            .map_err(sidecar_error)?;
        false
    };

    static DELETE: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        format!(
            "DELETE FROM window_rows WHERE item=?1 AND (collection,audience) NOT IN
         (SELECT collection,audience FROM ({ITEM_ROWS}) {OMIT_COLLECTIONS})"
        )
    });
    static PUT: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        format!("INSERT INTO window_rows SELECT * FROM ({ITEM_ROWS}) {OMIT_COLLECTIONS} {UPSERT}")
    });
    conn.prepare_cached(&DELETE)
        .map_err(sql_error)?
        .execute(params![id, excess])
        .map_err(sql_error)?;
    conn.prepare_cached(&PUT)
        .map_err(sql_error)?
        .execute(params![id, excess])
        .map_err(sql_error)?;
    Ok(newly_skipped)
}

pub(super) fn is_collection(conn: &Connection, id: &str) -> Result<bool> {
    conn.prepare_cached("SELECT EXISTS(SELECT 1 FROM items WHERE id=?1 AND kind='collections')")
        .map_err(sql_error)?
        .query_row([id], |row| row.get(0))
        .map_err(sql_error)
}

pub(super) fn collection_page(
    conn: &Connection,
    id: &str,
    after: &str,
    skipped: &mut Vec<String>,
) -> Result<Option<String>> {
    // The source range admits new edges and the sidecar range locates old
    // edges for deletion. Each branch seeks by collection and item first.
    let mut stmt = conn
        .prepare_cached(
            "SELECT item FROM (
           SELECT item FROM (SELECT note_id AS item FROM note_collection_members
             WHERE collection_id=?1 AND note_id>?2 ORDER BY note_id LIMIT ?3)
           UNION
           SELECT item FROM (SELECT item FROM window_rows INDEXED BY window_members
             WHERE collection=?1 AND item>?2 AND audience IN ('explicit','explicit_newest')
             ORDER BY item LIMIT ?3)
           UNION
           SELECT item FROM (SELECT item FROM window_skipped WHERE item>?2 ORDER BY item LIMIT ?3)
         ) ORDER BY item LIMIT ?3",
        )
        .map_err(sql_error)?;
    let ids = stmt
        .query_map(params![id, after, PAGE as i64], |row| {
            row.get::<_, String>(0)
        })
        .map_err(sql_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(sql_error)?;
    for note in &ids {
        if item(conn, note)? {
            skipped.push(note.clone());
        }
    }
    Ok((ids.len() == PAGE).then(|| ids.last().unwrap().clone()))
}
