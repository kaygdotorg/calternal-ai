use std::collections::BTreeMap;

use crate::{
    Diagnostic, Fields, Value,
    lines::{Site, forbidden, header_line},
};

use super::Entry;

mod render;
mod scalar_lines;
pub(super) use render::{quoted, render_entry};

#[derive(Clone, Copy)]
pub(super) struct Line<'a> {
    text: &'a str,
    indent: usize,
    site: Site,
    start: usize,
    end: usize,
}

impl<'a> Line<'a> {
    pub(super) fn new(
        text: &'a str,
        number: usize,
        start: usize,
        end: usize,
    ) -> Result<Self, Diagnostic> {
        let indent = text.len() - text.trim_start_matches(' ').len();
        let site = Site {
            line: number,
            column: indent + 1,
        };
        if text[indent..].starts_with('\t') {
            return Err(site.error("Indent frontmatter with spaces"));
        }
        Ok(Self {
            text,
            indent,
            site,
            start,
            end,
        })
    }

    fn significant(self) -> bool {
        let text = self.text[self.indent..].trim_end();
        !text.is_empty() && !text.starts_with('#')
    }
}

type Entries = BTreeMap<String, Entry>;
const MAX_ENTRIES: usize = 4096;
const MAX_KEY_BYTES: usize = 256;

fn entry(remaining: &mut usize, site: Site) -> Result<(), Diagnostic> {
    *remaining = remaining
        .checked_sub(1)
        .ok_or_else(|| site.error("Item frontmatter exceeds 4096 entries"))?;
    Ok(())
}

/// Validate a decoded key before storing it as item structure.
pub(super) fn check_key(key: &str, site: Site) -> Result<(), Diagnostic> {
    if key.is_empty() || key.len() > MAX_KEY_BYTES || key.chars().any(forbidden) {
        return Err(
            site.error("Use a nonempty key within 256 bytes without control or format characters")
        );
    }
    Ok(())
}

pub(super) fn parse(lines: &[Line<'_>]) -> Result<(Fields, Entries), Diagnostic> {
    let mut parser = Parser {
        lines,
        at: 0,
        remaining: MAX_ENTRIES,
    };
    let mut entries = Entries::new();
    let values = parser.map(0, 0, Some(&mut entries))?;
    if let Some(line) = parser.peek() {
        return Err(line.site.error("Unexpected frontmatter indentation"));
    }
    Ok((values, entries))
}

struct Parser<'a> {
    lines: &'a [Line<'a>],
    at: usize,
    remaining: usize,
}

impl<'a> Parser<'a> {
    fn peek(&mut self) -> Option<Line<'a>> {
        while self
            .lines
            .get(self.at)
            .is_some_and(|line| !line.significant())
        {
            self.at += 1
        }
        self.lines.get(self.at).copied()
    }

    fn map(
        &mut self,
        indent: usize,
        depth: usize,
        mut entries: Option<&mut Entries>,
    ) -> Result<Fields, Diagnostic> {
        let mut fields = Fields::new();
        while let Some(line) = self
            .peek()
            .filter(|line| line.indent == indent && list_entry(&line.text[indent..]).is_none())
        {
            entry(&mut self.remaining, line.site)?;
            let text = &line.text[indent..];
            let (key, value, colon) = key_value(text, line.site)?;
            if fields.contains_key(&key) {
                return Err(line.site.error(format!("Field {key} is written twice")));
            }
            let start = self.at;
            self.at += 1;
            let (value, comment) = without_comment(value);
            let prefix = text.len() - (text[colon + 1..].trim_start_matches(' ')).len();
            let value_site = line.site.after(&text[..prefix]);
            let mut trailing = comment.map(|index| line.start + indent + prefix + index..line.end);
            let parsed = self.value(
                value.trim_end_matches([' ', '\t']),
                line,
                value_site,
                indent,
                depth,
                &mut trailing,
            )?;
            let mut last = self.at;
            while last > start + 1 && !self.lines[last - 1].significant() && !is_block(value) {
                last -= 1
            }
            if let Some(entries) = entries.as_deref_mut() {
                entries.insert(
                    key.clone(),
                    Entry {
                        range: line.start..self.lines[last - 1].end,
                        first_line: line.start..line.end,
                        key_end: line.start + indent + colon + 1,
                        comment: trailing,
                        site: line.site,
                    },
                );
            }
            fields.insert(key, parsed);
        }
        Ok(fields)
    }

    fn value(
        &mut self,
        raw: &'a str,
        line: Line<'a>,
        site: Site,
        indent: usize,
        depth: usize,
        comment: &mut Option<std::ops::Range<usize>>,
    ) -> Result<Value, Diagnostic> {
        if depth > 64 {
            return Err(site.error("Keep frontmatter nesting within 64 levels"));
        }
        if is_block(raw) {
            return self.block(raw, line, indent);
        }
        if !raw.is_empty() {
            let raw = self.scalar_lines(raw, site, indent, comment)?;
            return scalar(&raw, site, depth, &mut self.remaining);
        }
        let Some(next) = self.peek() else {
            return Ok(Value::Empty);
        };
        if next.indent >= indent && list_entry(&next.text[next.indent..]).is_some() {
            return self.list(next.indent, depth + 1).map(Value::List);
        }
        if next.indent > indent {
            return self.map(next.indent, depth + 1, None).map(Value::Map);
        }
        Ok(Value::Empty)
    }

    fn list(&mut self, indent: usize, depth: usize) -> Result<Vec<Value>, Diagnostic> {
        let mut values = Vec::new();
        while let Some(line) = self.peek().filter(|line| line.indent == indent) {
            let Some(rest) = list_entry(&line.text[indent..]) else {
                break;
            };
            let (raw, _) = without_comment(rest);
            let raw = raw.trim_end_matches([' ', '\t']);
            let site = line.site.after("- ");
            entry(&mut self.remaining, site)?;
            self.at += 1;
            if !raw.is_empty() && mapping_colon(raw).is_some() && !raw.starts_with('[') {
                entry(&mut self.remaining, site)?;
                let (key, text, _) = key_value(raw, site)?;
                let value = self.value(
                    text.trim_end_matches([' ', '\t']),
                    line,
                    site,
                    indent + 2,
                    depth,
                    &mut None,
                )?;
                let mut fields = Fields::from([(key.clone(), value)]);
                if let Some(next) = self.peek().filter(|next| next.indent > indent) {
                    for (key, value) in self.map(next.indent, depth + 1, None)? {
                        if fields.insert(key.clone(), value).is_some() {
                            return Err(next.site.error(format!("Field {key} is written twice")));
                        }
                    }
                }
                values.push(Value::Map(fields));
            } else {
                // A sibling dash is the next value, not a nested collection.
                if raw.is_empty() && self.peek().is_none_or(|next| next.indent <= indent) {
                    values.push(Value::Empty);
                } else {
                    values.push(self.value(raw, line, site, indent, depth, &mut None)?);
                }
            }
        }
        Ok(values)
    }

    fn block(&mut self, flag: &str, line: Line<'a>, parent: usize) -> Result<Value, Diagnostic> {
        let mut content = Vec::new();
        let indent = flag
            .bytes()
            .find(u8::is_ascii_digit)
            .map(|digit| parent + usize::from(digit - b'0'))
            .or_else(|| {
                self.lines[self.at..]
                    .iter()
                    .find(|line| !line.text.trim().is_empty())
                    .map(|line| line.indent)
            })
            .unwrap_or(parent + 1)
            .max(parent + 1);
        while let Some(next) = self.lines.get(self.at) {
            if !next.text.trim().is_empty() && next.indent < indent {
                break;
            }
            content.push(next.text.get(indent..).unwrap_or(""));
            self.at += 1;
        }
        if flag.len() > 3
            || flag[1..]
                .chars()
                .any(|ch| !matches!(ch, '-' | '+' | '1'..='9'))
        {
            return Err(line.site.error("Use a valid block string indicator"));
        }
        let mut text = String::new();
        for (index, part) in content.iter().enumerate() {
            text.push_str(part);
            let fold = flag.starts_with('>')
                && !part.is_empty()
                && !part.starts_with(' ')
                && content
                    .get(index + 1)
                    .is_some_and(|next| !next.is_empty() && !next.starts_with(' '));
            text.push(if fold { ' ' } else { '\n' });
        }
        if !flag.contains('+') {
            let had_break = text.ends_with('\n');
            text.truncate(text.trim_end_matches('\n').len());
            if had_break && !flag.contains('-') {
                text.push('\n')
            }
        }
        Ok(Value::Text(text))
    }
}

fn is_block(text: &str) -> bool {
    text.starts_with(['|', '>'])
}

fn list_entry(text: &str) -> Option<&str> {
    if text == "-" {
        Some("")
    } else {
        text.strip_prefix("- ")
    }
}

fn mapping_colon(text: &str) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match quote {
            Some('"') if ch == '\\' => escaped = true,
            Some(end) if end == ch => quote = None,
            Some(_) => {}
            None if matches!(ch, '"' | '\'') && index == 0 => quote = Some(ch),
            None if ch == ':'
                && (index + 1 == text.len() || text[index + 1..].starts_with(' ')) =>
            {
                return Some(index);
            }
            None => {}
        }
    }
    None
}

fn key_value(text: &str, site: Site) -> Result<(String, &str, usize), Diagnostic> {
    let colon =
        mapping_colon(text).ok_or_else(|| site.error("Write an item field as key: value"))?;
    let key = text[..colon].trim_end();
    let key = match key.chars().next() {
        Some('"' | '\'') => decode_quote(key, site)?,
        Some('&' | '*' | '!' | '{' | '}' | '?' | '[' | ']' | '%' | '@' | '\x60') | None => {
            return Err(site.error("Use a plain or quoted key, without YAML extensions"));
        }
        _ => key.into(),
    };
    check_key(&key, site)?;
    if key == "<<" {
        return Err(site.error("YAML merge keys are forbidden"));
    }
    Ok((key, text[colon + 1..].trim_start_matches(' '), colon))
}

fn plain_comment(text: &str) -> (&str, Option<usize>) {
    let mut separated = true;
    for (index, ch) in text.char_indices() {
        if ch == '#' && separated {
            return (&text[..index], Some(index));
        }
        separated = matches!(ch, ' ' | '\t');
    }
    (text, None)
}

fn without_comment(text: &str) -> (&str, Option<usize>) {
    if !text.starts_with(['"', '\'', '[']) {
        return plain_comment(text);
    }
    let mut quote = None;
    let mut escaped = false;
    let mut previous = ' ';
    for (index, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match quote {
            Some('"') if ch == '\\' => escaped = true,
            Some(end) if ch == end => quote = None,
            Some(_) => {}
            None if matches!(ch, '"' | '\'')
                && (previous.is_whitespace() || matches!(previous, '[' | ',')) =>
            {
                quote = Some(ch)
            }
            None if ch == '#' && previous.is_whitespace() => {
                return (&text[..index], Some(index));
            }
            None => {}
        }
        previous = ch;
    }
    (text, None)
}

fn scalar(
    text: &str,
    site: Site,
    depth: usize,
    remaining: &mut usize,
) -> Result<Value, Diagnostic> {
    if depth > 64 {
        return Err(site.error("Keep frontmatter nesting within 64 levels"));
    }
    if text.is_empty() {
        return Ok(Value::Empty);
    }
    match text.as_bytes()[0] {
        b'"' | b'\'' => decode_quote(text, site).map(Value::Text),
        b'[' if text.starts_with("[[") && text.ends_with("]]") && !text.contains(',') => {
            Ok(Value::Text(text.into()))
        }
        b'[' => flow(text, site, depth, remaining),
        b'&' | b'*' | b'!' | b'{' | b'}' => Err(site
            .error("Anchors, aliases, tags and flow mappings are forbidden in item frontmatter")),
        _ if mapping_colon(text).is_some() => {
            Err(site.error("Quote a scalar containing a colon followed by a space"))
        }
        _ => Ok(match text {
            "null" | "Null" | "NULL" | "~" => Value::Empty,
            "true" | "True" | "TRUE" => Value::Bool(true),
            "false" | "False" | "FALSE" => Value::Bool(false),
            _ => {
                if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0o")) {
                    let radix = if text.starts_with("0x") { 16 } else { 8 };
                    if let Ok(number) = i128::from_str_radix(hex, radix) {
                        return Ok(Value::Number(number.to_string().parse().map_err(|_| {
                            site.error("This number exceeds the supported range")
                        })?));
                    }
                }
                text.parse()
                    .map_or_else(|_| Value::Text(text.into()), Value::Number)
            }
        }),
    }
}

fn flow(text: &str, site: Site, depth: usize, remaining: &mut usize) -> Result<Value, Diagnostic> {
    let tail = text
        .strip_prefix('[')
        .and_then(|text| text.strip_suffix(']'))
        .ok_or_else(|| site.error("Close the flow list with ]"))?;
    let mut nesting = 0_usize;
    let mut quote = None;
    let mut escape = false;
    let mut start = 0;
    let mut part_site = site.after("[");
    let mut values = Vec::new();
    for (index, ch) in tail.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        match quote {
            Some('"') if ch == '\\' => escape = true,
            Some(end) if ch == end => quote = None,
            Some(_) => {}
            None => match ch {
                '"' | '\'' if tail[start..index].trim().is_empty() || nesting > 0 => {
                    quote = Some(ch)
                }
                '[' => nesting += 1,
                ']' => {
                    nesting = nesting
                        .checked_sub(1)
                        .ok_or_else(|| site.error("Unexpected closing list bracket"))?
                }
                ',' if nesting == 0 => {
                    let part = tail[start..index].trim();
                    if part.is_empty() {
                        return Err(site.error("Put a value between list commas"));
                    }
                    entry(remaining, part_site)?;
                    values.push(scalar(part, part_site, depth + 1, remaining)?);
                    part_site = part_site.after(&tail[start..=index]);
                    start = index + 1;
                }
                _ => {}
            },
        }
    }
    if nesting > 0 || quote.is_some() {
        return Err(site.error("Close the list or quoted value"));
    }
    let part = tail[start..].trim();
    if !part.is_empty() {
        entry(remaining, part_site)?;
        values.push(scalar(part, part_site, depth + 1, remaining)?);
    }
    Ok(Value::List(values))
}

fn decode_quote(text: &str, site: Site) -> Result<String, Diagnostic> {
    let delimiter = text.chars().next().unwrap_or_default();
    let body = text
        .get(1..)
        .and_then(|text| text.strip_suffix(delimiter))
        .ok_or_else(|| site.error("Close the quoted item value"))?;
    let mut result = String::new();
    let mut chars = body.chars();
    while let Some(ch) = chars.next() {
        match (delimiter, ch) {
            ('\'', '\'') if chars.next() == Some('\'') => result.push('\''),
            ('"', '\\') => {
                let len = match chars.next() {
                    Some('0') => {
                        result.push('\0');
                        0
                    }
                    Some('a') => {
                        result.push('\x07');
                        0
                    }
                    Some('b') => {
                        result.push('\x08');
                        0
                    }
                    Some('t' | '\t') => {
                        result.push('\t');
                        0
                    }
                    Some('n') => {
                        result.push('\n');
                        0
                    }
                    Some('v') => {
                        result.push('\x0b');
                        0
                    }
                    Some('f') => {
                        result.push('\x0c');
                        0
                    }
                    Some('r') => {
                        result.push('\r');
                        0
                    }
                    Some('e') => {
                        result.push('\x1b');
                        0
                    }
                    Some(c @ ('"' | '\\' | '/' | ' ')) => {
                        result.push(c);
                        0
                    }
                    Some('N') => {
                        result.push('\u{0085}');
                        0
                    }
                    Some('_') => {
                        result.push('\u{00a0}');
                        0
                    }
                    Some('L') => {
                        result.push('\u{2028}');
                        0
                    }
                    Some('P') => {
                        result.push('\u{2029}');
                        0
                    }
                    Some('x') => 2,
                    Some('u') => 4,
                    Some('U') => 8,
                    _ => return Err(site.error("Invalid escape in quoted item text")),
                };
                if len > 0 {
                    let hex: String = chars.by_ref().take(len).collect();
                    let ch = (hex.len() == len)
                        .then(|| u32::from_str_radix(&hex, 16).ok())
                        .flatten()
                        .and_then(char::from_u32)
                        .ok_or_else(|| site.error("Use a valid Unicode escape"))?;
                    result.push(ch);
                }
            }
            (delimiter, ch) if delimiter == ch => {
                return Err(site.error("Unexpected quote inside item text"));
            }
            _ => result.push(ch),
        }
    }
    Ok(result)
}

pub(super) fn record_field(text: &str, site: Site) -> Result<Value, Diagnostic> {
    header_line(Site::START, text.len())?;
    let (_, value, colon) = key_value(text, site)?;
    let (value, _) = without_comment(value);
    scalar(value.trim_end(), site.after(&text[..colon + 1]), 0, &mut {
        MAX_ENTRIES
    })
}
