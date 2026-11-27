//! Apply changes to a staged item before its durable publication.
use super::*;

pub(super) fn apply(
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
                    .is_some_and(|operation| matches!(&operation.form, OperationForm::Change(Some(only)) if !only.iter().any(|field| field == "body")))
            {
                return Err(Error::forbidden(
                    "The built-in operation cannot change the body.",
                ));
            }
            declared::update(space, kind, item, fields, body.as_deref(), base)?;
            Ok(("change", None))
        }
        Change::Perform {
            operation,
            fields,
            base,
            ..
        } => {
            let patch = declared::plan(kind, &item.row, &operation, &fields)?;
            declared::update(space, kind, item, patch, None, base)?;
            Ok(("perform", None))
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
