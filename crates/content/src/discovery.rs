//! Per-space discovery over item heads. Projections share the content transaction.
use crate::{Content, Result, Summary, types::internal};
use calternal_types::SpaceId;
use rusqlite::{Connection, params};
use uuid::Uuid;

// A label is a name, not an item. The primary key groups it by kind and
// counts each matching head once. Trailing label keys support item updates.
pub(crate) fn install(conn: &Connection) -> Result<()> {
    let needs_rebuild: bool = conn.query_row(
        "SELECT NOT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='item_terms')",
        [], |r| r.get(0),
    ).map_err(internal)?;
    let tx = conn.unchecked_transaction().map_err(internal)?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS item_terms(
            kind TEXT NOT NULL, category TEXT NOT NULL, name TEXT NOT NULL, item TEXT NOT NULL REFERENCES items(id),
            PRIMARY KEY(kind,category,name,item)
        ) WITHOUT ROWID, STRICT;
        CREATE INDEX IF NOT EXISTS item_term_item ON item_terms(item);
        CREATE INDEX IF NOT EXISTS item_live_list
            ON items(kind,updated_at DESC,id) WHERE substr(path,1,7) <> '.trash/';
        CREATE INDEX IF NOT EXISTS item_trash
            ON items(kind,updated_at,id) WHERE substr(path,1,7) = '.trash/';
        CREATE INDEX IF NOT EXISTS item_title_key
            ON items(kind,coalesce(json_extract(metadata,'$.title_key'),lower(json_extract(metadata,'$.title'))),id)
            WHERE substr(path,1,7) <> '.trash/';
        CREATE TRIGGER IF NOT EXISTS item_terms_insert AFTER INSERT ON items
        WHEN substr(new.path,1,7) <> '.trash/'
        BEGIN
            INSERT INTO item_terms SELECT DISTINCT new.kind, 'label', value, new.id
                FROM json_each(new.metadata,'$.labels') WHERE type='text';
            INSERT INTO item_terms SELECT DISTINCT new.kind, 'link', value, new.id
                FROM json_each(new.metadata,'$.links') WHERE type='text';
        END;
        CREATE TRIGGER IF NOT EXISTS item_terms_update AFTER UPDATE OF metadata,path ON items
        BEGIN
            DELETE FROM item_terms WHERE item=old.id;
            INSERT INTO item_terms SELECT DISTINCT new.kind, 'label', value, new.id
                FROM json_each(new.metadata,'$.labels')
                WHERE type='text' AND substr(new.path,1,7) <> '.trash/';
            INSERT INTO item_terms SELECT DISTINCT new.kind, 'link', value, new.id
                FROM json_each(new.metadata,'$.links')
                WHERE type='text' AND substr(new.path,1,7) <> '.trash/';
        END;",
    )
    .map_err(internal)?;
    if needs_rebuild {
        tx.execute_batch(
            "INSERT INTO item_terms SELECT DISTINCT i.kind,'label',j.value,i.id
             FROM items i, json_each(i.metadata,'$.labels') j
             WHERE j.type='text' AND substr(i.path,1,7) <> '.trash/';
             INSERT INTO item_terms SELECT DISTINCT i.kind,'link',j.value,i.id
             FROM items i, json_each(i.metadata,'$.links') j
             WHERE j.type='text' AND substr(i.path,1,7) <> '.trash/';",
        )
        .map_err(internal)?;
    }
    tx.commit().map_err(internal)
}

// An upper bound under SQLite's binary collation. Increment the last scalar
// with a successor and discard the suffix, skipping the surrogate gap.
fn prefix_end(prefix: &str) -> Option<String> {
    for (index, ch) in prefix.char_indices().rev() {
        let next = u32::from(ch) + 1;
        if let Some(ch) = char::from_u32(if next == 0xd800 { 0xe000 } else { next }) {
            let mut end = prefix[..index].to_owned();
            end.push(ch);
            return Some(end);
        }
    }
    None
}

impl Content {
    /// Read current, visible item summaries using one indexed title-prefix or label
    /// query in an authorized space. Exactly one selector must be set. No userdata
    /// reads, and at most 100 result rows.
    pub fn discover(
        &self,
        space: SpaceId,
        kind: &str,
        title_prefix: Option<&str>,
        label: Option<&str>,
    ) -> Result<Vec<Summary>> {
        self.with_space(space, |s| {
            let end = title_prefix.and_then(prefix_end);
            let (sql, selector) = match (title_prefix, label) {
                (Some(prefix), None) => {
                    let sql = if end.is_some() {
                        "SELECT id,metadata,revision,updated_at,path FROM items
                         WHERE kind=?1 AND substr(path,1,7) <> '.trash/'
                         AND coalesce(json_extract(metadata,'$.title_key'),lower(json_extract(metadata,'$.title'))) >= ?2
                         AND coalesce(json_extract(metadata,'$.title_key'),lower(json_extract(metadata,'$.title'))) < ?3
                         ORDER BY coalesce(json_extract(metadata,'$.title_key'),lower(json_extract(metadata,'$.title'))),id LIMIT 100"
                    } else {
                        "SELECT id,metadata,revision,updated_at,path FROM items
                         WHERE kind=?1 AND substr(path,1,7) <> '.trash/'
                         AND coalesce(json_extract(metadata,'$.title_key'),lower(json_extract(metadata,'$.title'))) >= ?2
                         AND ?3 IS NULL
                         ORDER BY coalesce(json_extract(metadata,'$.title_key'),lower(json_extract(metadata,'$.title'))),id LIMIT 100"
                    };
                    (sql, prefix)
                }
                (None, Some(label)) => (
                    "SELECT i.id,i.metadata,i.revision,i.updated_at,i.path
                     FROM item_terms l JOIN items i ON i.id=l.item
                     WHERE l.kind=?1 AND l.category='label' AND l.name=?2 AND ?3 IS NULL
                     ORDER BY i.updated_at DESC,i.id LIMIT 100",
                    label,
                ),
                _ => return Err(crate::Error::invalid_input("select either a title prefix or a label")),
            };
            let mut stmt = s.conn.prepare_cached(sql).map_err(internal)?;
            let values = params![kind, selector, end];
            let rows = stmt.query_map(values, |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            }).map_err(internal)?;
            rows.map(|row| {
                let (id, metadata, revision, updated_at, path) = row.map_err(internal)?;
                Ok(Summary {
                    id: Uuid::parse_str(&id).map_err(internal)?,
                    space,
                    kind: kind.into(),
                    path,
                    metadata: serde_json::from_str(&metadata).map_err(internal)?,
                    revision,
                    updated_at,
                })
            }).collect()
        })
    }

    /// Read up to 100 labels and counts, in name order, using one indexed prefix
    /// query in an authorized space. Counts include only current visible items.
    pub fn labels(&self, space: SpaceId, kind: &str, prefix: &str) -> Result<Vec<(String, u32)>> {
        self.with_space(space, |s| {
            let end = prefix_end(prefix);
            let mut stmt = s.conn.prepare_cached(if end.is_some() {
                "SELECT name,count(*) FROM item_terms WHERE kind=?1 AND category='label' AND name>=?2 AND name<?3 GROUP BY name ORDER BY name LIMIT 100"
            } else {
                "SELECT name,count(*) FROM item_terms WHERE kind=?1 AND category='label' AND name>=?2 AND ?3 IS NULL GROUP BY name ORDER BY name LIMIT 100"
            }).map_err(internal)?;
            let rows = stmt.query_map(params![kind, prefix, end], |r| Ok((r.get(0)?,r.get(1)?))).map_err(internal)?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(internal)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_prefix_ranges_cover_unicode() {
        assert_eq!(prefix_end("abc").as_deref(), Some("abd"));
        assert_eq!(prefix_end("a\u{10ffff}").as_deref(), Some("b"));
        assert_eq!(prefix_end("\u{d7ff}").as_deref(), Some("\u{e000}"));
        assert_eq!(prefix_end(""), None);
    }
}
