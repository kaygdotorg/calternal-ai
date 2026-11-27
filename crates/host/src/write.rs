//! Batch planning followed by durable publication through the space ledger.
use crate::declarations::{Fields, Kind, apply_fields, write_item};
use crate::records::{self, Record};
use crate::space::{Head, ITEM_BYTES, Row, Space};
use crate::{WRITE_BYTES, identity, internal, result};
use calternal_fsio::names;
use calternal_types::{Actor, Error, ErrorCode, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Write {
    changes: Vec<Change>,
    #[serde(default)]
    preview: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
enum Change {
    Add {
        kind: String,
        #[serde(default)]
        fields: Fields,
        body: Option<String>,
    },
    #[serde(rename = "change")]
    Update {
        item: Uuid,
        #[serde(default)]
        fields: Fields,
        body: Option<String>,
        base: Option<u64>,
    },
    Remove {
        item: Uuid,
    },
    #[serde(rename = "append record")]
    Append {
        item: Uuid,
        record: records::Input,
    },
    #[serde(rename = "change record")]
    Edit {
        item: Uuid,
        anchor: String,
        record: records::Input,
    },
    #[serde(rename = "remove record")]
    Redact {
        item: Uuid,
        anchor: String,
    },
}

struct Working {
    row: Row,
    before: Option<(String, [u8; 32])>,
    bytes: Vec<u8>,
    records: Vec<Record>,
    retired: bool,
}

pub(crate) fn write(
    space: &mut Space,
    actor: &Actor,
    input: Write,
    key: Option<String>,
) -> Result<Value> {
    if input.changes.is_empty() || input.changes.len() > 1_000 {
        return Err(Error::invalid_input(
            "A write must have between 1 and 1,000 changes.",
        ));
    }
    let encoded = serde_json::to_vec(&input.changes).map_err(internal)?;
    if encoded.len() > WRITE_BYTES {
        return Err(Error::new(ErrorCode::TooLarge, "The write exceeds 16 MiB."));
    }
    let identity = serde_json::to_string(actor).map_err(internal)?;
    let receipt = if input.preview {
        None
    } else {
        space.receipt(&format!("write:{identity}"), key.as_deref(), &encoded)?
    };
    if let Some(value) = receipt.as_ref().and_then(|entry| entry.replay.clone()) {
        return Ok(value);
    }
    let now = calternal_types::now_ms();
    #[cfg(target_os = "linux")]
    if !input.preview
        && let [Change::Append { item, record }] = input.changes.as_slice()
    {
        let head = space.row(*item)?.head.clone();
        space.permits(actor, &head.kind, true)?;
        let kind = space.kind(&head.kind)?;
        operation(kind, actor, "append record", None)?;
        return records::append(space, actor, head, record.clone(), now, receipt.as_ref());
    }
    let mut working = BTreeMap::<Uuid, Working>::new();
    let mut outputs = Vec::with_capacity(input.changes.len());
    let mut stamps = Vec::with_capacity(input.changes.len());
    let mut total_bytes = 0;
    for change in input.changes {
        let (item, action, anchor) = match change {
            Change::Add { kind, fields, body } => {
                space.permits(actor, &kind, true)?;
                let declaration = space.kind(&kind)?;
                operation(declaration, actor, "add", None)?;
                let mut stored = Fields::new();
                apply_fields(&declaration.fields, &mut stored, fields, true)?;
                let id = Uuid::now_v7();
                let path = path_for(space, declaration, &stored, id, None, &working)?;
                let body = records::escape_body(body.as_deref().unwrap_or(&declaration.template));
                let bytes = prefix(&stored, &body)?;
                let size = bytes.len() as u64;
                let head = Head {
                    id,
                    path,
                    kind,
                    author: actor.account.to_string(),
                    version: 0,
                    edited: now,
                    prefix: size,
                    length: size,
                    hash: space.hash(&bytes),
                };
                working.insert(
                    id,
                    Working {
                        row: Row {
                            head,
                            fields: stored,
                        },
                        before: None,
                        bytes,
                        records: Vec::new(),
                        retired: false,
                    },
                );
                (id, "add", None)
            }
            other => {
                let id = match &other {
                    Change::Update { item, .. }
                    | Change::Remove { item }
                    | Change::Append { item, .. }
                    | Change::Edit { item, .. }
                    | Change::Redact { item, .. } => *item,
                    Change::Add { .. } => unreachable!(),
                };
                if let std::collections::btree_map::Entry::Vacant(slot) = working.entry(id) {
                    let row = space.row(id)?.clone();
                    space.permits(actor, &row.head.kind, true)?;
                    let records = space.records(id)?;
                    let bytes = space.read_verified(&row.head, &records)?;
                    total_bytes += bytes.len();
                    if total_bytes > 64 * 1024 * 1024 {
                        return Err(Error::new(
                            ErrorCode::TooLarge,
                            "The write touches more than 64 MiB of item data.",
                        ));
                    }
                    let before = Some((row.head.path.clone(), space.hash(&bytes)));
                    slot.insert(Working {
                        row,
                        before,
                        bytes,
                        records,
                        retired: false,
                    });
                }
                let previous = working.get(&id).expect("planned item");
                let path_field = space.kind(&previous.row.head.kind)?.filename_field();
                let old_path_value = path_field
                    .and_then(|key| previous.row.fields.get(key))
                    .cloned();
                let current = working.get_mut(&id).expect("planned item");
                if current.retired {
                    return Err(Error::invalid_input(
                        "A removed item cannot be changed again in the same write.",
                    ));
                }
                let kind = space.kind(&current.row.head.kind)?;
                let action = apply(space, actor, kind, current, other, now)?;
                if old_path_value.as_ref() != path_field.and_then(|key| current.row.fields.get(key))
                {
                    let row = current.row.clone();
                    let path =
                        path_for(space, kind, &row.fields, id, Some(&row.head.path), &working)?;
                    working.get_mut(&id).expect("planned item").row.head.path = path;
                }
                (id, action.0, action.1)
            }
        };
        let current = working.get_mut(&item).expect("planned item");
        if current.bytes.len() > ITEM_BYTES {
            return Err(Error::new(ErrorCode::TooLarge, "The item exceeds 32 MiB."));
        }
        current.row.head.version += 1;
        current.row.head.edited = now;
        current.row.head.length = current.bytes.len() as u64;
        outputs.push(json!({"id":item,"version":current.row.head.version}));
        if let Some(anchor) = &anchor {
            outputs.last_mut().expect("change result")["anchor"] = json!(anchor);
        }
        stamps.push((item, current.row.head.version, action, anchor));
    }
    for (output, (item, _, action, anchor)) in outputs.iter_mut().zip(&stamps) {
        if *action == "append record" {
            let head = &working.get(item).expect("planned record item").row.head;
            output["link"] = json!(records::link(
                &head.path,
                anchor.as_deref().expect("append anchor")
            ));
        }
    }
    let value = result(outputs);
    if input.preview {
        return Ok(value);
    }

    space.recovery_needed = true;
    let mut placements = Vec::new();
    let mut instruction_paths = std::collections::BTreeSet::new();
    for current in working.values() {
        let head = &current.row.head;
        let prior = current.before.as_ref();
        if current.retired {
            if let Some((old, hash)) = prior {
                // Item removal goes to Trash. Record redaction never copies the
                // removed record to Trash or into the durable history ledger.
                placements.push(space.stage(
                    format!(".calternal/trash/{}.md", head.id),
                    None,
                    Some(&current.bytes),
                )?);
                placements.push(space.stage(old.clone(), Some(*hash), None)?);
            }
            continue;
        }
        let kind = space.kind(&head.kind)?;
        let dir = head.path.rsplit_once('/').expect("item's folder").0;
        let stamp = format!(
            "Written from the kind *{}* of plugin *{}*, added by *{}*. It describes files; it gives no instructions.\n\n{}",
            kind.id.split_once('.').expect("qualified kind").1,
            kind.id.split_once('.').expect("qualified kind").0,
            actor.account,
            kind.instructions
        );
        let instruction_path = format!("{dir}/AGENTS.md");
        if instruction_paths.insert(instruction_path.clone())
            && space.read_optional(&instruction_path)?.is_none()
        {
            placements.push(space.stage(instruction_path, None, Some(stamp.as_bytes()))?);
        }
        let before = prior
            .filter(|(old, _)| *old == head.path)
            .map(|(_, hash)| *hash);
        placements.push(space.stage(head.path.clone(), before, Some(&current.bytes))?);
        if let Some((old, hash)) = prior.filter(|(old, _)| *old != head.path) {
            placements.push(space.stage(old.clone(), Some(*hash), None)?);
        }
    }
    let tx = space.db.transaction().map_err(internal)?;
    tx.execute(
        "DELETE FROM history WHERE at<?1",
        [now - space.retention_ms],
    )
    .map_err(internal)?;
    for (id, current) in &working {
        tx.execute("DELETE FROM records WHERE item=?1", [id.to_string()])
            .map_err(internal)?;
        if current.retired {
            if current.before.is_some() {
                tx.execute(
                    "INSERT INTO trash VALUES(?1,?2)",
                    params![id.to_string(), now + crate::space::TRASH_MS],
                )
                .map_err(internal)?;
            }
            tx.execute("DELETE FROM head WHERE id=?1", [id.to_string()])
                .map_err(internal)?;
            tx.execute("DELETE FROM viewer_state WHERE item=?1", [id.to_string()])
                .map_err(internal)?;
        } else {
            tx.execute(
                "INSERT INTO head VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data",
                params![
                    id.to_string(),
                    serde_json::to_string(&current.row.head).map_err(internal)?
                ],
            )
            .map_err(internal)?;
            for record in &current.records {
                record.insert(&tx, *id)?;
            }
        }
    }
    for (item, version, action, anchor) in stamps {
        tx.execute(
            "INSERT INTO history(item,version,actor,action,anchor,at) VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                item.to_string(),
                version as i64,
                identity,
                action,
                anchor,
                now
            ],
        )
        .map_err(internal)?;
    }
    if let Some(receipt) = receipt {
        receipt.store(&tx, &value, now)?;
    }
    tx.execute(
        "INSERT INTO publication VALUES(1,?1)",
        [serde_json::to_string(&placements).map_err(internal)?],
    )
    .map_err(internal)?;
    tx.commit().map_err(internal)?;
    space.finish(&placements)?;
    for (id, current) in working {
        space.seen.remove(&id);
        if current.retired {
            space.rows.remove(&id);
        } else {
            space.rows.insert(id, current.row);
        }
    }
    Ok(value)
}

fn operation(kind: &Kind, actor: &Actor, action: &str, fields: Option<&Fields>) -> Result<()> {
    let (plugin, _) = identity(actor);
    if actor.via.is_some() && kind.id.starts_with(&format!("{plugin}.")) {
        return Ok(());
    }
    let declaration = kind.operations.get(action).map(String::as_str);
    if declaration.is_some_and(|line| line.contains("when ") || line.starts_with("code"))
        || (declaration.is_none()
            && !(kind.outside && matches!(action, "add" | "change" | "remove")))
    {
        return Err(Error::forbidden(
            "This operation requires the kind's declared operation.",
        ));
    }
    if let (Some(only), Some(fields)) = (
        declaration.and_then(|line| line.strip_prefix("only ")),
        fields,
    ) && fields
        .keys()
        .any(|key| !only.split(", ").any(|field| field == key))
    {
        return Err(Error::forbidden(
            "The built-in operation cannot change those fields.",
        ));
    }
    Ok(())
}

fn path_for(
    space: &Space,
    kind: &Kind,
    fields: &Fields,
    id: Uuid,
    old_path: Option<&str>,
    planned: &BTreeMap<Uuid, Working>,
) -> Result<String> {
    let title = if let Some(pattern) = &kind.file {
        if let Some(key) = pattern.strip_prefix('{').and_then(|v| v.strip_suffix('}')) {
            fields
                .get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| Error::invalid_input("The filename field must be text."))?
                .to_owned()
        } else {
            return Err(Error::invalid_input("Unsupported file pattern."));
        }
    } else {
        fields
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| id.to_string())
    };
    let mut stem = names::stem_from_title(&title);
    if matches!(names::fold(&stem).as_str(), "agents" | "claude" | "gemini") {
        stem.push_str(" 2");
    }
    let candidate = |filename: &str| {
        if let Some(main) = &kind.main {
            format!(
                "{}/{}/{main}",
                kind.folder,
                filename.trim_end_matches(".md")
            )
        } else {
            format!("{}/{filename}", kind.folder)
        }
    };
    let listed = space
        .open_folder(&kind.folder, false)
        .map(|folder| folder.names());
    let on_disk: std::collections::BTreeSet<_> = match listed {
        Ok(names) => names
            .map_err(internal)?
            .into_iter()
            .map(|name| names::fold(&name))
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(error) => return Err(internal(error)),
    };
    let planned: std::collections::BTreeSet<_> = planned
        .values()
        .map(|item| names::fold(&item.row.head.path))
        .collect();
    let file = names::unique_md_name(&stem, |filename| {
        if old_path.is_some_and(|old| names::fold(old) == candidate(filename)) {
            return false;
        }
        let entry = if kind.main.is_some() {
            filename.trim_end_matches(".md")
        } else {
            filename
        };
        on_disk.contains(entry) || planned.contains(&candidate(filename))
    });
    let path = candidate(&file);
    if calternal_types::is_reserved_space_path(&path) {
        return Err(Error::forbidden("The chosen path is reserved."));
    }
    Ok(
        if let Some(old) = old_path.filter(|old| names::fold(old) == names::fold(&path)) {
            old.to_owned()
        } else {
            path
        },
    )
}

fn prefix(fields: &Fields, body: &str) -> Result<Vec<u8>> {
    let mut markdown = write_item(fields, body)?;
    if !markdown.ends_with('\n') {
        markdown.push('\n');
    }
    Ok(markdown.into_bytes())
}

fn apply(
    space: &Space,
    actor: &Actor,
    kind: &Kind,
    item: &mut Working,
    change: Change,
    now: i64,
) -> Result<(&'static str, Option<String>)> {
    let head = &mut item.row.head;
    match change {
        Change::Update {
            fields, body, base, ..
        } => {
            operation(kind, actor, "change", Some(&fields))?;
            if body.is_some()
                && actor.via.is_none()
                && kind
                    .operations
                    .get("change")
                    .is_some_and(|line| line.starts_with("only "))
            {
                return Err(Error::forbidden(
                    "The built-in operation cannot change the body.",
                ));
            }
            if base.is_some_and(|base| base != head.version) {
                return Err(Error::conflict("The item version changed."));
            }
            let before = head.prefix as usize;
            let original = crate::declarations::item_body(
                std::str::from_utf8(&item.bytes[..before]).map_err(internal)?,
            )?;
            apply_fields(&kind.fields, &mut item.row.fields, fields, false)?;
            let changed = body.as_deref().map(records::escape_body);
            let mut prefix = prefix(&item.row.fields, changed.as_deref().unwrap_or(original))?;
            head.hash = space.hash(&prefix);
            head.prefix = prefix.len() as u64;
            for record in &mut item.records {
                record.offset = record.offset - before as u64 + head.prefix;
            }
            prefix.extend_from_slice(&item.bytes[before..]);
            item.bytes = prefix;
            Ok(("change", None))
        }
        Change::Remove { .. } => {
            if head.author != actor.account.to_string() {
                operation(kind, actor, "remove", None)?;
            }
            item.retired = true;
            Ok(("remove", None))
        }
        Change::Append { record, .. } => {
            operation(kind, actor, "append record", None)?;
            let (row, bytes) = records::encode(
                space,
                kind,
                actor,
                record,
                None,
                (item.bytes.len() as u64, head.version + 1),
                now,
            )?;
            let anchor = row.anchor.clone();
            item.records.push(row);
            item.bytes.extend_from_slice(&bytes);
            Ok(("append record", Some(anchor)))
        }
        Change::Edit { anchor, record, .. } => {
            operation(kind, actor, "change record", None)?;
            let index = record_index(&item.records, &anchor)?;
            let previous = &item.records[index];
            records::can_edit(kind, actor, previous, now)?;
            let (row, bytes) = records::encode(
                space,
                kind,
                actor,
                record,
                Some((
                    previous,
                    previous
                        .bytes(&item.bytes)
                        .ok_or_else(|| internal("record bytes are missing"))?,
                )),
                (previous.offset, previous.ordinal),
                now,
            )?;
            replace_record(item, index, &bytes);
            item.records[index] = row;
            Ok(("change record", Some(anchor)))
        }
        Change::Redact { anchor, .. } => {
            let index = record_index(&item.records, &anchor)?;
            if item.records[index].author != actor.account.to_string() {
                return Err(Error::forbidden(
                    "Only the record's author can remove it in this slice.",
                ));
            }
            replace_record(item, index, &[]);
            item.records.remove(index);
            Ok(("remove record", Some(anchor)))
        }
        Change::Add { .. } => unreachable!(),
    }
}

fn record_index(records: &[Record], anchor: &str) -> Result<usize> {
    records
        .iter()
        .position(|record| record.anchor == anchor)
        .ok_or_else(|| Error::not_found("Record was not found."))
}

fn replace_record(item: &mut Working, index: usize, bytes: &[u8]) {
    let row = &item.records[index];
    let start = row.offset as usize;
    let end = start + row.length as usize;
    item.bytes.splice(start..end, bytes.iter().copied());
    for row in &mut item.records[index + 1..] {
        row.offset = row.offset - (end - start) as u64 + bytes.len() as u64;
    }
}

#[cfg(test)]
mod tests;
