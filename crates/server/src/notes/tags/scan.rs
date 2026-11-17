//! Finding `#tags` in the text of a note.
//!
//! A tag is `#` followed by letters, digits, `-`, `_` and `/`, starting
//! with a letter: `#work`, `#area/journal`. The `#` must start the line
//! or follow a space or `(`, so `not#tag`, `\#tag`, `**#tag**` and
//! headings (`# Title`) are not tags.
//!
//! Some text only looks like text, and a `#` there is never a tag:
//! - fenced code blocks (```` ``` ```` or `~~~`, also inside quotes and
//!   lists),
//! - inline code (`` `#x` ``, any number of backticks),
//! - link targets (`[t](x.md#part)`, `[t](#heading)`) and reference
//!   definitions (`[ref]: https://…`),
//! - web addresses (`https://x.com/#a`, `www.x.com/#a`) and anything in
//!   `<…>` (`<https://…>`, HTML).
//!
//! The note list, search and tag renames all read tags through here, so a
//! tag means the same thing everywhere, and a rename changes exactly the
//! tags the sidebar counts.

/// Where one tag's name sits in the text: byte offsets, without the `#`
/// and without stray `/` at either end.
pub type Span = std::ops::Range<usize>;

/// Every tag in `body` (text after the properties), in order.
pub fn spans(body: &str) -> Vec<Span> {
    let Some(last_hash) = body.rfind('#') else {
        return Vec::new();
    };
    // Nothing after the last hash's line can contain a tag. Keep that
    // entire line so code, addresses and link targets still parse normally.
    let end = body[last_hash..]
        .find('\n')
        .map_or(body.len(), |offset| last_hash + offset + 1);
    let body = &body[..end];
    let mut out = Vec::new();
    let mut fence: Option<Fence> = None;
    let mut at = 0;
    for line in body.split_inclusive('\n') {
        let start = at;
        at += line.len();
        let line = line.strip_suffix('\n').unwrap_or(line);
        match &fence {
            Some(f) => {
                if f.closes(line) {
                    fence = None;
                }
            }
            None => match Fence::opens(line) {
                Some(f) => fence = Some(f),
                None => in_line(line, start, &mut out),
            },
        }
    }
    out
}

/// An open fenced code block: its character and how many.
struct Fence {
    mark: u8,
    len: usize,
}

impl Fence {
    /// The fence this line opens, if it opens one.
    fn opens(line: &str) -> Option<Fence> {
        let rest = strip_containers(line);
        let mark = *rest.as_bytes().first()?;
        if mark != b'`' && mark != b'~' {
            return None;
        }
        let len = rest.bytes().take_while(|&b| b == mark).count();
        // A backtick fence's info text can't hold a backtick (that's
        // inline code instead).
        let info_ok = mark == b'~' || !rest[len..].contains('`');
        (len >= 3 && info_ok).then_some(Fence { mark, len })
    }

    /// True when `line` closes this fence: the same mark, at least as
    /// many, and nothing after but spaces.
    fn closes(&self, line: &str) -> bool {
        let rest = line.trim_start_matches([' ', '\t', '>']);
        let n = rest.bytes().take_while(|&b| b == self.mark).count();
        n >= self.len && rest[n..].trim().is_empty()
    }
}

/// The line without leading spaces, quote marks and list markers.
fn strip_containers(line: &str) -> &str {
    let mut rest = line;
    loop {
        let before = rest.len();
        rest = rest.trim_start_matches([' ', '\t', '>']);
        for marker in ["- ", "* ", "+ "] {
            if let Some(r) = rest.strip_prefix(marker) {
                rest = r;
            }
        }
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0
            && let Some(r) = rest[digits..]
                .strip_prefix(". ")
                .or_else(|| rest[digits..].strip_prefix(") "))
        {
            rest = r;
        }
        if rest.len() == before {
            return rest;
        }
    }
}

/// Tags in one line outside code, link targets and addresses. `base` is
/// the line's offset in the body.
fn in_line(line: &str, base: usize, out: &mut Vec<Span>) {
    // Fence tracking happens before this call. A line without a hash
    // cannot contain a tag, so skip its code, link and address parsing.
    if !line.contains('#') {
        return;
    }
    let b = line.as_bytes();
    // `[ref]: target` defines a link: nothing after the `]:` is text.
    let def_end = reference_definition(line);
    let mut i = 0;
    while i < b.len() {
        if def_end.is_some_and(|end| i >= end) {
            return;
        }
        match b[i] {
            // An escaped character is plain text (`\#not-a-tag`).
            b'\\' => i += 1 + line[i + 1..].chars().next().map_or(0, char::len_utf8),
            b'`' => i = past_code(b, i),
            b']' if b.get(i + 1) == Some(&b'(') => i = past_target(b, i + 1),
            b'<' if b
                .get(i + 1)
                .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, b'/' | b'!')) =>
            {
                i = match line[i..].find('>') {
                    Some(end) => i + end + 1,
                    None => i + 1,
                }
            }
            b'#' if starts_word(line, i) => match name(line, i + 1) {
                Some((s, e, next)) => {
                    out.push(base + s..base + e);
                    i = next;
                }
                None => i += 1,
            },
            c if c.is_ascii_alphabetic() && starts_word(line, i) => {
                i = past_address(line, i);
            }
            _ => i += 1,
        }
    }
}

/// True when position `i` starts a word: the line's start, after a space
/// or after `(`.
fn starts_word(line: &str, i: usize) -> bool {
    match line[..i].chars().next_back() {
        None => true,
        Some(c) => c.is_whitespace() || c == '(',
    }
}

/// The tag name after a `#` at `from - 1`: `(start, end, next)`, where
/// `next` is where scanning goes on. `None` if it's not a tag.
fn name(line: &str, from: usize) -> Option<(usize, usize, usize)> {
    let len: usize = line[from..]
        .chars()
        .take_while(|&c| is_tag_char(c))
        .map(char::len_utf8)
        .sum();
    let raw = &line[from..from + len];
    let lead = raw.len() - raw.trim_start_matches('/').len();
    let core = raw.trim_matches('/');
    let first = core.chars().next()?;
    first
        .is_alphabetic()
        .then_some((from + lead, from + lead + core.len(), from + len))
}

/// Letters, digits, `-`, `_` and `/` make up a tag.
pub fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '/' | '-' | '_')
}

/// Past inline code that starts with the backticks at `i`. Backticks with
/// no closing run of the same length are plain text.
fn past_code(b: &[u8], i: usize) -> usize {
    let n = run(b, i, b'`');
    let mut j = i + n;
    while j < b.len() {
        if b[j] == b'`' {
            let m = run(b, j, b'`');
            if m == n {
                return j + m;
            }
            j += m;
        } else {
            j += 1;
        }
    }
    i + n
}

fn run(b: &[u8], i: usize, c: u8) -> usize {
    b[i..].iter().take_while(|&&x| x == c).count()
}

/// Past a link target that starts with the `(` at `i`: `(<…>)` or
/// `(…)` with balanced parentheses. An unclosed target runs to the end of
/// the line (it might still be one; never call it text).
fn past_target(b: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    if b.get(j) == Some(&b'<') {
        while j < b.len() && b[j] != b'>' {
            j += 1;
        }
    }
    let mut depth = 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 1,
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return j + 1;
                }
            }
            _ => {}
        }
        j += 1;
    }
    b.len()
}

/// Past a web address starting at `i` (`https://…`, `www.…`), up to the
/// next space. Anything else just steps over the letter.
fn past_address(line: &str, i: usize) -> usize {
    let rest = &line[i..];
    let scheme = rest
        .bytes()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'.' | b'-'))
        .count();
    let is_address = rest[scheme..].starts_with("://") || rest.starts_with("www.");
    if !is_address {
        return i + 1;
    }
    i + rest.find(char::is_whitespace).unwrap_or(rest.len())
}

/// For `[label]: target`, where the target starts.
fn reference_definition(line: &str) -> Option<usize> {
    let lead = line.len() - line.trim_start_matches(' ').len();
    if lead > 3 || !line[lead..].starts_with('[') {
        return None;
    }
    let close = line[lead..].find("]:")?;
    Some(lead + close + 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(body: &str) -> Vec<&str> {
        spans(body).into_iter().map(|s| &body[s]).collect()
    }

    #[test]
    fn plain_tags() {
        assert_eq!(names("#a b #c/d (#e) #f-g_h."), ["a", "c/d", "e", "f-g_h"]);
        assert_eq!(names("#über #日本 #a1"), ["über", "日本", "a1"]);
        assert_eq!(names("#work/ #/work"), ["work", "work"]);
    }

    #[test]
    fn not_tags() {
        assert!(names("not#tag \\#esc **#bold** #1x # heading ## two #").is_empty());
        assert!(names("# Heading\n## Sub\n").is_empty());
    }

    #[test]
    fn hash_free_lines_still_open_and_close_code_fences() {
        assert_eq!(
            names(
                "ordinary text\n```rust\n#hidden\n```\n#visible\n~~~\n#also-hidden\n~~~\n#after\n"
            ),
            ["visible", "after"]
        );
        assert_eq!(
            names("> - ```text\n#hidden\n> ```\n#visible\n"),
            ["visible"]
        );
    }

    #[test]
    fn headings_and_quotes_keep_their_tags() {
        assert_eq!(
            names("## Plans #work\n> quote #q\n- item #i\n"),
            ["work", "q", "i"]
        );
    }

    #[test]
    fn code_is_never_tags() {
        assert!(names("`#a` ``x #b ` y`` `` ` #c ` ``").is_empty());
        assert_eq!(names("`unclosed #a"), ["a"], "a lone backtick is text");
        assert!(names("```\n#a\n```\n~~~md\n#b\n~~~\n").is_empty());
        assert!(names("````\n```\n#a\n````\n").is_empty());
        assert!(names("> ```\n> #a\n> ```\n- ```\n  #b\n  ```\n").is_empty());
        assert_eq!(names("```\n#a\n```\n#b\n"), ["b"]);
        assert!(names("```\n#a never closed\n#b\n").is_empty());
    }

    #[test]
    fn link_targets_and_addresses_are_never_tags() {
        assert!(names("[t](#a) [t](<x.md#b>) [t](x.md#c) [t](y(#d))").is_empty());
        assert_eq!(names("[see #a](<x.md>)"), ["a"], "link text is text");
        assert!(
            names("https://x.com/#a www.x.com/#b <https://x.com/#c> <a href=\"#d\">").is_empty()
        );
        assert!(names("[ref]: https://x.com/ #e").is_empty());
        assert_eq!(names("https://x.com #a"), ["a"]);
    }
}
