//! Turning "old text, new text" into small edits.
//!
//! Used when a file changes outside Calternal or an agent replaces a whole
//! note: the change becomes a few precise edits instead of "delete
//! everything, insert everything", so nobody's typing is lost and the
//! history stays readable.
//!
//! Two rules keep it safe:
//! - Positions count UTF-16 code units, the same unit the browser and the
//!   live document use, so an edit never lands in the wrong place.
//! - Edits never cut through a character: diffing works on whole
//!   grapheme clusters, so an emoji, a flag or an accented letter is
//!   always kept or replaced whole.
//!
//! And one rule keeps merges tidy: lines are compared first, then the
//! characters inside changed lines. A character diff alone likes to slide
//! along repeated text (adding `<b>\n` before `<a>\n` can come out as
//! inserting `b>\n<` after the first `<`; deleting a line can take the
//! next line's first character with it). The text is the same, but
//! someone typing at that line start at the same moment would land inside
//! another line and garble both (#3). So lines changed in place get
//! character edits within each line, and lines added, removed or
//! regrouped are replaced as whole lines.

use serde::{Deserialize, Serialize};
use similar::{Algorithm, DiffOp, capture_diff_slices_deadline};
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;

/// One edit, in UTF-16 positions of the text it applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    /// Where the edit starts.
    pub at: u32,
    /// How many UTF-16 units to delete there.
    pub delete: u32,
    /// What to insert there.
    pub insert: String,
}

/// UTF-16 length of a string.
fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// The edits that turn `old` into `new`, ordered from the end of the text
/// to the start, so applying them in order never shifts the positions of
/// edits still to come.
pub(super) fn edits(old: &str, new: &str) -> Vec<Edit> {
    if old == new {
        return Vec::new();
    }
    // A deadline keeps pathological inputs fast; the result stays correct,
    // just less minimal.
    let deadline = Instant::now() + Duration::from_millis(200);
    let a: Vec<&str> = old.split_inclusive('\n').collect();
    let b: Vec<&str> = new.split_inclusive('\n').collect();
    let ops = capture_diff_slices_deadline(Algorithm::Myers, &a, &b, Some(deadline));
    let mut out = Vec::new();
    let mut pos = 0;
    for op in ops {
        let (old_range, new_range) = match op {
            DiffOp::Equal { old_index, len, .. } => {
                pos += utf16_len(&a[old_index..old_index + len].concat());
                continue;
            }
            op => (op.old_range(), op.new_range()),
        };
        let (was, now) = (&a[old_range], &b[new_range]);
        if was.len() == now.len() {
            // Lines changed in place: character edits, line by line, so
            // no edit reaches across a line break.
            for (x, y) in was.iter().zip(now) {
                for e in chars(x, y, deadline) {
                    push(&mut out, pos + e.at, e.delete, e.insert);
                }
                pos += utf16_len(x);
            }
        } else {
            // Lines added, removed or regrouped: whole lines.
            let was = was.concat();
            push(&mut out, pos, utf16_len(&was), now.concat());
            pos += utf16_len(&was);
        }
    }
    out.reverse();
    out
}

/// The edits inside one changed stretch of lines, character by character,
/// first to last, with positions from the stretch's start.
fn chars(old: &str, new: &str, deadline: Instant) -> Vec<Edit> {
    let a: Vec<&str> = old.graphemes(true).collect();
    let b: Vec<&str> = new.graphemes(true).collect();

    // Common start and end first: most changes are small and local.
    let pre = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let max_suf = a.len().min(b.len()) - pre;
    let suf = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(max_suf)
        .take_while(|(x, y)| x == y)
        .count();
    let (am, bm) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    let base: u32 = a[..pre].iter().map(|g| utf16_len(g)).sum();
    let ops = capture_diff_slices_deadline(Algorithm::Myers, am, bm, Some(deadline));

    let mut out = Vec::new();
    let mut pos = base;
    for op in ops {
        match op {
            DiffOp::Equal { len, old_index, .. } => {
                pos += am[old_index..old_index + len]
                    .iter()
                    .map(|g| utf16_len(g))
                    .sum::<u32>();
            }
            DiffOp::Delete {
                old_index, old_len, ..
            } => {
                let n: u32 = am[old_index..old_index + old_len]
                    .iter()
                    .map(|g| utf16_len(g))
                    .sum();
                push(&mut out, pos, n, String::new());
                pos += n;
            }
            DiffOp::Insert {
                new_index, new_len, ..
            } => {
                push(
                    &mut out,
                    pos,
                    0,
                    bm[new_index..new_index + new_len].concat(),
                );
            }
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                let n: u32 = am[old_index..old_index + old_len]
                    .iter()
                    .map(|g| utf16_len(g))
                    .sum();
                push(
                    &mut out,
                    pos,
                    n,
                    bm[new_index..new_index + new_len].concat(),
                );
                pos += n;
            }
        }
    }
    out
}

/// Adds an edit, merging it with the previous one when they touch, so a
/// replaced word is one edit rather than a delete plus an insert.
fn push(out: &mut Vec<Edit>, at: u32, delete: u32, insert: String) {
    if let Some(last) = out.last_mut()
        && last.at + last.delete == at
    {
        last.delete += delete;
        last.insert.push_str(&insert);
        return;
    }
    out.push(Edit { at, delete, insert });
}

/// Applies edits (as returned by [`edits`]) to a plain string. Used by
/// tests and by callers that need the result without a live document.
#[cfg(test)]
fn apply(text: &str, edits: &[Edit]) -> String {
    let mut units: Vec<u16> = text.encode_utf16().collect();
    for e in edits {
        let at = e.at as usize;
        let end = at + e.delete as usize;
        units.splice(at..end, e.insert.encode_utf16());
    }
    String::from_utf16(&units).expect("edits keep text valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn boundaries(s: &str) -> Vec<u32> {
        let mut out = vec![0];
        let mut pos = 0;
        for g in s.graphemes(true) {
            pos += utf16_len(g);
            out.push(pos);
        }
        out
    }

    #[test]
    fn small_change_is_a_small_edit() {
        let e = edits("hello world", "hello brave world");
        assert_eq!(
            e,
            vec![Edit {
                at: 6,
                delete: 0,
                insert: "brave ".into()
            }]
        );
    }

    #[test]
    fn an_inserted_line_starts_at_a_line_start() {
        let old = "x\n<a>\n";
        let e = edits(old, "x\n<b>\n<a>\n");
        assert_eq!(
            e,
            vec![Edit {
                at: 2,
                delete: 0,
                insert: "<b>\n".into()
            }]
        );
        // Also at the very start of the text.
        let e = edits("<a>\n", "<b>\n<a>\n");
        assert_eq!(e[0].at, 0);
        assert_eq!(e[0].insert, "<b>\n");
    }

    #[test]
    fn emoji_positions_are_utf16() {
        let old = "\u{1f44d} hi";
        let e = edits(old, "\u{1f44d} hey");
        assert_eq!(apply(old, &e), "\u{1f44d} hey");
        assert!(e.iter().all(|e| e.at >= 3), "emoji is 2 units plus a space");
    }

    #[test]
    fn edits_round_trip_without_splitting_graphemes() {
        let texts = [
            "",
            "e\u{301}",
            "👍🏽",
            "👨‍👩‍👧",
            "🇮🇳漢字",
            "של\n#a\n",
            "a\na\nb\n",
        ];
        for a in texts {
            for b in texts {
                let edits = edits(a, b);
                assert_eq!(apply(a, &edits), b);
                let boundaries = boundaries(a);
                for edit in edits {
                    assert!(boundaries.contains(&edit.at));
                    assert!(boundaries.contains(&(edit.at + edit.delete)));
                }
            }
        }
    }
}
