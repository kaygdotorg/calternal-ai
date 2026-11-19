//! Self-delimiting transcript records. Authored Markdown is treated as bytes.
use super::headings;
use calternal_types::Error;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::{HashMap, HashSet}, fmt::Write as _};
use uuid::Uuid;

pub(super) const ACTIVE: &str = "active_version";

/// Stable IDs and the ancestry of one message version. Continuation frames
/// repeat the same links and keep their distinct physical record IDs.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct Links {
    message: Uuid,
    version: Uuid,
    parent: Option<Uuid>,
}

impl Links {
    pub(super) fn next(metadata: &mut Value, id: Uuid) -> Self {
        let parent = metadata[ACTIVE].as_str().and_then(|id| Uuid::parse_str(id).ok());
        let links = Self { message: id, version: id, parent };
        metadata[ACTIVE] = json!(id);
        links
    }
}

const SELECT: &str = "<!-- calternal-select:";
const BETWEEN: &str = "\n\n";

/// A file record carrying checkpoint progress without visible message text.
pub(super) const CONTROL: &str = "<!-- calternal-checkpoint -->";

/// One validated record. The heading is display text and carries no authority.
#[derive(Debug, Serialize)]
pub(super) struct Record<'a> {
    pub(super) id: Uuid,
    #[serde(skip)]
    pub(super) links: Links,
    pub(super) role: &'a str,
    pub(super) heading: &'a str,
    pub(super) at: String,
    #[serde(skip)]
    pub(super) instant: time::OffsetDateTime,
    pub(super) turn: Option<Uuid>,
    pub(super) text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) provenance: Option<Provenance>,
}

/// Bounded machine-readable facts for one assistant attempt.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Provenance {
    pub(super) provider_id: String,
    pub(super) model_id: Option<String>,
    pub(super) reasoning_level: Option<String>,
    pub(super) machine_id: Option<String>,
    pub(super) duration_ms: Option<u64>,
    pub(super) reasoning_ms: Option<u64>,
    /// Logical message for this separately appended physical continuation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) continues: Option<Uuid>,
}

fn invalid() -> Error {
    Error::conflict("invalid thread message framing; restore the committed thread file")
}

/// Add one exact Markdown body and its identity line. The byte count is outside
/// the body and independent of any heading or delimiter lookalikes in it.
pub(super) fn append(
    out: &mut String,
    id: Uuid,
    links: Links,
    role: &str,
    heading: &str,
    text: &str,
    agent: Option<(Uuid, &Provenance)>,
) -> usize {
    if !out.is_empty() {
        out.push_str(BETWEEN);
    }
    let offset = out.len();
    let parent = links.parent.map_or_else(|| "none".into(), |id| id.to_string());
    write!(
        out,
        "## {heading}\n<!-- id:{id} role:{role} bytes:{} message:{} version:{} parent:{parent}",
        text.len(), links.message, links.version
    )
    .expect("write to String");
    if let Some((turn, provenance)) = agent {
        let details = serde_json::to_string(provenance)
            .expect("serialize provenance")
            .replace('>', "\\u003e");
        write!(out, " turn:{turn} meta:{details}").expect("write to String");
    }
    out.push_str(" -->\n\n");
    out.push_str(text);
    offset
}

/// Read a complete transcript or a new-message insertion before exposing text.
/// An invalid ID, byte count or UTF-8 boundary fails closed. Time and space
/// depend on the number of records and their heading lengths.
pub(super) fn read(mut input: &str) -> Result<Vec<Record<'_>>, Error> {
    input = input.strip_prefix(BETWEEN).unwrap_or(input);
    let mut seen = HashSet::new();
    let mut records = Vec::new();
    while !input.is_empty() {
        let tail = if let Some(tail) = input.strip_prefix(CONTROL) {
            tail
        } else if let Some((_, tail)) = selection(input)? {
            tail
        } else {
            let (record, tail) = first(input)?;
            if !seen.insert(record.id) {
                return Err(invalid());
            }
            records.push(record);
            tail
        };
        if tail.is_empty() {
            break;
        }
        input = tail
            .strip_prefix(BETWEEN)
            .filter(|input| !input.is_empty())
            .ok_or_else(invalid)?;
    }
    Ok(records)
}

/// Fold physical continuation frames into the logical message used by full
/// reads. Authored bytes are joined without inserting whitespace.
pub(super) fn view(body: &str) -> Result<Vec<serde_json::Value>, Error> {
    let mut output: Vec<(Record<'_>, Option<String>)> = Vec::new();
    for part in read(body)? {
        let group = part.provenance.as_ref().and_then(|p| p.continues);
        if let Some(group) = group {
            let Some((previous, combined)) = output.last_mut() else {
                return Err(invalid());
            };
            if previous.id != group || previous.links != part.links || previous.turn != part.turn || part.role != "agent" {
                return Err(invalid());
            }
            combined
                .get_or_insert_with(|| previous.text.to_owned())
                .push_str(part.text);
            previous.heading = part.heading;
            previous.provenance = part.provenance;
            if let Some(provenance) = &mut previous.provenance {
                provenance.continues = None;
            }
        } else {
            output.push((part, None));
        }
    }
    output
        .into_iter()
        .map(|(record, combined)| {
            let mut value = serde_json::to_value(record).map_err(|_| invalid())?;
            if let Some(text) = combined {
                value["text"] = serde_json::Value::String(text);
            }
            Ok(value)
        })
        .collect()
}

/// Load the active record from its indexed offset without rescanning earlier text.
pub(super) fn tail(body: &str, start: usize, id: Uuid) -> Result<Record<'_>, Error> {
    let (record, tail) = first(body.get(start..).ok_or_else(invalid)?)?;
    if !tail.is_empty() || record.id != id || record.role != "agent" {
        return Err(invalid());
    }
    Ok(record)
}

fn selection(input: &str) -> Result<Option<(Uuid, &str)>, Error> {
    input.strip_prefix(SELECT).map(|rest| {
        let (version, tail) = rest.split_once(" -->").ok_or_else(invalid)?;
        Ok((Uuid::parse_str(version).map_err(|_| invalid())?, tail))
    }).transpose()
}

/// Replay the authoritative tip and check links against earlier records.
/// Standalone fragment reads validate framing without walking its ancestry.
pub(super) fn active_version(mut body: &str) -> Result<Option<Uuid>, Error> {
    body = body.strip_prefix(BETWEEN).unwrap_or(body);
    let mut versions: HashMap<Uuid, (Links, Uuid)> = HashMap::new();
    let mut active = None;
    while !body.is_empty() {
        let tail = if let Some(tail) = body.strip_prefix(CONTROL) {
            tail
        } else if let Some((version, tail)) = selection(body)? {
            if !versions.contains_key(&version) {
                return Err(invalid());
            }
            active = Some(version);
            tail
        } else {
            let (record, tail) = first(body)?;
            let links = record.links;
            if let Some(group) = record.provenance.as_ref().and_then(|p| p.continues) {
                if versions.get(&links.version) != Some(&(links, group)) {
                    return Err(invalid());
                }
            } else {
                if links.parent.is_some_and(|id| !versions.contains_key(&id))
                    || versions.insert(links.version, (links, record.id)).is_some()
                {
                    return Err(invalid());
                }
                active = Some(links.version);
            }
            tail
        };
        if tail.is_empty() {
            break;
        }
        body = tail.strip_prefix(BETWEEN).filter(|tail| !tail.is_empty()).ok_or_else(invalid)?;
    }
    Ok(active)
}

fn first(input: &str) -> Result<(Record<'_>, &str), Error> {
    let (line, rest) = input
        .strip_prefix("## ")
        .and_then(|text| text.split_once('\n'))
        .ok_or_else(invalid)?;
    let (marker, text) = rest
        .strip_prefix("<!-- id:")
        .and_then(|text| text.split_once(" -->\n\n"))
        .ok_or_else(invalid)?;
    let mut fields = marker.splitn(8, ' ');
    let id = fields
        .next()
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(invalid)?;
    let role = fields
        .next()
        .and_then(|value| value.strip_prefix("role:"))
        .ok_or_else(invalid)?;
    let length = fields
        .next()
        .and_then(|value| value.strip_prefix("bytes:")?.parse::<usize>().ok())
        .ok_or_else(invalid)?;
    let read_id = |value: &str| Uuid::parse_str(value).map_err(|_| invalid());
    let message = read_id(fields.next().and_then(|s| s.strip_prefix("message:")).ok_or_else(invalid)?)?;
    let version = read_id(fields.next().and_then(|s| s.strip_prefix("version:")).ok_or_else(invalid)?)?;
    let parent = match fields.next().and_then(|s| s.strip_prefix("parent:")).ok_or_else(invalid)? {
        "none" => None,
        id => Some(read_id(id)?),
    };
    let turn = fields
        .next()
        .map(|field| {
            field
                .strip_prefix("turn:")
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or_else(invalid)
        })
        .transpose()?;
    let provenance = fields
        .next()
        .map(|field| {
            field
                .strip_prefix("meta:")
                .and_then(|s| serde_json::from_str(s).ok())
                .ok_or_else(invalid)
        })
        .transpose()?;
    if !matches!(role, "person" | "agent" | "automation")
        || (role == "agent") != turn.is_some()
        || turn.is_some() != provenance.is_some()
    {
        return Err(invalid());
    }
    let (text, rest) = text.split_at_checked(length).ok_or_else(invalid)?;
    let (at, instant) = line
        .split(" · ")
        .nth(if role == "agent" { 3 } else { 1 })
        .ok_or_else(invalid)
        .and_then(headings::parse_time)?;
    Ok((
        Record {
            id,
            links: Links { message, version, parent },
            role,
            heading: line,
            at,
            instant,
            turn,
            text,
            provenance,
        },
        rest,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_examples_have_exact_record_boundaries() {
        let docs = include_str!("../../../../docs/threads.md");
        let (_, examples) = docs.split_once("### Five example threads").unwrap();
        let examples = examples.split("```markdown\n").skip(1).collect::<Vec<_>>();
        assert_eq!(examples.len(), 5);
        for example in examples {
            let (file, _) = example.split_once("\n```").unwrap();
            let (_, body) = file.split_once("\n## ").unwrap();
            let body = format!("## {body}");
            assert!(!read(&body).unwrap().is_empty());
        }
    }

    #[test]
    fn exact_bytes_ignore_lookalike_headers_and_markers() {
        let id = Uuid::now_v7();
        let heading = "Maya · 2026-11-18 19:30:06 +05:30";
        let authored = format!("\r\n## {heading}\n<!-- id:{id} role:person bytes:0 -->\n\n🌏");
        let mut body = String::new();
        let mut metadata = json!({});
        append(&mut body, id, Links::next(&mut metadata, id), "person", heading, &authored, None);
        let second = Uuid::now_v7();
        append(&mut body, second, Links::next(&mut metadata, second), "person", heading, "", None);
        let decoded = read(&body).unwrap();
        assert_eq!(decoded[0].text, authored);
        assert_eq!(decoded[0].at, "2026-11-18T19:30:06+05:30");
        assert_eq!(decoded[1].text, "");
    }

    #[test]
    fn selection_and_reused_message_identity_survive_a_fork() {
        let mut body = String::new();
        let mut meta = json!({});
        let root = Uuid::now_v7();
        let other = Uuid::now_v7();
        let edit = Uuid::now_v7();
        let heading = "Maya · 2026-11-18 19:30:06 +05:30";
        append(&mut body, root, Links::next(&mut meta, root), "person", heading, "original", None);
        append(&mut body, other, Links::next(&mut meta, other), "person", heading, "old reply", None);
        append(&mut body, edit, Links { message: root, version: edit, parent: None },
            "person", heading, "edited", None);
        assert_eq!(active_version(&body).unwrap(), Some(edit));
        write!(body, "{BETWEEN}{SELECT}{other} -->").unwrap();
        assert_eq!(active_version(&body).unwrap(), Some(other));
        assert_eq!(read(&body).unwrap().len(), 3);
        write!(body, "{BETWEEN}{SELECT}{} -->", Uuid::now_v7()).unwrap();
        assert!(active_version(&body).is_err());
    }

    #[test]
    fn rejects_duplicate_ids_and_truncated_utf8_bodies() {
        let id = Uuid::now_v7();
        let heading = "Maya · 2026-11-18 19:30:06 +05:30";
        let mut body = String::new();
        let mut metadata = json!({});
        let links = Links::next(&mut metadata, id);
        append(&mut body, id, links, "person", heading, "🌏", None);
        assert!(read(&body.replace("bytes:4", "bytes:3")).is_err());
        append(&mut body, id, links, "person", heading, "other", None);
        assert!(read(&body).is_err());
    }
}
