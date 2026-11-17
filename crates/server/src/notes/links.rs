//! Relative Markdown note links, adapted from ADR 0024 and the reference engine.
use calternal_fsio::names;
use pulldown_cmark::{Event, LinkType, Parser, Tag};
use std::collections::BTreeSet;

struct Link {
    span: (usize, usize),
    text: String,
    target: String,
    anchor: Option<String>,
}

fn find(markdown: &str) -> impl Iterator<Item = Link> {
    let body = super::split_frontmatter(markdown).1;
    let offset = markdown.len() - body.len();
    Parser::new(body)
        .into_offset_iter()
        .filter_map(move |(event, range)| {
            // The parser skips inline and block code, escapes and images. The
            // reference scanner limits rewrites to inline links with stable spans.
            if !matches!(
                event,
                Event::Start(Tag::Link {
                    link_type: LinkType::Inline,
                    ..
                })
            ) {
                return None;
            }
            let (mut link, end) = parse_at(&body[range.clone()], 0)?;
            if !link.target.starts_with("Notes/") || end != range.len() {
                return None;
            }
            link.span = (offset + range.start, offset + range.end);
            Some(link)
        })
}

/// Fold each relative target once for index lookup during a later rename.
pub(super) fn targets(markdown: &str) -> BTreeSet<String> {
    if !markdown.contains("](") {
        return BTreeSet::new();
    }
    find(markdown)
        .map(|link| names::fold(&link.target))
        .collect()
}

/// Repoint links to the old path, preserving anchors and user-authored labels.
pub(super) fn retarget(
    markdown: &str,
    old: &str,
    new: &str,
    old_title: &str,
    title: &str,
) -> Option<String> {
    let key = names::fold(old);
    let destination = new.strip_prefix("Notes/")?;
    let stem = old
        .strip_prefix("Notes/")
        .and_then(|p| p.strip_suffix(".md"));
    let edits: Vec<_> = find(markdown)
        .filter(|link| names::fold(&link.target) == key)
        .map(|link| {
            let (start, end) = link.span;
            let original = &markdown[start..end];
            let next = if link.text.trim() == old_title || Some(link.text.trim()) == stem {
                format(title, destination, link.anchor.as_deref())
            } else {
                replace_target(original, destination, link.anchor.as_deref())
            };
            (start..end, next)
        })
        .filter(|(span, next)| &markdown[span.clone()] != next)
        .collect();
    if edits.is_empty() {
        return None;
    }
    let mut next = markdown.to_owned();
    for (span, replacement) in edits.into_iter().rev() {
        next.replace_range(span, &replacement);
    }
    Some(next)
}

fn parse_at(line: &str, at: usize) -> Option<(Link, usize)> {
    let close = matching_bracket(line, at)?;
    let rest = &line[close + 1..];
    let inner = rest.strip_prefix('(')?;
    let (raw, used) = if let Some(a) = inner.strip_prefix('<') {
        let end = a.find('>')?;
        a[end + 1..]
            .starts_with(')')
            .then_some((&a[..end], end + 2))?
    } else {
        let end = inner.find(')')?;
        let raw = &inner[..end];
        (!raw.contains(' ')).then_some((raw, end))?
    };
    let after = close + 2 + used + 1;
    let (target, anchor) = resolve_note_destination("Notes", raw)?;
    Some((
        Link {
            span: (0, 0),
            text: unescape_label(&line[at + 1..close]),
            target,
            anchor,
        },
        after,
    ))
}

/// Resolve a parsed Markdown destination using the same rules as the link index.
/// This is a read helper; callers still decide whether a binding is writable.
fn resolve_note_destination(dir: &str, raw: &str) -> Option<(String, Option<String>)> {
    // Split URL syntax before decoding: an encoded # belongs to the file name.
    if raw.starts_with('/') || raw.contains("://") || raw.starts_with("mailto:") {
        return None;
    }
    let (path, anchor) = match raw.split_once('#') {
        Some((p, a)) => (percent_decode(p), Some(percent_decode(a))),
        None => (percent_decode(raw), None),
    };
    if !path.to_lowercase().ends_with(".md") {
        return None;
    }
    let target = resolve(dir, &path)?;
    Some((target, anchor))
}

/// The `]` that closes the `[` at `open`, allowing nested brackets.
fn matching_bracket(line: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut escaped = false;
    for (i, c) in line[open..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + i);
                }
            }
            '\n' => return None,
            _ => {}
        }
    }
    None
}

fn unescape_label(label: &str) -> String {
    let mut out = String::new();
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek().is_some_and(|c| c.is_ascii_punctuation()) {
            out.push(chars.next().expect("peeked character"));
        } else {
            out.push(c);
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && b[i + 1].is_ascii_hexdigit()
            && b[i + 2].is_ascii_hexdigit()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_owned())
}

/// Resolves `path` written in a note in folder `dir` to a path inside
/// `Notes/`. `None` if it climbs out of `Notes/`.
fn resolve(dir: &str, path: &str) -> Option<String> {
    let mut parts: Vec<&str> = if path.starts_with('/') {
        Vec::new()
    } else {
        dir.split('/').filter(|p| !p.is_empty()).collect()
    };
    for p in path.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    (!parts.is_empty()).then(|| names::nfc(&parts.join("/")))
}

/// The standard way to write a link.
fn format(text: &str, target_from_here: &str, anchor: Option<&str>) -> String {
    let text: String = text
        .chars()
        .flat_map(|c| {
            if matches!(c, '\\' | '[' | ']' | '*' | '_' | '`') {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect();
    format_label(&text, target_from_here, anchor)
}

// A custom label already present in markdown retains its own formatting.
fn format_label(text: &str, target_from_here: &str, anchor: Option<&str>) -> String {
    let target_from_here = encode_destination(target_from_here);
    let anchor = anchor.map(encode_destination);
    match anchor {
        Some(a) => format!("[{text}](<{target_from_here}#{a}>)"),
        None => format!("[{text}](<{target_from_here}>)"),
    }
}

// Angle destinations support spaces and Unicode literally. URL delimiters,
// percent signs and Markdown delimiters must be encoded before adding an anchor.
pub(super) fn encode_destination(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        if matches!(c, '#' | '?' | '%' | '<' | '>' | '\\') || c.is_control() {
            for b in c.to_string().bytes() {
                use std::fmt::Write;
                write!(out, "%{b:02X}").expect("writing a string");
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Preserve an existing markdown label while replacing its destination.
fn replace_target(original: &str, target: &str, anchor: Option<&str>) -> String {
    let close = matching_bracket(original, 0).expect("a parsed label");
    format_label(&original[1..close], target, anchor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_keep_anchors_labels_and_code() {
        let text = "[Old Plan](<Old Plan.md#^here>) [**alias**](Old%20Plan.md) `[Old Plan](<Old Plan.md>)`";
        assert_eq!(targets(text), BTreeSet::from(["notes/old plan.md".into()]));
        assert_eq!(
            retarget(
                text,
                "Notes/Old Plan.md",
                "Notes/New Plan.md",
                "Old Plan",
                "New Plan"
            )
            .unwrap(),
            "[New Plan](<New Plan.md#^here>) [**alias**](<New Plan.md>) `[Old Plan](<Old Plan.md>)`"
        );
    }
}
