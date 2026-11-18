//! Bounded typo verification after the title bigram index narrows candidates.
use crate::{Result, types::internal};
use rusqlite::{Connection, functions::FunctionFlags};
use std::{collections::BTreeSet, fmt::Write};

pub(super) fn install(conn: &Connection) -> Result<()> {
    let flags = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC;
    conn.create_scalar_function("search_grams", 1, flags, |ctx| {
        let title = ctx.get_raw(0).as_str()?;
        let tokens: BTreeSet<_> = words(title).flat_map(grams).collect();
        Ok(tokens.into_iter().collect::<Vec<_>>().join(" "))
    })
    .map_err(internal)?;
    conn.create_scalar_function(
        "search_one_edit",
        2,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let title = ctx.get_raw(0).as_str()?;
            let term: Vec<_> = ctx.get_raw(1).as_str()?.chars().collect();
            Ok(words(title).any(|word| {
                let word: Vec<_> = word.chars().collect();
                word.len().abs_diff(term.len()) <= 1 && distance(&word, &term, 1) == Some(1)
            }))
        },
    )
    .map_err(internal)
}

fn words(title: &str) -> impl Iterator<Item = &str> {
    title
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
}

// Character pairs with word-boundary markers, ported from the earlier vocabulary
// index. Encode pairs as FTS tokens so punctuation is never query syntax.
fn grams(term: &str) -> Vec<String> {
    let mut chars = vec!['\u{2}'];
    chars.extend(term.chars());
    chars.push('\u{3}');
    chars
        .windows(2)
        .map(|pair| {
            let mut token = String::new();
            write!(token, "b{:06x}{:06x}", pair[0] as u32, pair[1] as u32).expect("string writes");
            token
        })
        .collect()
}

/// Compile a single 3–32 character term to a bounded typo candidate expression.
pub fn typo_expression(term: &str) -> Option<String> {
    if !(3..=32).contains(&term.chars().count()) || !term.chars().all(char::is_alphanumeric) {
        return None;
    }
    let tokens: BTreeSet<_> = grams(term).into_iter().collect();
    Some(tokens.into_iter().collect::<Vec<_>>().join(" OR "))
}

/// Edit distance with adjacent swaps counted as one edit (`teh` → `the`),
/// or `None` once it's over `max`.
fn distance(a: &[char], b: &[char], max: usize) -> Option<usize> {
    let (n, m) = (a.len(), b.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        let mut best = usize::MAX;
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut v = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                v = v.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = v;
            best = best.min(v);
        }
        if best > max {
            return None;
        }
    }
    (d[n][m] <= max).then_some(d[n][m])
}
