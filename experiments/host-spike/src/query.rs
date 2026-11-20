//! Questions: the one query form over one kind, answered two ways.
//!
//! `from: "scan"` reads every file host state lists for the kind and parses its
//! frontmatter (the kind comes from host state, never from the file). The
//! default, `from: "index"`, reads the `fields` table in appdata, which
//! [`rebuild`] fills from the same files at open and [`index_file`] keeps
//! current after each host write. Both must give the same answer; a test checks it.
use crate::call::Ctx;
use crate::decl::Query;
use crate::fm;
use crate::host::{Space, fail};
use calternal_types::Error;
use serde_json::{Map, Value, json};
use std::cmp::Ordering;
use std::io::Read;

const MAX_RESULTS: usize = 500;

/// One answer: the file's path and its fields (common fields included).
type Item = (String, Map<String, Value>);

/// Resolve the query a call asks: inline (`input.kind`, `where`, `sort`) or a
/// collection of the acting plugin (`input.collection`).
fn resolve(space: &Space, plugin: &str, input: &Value) -> Result<Query, Error> {
    if let Some(name) = input.get("collection").and_then(Value::as_str) {
        let d = space
            .plugins
            .get(plugin)
            .ok_or_else(|| Error::forbidden("plugin not installed"))?;
        return d
            .collections
            .get(name)
            .cloned()
            .ok_or_else(|| Error::not_found("no such collection"));
    }
    serde_json::from_value(json!({
        "kind": input.get("kind").cloned().unwrap_or(Value::Null),
        "where": input.get("where").cloned().unwrap_or(json!({})),
        "sort": input.get("sort").cloned().unwrap_or(Value::Null),
        "limit": input.get("limit").cloned().unwrap_or(Value::Null),
    }))
    .map_err(|e| Error::invalid_input(format!("query: {e}")))
}

/// The kind a query asks about, for `authorize`.
///
/// # Errors
/// As [`resolve`].
pub fn asked_kind(space: &Space, plugin: &str, input: &Value) -> Result<String, Error> {
    Ok(resolve(space, plugin, input)?.kind)
}

/// `query {kind, where?, sort?, limit?, from?}` or `query {collection, from?}`.
/// Cost (index): one indexed read per field of each candidate file; (scan): one
/// file read per file of the kind.
///
/// # Errors
/// `invalid_input` for an unindexed field or unknown kind.
pub fn query(ctx: &Ctx, space: &mut Space, input: &Value) -> Result<Value, Error> {
    let q = resolve(space, &ctx.plugin, input)?;
    let (_, kind) = space
        .kind(&q.kind)
        .ok_or_else(|| Error::invalid_input("unknown kind"))?;
    kind.check_query(&q)?;
    let scan = input.get("from").and_then(Value::as_str) == Some("scan");
    let mut items = if scan {
        scan_kind(space, &q.kind)?
    } else {
        from_index(space, &q.kind)?
    };
    items.retain(|(_, f)| {
        q.filter
            .iter()
            .all(|(k, want)| matches(f.get(k).unwrap_or(&Value::Null), want))
    });
    if let Some(sort) = &q.sort {
        let (desc, key) = sort
            .strip_prefix('-')
            .map_or((false, sort.as_str()), |k| (true, k));
        items.sort_by(|a, b| {
            let o = cmp(
                a.1.get(key).unwrap_or(&Value::Null),
                b.1.get(key).unwrap_or(&Value::Null),
            );
            if desc { o.reverse() } else { o }
        });
    }
    items.truncate(q.limit.unwrap_or(MAX_RESULTS).min(MAX_RESULTS));
    let items: Vec<Value> = items
        .into_iter()
        .map(|(path, f)| json!({ "path": path, "fields": f }))
        .collect();
    Ok(
        json!({ "kind": q.kind, "count": items.len(), "items": items, "from": if scan { "scan" } else { "index" } }),
    )
}

fn matches(have: &Value, want: &Value) -> bool {
    match want {
        Value::Object(op) if op.contains_key("gt") => cmp(have, &op["gt"]) == Ordering::Greater,
        Value::Object(op) if op.contains_key("in") => {
            op["in"].as_array().is_some_and(|a| a.contains(have))
        }
        other => have == other,
    }
}

fn cmp(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x
            .as_f64()
            .partial_cmp(&y.as_f64())
            .unwrap_or(Ordering::Equal),
        (Value::String(x), Value::String(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        _ => a.to_string().cmp(&b.to_string()),
    }
}

/// Fields of one file's text plus the common fields from host state. The
/// frontmatter's own `id`/`kind` lines are ignored: host state wins (T4).
fn fields_of(id: &str, kind: &str, created: i64, text: &str) -> Map<String, Value> {
    let mut f: Map<String, Value> = fm::fields(text)
        .into_iter()
        .filter(|(k, _)| k != "id" && k != "kind")
        .collect();
    f.insert("id".into(), json!(id));
    f.insert("kind".into(), json!(kind));
    f.insert("created".into(), json!(created));
    f
}

/// The kind's files, as `(id, path, created)`, from host state.
fn files_of(space: &Space, kind: &str) -> Result<Vec<(String, String, i64)>, Error> {
    let mut stmt = space
        .db
        .prepare_cached("SELECT id, path, created_at FROM files WHERE kind = ?1")
        .map_err(fail)?;
    let rows = stmt
        .query_map([kind], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(fail)?;
    rows.collect::<Result<_, _>>().map_err(fail)
}

fn read(space: &Space, path: &str) -> Result<Option<String>, Error> {
    let mut text = String::new();
    match space.folder.open_file(path) {
        Ok(mut f) => {
            // A record file's frontmatter is all a query needs; read only its first 16 KiB.
            f.by_ref()
                .take(16 * 1024)
                .read_to_string(&mut text)
                .map_err(fail)?;
            Ok(Some(text))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(fail(e)),
    }
}

fn scan_kind(space: &Space, kind: &str) -> Result<Vec<Item>, Error> {
    let mut out = Vec::new();
    for (id, path, created) in files_of(space, kind)? {
        if let Some(text) = read(space, &path)? {
            out.push((path, fields_of(&id, kind, created, &text)));
        }
    }
    Ok(out)
}

fn from_index(space: &Space, kind: &str) -> Result<Vec<Item>, Error> {
    let paths: std::collections::HashMap<String, String> = files_of(space, kind)?
        .into_iter()
        .map(|(id, p, _)| (id, p))
        .collect();
    let mut stmt = space
        .index
        .prepare_cached("SELECT file, key, value FROM fields WHERE kind = ?1 ORDER BY file")
        .map_err(fail)?;
    let rows = stmt
        .query_map([kind], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(fail)?;
    let mut out: Vec<(String, Map<String, Value>)> = Vec::new();
    let mut last = String::new();
    for row in rows {
        let (file, key, value) = row.map_err(fail)?;
        if file != last {
            let path = paths.get(&file).cloned().unwrap_or_default();
            out.push((path, Map::new()));
            last = file;
        }
        out.last_mut()
            .expect("pushed above")
            .1
            .insert(key, serde_json::from_str(&value).map_err(fail)?);
    }
    Ok(out)
}

/// Put one file's fields into the index (after a host write).
///
/// # Errors
/// `internal` on SQL failure. Cost: one delete and one insert per field, not synced.
pub fn index_file(space: &mut Space, id: &str, kind: &str, text: &str) -> Result<(), Error> {
    let created: i64 = space
        .db
        .query_row("SELECT created_at FROM files WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .map_err(fail)?;
    let tx = space.index.transaction().map_err(fail)?;
    tx.execute("DELETE FROM fields WHERE file = ?1", [id])
        .map_err(fail)?;
    for (k, v) in fields_of(id, kind, created, text) {
        tx.execute(
            "INSERT INTO fields(file, kind, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, kind, k, v.to_string()],
        )
        .map_err(fail)?;
    }
    tx.commit().map_err(fail)
}

/// Rebuild the whole index from the files and host state (at open). Edits made
/// on disk while the server ran are seen only by `from: "scan"` until the next
/// rebuild: there is no watcher in the spike.
///
/// # Errors
/// `internal` on I/O or SQL failure.
pub fn rebuild(space: &mut Space) -> Result<(), Error> {
    space
        .index
        .execute("DELETE FROM fields", [])
        .map_err(fail)?;
    let all: Vec<(String, String, String)> = {
        let mut stmt = space
            .db
            .prepare("SELECT id, kind, path FROM files")
            .map_err(fail)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(fail)?;
        rows.collect::<Result<_, _>>().map_err(fail)?
    };
    for (id, kind, path) in all {
        if let Some(text) = read(space, &path)? {
            index_file(space, &id, &kind, &text)?;
        }
    }
    Ok(())
}
