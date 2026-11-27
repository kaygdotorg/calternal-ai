//! A bounded scanner for the two line-item forms.

use std::{collections::BTreeMap, ops::Range};

use crate::{Diagnostic, lines::Site};

/// A checkbox or calendar entry recognized in a Markdown note.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteLine<'a> {
    /// The original byte range of the physical line, without its line ending.
    pub range: Range<usize>,
    /// The one-based physical line number.
    pub number: usize,
    /// Raw targets of directly nested standalone link bullets in source order.
    pub child_links: Vec<&'a str>,
    /// The notation and borrowed contents of this line.
    pub item: LineItem<'a>,
}

/// Data read from one of the two line-item forms.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LineItem<'a> {
    /// A Markdown list item with a one-character box.
    Checkbox {
        /// The character inside the box, as written.
        mark: char,
        /// The byte range of the character for a source-preserving edit.
        mark_range: Range<usize>,
        /// The remaining Markdown, including any child item link for host resolution.
        body: &'a str,
    },
    /// A time or all-day entry, whose civil date comes from the note.
    Time {
        /// Start clock in HH:MM, or absent for an all-day entry.
        start: Option<&'a str>,
        /// End clock in HH:MM if written.
        end: Option<&'a str>,
        /// Remaining Markdown used for the event title.
        title: &'a str,
    },
}

const MAX_ITEMS: usize = 1_000;
const MAX_NOTE_LINES: usize = 65_536;
const MAX_NOTE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Region {
    Body,
    Frontmatter,
    Excluded,
}

/// Scan a full note, including any frontmatter, for checkbox and time entries.
///
/// Emits both entries when a list item contains a checkbox followed by a time.
/// Rejects notes exceeding 32 MiB, 65,536 physical lines or 1,000 entries.
pub fn scan_note_lines(source: &str) -> Result<Vec<NoteLine<'_>>, Diagnostic> {
    if source.len() > MAX_NOTE_BYTES {
        return Err(Site::START.error("A note with line items may be at most 32 MiB"));
    }
    let lines: Vec<_> = source.lines().take(MAX_NOTE_LINES + 1).collect();
    if lines.len() > MAX_NOTE_LINES {
        return Err(Site::START.error("A note with line items may contain at most 65,536 lines"));
    }
    let regions = regions(&lines);
    let mut items: Vec<NoteLine<'_>> = Vec::new();
    let mut parents: Vec<(usize, Range<usize>)> = Vec::new();
    for (index, (&line, region)) in lines.iter().zip(regions).enumerate() {
        if region != Region::Body {
            continue;
        }
        let offset = line.as_ptr() as usize - source.as_ptr() as usize;
        let (text, quoted) = content(line);
        let list = list_body(text);
        let mut title = list.unwrap_or(text);
        let column = text.as_ptr() as usize - line.as_ptr() as usize;
        let item_start = items.len();
        if !text.is_empty() && (list.is_some() || !quoted) {
            while parents
                .last()
                .is_some_and(|(child_column, _)| column < *child_column)
            {
                parents.pop();
            }
        }
        if let Some((child_column, parent)) = parents.last()
            && column == *child_column
            && let Some(link) = list.and_then(direct_link)
        {
            for parent_item in &mut items[parent.clone()] {
                parent_item.child_links.push(link);
            }
        }
        let mut add = |item| {
            if items.len() >= MAX_ITEMS {
                return Err(Site {
                    line: index + 1,
                    column: 1,
                }
                .error("A note may contain at most 1,000 line items"));
            }
            items.push(NoteLine {
                range: offset..offset + line.len(),
                number: index + 1,
                child_links: Vec::new(),
                item,
            });
            Ok(())
        };
        if let Some((mark, body)) = list.and_then(checkbox) {
            let start = title.as_ptr() as usize - source.as_ptr() as usize + 1;
            add(LineItem::Checkbox {
                mark,
                mark_range: start..start + mark.len_utf8(),
                body,
            })?;
            title = body;
        }
        if !quoted && let Some(item) = time_item(title) {
            add(item)?;
        }
        if let Some(body) = list
            && items.len() > item_start
        {
            let child_column = body.as_ptr() as usize - line.as_ptr() as usize;
            parents.push((child_column, item_start..items.len()));
        }
    }
    Ok(items)
}

fn content(mut text: &str) -> (&str, bool) {
    let mut quoted = false;
    loop {
        text = text.trim_start_matches([' ', '\t']);
        let Some(rest) = text.strip_prefix('>') else {
            return (text, quoted);
        };
        text = rest;
        quoted = true;
    }
}

fn list_body(text: &str) -> Option<&str> {
    let first = text.as_bytes().first()?;
    let tail = if matches!(first, b'-' | b'+' | b'*') {
        &text[1..]
    } else {
        let n = text.bytes().take_while(u8::is_ascii_digit).count();
        if !(1..=9).contains(&n) {
            return None;
        }
        text[n..].strip_prefix(['.', ')'])?
    };
    tail.starts_with([' ', '\t'])
        .then(|| tail.trim_start_matches([' ', '\t']))
}

fn direct_link(text: &str) -> Option<&str> {
    let text = text.trim_end();
    if let Some(link) = text
        .strip_prefix("[[")
        .and_then(|text| text.strip_suffix("]]"))
    {
        let target = link.split('|').next()?;
        return (!target.is_empty() && !target.contains(['[', ']'])).then_some(target);
    }
    let (label, rest) = text.strip_prefix('[')?.split_once("](")?;
    if label.is_empty() || label.contains(['[', ']']) {
        return None;
    }
    let link = rest.strip_suffix(')')?;
    let target = if let Some(target) = link
        .strip_prefix('<')
        .and_then(|text| text.strip_suffix('>'))
    {
        if target.contains(['<', '>']) {
            return None;
        }
        target
    } else {
        if link.contains(['(', ')', '<', '>']) || link.contains(char::is_whitespace) {
            return None;
        }
        link
    };
    (!target.is_empty()).then_some(target)
}

fn checkbox(text: &str) -> Option<(char, &str)> {
    let tail = text.strip_prefix('[')?;
    let mark = tail.chars().next()?;
    if mark.is_control() || matches!(mark, '[' | ']' | '\\') {
        return None;
    }
    let body = tail[mark.len_utf8()..].strip_prefix(']')?;
    (body.is_empty() || body.starts_with([' ', '\t']))
        .then(|| (mark, body.trim_start_matches([' ', '\t'])))
}

fn clock_prefix(text: &str) -> Option<(&str, &str)> {
    let clock = text.get(..5)?;
    let rest = &text[5..];
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
    let (hour, minute) = clock.split_once(':')?;
    let valid = |part: &str, max| {
        part.len() == 2
            && part.bytes().all(|byte| byte.is_ascii_digit())
            && part.parse::<u8>().is_ok_and(|value| value <= max)
    };
    if !valid(hour, 23) || !valid(minute, 59) {
        return None;
    }
    Some((clock, rest.trim_start_matches([' ', '\t'])))
}

fn time_item(text: &str) -> Option<LineItem<'_>> {
    if let Some(tail) = text.strip_prefix("all day")
        && (tail.is_empty() || tail.starts_with([' ', '\t']))
    {
        return Some(LineItem::Time {
            start: None,
            end: None,
            title: tail.trim_start(),
        });
    }
    let (start, tail) = clock_prefix(text)?;
    let (end, title) = if let Some(range) = tail.strip_prefix("- ") {
        let (end, title) = clock_prefix(range.trim_start())?;
        (Some(end), title)
    } else {
        (None, tail)
    };
    Some(LineItem::Time {
        start: Some(start),
        end,
        title,
    })
}

fn fence(text: &str) -> Option<(u8, usize, &str)> {
    let leader = *text.as_bytes().first()?;
    if !matches!(leader, b'`' | b'~') {
        return None;
    }
    let length = text.bytes().take_while(|byte| *byte == leader).count();
    (length >= 3).then(|| (leader, length, &text[length..]))
}

fn regions(lines: &[&str]) -> Vec<Region> {
    let mut regions = vec![Region::Body; lines.len()];
    let mut frontmatter = lines.first() == Some(&"---");
    let mut fenced = None;
    let mut list_indents = Vec::new();
    let mut html_block = false;
    let mut comment = false;
    let mut token_runs = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if frontmatter {
            regions[index] = Region::Frontmatter;
            frontmatter = index == 0 || *line != "---";
            continue;
        }
        let (text, _) = content(line);
        if let Some((leader, length)) = fenced {
            regions[index] = Region::Excluded;
            if fence(text)
                .is_some_and(|(ch, n, tail)| ch == leader && n >= length && tail.trim().is_empty())
            {
                fenced = None;
            }
            continue;
        }
        if comment || text.contains("<!--") {
            let mut tail = if comment {
                text
            } else {
                text.split_once("<!--").unwrap().1
            };
            regions[index] = Region::Excluded;
            loop {
                let Some((_, after)) = tail.split_once("-->") else {
                    comment = true;
                    break;
                };
                let Some((_, rest)) = after.split_once("<!--") else {
                    comment = false;
                    break;
                };
                tail = rest;
            }
            continue;
        }
        if text.is_empty() {
            html_block = false;
            continue;
        }
        if html_block || html_tag(text).is_some() {
            html_block = true;
            regions[index] = Region::Excluded;
            continue;
        }
        let indent = line
            .bytes()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .map(|byte| if byte == b'\t' { 4 } else { 1 })
            .sum::<usize>();
        while list_indents.last().is_some_and(|level| indent < *level) {
            list_indents.pop();
        }
        if indent >= list_indents.last().copied().unwrap_or(0) + 4 {
            regions[index] = Region::Excluded;
            continue;
        }
        let list = list_body(text);
        if let Some(body) = list {
            let level = indent + text.len() - body.len();
            list_indents.push(level);
        }
        if let Some((leader, length, tail)) = fence(list.unwrap_or(text))
            && (leader == b'~' || !tail.contains('`'))
        {
            fenced = Some((leader, length));
            regions[index] = Region::Excluded;
            continue;
        }
        if text
            .match_indices('<')
            .any(|(at, _)| html_tag(&text[at..]).is_some())
        {
            regions[index] = Region::Excluded;
            continue;
        }
        backticks(text, index, &mut token_runs);
    }
    exclude_inline_code(&mut regions, &token_runs, lines);
    regions
}

fn html_tag(text: &str) -> Option<()> {
    let text = text.strip_prefix('<')?.trim_start_matches('/');
    let length = text
        .bytes()
        .take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
        .count();
    if length == 0 || !text.as_bytes()[0].is_ascii_alphabetic() {
        return None;
    }
    let tail = &text[length..];
    (tail.starts_with([' ', '\t', '>', '/']) && tail.contains('>')).then_some(())
}

fn backticks(text: &str, line: usize, runs: &mut Vec<(usize, usize)>) {
    let mut bytes = text.bytes().peekable();
    while let Some(byte) = bytes.next() {
        if byte == b'\\' && bytes.peek() == Some(&b'`') {
            bytes.next();
        } else if byte == b'`' {
            let mut count = 1;
            while bytes.next_if_eq(&b'`').is_some() {
                count += 1;
            }
            runs.push((line, count));
        }
    }
}

fn exclude_inline_code(regions: &mut [Region], runs: &[(usize, usize)], lines: &[&str]) {
    let mut next_by_count = BTreeMap::new();
    let mut closing = vec![None; runs.len()];
    let mut boundary = lines.len();
    for (index, &(line, count)) in runs.iter().enumerate().rev() {
        if (line..boundary).any(|at| regions[at] != Region::Body || lines[at].trim().is_empty()) {
            next_by_count.clear();
        }
        boundary = line;
        closing[index] = next_by_count.insert(count, index);
    }
    let mut index = 0;
    while index < runs.len() {
        if let Some(end) = closing[index] {
            regions[runs[index].0..=runs[end].0].fill(Region::Excluded);
            index = end + 1;
        } else {
            index += 1;
        }
    }
}

#[cfg(test)]
mod tests;
