//! Column alignment for [`super::Block::Table`] and labelled key/value blocks.

/// Narrowest a column is shrunk to when a line is too wide for the terminal.
const MIN_SHRUNK: usize = 12;

/// Number of characters in `s`; the unit widths are measured in (no East Asian
/// width handling: wide glyphs may misalign by a column).
fn chars(s: &str) -> usize {
    s.chars().count()
}

/// Cut `s` to at most `max` characters, ending in `…` when something was removed.
fn cut(s: &str, max: usize) -> String {
    if chars(s) <= max {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Replace line breaks and tabs so a cell never breaks the row layout.
fn flat(cell: &str) -> String {
    cell.replace(['\n', '\r', '\t'], " ")
}

/// Render `headers` and `rows` as columns separated by two spaces. With a `width`,
/// the widest column is shrunk (never below 12 characters) until each line fits;
/// without one nothing is cut, so piped output is lossless. Cost: one pass over
/// the cells to measure and one to write. `bold` wraps the header line in ANSI bold.
pub(super) fn table(
    headers: &[String],
    rows: &[Vec<String>],
    width: Option<usize>,
    bold: bool,
) -> String {
    let rows: Vec<Vec<String>> = rows
        .iter()
        .map(|r| r.iter().map(|c| flat(c)).collect())
        .collect();
    let mut w: Vec<usize> = headers.iter().map(|h| chars(h)).collect();
    for r in &rows {
        for (i, c) in r.iter().enumerate().take(w.len()) {
            w[i] = w[i].max(chars(c));
        }
    }
    if let Some(limit) = width {
        let gaps = 2 * w.len().saturating_sub(1);
        while w.iter().sum::<usize>() + gaps > limit {
            let (i, &widest) = w
                .iter()
                .enumerate()
                .max_by_key(|(_, v)| **v)
                .expect("column");
            if widest <= MIN_SHRUNK {
                break;
            }
            w[i] = (widest - (w.iter().sum::<usize>() + gaps - limit)).max(MIN_SHRUNK);
        }
    }
    let line = |cells: &[String]| {
        let mut s = String::new();
        for (i, width) in w.iter().enumerate() {
            let cell = cut(cells.get(i).map_or("", String::as_str), *width);
            s.push_str(&cell);
            if i + 1 < w.len() {
                s.push_str(&" ".repeat(width - chars(&cell) + 2));
            }
        }
        s
    };
    let head = line(headers);
    let mut out = if bold {
        format!("\x1b[1m{head}\x1b[0m\n")
    } else {
        format!("{head}\n")
    };
    for r in &rows {
        out.push_str(&line(r));
        out.push('\n');
    }
    out
}

/// Render `label:  value` lines with the values aligned. Values are never cut (a
/// path or id must stay copyable); the terminal wraps them. Cost: one pass.
pub(super) fn fields(items: &[(String, String)]) -> String {
    let label = items.iter().map(|(k, _)| chars(k)).max().unwrap_or(0) + 1;
    let mut out = String::new();
    for (k, v) in items {
        out.push_str(&format!("{:<label$}  {}\n", format!("{k}:"), flat(v)));
    }
    out
}
