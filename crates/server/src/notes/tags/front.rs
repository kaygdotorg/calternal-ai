//! Tags in the properties: `tags: a, b`, `tags: [a, "b"]`, or a `tags:`
//! line followed by `- a` lines. Quotes and a leading `#` are not part
//! of the name.

use super::scan::Span;

/// Every tag in the properties block `head` (the text up to the end of
/// the closing `---`), in order.
pub fn spans(head: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut in_list = false;
    let mut at = 0;
    for line in head.split_inclusive('\n') {
        let start = at;
        at += line.len();
        let line = line.strip_suffix('\n').unwrap_or(line);
        if let Some(rest) = line.strip_prefix("tags:") {
            let v = rest.trim();
            in_list = v.is_empty();
            let mut from = start + "tags:".len();
            for part in rest.split(',') {
                let mut s = from..from + part.len();
                // `[a` and `b]`: the brackets aren't part of a name.
                trim(head, &mut s, |c| c.is_whitespace() || c == '[' || c == ']');
                push(head, s, &mut out);
                from += part.len() + 1;
            }
        } else if in_list {
            let lead = line.len() - line.trim_start().len();
            match line[lead..].strip_prefix("- ") {
                Some(item) => {
                    let from = start + line.len() - item.len();
                    push(head, from..from + item.len(), &mut out);
                }
                None => in_list = false,
            }
        }
    }
    out
}

/// Adds the name inside `s`, without spaces, quotes or a `#`.
fn push(head: &str, mut s: Span, out: &mut Vec<Span>) {
    trim(head, &mut s, |c| {
        c.is_whitespace() || matches!(c, '"' | '\'' | '#')
    });
    if !s.is_empty() {
        out.push(s);
    }
}

/// Shrinks `s` from both ends while `strip` matches.
fn trim(text: &str, s: &mut Span, strip: impl Fn(char) -> bool) {
    let t = &text[s.clone()];
    let left = t.len() - t.trim_start_matches(&strip).len();
    let right = t.trim_end_matches(&strip).len();
    *s = if right <= left {
        s.start..s.start
    } else {
        s.start + left..s.start + right
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(head: &str) -> Vec<&str> {
        spans(head).into_iter().map(|s| &head[s]).collect()
    }

    #[test]
    fn every_way_of_writing_tags() {
        assert_eq!(names("---\ntags: a, b/c\n---\n"), ["a", "b/c"]);
        assert_eq!(names("tags: [a, \"#b\", 'c']\n"), ["a", "b", "c"]);
        assert_eq!(
            names("tags:\n  - a\n  - \"b\"\ntitle: x\n- no\n"),
            ["a", "b"]
        );
        assert_eq!(names("tags: []\n"), Vec::<&str>::new());
        assert_eq!(names("title: tags: a\n"), Vec::<&str>::new());
    }
}
