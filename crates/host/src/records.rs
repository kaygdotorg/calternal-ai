//! Ledger-backed Markdown records. Disk headers carry no authority.
use crate::declarations::{Fields, Kind, OperationForm, apply_fields};
use crate::internal;
use crate::space::{Head, Space};
use calternal_types::{Actor, Error, Result, Via};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use uuid::Uuid;

mod append;
mod linked;
mod seal;

#[cfg(target_os = "linux")]
pub(crate) use append::append;
pub(crate) use append::recover_pending;

const CROCKFORD: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Signature(u64, u64, i64, i64, i64, i64, u64);

impl From<&Metadata> for Signature {
    fn from(m: &Metadata) -> Self {
        Self(
            m.dev(),
            m.ino(),
            m.ctime(),
            m.ctime_nsec(),
            m.mtime(),
            m.mtime_nsec(),
            m.len(),
        )
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct Record {
    pub(crate) anchor: String,
    pub(crate) offset: u64,
    pub(crate) length: u64,
    pub(crate) author: String,
    #[serde(default)]
    pub(crate) run: Option<RunWriter>,
    pub(crate) at: i64,
    pub(crate) hash: [u8; 32],
    pub(crate) ordinal: u64,
}

/// Attribution captured from the run's start and committed with the record.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct RunWriter {
    pub(crate) id: Uuid,
    pub(crate) plugin: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    #[serde(default)]
    fields: Fields,
    body: Option<String>,
}

impl Space {
    pub(crate) fn records(&self, item: Uuid) -> Result<Vec<Record>> {
        self.records_limited(item, -1)
    }

    pub(crate) fn records_limited(&self, item: Uuid, limit: i64) -> Result<Vec<Record>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT anchor,offset,length,author,at,hash,ordinal,run_writer FROM records WHERE item=?1 ORDER BY ordinal LIMIT ?2",
        ).map_err(internal)?;
        let rows = stmt
            .query_map(params![item.to_string(), limit], |row| {
                Ok((
                    row.get(0)?,
                    row.get::<_, i64>(1)? as u64,
                    row.get::<_, i64>(2)? as u64,
                    row.get(3)?,
                    row.get(4)?,
                    row.get::<_, Vec<u8>>(5)?,
                    row.get::<_, i64>(6)? as u64,
                    row.get::<_, Option<String>>(7)?,
                ))
            })
            .map_err(internal)?;
        rows.map(|row| {
            let (anchor, offset, length, author, at, hash, ordinal, run) = row.map_err(internal)?;
            Ok(Record {
                anchor,
                offset,
                length,
                author,
                run: run
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()
                    .map_err(internal)?,
                at,
                ordinal,
                hash: hash
                    .try_into()
                    .map_err(|_| internal("invalid record hash"))?,
            })
        })
        .collect()
    }

    pub(crate) fn read_verified(&self, head: &Head, records: &[Record]) -> Result<Vec<u8>> {
        let bytes = self.read_item(head)?;
        let prefix = bytes.get(..head.prefix as usize);
        let verified = prefix.is_some_and(|p| self.hash(p) == head.hash)
            && bytes.len() as u64 == head.length
            && records.iter().all(|row| {
                row.bytes(&bytes)
                    .is_some_and(|data| self.hash(data) == row.hash)
            });
        if !verified {
            return Err(Error::conflict("The item has outside changes."));
        }
        Ok(bytes)
    }

    pub(crate) fn record_views(
        &self,
        head: &Head,
        bytes: &[u8],
        rows: &[Record],
        kind: &Kind,
    ) -> Result<Vec<Value>> {
        let prefix = bytes
            .get(..head.prefix as usize)
            .ok_or_else(|| Error::conflict("The item was shortened on disk."))?;
        if self.hash(prefix) != head.hash {
            return Err(Error::conflict("The item body has outside changes."));
        }
        let tail = std::str::from_utf8(&bytes[head.prefix as usize..]).map_err(internal)?;
        if bytes.len() as u64 == head.length
            && rows.iter().all(|row| {
                row.bytes(bytes)
                    .is_some_and(|data| self.hash(data) == row.hash)
            })
        {
            return rows
                .iter()
                .map(|row| self.record_view(row.bytes(bytes).expect("verified record"), row, kind))
                .collect();
        }
        // For a changed file, compare each region with its ledger hash before
        // exposing any attribution. A copied or altered header stays outside.
        let mut regions: Vec<_> = tail
            .split_inclusive('\n')
            .scan(head.prefix as usize, |offset, line| {
                let start = *offset;
                *offset += line.len();
                Some((start, line.trim_end_matches('\n')))
            })
            .filter_map(|(start, line)| header_anchor(line).map(|anchor| (start, anchor)))
            .collect();
        regions.push((bytes.len(), ""));
        if regions[0].0 != head.prefix as usize {
            return Err(Error::conflict("The record stream has outside changes."));
        }
        let by_anchor: std::collections::BTreeMap<_, _> =
            rows.iter().map(|row| (row.anchor.as_str(), row)).collect();
        let mut views = Vec::new();
        let mut outside = std::collections::BTreeSet::new();
        for pair in regions.windows(2) {
            let (start, anchor) = pair[0];
            let data = &bytes[start..pair[1].0];
            let verified = by_anchor
                .get(anchor)
                .filter(|row| row.offset == start as u64 && self.hash(data) == row.hash);
            if let Some(row) = verified {
                views.push(self.record_view(data, row, kind)?);
            } else {
                views.push(json!({"anchor":anchor, "author":"outside", "writer":{"type":"outside"}, "at":null, "fields":{}, "body":null}));
                if outside.len() < 1_000 {
                    outside.insert(anchor);
                }
            }
        }
        if !outside.is_empty() {
            let tx = self.db.unchecked_transaction().map_err(internal)?;
            let now = calternal_types::now_ms();
            tx.execute("DELETE FROM history WHERE at<?1", [now - self.retention_ms])
                .map_err(internal)?;
            for anchor in outside {
                tx.execute(
                    "INSERT OR IGNORE INTO history(item,version,actor,action,anchor,at)
                      VALUES(?1,?2,'outside','outside record',?3,?4)",
                    params![head.id.to_string(), head.version as i64, anchor, now],
                )
                .map_err(internal)?;
            }
            tx.commit().map_err(internal)?;
        }
        Ok(views)
    }
}

impl Record {
    pub(crate) fn bytes<'a>(&self, bytes: &'a [u8]) -> Option<&'a [u8]> {
        bytes.get(self.offset as usize..self.offset.checked_add(self.length)? as usize)
    }

    pub(crate) fn insert(&self, tx: &Connection, item: Uuid) -> Result<()> {
        tx.execute(
            "INSERT INTO records(item,anchor,offset,length,author,at,hash,ordinal,run_writer)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                item.to_string(),
                self.anchor,
                self.offset as i64,
                self.length as i64,
                self.author,
                self.at,
                &self.hash[..],
                self.ordinal as i64,
                self.run
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .map_err(internal)?
            ],
        )
        .map_err(internal)?;
        Ok(())
    }
}

fn anchor() -> Result<String> {
    let mut bytes = [0; 10];
    getrandom::fill(&mut bytes).map_err(internal)?;
    Ok(anchor_text(bytes))
}

fn anchor_text(bytes: [u8; 10]) -> String {
    let mut expanded = [0; 16];
    expanded[..10].copy_from_slice(&bytes);
    let bits = u128::from_le_bytes(expanded);
    (0..16)
        .map(|i| CROCKFORD[((bits >> (75 - 5 * i)) & 31) as usize] as char)
        .collect()
}

pub(crate) fn link(path: &str, anchor: &str) -> String {
    const ESCAPE: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
        .add(b' ')
        .add(b'%')
        .add(b'#')
        .add(b'<')
        .add(b'>')
        .add(b'?');
    let destination = percent_encoding::utf8_percent_encode(path, ESCAPE);
    format!("[Record](<{destination}#^{anchor}>)")
}

pub(crate) fn valid_anchor(anchor: &str) -> bool {
    anchor.len() == 16 && anchor.bytes().all(|b| CROCKFORD.contains(&b))
}

fn header_anchor(line: &str) -> Option<&str> {
    let line = line
        .strip_suffix(seal::SUFFIX)
        .and_then(|line| line.rsplit_once(seal::PREFIX))
        .map_or(line, |(header, _)| header);
    let (_, rest) = line.strip_prefix("**")?.split_once("** · ")?;
    let (when, anchor) = rest.rsplit_once(" ^")?;
    (when.len() == 16 && valid_anchor(anchor)).then_some(anchor)
}

pub(crate) fn escape_body(body: &str) -> String {
    let mut result = String::with_capacity(body.len());
    for line in body.split_inclusive('\n') {
        let raw = line.trim_end_matches('\n').trim_start_matches('\\');
        if raw == "---" || header_anchor(raw).is_some() {
            result.push('\\');
        }
        result.push_str(line);
    }
    result
}

impl Space {
    fn person_name(&self, author: &str) -> Option<&str> {
        self.owner_display.as_ref().and_then(|(account, name)| {
            (calternal_types::AccountId::parse(author).ok() == Some(*account))
                .then_some(name.as_str())
        })
    }

    fn record_view(&self, data: &[u8], row: &Record, kind: &Kind) -> Result<Value> {
        let (fields, body) = content(data, kind, true)?;
        let name = self.person_name(&row.author);
        // Legacy rows carry the run ID in their ledger-hashed bytes.
        let run = row.run.as_ref().map(|run| run.id).or_else(|| {
            let (header, _) = std::str::from_utf8(data).ok()?.split_once('\n')?;
            seal::run_stamp(header).map(|(id, _)| id)
        });
        let mut record = json!({"anchor":row.anchor, "author":row.author, "at":row.at, "fields":fields, "body":body});
        record["writer"] = if let Some(id) = run {
            record["run"] = json!(id);
            json!({"type":"run", "id":id, "plugin":row.run.as_ref().map(|run| &run.plugin), "starter":{"account":row.author, "name":name}})
        } else {
            json!({"type":"person", "account":row.author, "name":name})
        };
        Ok(record)
    }
}

fn content(data: &[u8], kind: &Kind, include_fields: bool) -> Result<(Fields, String)> {
    let (_, tail) = std::str::from_utf8(data)
        .map_err(internal)?
        .split_once('\n')
        .ok_or_else(|| internal("missing record header"))?;
    let mut fields = Fields::new();
    let mut body = tail;
    while let Some((line, rest)) = body.split_once('\n') {
        let Some((name, raw)) = line.split_once(": ") else {
            break;
        };
        if !kind.records.contains_key(name) {
            break;
        }
        if include_fields {
            fields.insert(
                name.to_owned(),
                serde_json::from_str(raw).map_err(internal)?,
            );
        }
        body = rest;
    }
    let body: String = body
        .strip_prefix('\n')
        .unwrap_or(body)
        .trim_end_matches('\n')
        .split_inclusive('\n')
        .map(|line| {
            if let Some(raw) = line.strip_prefix('\\') {
                let shape = raw.trim_start_matches('\\').trim_end_matches('\n');
                if shape == "---" || header_anchor(shape).is_some() {
                    return raw;
                }
            }
            line
        })
        .collect();
    Ok((fields, body))
}

pub(crate) fn encode(
    space: &Space,
    kind: &Kind,
    actor: &Actor,
    input: Input,
    existing: Option<(&Record, &[u8])>,
    (offset, ordinal): (u64, u64),
    now: i64,
) -> Result<(Record, Vec<u8>)> {
    if kind.records.is_empty() {
        return Err(Error::forbidden("This kind does not hold records."));
    }
    let (mut fields, previous_body) = existing
        .map(|(_, data)| content(data, kind, true))
        .transpose()?
        .unwrap_or_default();
    apply_fields(&kind.records, &mut fields, input.fields, existing.is_none())?;
    let (anchor, author, at, run) = match existing {
        Some((row, _)) => (
            row.anchor.clone(),
            row.author.clone(),
            row.at,
            row.run.clone(),
        ),
        None => {
            let run = match actor.via {
                Some(Via::Run { id, .. }) => Some(RunWriter {
                    id,
                    plugin: space.runs.writer_plugin(id, actor.account)?.to_owned(),
                }),
                _ => None,
            };
            (anchor()?, actor.account.to_string(), now, run)
        }
    };
    let mut data = if let Some((_, bytes)) = existing {
        // Preserve the original attribution and stamp on edits.
        let (header, _) = std::str::from_utf8(bytes)
            .map_err(internal)?
            .split_once('\n')
            .ok_or_else(|| internal("missing record header"))?;
        format!("{header}\n")
    } else {
        let display = if let Some(run) = &run {
            &space
                .plugins
                .get(&run.plugin)
                .ok_or_else(|| Error::not_found("The run's harness is unavailable."))?
                .manifest
                .name
        } else {
            space.person_name(&author).unwrap_or(&author)
        };
        let mut display_name = String::new();
        for letter in display.chars().filter(|letter| !letter.is_control()) {
            if matches!(letter, '*' | '·' | '\\') {
                display_name.push('\\');
            }
            display_name.push(letter);
        }
        let when = time::OffsetDateTime::from_unix_timestamp(at.div_euclid(1000))
            .map_err(internal)?
            .format(time::macros::format_description!(
                "[year]-[month]-[day] [hour]:[minute]"
            ))
            .map_err(internal)?;
        format!("**{display_name}** · {when} ^{anchor}\n")
    };
    for (name, value) in fields {
        data.push_str(&format!("{name}: {value}\n"));
    }
    data.push('\n');
    data.push_str(&escape_body(
        input
            .body
            .as_deref()
            .unwrap_or(&previous_body)
            .trim_end_matches('\n'),
    ));
    data.push_str("\n\n");
    let data = data.into_bytes();
    let ordinal = existing.map_or(ordinal, |(row, _)| row.ordinal);
    let row = Record {
        anchor,
        offset,
        author,
        run,
        at,
        ordinal,
        length: data.len() as u64,
        hash: space.hash(&data),
    };
    Ok((row, data))
}

pub(crate) fn can_edit(kind: &Kind, actor: &Actor, record: &Record, now: i64) -> Result<()> {
    if record.author != actor.account.to_string() {
        return Err(Error::forbidden("Only the record's author can change it."));
    }
    if let Some(operation) = kind.operations.get("change record")
        && let OperationForm::ChangeRecord {
            within: Some(calternal_format::Value::Duration { months, seconds }),
            ..
        } = &operation.form
    {
        if *months != 0 {
            return Err(Error::invalid_input(
                "An edit window must specify elapsed time.",
            ));
        }
        let limit = seconds
            .checked_mul(1000)
            .ok_or_else(|| Error::invalid_input("Invalid edit window."))?;
        if now.saturating_sub(record.at) > limit {
            return Err(Error::forbidden("The record's edit window has closed."));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
