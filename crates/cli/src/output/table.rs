//! Width-aware alignment for [`super::Block::Table`] and labelled key/value blocks.

use std::cmp::Reverse;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Width below which rows are easier to scan as labelled blocks.
const STACKED_BELOW: usize = 48;

/// A table column's shrinking, hiding, and clipping rules.
///
/// Smaller priorities shrink and hide first; the minimum includes the heading.
#[derive(Clone, Copy)]
struct Column {
    min_width: usize,
    priority: u8,
    clip: Clip,
}

/// How a long value keeps the most useful part of its text.
#[derive(Clone, Copy)]
enum Clip {
    Middle,
    End,
}

/// Width in terminal columns, including wide CJK glyphs and excluding combining marks.
fn width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Width of one Unicode scalar value in a terminal cell.
fn char_width(c: char) -> usize {
    UnicodeWidthChar::width(c).unwrap_or(0)
}

/// Replace terminal controls so cell contents cannot move or overwrite output.
fn flat(cell: &str) -> String {
    cell.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Clip at the end of text, reserving one display column for an ellipsis.
fn clip_end(s: &str, max: usize) -> String {
    if width(s) <= max {
        return s.to_owned();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let cw = char_width(c);
        if used + cw > max.saturating_sub(1) {
            break;
        }
        out.push(c);
        used += cw;
    }
    out.push('…');
    out
}

/// Clip an identifier in the middle so both ends remain visible.
fn clip_middle(s: &str, max: usize) -> String {
    if width(s) <= max {
        return s.to_owned();
    }
    if max == 0 {
        return String::new();
    }
    if max == 1 {
        return "…".to_owned();
    }
    let chars: Vec<char> = s.chars().collect();
    let left_limit = max.saturating_sub(1).div_ceil(2);
    let right_limit = max.saturating_sub(1) / 2;
    let mut left = String::new();
    let mut used_left = 0;
    let mut left_end = 0;
    for (i, c) in chars.iter().enumerate() {
        let cw = char_width(*c);
        if used_left + cw > left_limit {
            break;
        }
        left.push(*c);
        used_left += cw;
        left_end = i + 1;
    }
    let mut right_chars = Vec::new();
    let mut used_right = 0;
    let mut right_start = chars.len();
    for (i, c) in chars.iter().enumerate().rev() {
        if i < left_end {
            break;
        }
        let cw = char_width(*c);
        if used_right + cw > right_limit {
            break;
        }
        right_chars.push(*c);
        used_right += cw;
        right_start = i;
    }
    let right: String = if right_start < left_end {
        String::new()
    } else {
        right_chars.into_iter().rev().collect()
    };
    format!("{left}…{right}")
}

/// Column policy keyed by the stable human-readable heading used by CLI tables.
fn column(header: &str) -> Column {
    let (min_width, priority, clip) = match header {
        "ID" => (13, 100, Clip::Middle),
        "PLUGIN" | "OPERATION" => (14, 95, Clip::Middle),
        "TITLE" | "NAME" => (12, 90, Clip::End),
        "WORKER" => (10, 75, Clip::End),
        "OWNER" => (13, 70, Clip::Middle),
        "UPDATED" => (20, 70, Clip::End),
        "PLATFORM" => (12, 55, Clip::End),
        "ACCESS" | "STATE" => (8, 50, Clip::End),
        "REVISION" | "ROLE" => (8, 45, Clip::End),
        "VERSION" => (8, 40, Clip::End),
        "ENABLED" => (7, 35, Clip::End),
        "GENERATION" => (10, 30, Clip::End),
        "SHA-256" => (12, 15, Clip::Middle),
        "DESCRIPTION" => (16, 10, Clip::End),
        _ => (width(header).max(12), 50, Clip::End),
    };
    Column {
        min_width: min_width.max(width(header)),
        priority,
        clip,
    }
}

/// Render the cells without breaking a line, using two spaces between columns.
fn line(cells: &[String], indexes: &[usize], widths: &[usize], columns: &[Column]) -> String {
    let mut out = String::new();
    for (position, &index) in indexes.iter().enumerate() {
        let cell = cells.get(index).map_or("", String::as_str);
        let value = match columns[index].clip {
            Clip::Middle => clip_middle(cell, widths[position]),
            Clip::End => clip_end(cell, widths[position]),
        };
        out.push_str(&value);
        if position + 1 < indexes.len() {
            out.push_str(&" ".repeat(widths[position].saturating_sub(width(&value)) + 2));
        }
    }
    out
}

/// Width of a selected table layout, including the two-cell column gaps.
fn total_width(widths: &[usize], indexes: &[usize]) -> usize {
    indexes.iter().map(|&i| widths[i]).sum::<usize>() + 2 * indexes.len().saturating_sub(1)
}

/// Render rows as labelled blocks; all cell values are shortened to the terminal width.
fn stacked(headers: &[String], rows: &[Vec<String>], columns: &[Column], limit: usize) -> String {
    let label_width = headers.iter().map(|h| width(h)).max().unwrap_or(0);
    let mut out = String::new();
    for (row_index, row) in rows.iter().enumerate() {
        if row_index > 0 {
            out.push('\n');
        }
        for (index, header) in headers.iter().enumerate() {
            let label_budget = limit.min(label_width);
            let label = clip_end(header, label_budget);
            let separator = if width(&label) + 2 <= limit {
                ": "
            } else if width(&label) + 1 <= limit {
                ":"
            } else {
                ""
            };
            let value_budget = limit.saturating_sub(width(&label) + width(separator));
            let value = row.get(index).map_or("", String::as_str);
            let value = match columns[index].clip {
                Clip::Middle => clip_middle(value, value_budget),
                Clip::End => clip_end(value, value_budget),
            };
            out.push_str(&label);
            out.push_str(separator);
            out.push_str(&value);
            out.push('\n');
        }
    }
    out
}

/// Wrap a notice between words so it never overruns a known terminal width.
fn wrap_notice(text: &str, width_limit: usize) -> String {
    let mut out = String::new();
    let mut line_width = 0;
    for word in text.split_whitespace() {
        let word_width = width(word);
        let gap = usize::from(line_width > 0);
        if line_width > 0 && line_width + gap + word_width > width_limit {
            out.push('\n');
            line_width = 0;
        } else if gap > 0 {
            out.push(' ');
            line_width += 1;
        }
        out.push_str(word);
        line_width += word_width;
    }
    out.push('\n');
    out
}

/// Render a table. Flexible cells shrink first; low-priority columns are then hidden.
/// With no width, all text remains intact for pipes and redirected output.
pub(super) fn table(
    headers: &[String],
    rows: &[Vec<String>],
    width_limit: Option<usize>,
    bold: bool,
) -> String {
    let headers: Vec<String> = headers.iter().map(|h| flat(h)).collect();
    let rows: Vec<Vec<String>> = rows
        .iter()
        .map(|r| r.iter().map(|cell| flat(cell)).collect())
        .collect();
    let columns: Vec<Column> = headers.iter().map(|header| column(header)).collect();

    if let Some(limit) = width_limit {
        if limit < STACKED_BELOW {
            return stacked(&headers, &rows, &columns, limit.max(1));
        }

        let mut widths: Vec<usize> = headers.iter().map(|h| width(h)).collect();
        for row in &rows {
            for (i, cell) in row.iter().enumerate().take(widths.len()) {
                widths[i] = widths[i].max(width(cell));
            }
        }
        let mut indexes: Vec<usize> = (0..headers.len()).collect();
        while total_width(&widths, &indexes) > limit {
            let total = total_width(&widths, &indexes);
            let excess = total - limit;
            let shrinkable = indexes
                .iter()
                .copied()
                .filter(|&i| widths[i] > columns[i].min_width)
                .min_by_key(|&i| (columns[i].priority, Reverse(i)));
            if let Some(i) = shrinkable {
                let shrink = excess.min(widths[i] - columns[i].min_width);
                widths[i] -= shrink;
                continue;
            }
            if indexes.len() <= 1 {
                break;
            }
            let drop_at = indexes
                .iter()
                .enumerate()
                .min_by_key(|(_, i)| (columns[**i].priority, Reverse(**i)))
                .map(|(at, _)| at)
                .expect("non-empty columns");
            indexes.remove(drop_at);
        }
        let mut out = String::new();
        let visible_widths = widths_for(&widths, &indexes);
        let head = line(&headers, &indexes, &visible_widths, &columns);
        if bold {
            out.push_str("\x1b[1m");
        }
        out.push_str(&head);
        if bold {
            out.push_str("\x1b[0m");
        }
        out.push('\n');
        for row in &rows {
            out.push_str(&line(row, &indexes, &visible_widths, &columns));
            out.push('\n');
        }
        let hidden = headers.len() - indexes.len();
        if hidden > 0 {
            let noun = if hidden == 1 { "column" } else { "columns" };
            let notice = format!("{hidden} {noun} hidden; use --json or a wider terminal.");
            out.push_str(&wrap_notice(&notice, limit));
        }
        return out;
    }

    let mut widths: Vec<usize> = headers.iter().map(|h| width(h)).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate().take(widths.len()) {
            widths[i] = widths[i].max(width(cell));
        }
    }
    let indexes: Vec<usize> = (0..headers.len()).collect();
    let head = line(&headers, &indexes, &widths, &columns);
    let mut out = if bold {
        format!("\x1b[1m{head}\x1b[0m\n")
    } else {
        format!("{head}\n")
    };
    for row in &rows {
        out.push_str(&line(row, &indexes, &widths, &columns));
        out.push('\n');
    }
    out
}

/// Select widths in visible-column order while keeping full-index column metadata.
fn widths_for(widths: &[usize], indexes: &[usize]) -> Vec<usize> {
    indexes.iter().map(|&i| widths[i]).collect()
}

/// Render `label:  value` lines with values aligned; values are not shortened.
pub(super) fn fields(items: &[(String, String)]) -> String {
    let label = items
        .iter()
        .map(|(key, _)| width(&format!("{key}:")))
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for (k, v) in items {
        let key = format!("{k}:");
        out.push_str(&key);
        out.push_str(&" ".repeat(label.saturating_sub(width(&key))));
        out.push_str("  ");
        out.push_str(&flat(v));
        out.push('\n');
    }
    out
}

/// Fit a terminal hint to one line without splitting a word unexpectedly.
pub(super) fn fit_line(text: &str, width_limit: Option<usize>) -> String {
    let text = flat(text);
    width_limit.map_or(text.clone(), |limit| clip_end(&text, limit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_tables_stack_with_middle_ids_and_end_text_clipping() {
        let headers = vec!["ID".into(), "TITLE".into()];
        let rows = vec![vec![
            "12345678-1234-1234-1234-123456789012".into(),
            "A very long title that needs shortening".into(),
        ]];
        let rendered = table(&headers, &rows, Some(32), false);
        assert!(rendered.contains("ID: 12345678-1234-…-123456789012"));
        assert!(rendered.contains("TITLE: A very long title that n…"));
        assert!(rendered.lines().all(|line| width(line) <= 32));
    }

    #[test]
    fn wide_cells_align_by_terminal_columns_and_controls_are_flattened() {
        let headers = vec!["ID".into(), "TITLE".into()];
        let rows = vec![vec!["note-1".into(), "東京\tCafe\nnotes".into()]];
        let rendered = table(&headers, &rows, Some(80), false);
        let lines: Vec<_> = rendered.lines().collect();
        let header_column = width(&lines[0][..lines[0].find("TITLE").unwrap()]);
        let row_column = width(&lines[1][..lines[1].find("東京").unwrap()]);
        assert_eq!(header_column, row_column);
        assert!(lines[1].contains("東京 Cafe notes"));
    }

    #[test]
    fn low_priority_columns_are_hidden_with_a_notice() {
        let headers = vec![
            "ID".into(),
            "OWNER".into(),
            "NAME".into(),
            "PLATFORM".into(),
            "STATE".into(),
            "DESCRIPTION".into(),
            "SHA-256".into(),
        ];
        let rows = vec![vec![
            "i".repeat(36),
            "o".repeat(43),
            "n".repeat(30),
            "p".repeat(20),
            "s".repeat(12),
            "d".repeat(60),
            "h".repeat(64),
        ]];
        let rendered = table(&headers, &rows, Some(80), false);
        assert!(rendered.contains("1 column hidden; use --json or a wider terminal."));
        assert!(rendered.lines().all(|line| width(line) <= 80));
    }

    #[test]
    fn unbounded_tables_keep_full_cells_for_pipes() {
        let headers = vec!["ID".into(), "TITLE".into()];
        let long = "a".repeat(96);
        let rows = vec![vec!["id".into(), long.clone()]];
        let rendered = table(&headers, &rows, None, false);
        assert!(rendered.contains(&long));
    }
}
