//! Ledger-backed Markdown records. Disk headers carry no authority.
use crate::declarations::{Fields, Kind, apply_fields};
use crate::internal;
use crate::space::{Head, Space};
use calternal_types::{Actor, Error, Result};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use uuid::Uuid;

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

#[derive(Clone)]
pub(crate) struct Record {
    pub(crate) anchor: String,
    pub(crate) offset: u64,
    pub(crate) length: u64,
    pub(crate) author: String,
    pub(crate) at: i64,
    pub(crate) hash: [u8; 32],
    pub(crate) ordinal: u64,
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
        let mut stmt = self.db.prepare_cached(
            "SELECT anchor,offset,length,author,at,hash,ordinal FROM records WHERE item=?1 ORDER BY offset",
        ).map_err(internal)?;
        let rows = stmt
            .query_map([item.to_string()], |row| {
                Ok((
                    row.get(0)?,
                    row.get::<_, i64>(1)? as u64,
                    row.get::<_, i64>(2)? as u64,
                    row.get(3)?,
                    row.get(4)?,
                    row.get::<_, Vec<u8>>(5)?,
                    row.get::<_, i64>(6)? as u64,
                ))
            })
            .map_err(internal)?;
        rows.map(|row| {
            let (anchor, offset, length, author, at, hash, ordinal) = row.map_err(internal)?;
            Ok(Record {
                anchor,
                offset,
                length,
                author,
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
                .map(|row| {
                    view(
                        row.bytes(bytes).expect("verified record"),
                        &row.anchor,
                        &row.author,
                        row.at,
                        kind,
                    )
                })
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
        regions.windows(2).map(|pair| {
            let (start, anchor) = pair[0];
            let data = &bytes[start..pair[1].0];
            match by_anchor.get(anchor).filter(|row| row.offset == start as u64 && self.hash(data) == row.hash) {
                Some(row) => view(data, anchor, &row.author, row.at, kind),
                None => Ok(json!({"anchor":anchor, "author":"outside", "at":null, "fields":{}, "body":null})),
            }
        }).collect()
    }
}

impl Record {
    pub(crate) fn bytes<'a>(&self, bytes: &'a [u8]) -> Option<&'a [u8]> {
        bytes.get(self.offset as usize..self.offset.checked_add(self.length)? as usize)
    }

    pub(crate) fn insert(&self, tx: &Connection, item: Uuid) -> Result<()> {
        tx.execute(
            "INSERT INTO records VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                item.to_string(),
                self.anchor,
                self.offset as i64,
                self.length as i64,
                self.author,
                self.at,
                &self.hash[..],
                self.ordinal as i64
            ],
        )
        .map_err(internal)?;
        Ok(())
    }
}

fn anchor() -> String {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes[..10]).expect("operating system randomness");
    let bits = u128::from_le_bytes(bytes);
    (0..16)
        .map(|i| CROCKFORD[((bits >> (75 - 5 * i)) & 31) as usize] as char)
        .collect()
}

pub(crate) fn valid_anchor(anchor: &str) -> bool {
    anchor.len() == 16 && anchor.bytes().all(|b| CROCKFORD.contains(&b))
}

fn header_anchor(line: &str) -> Option<&str> {
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

fn view(data: &[u8], anchor: &str, author: &str, at: i64, kind: &Kind) -> Result<Value> {
    let (fields, body) = content(data, kind)?;
    Ok(json!({"anchor":anchor, "author":author, "at":at, "fields":fields, "body":body}))
}

fn content(data: &[u8], kind: &Kind) -> Result<(Fields, String)> {
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
        fields.insert(
            name.to_owned(),
            serde_json::from_str(raw).map_err(internal)?,
        );
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
    offset: u64,
    ordinal: u64,
    now: i64,
) -> Result<(Record, Vec<u8>)> {
    if kind.records.is_empty() {
        return Err(Error::forbidden("This kind does not hold records."));
    }
    let (mut fields, previous_body) = existing
        .map(|(_, data)| content(data, kind))
        .transpose()?
        .unwrap_or_default();
    apply_fields(&kind.records, &mut fields, input.fields, existing.is_none())?;
    let (anchor, author, at) = existing
        .map(|(r, _)| (r.anchor.clone(), r.author.clone(), r.at))
        .unwrap_or_else(|| (anchor(), actor.account.to_string(), now));
    let when = time::OffsetDateTime::from_unix_timestamp(at / 1000)
        .map_err(internal)?
        .format(time::macros::format_description!(
            "[year]-[month]-[day] [hour]:[minute]"
        ))
        .map_err(internal)?;
    let mut data = format!("**{author}** · {when} ^{anchor}\n");
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
    if let Some(raw) = kind.operations.get("change record").and_then(|line| {
        line.split(", ")
            .find_map(|part| part.strip_prefix("within "))
    }) {
        let (number, unit) = raw.split_at(raw.len().saturating_sub(1));
        let multiplier = match unit {
            "h" => 3_600_000,
            "m" => 60_000,
            "d" => 86_400_000,
            _ => return Err(Error::invalid_input("Unsupported edit window.")),
        };
        let limit = number
            .parse::<i64>()
            .ok()
            .and_then(|n| n.checked_mul(multiplier))
            .ok_or_else(|| Error::invalid_input("Invalid edit window."))?;
        if now.saturating_sub(record.at) > limit {
            return Err(Error::forbidden("The record's edit window has closed."));
        }
    }
    Ok(())
}

/// Fast path adapted from the host spike. The checked directory descriptor
/// pins the read handle and procfs reopens the same inode with write access.
#[cfg(target_os = "linux")]
pub(crate) fn append(
    space: &mut Space,
    actor: &Actor,
    mut head: Head,
    input: Input,
    now: i64,
    receipt: Option<&crate::space::Receipt>,
    attribution: &str,
) -> Result<Value> {
    use std::io::Read;
    use std::os::{fd::AsRawFd, unix::fs::FileExt};

    let mut read = space.open_regular(&head.path).map_err(internal)?;
    let metadata = read.metadata().map_err(internal)?;
    let initial = Signature::from(&metadata);
    if metadata.nlink() != 1 || metadata.len() != head.length {
        return Err(Error::conflict("The record stream has outside changes."));
    }
    if space.seen.get(&head.id) != Some(&initial) {
        let rows = space.records(head.id)?;
        let mut bytes = Vec::with_capacity(head.length as usize);
        (&mut read)
            .take(head.length + 1)
            .read_to_end(&mut bytes)
            .map_err(internal)?;
        if bytes.len() as u64 != head.length
            || !bytes
                .get(..head.prefix as usize)
                .is_some_and(|p| space.hash(p) == head.hash)
            || rows.iter().any(|row| {
                !row.bytes(&bytes)
                    .is_some_and(|data| space.hash(data) == row.hash)
            })
        {
            return Err(Error::conflict("The record stream has outside changes."));
        }
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(format!("/proc/self/fd/{}", read.as_raw_fd()))
        .map_err(internal)?;
    if Signature::from(&file.metadata().map_err(internal)?) != initial {
        return Err(Error::conflict(
            "The record stream changed during the call.",
        ));
    }
    let kind = space.kind(&head.kind)?;
    let end = head.length;
    head.version += 1;
    let (record, bytes) = encode(space, kind, actor, input, None, end, head.version, now)?;
    head.length += bytes.len() as u64;
    head.edited = now;
    if head.length > crate::space::ITEM_BYTES as u64 {
        return Err(Error::new(
            calternal_types::ErrorCode::TooLarge,
            "The item exceeds 32 MiB.",
        ));
    }
    let output = crate::result(vec![
        json!({"id":head.id,"version":head.version,"anchor":record.anchor}),
    ]);
    let tx = space.db.transaction().map_err(internal)?;
    tx.execute(
        "DELETE FROM history WHERE at<?1",
        [now - space.retention_ms],
    )
    .map_err(internal)?;
    record.insert(&tx, head.id)?;
    tx.execute(
        "UPDATE head SET data=?2 WHERE id=?1",
        params![
            head.id.to_string(),
            serde_json::to_string(&head).map_err(internal)?
        ],
    )
    .map_err(internal)?;
    tx.execute("INSERT INTO history(item,version,actor,action,anchor,at) VALUES(?1,?2,?3,'append record',?4,?5)", params![head.id.to_string(),head.version as i64,attribution,record.anchor,now]).map_err(internal)?;
    if let Some(receipt) = receipt {
        receipt.store(&tx, &output, now)?;
    }
    if let Err(error) = file
        .write_all_at(&bytes, end)
        .and_then(|_| file.sync_data())
    {
        file.set_len(end)
            .and_then(|_| file.sync_data())
            .map_err(internal)?;
        return Err(internal(error));
    }
    let signature = Signature::from(&file.metadata().map_err(internal)?);
    if let Err(error) = tx.commit() {
        space.recovery_needed = true;
        return Err(internal(error));
    }
    space.seen.insert(head.id, signature);
    let id = head.id;
    space.rows.get_mut(&id).expect("indexed record item").head = head;
    Ok(output)
}
