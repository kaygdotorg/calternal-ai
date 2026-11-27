//! Definition grammar and source positions shared by the schema parsers.

use crate::Diagnostic;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Site {
    pub line: usize,
    pub column: usize,
}

impl Site {
    pub const START: Self = Self { line: 1, column: 1 };

    pub fn error(self, message: impl Into<String>) -> Diagnostic {
        Diagnostic::at(self.line, self.column, message)
    }

    pub fn after(self, text: &str) -> Self {
        Self {
            column: self.column + text.chars().count(),
            ..self
        }
    }
}

#[derive(Debug)]
pub(crate) struct Line {
    pub text: String,
    pub site: Site,
    pub depth: usize,
    continuations: Vec<(usize, Site)>,
}

impl Line {
    pub fn site_at(&self, offset: usize) -> Site {
        let (start, site) = self
            .continuations
            .iter()
            .rev()
            .find(|(start, _)| *start <= offset)
            .copied()
            .unwrap_or((0, self.site));
        site.after(&self.text[start..offset])
    }

    pub fn parts<'a>(&'a self, value: &'a str) -> Result<Vec<(&'a str, Site)>, Diagnostic> {
        let start = value.as_ptr() as usize - self.text.as_ptr() as usize;
        parts_at(value, |offset| self.site_at(start + offset))
    }

    pub fn entry(&self) -> Result<Option<(&str, &str, Site)>, Diagnostic> {
        let scan = scan(&self.text, |offset| self.site_at(offset))?;
        let text = self.text[..scan.end].trim_end();
        Ok(scan.colon.map(|colon| {
            let value = text[colon + 1..].trim_start_matches(' ');
            let value_start = text.len() - value.len();
            (&text[..colon], value, self.site_at(value_start))
        }))
    }

    pub fn content(&self) -> Result<&str, Diagnostic> {
        let scan = scan(&self.text, |offset| self.site_at(offset))?;
        Ok(self.text[..scan.end].trim_end())
    }
}

pub(crate) struct Document<'a> {
    pub header: Vec<Line>,
    pub body: &'a str,
    pub body_site: Site,
}

const HEADER_BYTES: usize = 256 * 1024;
const HEADER_PHYSICAL_LINES: usize = 4096;
const HEADER_LOGICAL_LINES: usize = 2048;

/// Limit the portion visited when searching for a frontmatter delimiter.
pub(crate) fn header_window(source: &str) -> &str {
    // Leave room to inspect the closing delimiter and its CRLF.
    &source[..source.floor_char_boundary(source.len().min(HEADER_BYTES + 5))]
}

/// Refuse work beyond the frontmatter byte and physical-line budgets.
pub(crate) fn header_line(site: Site, bytes: usize) -> Result<(), Diagnostic> {
    if bytes > HEADER_BYTES || site.line > HEADER_PHYSICAL_LINES + 1 {
        return Err(site.error("Frontmatter exceeds 256 KiB or 4096 physical lines"));
    }
    Ok(())
}

pub(crate) fn document(source: &str) -> Result<Document<'_>, Diagnostic> {
    let window = header_window(source);
    let mut physical = window.split_inclusive('\n').enumerate();
    if !physical
        .next()
        .is_some_and(|(_, line)| line.ends_with('\n') && newline(line) == "---")
    {
        return Err(Site::START.error("Start the definition with a line containing only ---"));
    }
    let mut header: Vec<Line> = Vec::new();
    let mut offset = source.find('\n').unwrap_or(0) + 1;
    for (index, raw) in physical {
        offset += raw.len();
        let line = index + 1;
        let text = newline(raw);
        let site = Site { line, column: 1 };
        if text == "---" && (raw.ends_with('\n') || offset == source.len()) {
            for entry in &header {
                entry.content()?;
            }
            return Ok(Document {
                header,
                body: &source[offset..],
                body_site: Site {
                    line: line + 1,
                    column: 1,
                },
            });
        }
        header_line(site, offset)?;
        if let Some(tab) = text.find('\t') {
            return Err(site
                .after(&text[..tab])
                .error("Use spaces instead of a tab"));
        }
        if let Some((offset, _)) = text.char_indices().find(|(_, ch)| forbidden(*ch)) {
            return Err(site
                .after(&text[..offset])
                .error("Use text without control or format characters"));
        }
        if !is_nfc(text) {
            return Err(site.error("Use Unicode text normalized to NFC"));
        }
        let content = text.trim_start_matches(' ');
        let indent = text.len() - content.len();
        let site = site.after(&text[..indent]);
        if content.is_empty() || content == "#" || content.starts_with("# ") {
            continue;
        }
        if !indent.is_multiple_of(2) {
            return Err(site.error("Indent definitions in steps of two spaces"));
        }
        let depth = indent / 2;
        let previous_depth = header.last().map(|entry| entry.depth);
        match previous_depth {
            Some(previous) if depth > previous + 1 => {
                let previous = header.last_mut().unwrap();
                previous.text.push(' ');
                previous.continuations.push((previous.text.len(), site));
                previous.text.push_str(content);
            }
            None if depth != 0 => return Err(site.error("Start the first key at the left margin")),
            _ if depth > 64 => return Err(site.error("Definition nesting exceeds 64 levels")),
            _ => {
                if header.len() == HEADER_LOGICAL_LINES {
                    return Err(site.error("Definition frontmatter exceeds 2048 entries"));
                }
                header.push(Line {
                    text: content.to_owned(),
                    site,
                    depth,
                    continuations: Vec::new(),
                });
            }
        }
    }
    Err(Site {
        line: window.lines().count() + 1,
        column: 1,
    }
    .error("Close the definition with a line containing only ---"))
}

pub(crate) fn newline(raw: &str) -> &str {
    raw.strip_suffix("\r\n")
        .or_else(|| raw.strip_suffix('\n'))
        .unwrap_or(raw)
}

struct Scan {
    end: usize,
    colon: Option<usize>,
    commas: Vec<usize>,
}

fn scan(text: &str, site_at: impl Fn(usize) -> Site) -> Result<Scan, Diagnostic> {
    let mut result = Scan {
        end: text.len(),
        colon: None,
        commas: Vec::new(),
    };
    let mut groups = Vec::new();
    let mut quote = None;
    let mut chars = text.char_indices().peekable();
    while let Some((offset, ch)) = chars.next() {
        let next = chars.peek().map(|(_, c)| *c);
        let previous = text[..offset].chars().next_back();
        if let Some((delimiter, _)) = quote {
            if ch == delimiter {
                quote = None;
            } else if ch == '\\' && delimiter == '"' {
                if !matches!(next, Some('"' | '\\')) {
                    return Err(site_at(offset).error(
                        "Escape only a double quote or backslash in quoted definition text",
                    ));
                }
                chars.next();
            }
            continue;
        }
        if matches!(ch, '"' | '\x60') {
            quote = Some((ch, offset));
            continue;
        }
        match ch {
            '(' => groups.push((')', offset)),
            '[' => groups.push((']', offset)),
            '<' if !matches!(next, None | Some(' ' | '=' | '<'))
                && !previous.is_some_and(|c| c.is_alphanumeric() || matches!(c, ')' | ']'))
                && text[offset + 1..].contains('>') =>
            {
                groups.push(('>', offset))
            }
            ')' | ']' | '>' if groups.last().is_some_and(|(end, _)| *end == ch) => {
                groups.pop();
            }
            ')' | ']' => return Err(site_at(offset).error(format!("Unexpected closing {ch}"))),
            '#' if groups.is_empty()
                && matches!(previous, None | Some(' '))
                && matches!(next, None | Some(' ')) =>
            {
                result.end = offset;
                break;
            }
            ':' if groups.is_empty()
                && matches!(next, None | Some(' '))
                && result.colon.is_none() =>
            {
                result.colon = Some(offset)
            }
            ',' if groups.is_empty() && next == Some(' ') => result.commas.push(offset),
            _ => {}
        }
        if groups.len() > 64 {
            return Err(site_at(offset).error("Definition groups exceed 64 levels"));
        }
    }
    if let Some((_, offset)) = quote {
        return Err(site_at(offset).error("Close the quoted text"));
    }
    if let Some((end, offset)) = groups.last() {
        return Err(site_at(*offset).error(format!("Close the group with {end}")));
    }
    if result.colon.is_none() && text[..result.end].trim_end().ends_with(':') {
        result.colon = Some(text[..result.end].trim_end().len() - 1);
    }
    Ok(result)
}

pub(crate) fn uncommented(text: &str, site: Site) -> Result<&str, Diagnostic> {
    let end = scan(text, |offset| site.after(&text[..offset]))?.end;
    Ok(text[..end].trim_end())
}

pub(crate) fn parts(text: &str, site: Site) -> Result<Vec<(&str, Site)>, Diagnostic> {
    parts_at(text, |offset| site.after(&text[..offset]))
}

fn parts_at(source: &str, site: impl Fn(usize) -> Site) -> Result<Vec<(&str, Site)>, Diagnostic> {
    let text = source.trim_end();
    let text = text
        .strip_prefix('[')
        .and_then(|inside| inside.strip_suffix(']'))
        .filter(|inside| scan(inside, |_| site(0)).is_ok())
        .unwrap_or(text);
    let offset = text.as_ptr() as usize - source.as_ptr() as usize;
    let scan = scan(text, |position| site(offset + position))?;
    let text = text[..scan.end].trim_end();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let mut start = 0;
    scan.commas
        .into_iter()
        .chain(std::iter::once(text.len()))
        .map(|end| {
            let part = text.get(start..end).unwrap_or_default();
            let trimmed = part.trim_start();
            let position = site(offset + start + part.len() - trimmed.len());
            start = end + 2;
            if trimmed.trim_end().is_empty() {
                Err(position.error("Write a value between commas"))
            } else {
                Ok((trimmed.trim_end(), position))
            }
        })
        .collect()
}

pub(crate) fn unquote(text: &str, site: Site) -> Result<String, Diagnostic> {
    let Some(inner) = text.strip_prefix('"') else {
        return Ok(text.into());
    };
    let inner = inner
        .strip_suffix('"')
        .ok_or_else(|| site.error("Close the quoted text"))?;
    let mut result = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.next() {
                Some(escaped @ ('"' | '\\')) => result.push(escaped),
                _ => return Err(site.error("Escape only a double quote or backslash")),
            },
            '"' => return Err(site.error("Escape a double quote inside quoted text")),
            _ => result.push(ch),
        }
    }
    Ok(result)
}

const FORMULA: &[&str] = &[
    "and",
    "or",
    "not",
    "in",
    "yes",
    "no",
    "true",
    "false",
    "empty",
    "this",
    "open item",
    "item",
    "period",
    "records",
    "settings",
    "state",
    "ask",
];
const BUILTINS: &[&str] = &["id", "kind", "created", "edited", "author", "at", "anchor"];
const OPERATIONS: &[&str] = &[
    "add",
    "change",
    "remove",
    "duplicate",
    "set",
    "open",
    "where",
    "timer",
    "after",
    "within",
    "only",
    "when",
    "code",
    "append record",
    "change record",
    "remove record",
    "swipe",
    "menu",
    "primary",
    "mark",
];
const CREDENTIALS: &[&str] = &["password", "token", "secret"];

pub(crate) fn field_name(name: &str, site: Site) -> Result<(), Diagnostic> {
    name_shape(name, site)?;
    if FORMULA.contains(&name)
        || BUILTINS.contains(&name)
        || OPERATIONS.contains(&name)
        || CREDENTIALS.contains(&name)
        || name
            .split(' ')
            .any(|word| ["and", "or", "not", "in"].contains(&word))
    {
        return Err(site.error(format!("The field name {name} is reserved")));
    }
    Ok(())
}

pub(crate) fn operation_name(name: &str, site: Site) -> Result<(), Diagnostic> {
    name_shape(name, site)?;
    if FORMULA.contains(&name) || BUILTINS.contains(&name) || OPERATIONS.contains(&name) {
        return Err(site.error(format!("The operation name {name} is reserved")));
    }
    Ok(())
}

fn name_shape(name: &str, site: Site) -> Result<(), Diagnostic> {
    if !name.chars().next().is_some_and(char::is_alphabetic)
        || name.chars().count() > 64
        || name.ends_with(' ')
        || name.contains("  ")
        || name
            .chars()
            .any(|ch| !(ch.is_alphanumeric() || matches!(ch, ' ' | '-' | '\'')))
        || !is_nfc(name)
    {
        return Err(site.error(format!("Use a name starting with a letter and containing at most 64 letters, digits, hyphens, apostrophes or single spaces: {name}")));
    }
    Ok(())
}

pub(crate) fn id(name: &str, site: Site) -> Result<(), Diagnostic> {
    if name.len() > 64
        || !name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        || name
            .bytes()
            .any(|ch| !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == b'-'))
    {
        return Err(site.error(format!("Use an ID starting with a lowercase ASCII letter and containing at most 64 lowercase letters, digits or hyphens: {name}")));
    }
    Ok(())
}

pub(crate) fn kind_id(name: &str, site: Site) -> Result<(), Diagnostic> {
    id(name, site)?;
    if FORMULA.contains(&name)
        || BUILTINS.contains(&name)
        || ["files", "lines", "space", "home"].contains(&name)
    {
        return Err(site.error(format!("The kind ID {name} is reserved")));
    }
    Ok(())
}

pub(crate) fn is_nfc(text: &str) -> bool {
    #[cfg(feature = "unicode")]
    {
        unicode_normalization::is_nfc(text)
    }
    #[cfg(not(feature = "unicode"))]
    {
        text.is_ascii()
    }
}

pub(crate) fn forbidden(ch: char) -> bool {
    if ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}') {
        return true;
    }
    #[cfg(feature = "unicode")]
    {
        use unicode_general_category::{GeneralCategory, get_general_category};
        get_general_category(ch) == GeneralCategory::Format
    }
    #[cfg(not(feature = "unicode"))]
    {
        !ch.is_ascii()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definition_lines_keep_physical_positions() {
        let source = "---\r\nfills:\r\n  calternal-chat:composer: frame frames/composer.html # comment\r\nwhere: count(tasks,\r\n    status = \"done, yes\") > 0\r\n---\r\nbody\r\n";
        let parsed = document(source).unwrap();
        assert_eq!(
            parsed.header[1].entry().unwrap().unwrap().0,
            "calternal-chat:composer"
        );
        assert_eq!(
            parsed.header[2].entry().unwrap().unwrap().1,
            "count(tasks, status = \"done, yes\") > 0"
        );
        assert_eq!(parsed.body, "body\r\n");
        assert_eq!(parsed.body_site.line, 7);
        let error = document(&source.replace("yes\"", "yes\\q\""))
            .err()
            .unwrap();
        assert_eq!((error.line, error.column), (5, 24));
    }

    #[test]
    fn lists_split_outside_groups() {
        let text = "set note = \"late, when overdue\", when count(tasks, open) = 0, #c96f4a";
        let parts: Vec<_> = parts(text, Site::START)
            .unwrap()
            .into_iter()
            .map(|(part, _)| part)
            .collect();
        assert_eq!(
            parts,
            [
                "set note = \"late, when overdue\"",
                "when count(tasks, open) = 0",
                "#c96f4a"
            ]
        );
        assert_eq!(
            unquote("\"say \\\"hi\\\"\"", Site::START).unwrap(),
            "say \"hi\""
        );
    }

    #[test]
    fn names_follow_appendix_a() {
        for field in ["due, soft", "ratio a:b", "reps in reserve", "password"] {
            assert!(field_name(field, Site::START).is_err());
        }
        assert!(field_name("is open", Site::START).is_ok());
        assert!(field_name("plan-day", Site::START).is_ok());
        assert!(kind_id("files", Site::START).is_err());
        assert!(kind_id("records", Site::START).is_err());
    }
}

#[cfg(test)]
mod appendix {
    use super::*;
    use crate::{EvalContext, Fields, Row, Value, evaluate, parse_expression};

    #[test]
    fn appendix_a_header_grammar() {
        let input = "---\nextends:\n  [calternal-mail.messages]:\n    fields:\n      saved: yes/no\nical: VEVENT\n  title: SUMMARY\nfills:\n  calternal-chat:composer: frame frames/composer.html\n---\n";
        let result = document(input).unwrap();
        assert_eq!(
            result.header[..4]
                .iter()
                .map(|line| line.depth)
                .collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        assert_eq!(
            result.header[1].entry().unwrap().unwrap().0,
            "[calternal-mail.messages]"
        );
        assert_eq!(result.header[4].entry().unwrap().unwrap().1, "VEVENT");
        assert_eq!(result.header[5].depth, 1);
        assert_eq!(
            result.header[7].entry().unwrap().unwrap().0,
            "calternal-chat:composer"
        );
        assert_eq!(
            result.header[7].entry().unwrap().unwrap().1,
            "frame frames/composer.html"
        );
        assert_eq!(
            parts("colour, default #c96f4a", Site::START).unwrap()[1].0,
            "default #c96f4a"
        );
    }

    #[cfg(feature = "unicode")]
    #[test]
    fn appendix_a_nfc_case_is_significant() {
        field_name("größe", Site::START).unwrap();
        field_name("Größe", Site::START).unwrap();
        let item = crate::parse_item("---\ngröße: 1\nGröße: 2\n---\n").unwrap();
        assert_ne!(item.fields()["größe"], item.fields()["Größe"]);
    }

    #[test]
    fn appendix_a_choices_and_durations() {
        let field = crate::field::test_declaration("choice(did not happen)", Site::START).unwrap();
        assert_eq!(
            field.field_type,
            Some(crate::FieldType::Choice(vec!["did not happen".into()]))
        );
        let title = Fields::from([
            ("status".into(), Value::Text("did not happen".into())),
            ("priority".into(), Value::Text("high".into())),
            ("urgent".into(), Value::Bool(true)),
        ]);
        let metadata = Fields::new();
        let row = Row {
            fields: &title,
            metadata: &metadata,
        };
        for condition in [
            "status = \"did not happen\"",
            "priority in (\"urgent\", \"high\")",
            "urgent",
            "not read",
        ] {
            assert_eq!(
                evaluate(
                    &parse_expression(condition).unwrap(),
                    row,
                    &EvalContext::default()
                )
                .unwrap(),
                Value::Bool(true)
            );
        }
        assert!(crate::dates::duration("6m").is_none());
        let diagnostic =
            crate::field::test_declaration("duration, default 2m", Site { line: 6, column: 4 })
                .unwrap_err();
        assert_eq!(diagnostic.line, 6);
        assert!(diagnostic.message.contains("2min"));
        assert!(parse_expression("now() + 1mo").is_ok());
    }
    #[test]
    fn definition_frontmatter_limits_count_continuations_and_entries() {
        let continuation = format!(
            "---\nfields:\n  title: text\n{}---\n",
            "      word\n".repeat(4096)
        );
        let entries = format!("---\n{}---\n", "name: word\n".repeat(2049));
        let bytes = format!("---\nname: {}\n---\n", "a".repeat(256 * 1024));
        for source in [&continuation, &entries, &bytes] {
            assert!(document(source).is_err());
        }
    }
}
