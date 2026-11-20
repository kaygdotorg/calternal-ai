//! Installing a declaration, creating files and setting their fields.
//!
//! A file's kind and ID are fixed at create in host state (owner question
//! D-kind, recommended option); the frontmatter's `id:` and `kind:` lines are a
//! readable copy, and a write puts them back if someone edited them.
use crate::call::{Ctx, receipt, stamp};
use crate::decl;
use crate::host::{Space, fail};
use crate::{fm, query};
use calternal_fsio::{TEMP_PREFIX, names};
use calternal_types::Error;
use serde_json::{Map, Value, json};
use std::io::{Read, Write};

/// `plugins.install {declaration}`: validate with the one validator, refuse a
/// kind another plugin owns, store it. Reinstalling the same ID replaces it.
///
/// # Errors
/// `invalid_input` from the validator; `conflict` for a kind owned by another plugin.
pub fn install(ctx: &Ctx, space: &mut Space, input: &Value) -> Result<Value, Error> {
    let raw = input
        .get("declaration")
        .ok_or_else(|| Error::input_field("/declaration", "a plugin declaration"))?;
    let d = decl::validate(raw)?;
    for kind in d.kinds.keys() {
        if let Some((owner, _)) = space.kind(kind)
            && owner.id != d.id
        {
            return Err(Error::conflict(format!(
                "kind {kind} belongs to {}",
                owner.id
            )));
        }
    }
    let digest = blake3::hash(raw.to_string().as_bytes())
        .to_hex()
        .to_string();
    let sentence = format!(
        "{} can create and change files of kind {}. It runs no code and cannot send data out.",
        d.name,
        if d.grant.write.is_empty() {
            "none".to_owned()
        } else {
            d.grant.write.join(", ")
        }
    );
    let result = json!({ "plugin": d.id, "digest": digest, "install": sentence,
        "kinds": d.kinds.keys().collect::<Vec<_>>(), "sidebar": d.sidebar });
    let tx = space.db.transaction().map_err(fail)?;
    tx.execute(
        "INSERT OR REPLACE INTO plugins(id, declaration, digest, installed_at, installed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![d.id, raw.to_string(), digest, calternal_types::now_ms(), ctx.person.to_string()],
    )
    .map_err(fail)?;
    receipt(&tx, ctx, "plugins.install", &result)?;
    tx.commit().map_err(fail)?;
    space.plugins.insert(d.id.clone(), d);
    Ok(result)
}

/// `files.create {kind, fields, body?}`: a Markdown file under the kind's folder.
/// Cost: 1 directory listing (name check), 1 ledger commit, 1 file write
/// (temp, fsync, rename, directory fsync), 1 index update.
///
/// # Errors
/// `invalid_input` for fields the kind does not accept, or a body on a record kind.
pub fn create(ctx: &Ctx, space: &mut Space, input: &Value) -> Result<Value, Error> {
    let kind_name = input
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let (_, kind) = space
        .kind(&kind_name)
        .ok_or_else(|| Error::invalid_input("unknown kind"))?;
    let kind = kind.clone();
    let empty = Map::new();
    let given = input
        .get("fields")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let fields = kind.check_fields(given, true)?;
    let body = input.get("body").and_then(Value::as_str).unwrap_or("");
    if kind.records && !body.is_empty() {
        return Err(Error::invalid_input(
            "a record file starts empty; add records with record.append",
        ));
    }
    let id = uuid::Uuid::now_v7().to_string();
    let text = fm::render(&id, &kind_name, &fields, body);
    let label = given
        .get("title")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| given.values().find_map(Value::as_str))
        .unwrap_or("Untitled");
    let folder = space.folder.open_sub(&kind.folder, true).map_err(fail)?;
    let taken: std::collections::HashSet<String> = folder
        .names()
        .map_err(fail)?
        .iter()
        .map(|n| names::fold(n))
        .collect();
    let name = names::unique_md_name(&names::stem_from_title(label), |n| taken.contains(n));
    let path = format!("{}/{name}", kind.folder);
    let hash = space.hash(text.as_bytes());
    let result = json!({ "id": id, "path": path, "rev": 1 });
    let tx = space.db.transaction().map_err(fail)?;
    tx.execute(
        "INSERT INTO files(id, kind, plugin, path, rev, len, prefix_len, hash, created_at) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5, ?6, ?7)",
        rusqlite::params![id, kind_name, ctx.plugin, path, text.len() as i64, hash, calternal_types::now_ms()],
    )
    .map_err(fail)?;
    stamp(&tx, ctx, &id, 1, None, "create", &hash)?;
    receipt(&tx, ctx, "files.create", &result)?;
    tx.commit().map_err(fail)?;
    let mut temp = folder
        .create_temp(&format!("{TEMP_PREFIX}{}", ctx.request))
        .map_err(fail)?;
    temp.file().write_all(text.as_bytes()).map_err(fail)?;
    temp.persist(|n| (n == 1).then(|| name.clone()))
        .map_err(fail)?;
    let seen = space.stat(&path)?;
    space.seen.insert(id.clone(), seen);
    query::index_file(space, &id, &kind_name, &text)?;
    Ok(result)
}

/// `files.set {file, fields, rev?}`: set frontmatter fields, keeping every
/// other byte. A change made on disk since the host's last write is first
/// recorded as a revision stamped `outside` (design A7).
/// Cost: 1 indexed read, 1 file read and hash, 1 ledger commit, 1 file write.
///
/// # Errors
/// `conflict` for a stale `rev`; `invalid_input` for bad fields;
/// `unavailable` for record files (see the report: it would move every record).
pub fn set(ctx: &Ctx, space: &mut Space, input: &Value) -> Result<Value, Error> {
    let row = space.file(input["file"].as_str().unwrap_or_default())?;
    let (_, kind) = space
        .kind(&row.kind)
        .ok_or_else(|| Error::unavailable("the kind's plugin is not installed"))?;
    if kind.records {
        return Err(Error::unavailable(
            "setting fields of a record file is not in the spike",
        ));
    }
    let empty = Map::new();
    let mut updates = kind.check_fields(
        input
            .get("fields")
            .and_then(Value::as_object)
            .unwrap_or(&empty),
        false,
    )?;
    let mut text = String::new();
    space
        .folder
        .open_file(&row.path)
        .map_err(fail)?
        .read_to_string(&mut text)
        .map_err(fail)?;
    let on_disk = space.hash(text.as_bytes());
    let outside = on_disk != row.hash;
    let rev = row.rev + i64::from(outside);
    if let Some(expected) = input.get("rev").and_then(Value::as_i64)
        && expected != rev
    {
        return Err(Error::conflict("the file changed since you read it")
            .with_details(json!({"reason": "stale_revision", "rev": rev})));
    }
    updates.insert(0, ("kind".into(), Value::String(row.kind.clone())));
    updates.insert(0, ("id".into(), Value::String(row.id.clone())));
    let new = fm::set(&text, &updates).unwrap_or_else(|| {
        format!(
            "{}\n{text}",
            fm::render(&row.id, &row.kind, &updates[2..], "").trim_end()
        )
    });
    let rev = rev + 1;
    let hash = space.hash(new.as_bytes());
    let result = json!({ "id": row.id, "rev": rev });
    let tx = space.db.transaction().map_err(fail)?;
    if outside {
        tx.execute(
            "INSERT INTO stamps(file, rev, op, person, plugin, request, at, hash) VALUES (?1, ?2, 'outside', 'outside', 'outside', ?3, ?4, ?5)",
            rusqlite::params![row.id, rev - 1, ctx.request, calternal_types::now_ms(), on_disk],
        )
        .map_err(fail)?;
    }
    tx.execute(
        "UPDATE files SET rev = ?2, len = ?3, prefix_len = ?3, hash = ?4 WHERE id = ?1",
        rusqlite::params![row.id, rev, new.len() as i64, hash],
    )
    .map_err(fail)?;
    stamp(&tx, ctx, &row.id, rev, None, "set", &hash)?;
    receipt(&tx, ctx, "files.set", &result)?;
    tx.commit().map_err(fail)?;
    replace(space, &row.path, new.as_bytes(), &ctx.request)?;
    query::index_file(space, &row.id, &row.kind, &new)?;
    Ok(result)
}

/// Replace a file below the space root atomically (temp, fsync, rename, directory fsync)
/// and remember what the host wrote.
///
/// # Errors
/// `internal` on I/O failure.
pub fn replace(space: &mut Space, path: &str, bytes: &[u8], request: &str) -> Result<(), Error> {
    let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
    let folder = if dir.is_empty() {
        space.folder.open_sub(".", false)
    } else {
        space.folder.open_sub(dir, false)
    }
    .map_err(fail)?;
    let mut temp = folder
        .create_temp(&format!("{TEMP_PREFIX}{request}"))
        .map_err(fail)?;
    temp.file().write_all(bytes).map_err(fail)?;
    temp.replace(name).map_err(fail)?;
    let id: Option<String> = space
        .db
        .query_row("SELECT id FROM files WHERE path = ?1", [path], |r| r.get(0))
        .ok();
    if let Some(id) = id {
        let seen = space.stat(path)?;
        space.seen.insert(id, seen);
    }
    Ok(())
}
