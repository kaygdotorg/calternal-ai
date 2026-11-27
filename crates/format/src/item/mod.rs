//! Source-preserving item frontmatter, with structured value decoding.

mod records;
pub use records::{Record, parse_records};
mod yaml;

use std::{collections::BTreeMap, ops::Range};

use crate::{
    Diagnostic, Fields, Value,
    lines::{Site, header_line, header_window, newline},
};

/// An item with decoded fields and the source needed for lossless field edits.
#[derive(Debug)]
pub struct Item<'a> {
    source: &'a str,
    fields: Fields,
    entries: BTreeMap<String, Entry>,
    header_end: Option<usize>,
    body_start: usize,
    ending: &'static str,
}

#[derive(Debug)]
pub(super) struct Entry {
    pub range: Range<usize>,
    pub first_line: Range<usize>,
    pub key_end: usize,
    pub comment: Option<usize>,
    pub site: Site,
}

impl<'a> Item<'a> {
    /// The raw Markdown source, including the original frontmatter.
    pub fn source(&self) -> &'a str {
        self.source
    }

    /// The item body exactly as supplied, including line endings.
    pub fn body(&self) -> &'a str {
        &self.source[self.body_start..]
    }

    /// The parsed file fields. Built-in metadata still comes from the host.
    pub fn fields(&self) -> &Fields {
        &self.fields
    }

    pub(crate) fn site(&self, key: &str) -> Site {
        self.entries
            .get(key)
            .map_or(Site::START, |entry| entry.site)
    }
}

/// Read frontmatter from a Markdown item. The body is never rewritten on read.
pub fn parse_item(source: &str) -> Result<Item<'_>, Diagnostic> {
    let window = header_window(source);
    let ending = if window
        .lines()
        .next()
        .is_some_and(|line| source[line.len()..].starts_with("\r\n"))
    {
        "\r\n"
    } else {
        "\n"
    };
    let mut lines = window.split_inclusive('\n').enumerate();
    let Some((_, opening)) = lines.next() else {
        return Ok(Item {
            source,
            fields: Fields::new(),
            entries: BTreeMap::new(),
            header_end: None,
            body_start: 0,
            ending,
        });
    };
    if newline(opening) != "---" {
        return Ok(Item {
            source,
            fields: Fields::new(),
            entries: BTreeMap::new(),
            header_end: None,
            body_start: 0,
            ending,
        });
    }
    let mut offset = opening.len();
    let mut yaml = Vec::new();
    for (index, raw) in lines {
        let text = newline(raw);
        if text == "---" && (raw.ends_with('\n') || offset + raw.len() == source.len()) {
            let (fields, entries) = yaml::parse(&yaml)?;
            return Ok(Item {
                source,
                fields,
                entries,
                header_end: Some(offset),
                body_start: offset + raw.len(),
                ending,
            });
        }
        let line = index + 1;
        header_line(Site { line, column: 1 }, offset + raw.len())?;
        yaml.push(yaml::Line::new(text, line, offset, offset + raw.len())?);
        offset += raw.len();
    }
    Err(Site {
        line: window.lines().count() + 1,
        column: 1,
    }
    .error("Close the item frontmatter with a line containing only ---"))
}

/// Apply a field overlay to an item's original source. Empty removes a field.
pub fn write_item(item: &Item<'_>, fields: &Fields) -> Result<String, Diagnostic> {
    if fields.is_empty() {
        return Ok(item.source.into());
    }
    let mut edits = Vec::new();
    let mut additions = String::new();
    for (key, value) in fields {
        yaml::check_key(key, item.site(key))?;
        if matches!(
            key.as_str(),
            "id" | "kind" | "created" | "edited" | "at" | "author" | "anchor"
        ) {
            return Err(item
                .site(key)
                .error(format!("The host supplies the field {key}")));
        }
        let entry = item.entries.get(key);
        if value != &Value::Empty && item.fields.get(key) == Some(value) {
            continue;
        }
        match (entry, value) {
            (Some(entry), Value::Empty) => {
                let replacement = entry.comment.map_or_else(String::new, |start| {
                    format!(
                        "{}{}",
                        item.source[start..entry.first_line.end].trim_end_matches(['\r', '\n']),
                        item.ending
                    )
                });
                edits.push((entry.range.clone(), replacement));
            }
            (Some(entry), value) => {
                let key_prefix = &item.source[entry.first_line.start..entry.key_end];
                let comment = entry
                    .comment
                    .map(|start| newline(&item.source[start..entry.first_line.end]));
                let replacement = yaml::render_entry(key_prefix, value, comment, item.ending)?;
                edits.push((entry.range.clone(), replacement));
            }
            (None, Value::Empty) => {}
            (None, value) => {
                let key = if key
                    .chars()
                    .all(|ch| ch.is_alphanumeric() || matches!(ch, ' ' | '-' | '\'' | '.' | '_'))
                    && key.starts_with(char::is_alphabetic)
                    && !key.ends_with(' ')
                {
                    format!("{key}:")
                } else {
                    format!("{}:", yaml::quoted(key))
                };
                additions.push_str(&yaml::render_entry(&key, value, None, item.ending)?);
            }
        }
    }
    if let Some(end) = item.header_end {
        if !additions.is_empty() {
            edits.push((end..end, additions))
        }
    } else if !additions.is_empty() {
        let ending = item.ending;
        return Ok(format!("---{ending}{additions}---{ending}{}", item.source));
    }
    edits.sort_by_key(|(range, _)| range.start);
    let mut output = String::with_capacity(item.source.len());
    let mut offset = 0;
    for (range, replacement) in edits {
        output.push_str(&item.source[offset..range.start]);
        output.push_str(&replacement);
        offset = range.end;
    }
    output.push_str(&item.source[offset..]);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_unknown_bytes_and_comments() {
        let source = "---\r\ntitle: 'Old' # keep\r\n# outside\r\nnew.field:\r\n  flag: false\r\n  notes: |\r\n    line one\r\n    line two\r\n---\r\n# Body\r\n";
        let item = parse_item(source).unwrap();
        assert_eq!(item.body(), "# Body\r\n");
        let updates = Fields::from([("title".into(), Value::Text("New".into()))]);
        let result = write_item(&item, &updates).unwrap();
        assert_eq!(result, source.replace("'Old'", "\"New\""));
        assert_eq!(
            parse_item(&result).unwrap().fields()["new.field"],
            item.fields()["new.field"]
        );
    }

    #[test]
    fn built_ins_and_disallowed_yaml_are_refused() {
        for bad in ["&a 1", "*alias", "!tag x", "{a: 1}", "['x'"] {
            assert!(
                parse_item(&format!("---\nfield: {bad}\n---\n")).is_err(),
                "{bad}"
            );
        }
        let item = parse_item("---\nid: fake\n---\n").unwrap();
        assert!(
            write_item(
                &item,
                &Fields::from([("id".into(), Value::Text("fake".into()))])
            )
            .is_err()
        );
    }
    #[test]
    fn frontmatter_limits_apply_before_body_and_reject_structural_controls() {
        let too_many_lines = format!("---\n{}---\n", "# ignored\n".repeat(4097));
        let too_many_bytes = format!("---\ntext: {}\n---\n", "a".repeat(256 * 1024));
        let too_many_entries = format!("---\nlist: [{}0]\n---\n", "0,".repeat(4096));
        for source in [&too_many_lines, &too_many_bytes, &too_many_entries] {
            assert!(parse_item(source).is_err());
        }
        for key in ["bad\u{202e}", r#""bad\u202e""#, r#""bad\u0001""#] {
            assert!(parse_item(&format!("---\nouter:\n  {key}: text\n---\n")).is_err());
        }
        let body = format!("\u{202e} literal\n{}", "a".repeat(256 * 1024));
        let source = format!("---\nvalid: yes\n---\n{body}");
        assert_eq!(parse_item(&source).unwrap().body(), body);
    }
}
