//! Covering index locators and bounded row materialization.

use super::{Key, WindowAnchor, WindowPlanReason, limits};
use crate::{Result, types::internal};
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Default)]
pub(super) struct Scope {
    pub(super) total: u64,
    pub(super) version: String,
}

pub(super) fn scope(conn: &Connection, scope: &(String, &str)) -> Result<Scope> {
    conn.prepare_cached(
        "SELECT total,version FROM window_scopes WHERE (collection,audience)=(?1,?2)",
    )
    .map_err(internal)?
    .query_row(params![scope.0, scope.1], |r| {
        Ok(Scope {
            total: r.get::<_, i64>(0)? as u64,
            version: r.get(1)?,
        })
    })
    .optional()
    .map_err(internal)
    .map(|scope| scope.unwrap_or_default())
}

const FORWARD: &str = "SELECT sort_key,item FROM window_rows INDEXED BY window_cover
    WHERE (collection,audience)=(?1,?2)
    ORDER BY sort_key,item LIMIT ?3 OFFSET ?4";

const REVERSE: &str = "SELECT sort_key,item FROM window_rows INDEXED BY window_cover
    WHERE (collection,audience)=(?1,?2)
    ORDER BY sort_key DESC,item DESC LIMIT ?3 OFFSET ?4";

const SEEK_REVERSE: &str = "SELECT sort_key,item FROM window_rows INDEXED BY window_cover
    WHERE (collection,audience)=(?1,?2) AND (sort_key,item)<(?4,?5)
    ORDER BY sort_key DESC,item DESC LIMIT ?3";

const SEEK: &str = "SELECT sort_key,item FROM window_rows INDEXED BY window_cover
    WHERE (collection,audience)=(?1,?2) AND (sort_key,item)>(?4,?5)
    ORDER BY sort_key,item LIMIT ?3";

#[derive(Debug, PartialEq)]
enum Scan<'a> {
    Forward(u64),
    Reverse(u64),
    Seek(&'a (Key, String), bool),
}

pub(super) struct Plan<'a> {
    scan: Scan<'a>,
    pub(super) take: u64,
    reversed: bool,
}

impl<'a> Plan<'a> {
    pub(super) fn new(
        total: u64,
        from: u64,
        width: u64,
        anchor: WindowAnchor,
        exact: bool,
        boundary: Option<&'a (Key, String)>,
        reversed: bool,
    ) -> std::result::Result<Self, WindowPlanReason> {
        use WindowPlanReason::*;
        let take = width.min(total.saturating_sub(from));
        if take > limits().live_limits.materialized_rows_per_window {
            return Err(MaterializationLimit);
        }
        let scan = if let Some(boundary) = boundary {
            Scan::Seek(boundary, reversed)
        } else if exact && take == 0 {
            Scan::Forward(0)
        } else {
            let opposite = total.saturating_sub(from + take);
            let flip = exact && opposite < from;
            let skip = if flip { opposite } else { from };
            let reverse = (anchor == WindowAnchor::End) ^ flip ^ reversed;
            if skip > limits().live_limits.index_jump_skips_per_window {
                return Err(SkipLimit);
            }
            if reverse {
                Scan::Reverse(skip)
            } else {
                Scan::Forward(skip)
            }
        };
        Ok(Self {
            scan,
            take,
            reversed,
        })
    }
}

fn statement<'a>(
    conn: &'a Connection,
    scope: &(String, &str),
    plan: &Plan<'_>,
) -> rusqlite::Result<rusqlite::CachedStatement<'a>> {
    let sql = match plan.scan {
        Scan::Forward(_) => FORWARD,
        Scan::Reverse(_) => REVERSE,
        Scan::Seek(_, false) => SEEK,
        Scan::Seek(_, true) => SEEK_REVERSE,
    };
    let mut stmt = conn.prepare_cached(sql)?;
    stmt.raw_bind_parameter(1, &scope.0)?;
    stmt.raw_bind_parameter(2, scope.1)?;
    stmt.raw_bind_parameter(3, plan.take as i64)?;
    match plan.scan {
        Scan::Forward(skip) | Scan::Reverse(skip) => stmt.raw_bind_parameter(4, skip as i64)?,
        Scan::Seek((key, id), _) => {
            stmt.raw_bind_parameter(4, key)?;
            stmt.raw_bind_parameter(5, id)?;
        }
    }
    Ok(stmt)
}

pub(super) fn keys(
    conn: &Connection,
    scope: &(String, &str),
    plan: Plan<'_>,
) -> Result<Vec<(Key, String)>> {
    let mut keys = Vec::with_capacity(plan.take as usize);
    if plan.take == 0 {
        return Ok(keys);
    }
    let mut stmt = statement(conn, scope, &plan).map_err(internal)?;
    let mut rows = stmt.raw_query();
    while let Some(row) = rows.next().map_err(internal)? {
        keys.push((row.get(0).map_err(internal)?, row.get(1).map_err(internal)?));
    }
    if matches!(plan.scan, Scan::Reverse(_) | Scan::Seek(_, true)) != plan.reversed {
        keys.reverse();
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{StatementStatus, functions::FunctionFlags};
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };

    #[test]
    fn keyset_visits_one_page_at_shallow_and_deep_offsets_even_with_equal_keys() {
        let conn = Connection::open_in_memory().unwrap();
        calternal_store::migrations::migrate(&conn, crate::db::SPACE_SCHEMA).unwrap();
        conn.execute_batch(
            "WITH RECURSIVE seq(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM seq WHERE n<25000)
             INSERT INTO items
             SELECT printf('%08d',n),'notes','{}','revision',1,'2026-11-19T00:00:00.000Z',
                    printf('%08d',n),printf('%08d',n),NULL,'materialized',NULL FROM seq;",
        )
        .unwrap();
        conn.execute_batch(include_str!("sidecar.sql")).unwrap();
        conn.execute_batch(
            "INSERT INTO window_rows
            SELECT 'notes','space',newest,id,title FROM window_live_notes",
        )
        .unwrap();
        let scope = super::super::WindowScope::SpaceNotes.key();
        let sort_key: i64 = conn
            .query_row("SELECT newest FROM window_live_notes LIMIT 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        let seen = Arc::new(AtomicU64::new(0));
        let counter = seen.clone();
        conn.create_scalar_function("visit", 1, FunctionFlags::SQLITE_UTF8, move |_| {
            counter.fetch_add(1, Ordering::Relaxed);
            Ok(true)
        })
        .unwrap();
        let probe_sql = SEEK.replace("ORDER BY", "AND visit(item) ORDER BY");
        let explain = format!("EXPLAIN QUERY PLAN {probe_sql}");
        let mut explain = conn.prepare(&explain).unwrap();
        let detail: String = explain
            .query_row(
                params![scope.0, scope.1, 200, sort_key, "00024000"],
                |row| row.get(3),
            )
            .unwrap();
        assert!(detail.contains("COVERING INDEX window_cover"), "{detail}");
        let mut steps = Vec::new();
        for index in [200, 24_000] {
            let boundary = (Key::Integer(sort_key), format!("{index:08}"));
            let plan = Plan::new(
                25_000,
                index,
                200,
                WindowAnchor::Start,
                true,
                Some(&boundary),
                false,
            )
            .unwrap();
            let mut stmt = statement(&conn, &scope, &plan).unwrap();
            stmt.reset_status(StatementStatus::VmStep);
            let plain = stmt
                .raw_query()
                .mapped(|row| row.get::<_, String>(1))
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            steps.push(stmt.get_status(StatementStatus::VmStep));
            assert_eq!(stmt.get_status(StatementStatus::FullscanStep), 0);
            assert_eq!(stmt.get_status(StatementStatus::Sort), 0);
            seen.store(0, Ordering::Relaxed);
            let probed = conn
                .prepare(&probe_sql)
                .unwrap()
                .query_map(
                    params![scope.0, scope.1, 200, boundary.0, boundary.1],
                    |row| row.get::<_, String>(1),
                )
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert_eq!(probed, plain);
            assert_eq!(probed.len(), 200);
            assert_eq!(seen.load(Ordering::Relaxed), 200);
            assert_eq!(probed.first().unwrap(), &format!("{:08}", index + 1));
        }
        assert_eq!(steps[0], steps[1]);
        // For random access the planner can take 200 rows from the nearer end.
        let plan = Plan::new(25_000, 24_800, 200, WindowAnchor::Start, true, None, false).unwrap();
        assert_eq!(plan.scan, Scan::Reverse(0));
        assert_eq!(
            keys(&conn, &scope, plan).unwrap().first().unwrap().1,
            "00024801"
        );
    }
}

pub(super) struct Projection<'a, 'fields> {
    statement: rusqlite::CachedStatement<'a>,
    title: &'fields str,
    title_path: String,
    fields: &'fields [super::WindowField],
}

impl<'a, 'fields> Projection<'a, 'fields> {
    pub(super) fn new(
        conn: &'a Connection,
        title: &'fields str,
        fields: &'fields [super::WindowField],
    ) -> Result<Self> {
        let sql = if fields.is_empty() {
            "SELECT CASE WHEN json_type(metadata,?2)='text' THEN json_extract(metadata,?2) ELSE '' END
             FROM items WHERE id=?1"
        } else {
            "SELECT metadata FROM items WHERE id=?1"
        };
        Ok(Self {
            statement: conn.prepare_cached(sql).map_err(internal)?,
            title,
            title_path: format!("$.{title}"),
            fields,
        })
    }

    pub(super) fn fields(&mut self, id: &str) -> Result<serde_json::Value> {
        use super::WindowField::*;
        use serde_json::{Map, Value, json};
        if self.fields.is_empty() {
            let title: String = self
                .statement
                .query_row([id, &self.title_path], |row| row.get(0))
                .map_err(internal)?;
            return Ok(json!({"title": title}));
        }
        let metadata: String = self
            .statement
            .query_row([id], |row| row.get(0))
            .map_err(internal)?;
        let metadata: Value = serde_json::from_str(&metadata).map_err(internal)?;
        let mut fields = Map::new();
        fields.insert(
            "title".into(),
            json!(metadata[self.title].as_str().unwrap_or("")),
        );
        for field in self.fields {
            let (key, value) = match field {
                Boolean(key) => (key, metadata[key].as_bool().map(Value::Bool)),
                Timestamp(key) => (
                    key,
                    metadata[key]
                        .as_str()
                        .filter(|text| {
                            time::OffsetDateTime::parse(
                                text,
                                &time::format_description::well_known::Rfc3339,
                            )
                            .is_ok()
                        })
                        .map(|text| Value::String(text.into())),
                ),
            };
            if let Some(value) = value {
                fields.insert(key.clone(), value);
            }
        }
        Ok(Value::Object(fields))
    }
}
