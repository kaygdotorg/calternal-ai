//! Per-space discovery over item heads. Projections share the content transaction.
use crate::{Content, Error, Result, Summary, db::Space, types::internal};
use calternal_types::{Page, SpaceId};
use rusqlite::{Connection, Row, params};
use uuid::Uuid;

// A label is a name, not an item. The primary key groups it by kind and
// counts each matching head once. Trailing label keys support item updates.
pub(crate) fn install(conn: &Connection) -> Result<()> {
    // application_id is the generation of these replaceable indexes in
    // content.db. The authoritative schema has its own user_version.
    const GENERATION: i64 = 1;
    let generation = conn
        .pragma_query_value(None, "application_id", |r| r.get::<_, i64>(0))
        .map_err(internal)?;
    if generation == GENERATION {
        return Ok(());
    }
    let tx = conn.unchecked_transaction().map_err(internal)?;
    tx.execute_batch(
        "DROP TRIGGER IF EXISTS item_terms_insert;
         DROP TRIGGER IF EXISTS item_terms_update;
         DROP TABLE IF EXISTS item_terms;
         DROP INDEX IF EXISTS item_list;
         DROP INDEX IF EXISTS item_live_list;
         DROP INDEX IF EXISTS item_trash;
         DROP INDEX IF EXISTS item_trash_page;
         DROP INDEX IF EXISTS item_title_key;",
    )
    .map_err(internal)?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS item_terms(
            kind TEXT NOT NULL, category TEXT NOT NULL, name TEXT NOT NULL, item TEXT NOT NULL REFERENCES items(id),
            PRIMARY KEY(kind,category,name,item)
        ) WITHOUT ROWID, STRICT;
        CREATE INDEX IF NOT EXISTS item_term_item ON item_terms(item);
        CREATE INDEX IF NOT EXISTS item_list ON items(kind,updated_at DESC,id);
        CREATE INDEX IF NOT EXISTS item_live_list
            ON items(kind,updated_at DESC,id) WHERE substr(path,1,7) <> '.trash/';
        CREATE INDEX IF NOT EXISTS item_trash
            ON items(kind,updated_at,id) WHERE substr(path,1,7) = '.trash/';
        CREATE INDEX IF NOT EXISTS item_trash_page
            ON items(kind,updated_at DESC,id)
            WHERE substr(path,1,7) = '.trash/' AND mat_state NOT IN ('purging','purged');
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
    tx.execute_batch(
        "INSERT INTO item_terms SELECT DISTINCT i.kind,'label',j.value,i.id
             FROM items i, json_each(i.metadata,'$.labels') j
             WHERE j.type='text' AND substr(i.path,1,7) <> '.trash/';
             INSERT INTO item_terms SELECT DISTINCT i.kind,'link',j.value,i.id
             FROM items i, json_each(i.metadata,'$.links') j
             WHERE j.type='text' AND substr(i.path,1,7) <> '.trash/';",
    )
    .map_err(internal)?;
    tx.pragma_update(None, "application_id", GENERATION)
        .map_err(internal)?;
    tx.commit().map_err(internal)
}

// An upper bound under SQLite's binary collation. Increment the last scalar
// with a successor and discard the suffix, skipping the surrogate gap.
pub(crate) fn prefix_end(prefix: &str) -> Option<String> {
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

/// Indexed order used for a page of current item summaries.
pub enum ItemOrder<'a> {
    /// Live items by descending update timestamp, then ascending item UUID.
    Updated {
        /// Exact indexed label, including descendants projected to this name.
        label: Option<&'a str>,
    },
    /// Trash items by descending move timestamp, then ascending item UUID.
    Trash,
    /// Live items by folded title and ascending item UUID under this prefix.
    Title(&'a str),
}

const TITLE_KEY: &str =
    "coalesce(json_extract(metadata,'$.title_key'),lower(json_extract(metadata,'$.title')))";

const UPDATED_PAGE: &str =
    "SELECT i.id,i.metadata,i.revision,i.updated_at,i.path,i.updated_at FROM items i
                         WHERE i.kind=?1 AND substr(i.path,1,7) <> '.trash/'
                         AND i.updated_at<=?2 AND (i.updated_at<?2 OR i.id>?3)
                         AND (?4 IS NULL OR EXISTS (
                             SELECT 1 FROM item_terms l WHERE l.kind=?1 AND l.category='label'
                             AND l.name=?4 AND l.item=i.id
                         ))
                         ORDER BY i.updated_at DESC,i.id LIMIT ?5";
const TRASH_PAGE: &str = "SELECT id,metadata,revision,updated_at,path,updated_at FROM items
                         WHERE kind=?1 AND substr(path,1,7) = '.trash/' AND mat_state NOT IN ('purging','purged')
                         AND updated_at<=?2 AND (updated_at<?2 OR id>?3) AND ?4 IS NULL
                         ORDER BY updated_at DESC,id LIMIT ?5";
const LABEL_RANGE_PAGE: &str = "SELECT name,count(*) FROM item_terms WHERE kind=?1 AND category='label' AND name>=?2 AND (NOT ?3 OR name>?2) AND name<?4 GROUP BY name ORDER BY name LIMIT ?5";
const LABEL_TAIL_PAGE: &str = "SELECT name,count(*) FROM item_terms WHERE kind=?1 AND category='label' AND name>=?2 AND (NOT ?3 OR name>?2) AND ?4 IS NULL GROUP BY name ORDER BY name LIMIT ?5";

// Keep the title expression in sync with the index declaration above.
fn title_queries() -> &'static [String; 2] {
    static SQL: std::sync::OnceLock<[String; 2]> = std::sync::OnceLock::new();
    SQL.get_or_init(|| {
        [
            format!(
                "SELECT id,metadata,revision,updated_at,path,{TITLE_KEY} FROM items
         WHERE kind=?1 AND substr(path,1,7) <> '.trash/'
         AND json_extract(metadata,'$.deleted') IS NOT 1
         AND {TITLE_KEY}>=?2 AND ({TITLE_KEY},id)>(?2,?3) AND {TITLE_KEY}<?4
         ORDER BY {TITLE_KEY},id LIMIT ?5"
            ),
            format!(
                "SELECT id,metadata,revision,updated_at,path,{TITLE_KEY} FROM items
         WHERE kind=?1 AND substr(path,1,7) <> '.trash/'
         AND json_extract(metadata,'$.deleted') IS NOT 1
         AND {TITLE_KEY}>=?2 AND ({TITLE_KEY},id)>(?2,?3) AND ?4 IS NULL
         ORDER BY {TITLE_KEY},id LIMIT ?5"
            ),
        ]
    })
}

impl Space {
    pub(crate) fn item_page(
        &self,
        kind: &str,
        order: ItemOrder<'_>,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<Page<Summary>> {
        let position: Option<(String, String)> = cursor
            .map(serde_json::from_str)
            .transpose()
            .map_err(|_| Error::invalid_input("invalid page cursor"))?;
        let (sql, first, id, other) = match order {
            ItemOrder::Title(prefix) => {
                let end = prefix_end(prefix);
                let sql = &title_queries()[usize::from(end.is_none())];
                let (first, id) = position.unwrap_or_else(|| (prefix.to_owned(), String::new()));
                (sql.as_str(), first, id, end)
            }
            order => {
                let (sql, label) = match order {
                    ItemOrder::Updated { label } => (UPDATED_PAGE, label),
                    ItemOrder::Trash => (TRASH_PAGE, None),
                    ItemOrder::Title(_) => unreachable!(),
                };
                // Persisted timestamps are UTC RFC 3339 strings beginning with a digit.
                let (first, id) = position.unwrap_or_else(|| ("~".into(), String::new()));
                (sql, first, id, label.map(str::to_owned))
            }
        };
        let mut stmt = self.conn.prepare_cached(sql).map_err(internal)?;
        let rows = stmt
            .query_map(params![kind, first, id, other, (limit + 1) as i64], |row| {
                let summary = read_head(row)?;
                Ok((summary, row.get::<_, String>(5)?))
            })
            .map_err(internal)?;
        let rows = rows
            .map(|row| {
                let ((id, metadata, revision, updated_at, path), key) = row.map_err(internal)?;
                Ok((
                    Summary {
                        id: Uuid::parse_str(&id).map_err(internal)?,
                        space: self.id,
                        kind: kind.into(),
                        path,
                        metadata: serde_json::from_str(&metadata).map_err(internal)?,
                        revision,
                        updated_at,
                    },
                    key,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let page = Page::from_lookahead(rows, limit, |(row, key)| {
            serde_json::json!([key, row.id]).to_string()
        });
        Ok(Page {
            rows: page.rows.into_iter().map(|(row, _)| row).collect(),
            next: page.next,
        })
    }
}

fn read_head(row: &Row<'_>) -> rusqlite::Result<(String, String, String, String, String)> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
    ))
}

impl Content {
    /// Read one indexed page of item summaries in an authorized space, up to
    /// 1,000 rows. Takes the space lock. An unfiltered newest Notes page seeks the
    /// covering window index and reads up to one metadata head per returned key.
    /// Label filters probe the exact term key per candidate. No body reads.
    pub fn item_page(
        &self,
        space: SpaceId,
        kind: &str,
        order: ItemOrder<'_>,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<Page<Summary>> {
        self.with_space(space, |s| {
            s.item_page(kind, order, cursor, limit.clamp(1, 1000))
        })
    }

    /// Read up to the requested number of label names and current note counts
    /// through the term index, capped at 1,000 names. One space-local query.
    pub fn label_page(
        &self,
        space: SpaceId,
        kind: &str,
        prefix: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<Page<(String, u32)>> {
        self.with_space(space, |s| {
            let end = prefix_end(prefix);
            let start = cursor.unwrap_or(prefix);
            let limit = limit.clamp(1, 1000);
            let mut stmt = s
                .conn
                .prepare_cached(if end.is_some() {
                    LABEL_RANGE_PAGE
                } else {
                    LABEL_TAIL_PAGE
                })
                .map_err(internal)?;
            let rows = stmt
                .query_map(
                    params![kind, start, cursor.is_some(), end, (limit + 1) as i64],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(internal)?;
            let rows = rows
                .collect::<rusqlite::Result<Vec<(String, u32)>>>()
                .map_err(internal)?;
            Ok(Page::from_lookahead(rows, limit, |row| row.0.clone()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_projection_rebuilds_terms_from_retained_heads() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE items(id TEXT PRIMARY KEY,kind TEXT,metadata TEXT,revision TEXT,
              updated_at TEXT,path TEXT,mat_state TEXT);
             INSERT INTO items VALUES ('x','notes','{\"labels\":[\"kept\"]}','rev','date','Notes/X.md','materialized');",
        ).unwrap();
        install(&conn).unwrap();
        conn.execute_batch("DELETE FROM item_terms; PRAGMA application_id=9;")
            .unwrap();
        install(&conn).unwrap();
        let (label, rev): (String, String) = conn
            .query_row(
                "SELECT item_terms.name,items.revision FROM item_terms JOIN items ON id=item",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((label.as_str(), rev.as_str()), ("kept", "rev"));
    }

    #[test]
    fn page_statements_seek_an_ordered_index_without_sorting() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE items(id TEXT PRIMARY KEY,kind TEXT,metadata TEXT,revision TEXT,
              updated_at TEXT,path TEXT,mat_state TEXT);
             CREATE INDEX item_list ON items(kind,updated_at DESC,id);",
        )
        .unwrap();
        install(&conn).unwrap();
        let cases = [
            (UPDATED_PAGE, "item_live_list", "updated_at"),
            (TRASH_PAGE, "item_trash_page", "updated_at"),
            (&title_queries()[0], "item_title_key", "<expr>"),
            (&title_queries()[1], "item_title_key", "<expr>"),
            (LABEL_RANGE_PAGE, "PRIMARY KEY", "name"),
            (LABEL_TAIL_PAGE, "PRIMARY KEY", "name"),
        ];
        for (sql, index, ranged_key) in cases {
            let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
            let plan = stmt
                .query_map(params!["notes", "k", "", Some("z"), 101], |row| {
                    row.get::<_, String>(3)
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
                .join("\n");
            assert!(
                plan.contains("SEARCH")
                    && plan.contains(index)
                    && (plan.contains(&format!("{ranged_key}>?"))
                        || plan.contains(&format!("{ranged_key}<?"))),
                "{plan}"
            );
            assert!(!plan.contains("TEMP B-TREE"), "{plan}");
        }
    }

    #[test]
    fn binary_prefix_ranges_cover_unicode() {
        assert_eq!(prefix_end("abc").as_deref(), Some("abd"));
        assert_eq!(prefix_end("a\u{10ffff}").as_deref(), Some("b"));
        assert_eq!(prefix_end("\u{d7ff}").as_deref(), Some("\u{e000}"));
        assert_eq!(prefix_end(""), None);
    }
}
