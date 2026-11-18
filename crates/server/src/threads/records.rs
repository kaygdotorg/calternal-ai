//! Self-delimiting transcript records. Authored Markdown is treated as bytes.
use super::headings;
use calternal_types::Error;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt::Write as _};
use uuid::Uuid;

const BETWEEN: &str = "\n\n";

/// One validated record. The heading is display text and carries no authority.
#[derive(Debug, Serialize)]
pub(super) struct Record<'a> {
    pub(super) id: Uuid,
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
}

fn invalid() -> Error {
    Error::conflict("invalid thread message framing; restore the committed thread file")
}

/// Add one exact Markdown body and its identity line. The byte count is outside
/// the body and independent of any heading or delimiter lookalikes in it.
pub(super) fn append(
    out: &mut String,
    id: Uuid,
    role: &str,
    heading: &str,
    text: &str,
    agent: Option<(Uuid, &Provenance)>,
) -> usize {
    if !out.is_empty() {
        out.push_str(BETWEEN);
    }
    let offset = out.len();
    write!(
        out,
        "## {heading}\n<!-- id:{id} role:{role} bytes:{}",
        text.len()
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
        let (record, tail) = first(input)?;
        if !seen.insert(record.id) {
            return Err(invalid());
        }
        records.push(record);
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

/// Load the active record from its indexed offset without rescanning earlier text.
pub(super) fn tail(body: &str, start: usize, id: Uuid) -> Result<Record<'_>, Error> {
    let (record, tail) = first(body.get(start..).ok_or_else(invalid)?)?;
    if !tail.is_empty() || record.id != id || record.role != "agent" {
        return Err(invalid());
    }
    Ok(record)
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
    let mut fields = marker.splitn(5, ' ');
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
        append(&mut body, id, "person", heading, &authored, None);
        append(&mut body, Uuid::now_v7(), "person", heading, "", None);
        let decoded = read(&body).unwrap();
        assert_eq!(decoded[0].text, authored);
        assert_eq!(decoded[0].at, "2026-11-18T19:30:06+05:30");
        assert_eq!(decoded[1].text, "");
    }

    #[test]
    fn rejects_duplicate_ids_and_truncated_utf8_bodies() {
        let id = Uuid::now_v7();
        let heading = "Maya · 2026-11-18 19:30:06 +05:30";
        let mut body = String::new();
        append(&mut body, id, "person", heading, "🌏", None);
        assert!(read(&body.replace("bytes:4", "bytes:3")).is_err());
        append(&mut body, id, "person", heading, "other", None);
        assert!(read(&body).is_err());
    }
}
