use std::ops::Range;

use crate::{
    Diagnostic, Fields, Kind, dates, field,
    lines::{Site, newline},
};

use super::{Item, yaml};

/// A record-shaped span found in an item's Markdown body.
///
/// Header-shaped text is provisional until the host matches the span and
/// anchor to the ledger. Authorship comes from that ledger.
#[derive(Debug)]
pub struct Record<'a> {
    /// Candidate anchor found in the file.
    pub anchor: &'a str,
    /// Byte range in the original item, covering header, fields and body.
    pub range: Range<usize>,
    /// Parsed values for the declared record fields.
    pub fields: Fields,
    /// The record's Markdown body after its declared fields.
    pub body: &'a str,
}

/// Decode candidate records from a Markdown item in one pass over the body.
/// Only consecutive declared fields after a header are read as fields.
pub fn parse_records<'a>(kind: &Kind, item: &Item<'a>) -> Result<Vec<Record<'a>>, Diagnostic> {
    let source = item.source();
    let mut offset = item.body_start;
    let initial_line = source[..offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    let mut current = None;
    let mut records = Vec::new();
    for (index, raw) in item.body().split_inclusive('\n').enumerate() {
        let line = newline(raw);
        let anchor = header(line);
        if anchor.is_some() || line == "---" {
            if let Some(span) = current.take() {
                records.push(finish(span, offset, source));
            }
            if let Some(anchor) = anchor {
                current = Some(Span {
                    anchor,
                    start: offset,
                    body_start: offset + raw.len(),
                    fields: Fields::new(),
                    reading_fields: true,
                });
            }
        } else if let Some(span) = &mut current
            && span.reading_fields
        {
            let site = Site {
                line: initial_line + index,
                column: 1,
            };
            let declared = line.split_once(':').and_then(|(key, _)| {
                kind.records
                    .get(key)?
                    .field_type
                    .as_ref()
                    .map(|kind| (key, kind))
            });
            if let Some((key, ty)) = declared {
                let value = yaml::record_field(line, site)?;
                let value = field::coerce(ty, &value, site)?;
                if span.fields.insert(key.into(), value).is_some() {
                    return Err(site.error(format!("Record field {key} is written twice")));
                }
                span.body_start = offset + raw.len();
            } else {
                span.reading_fields = false;
            }
        }
        offset += raw.len();
    }
    if let Some(span) = current {
        records.push(finish(span, offset, source));
    }
    Ok(records)
}

struct Span<'a> {
    anchor: &'a str,
    start: usize,
    body_start: usize,
    fields: Fields,
    reading_fields: bool,
}

fn finish<'a>(span: Span<'a>, end: usize, text: &'a str) -> Record<'a> {
    Record {
        anchor: span.anchor,
        range: span.start..end,
        fields: span.fields,
        body: &text[span.body_start..end],
    }
}

fn header(text: &str) -> Option<&str> {
    let (name, tail) = text.strip_prefix("**")?.split_once("** · ")?;
    let (receipt, suffix) = tail.split_once(" ^")?;
    let anchor = if let Some((anchor, comment)) = suffix.split_once(' ') {
        if !comment.starts_with("<!--") || !comment.ends_with("-->") {
            return None;
        }
        anchor
    } else {
        suffix
    };
    if name.is_empty()
        || receipt.len() != 16
        || !receipt.get(10..)?.starts_with(' ')
        || dates::time(receipt).is_none()
        || !field::anchor(anchor)
    {
        return None;
    }
    Some(anchor)
}
