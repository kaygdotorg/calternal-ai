//! Civil recurrence independently of clocks and time zone databases.

use std::collections::{BTreeMap, BTreeSet};

use crate::{Diagnostic, Value, dates, lines::Site};

mod rule;
mod search;
pub(crate) use rule::Stamp;
use rule::{Frequency, parse_at};

/// A checked iCalendar recurrence rule. Expansion needs the original DTSTART.
#[derive(Clone, Debug)]
pub struct RepeatRule {
    frequency: Frequency,
    interval: i64,
    count: Option<i64>,
    until: Option<Stamp>,
    week_start: i64,
    weekdays: Vec<(i64, i64)>,
    month_days: Vec<i64>,
    months: Vec<i64>,
    year_days: Vec<i64>,
    week_numbers: Vec<i64>,
    hours: Vec<i64>,
    minutes: Vec<i64>,
    seconds: Vec<i64>,
    positions: Vec<i64>,
}

/// One visible exception supplied by the host for a single series.
#[derive(Clone, Copy, Debug)]
pub struct OccurrenceOverride<'a> {
    /// Stable original slot (a date or civil timestamp).
    pub original: &'a str,
    /// The moved occurrence's new start, in the same civil calendar.
    pub start: &'a str,
}

/// A series slot in a requested calendar window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Occurrence {
    /// Original occurrence identity, even if its start was overridden.
    pub original: Value,
    /// Effective start, after applying any supplied override.
    pub start: Value,
}

/// Parse one iCalendar RRULE, accepting the optional RRULE prefix.
pub fn parse_repeat(text: &str) -> Result<RepeatRule, Diagnostic> {
    parse_at(text, Site::START)
}

/// Return the next start strictly after the supplied date or time, within ten
/// civil years. The original seed determines the interval phase and clock.
/// Zoned callers provide civil times in the series zone and resolve offsets.
pub fn next_occurrence(
    rule: &RepeatRule,
    start: &str,
    after: &str,
) -> Result<Option<Value>, Diagnostic> {
    next_rule(rule, start, after, &mut 50_000, Site::START)
}

pub(crate) fn next_in(
    rule: &str,
    start: &str,
    after: &str,
    steps: &mut usize,
    site: Site,
) -> Result<Option<Value>, Diagnostic> {
    next_rule(&parse_at(rule, site)?, start, after, steps, site)
}

fn next_rule(
    rule: &RepeatRule,
    start: &str,
    after: &str,
    steps: &mut usize,
    site: Site,
) -> Result<Option<Value>, Diagnostic> {
    let seed = Stamp::parse(start).ok_or_else(|| site.error("Supply a valid recurrence start"))?;
    let after =
        Stamp::parse(after).ok_or_else(|| site.error("Supply a valid recurrence boundary"))?;
    let day = after.at.div_euclid(86400);
    let end = dates::shifted_day(day, 120, 0).unwrap_or(day) * 86400 + after.at.rem_euclid(86400);
    let mut next = None;
    rule.search(&seed, after.at + 1, end + 1, steps, site, |at| {
        next = seed.value(at);
        false
    })?;
    Ok(next)
}

/// Expand starts within the half-open civil window. A date exclusion
/// removes every occurrence on that date. An override replaces its original
/// slot and is included if its effective start is in the window.
/// The query is limited to 1,000 visible occurrences and a ten-year window.
pub fn expand_repeat(
    rule: &RepeatRule,
    start: &str,
    window_start: &str,
    window_end: &str,
    exclusions: &[&str],
    overrides: &[OccurrenceOverride<'_>],
) -> Result<Vec<Occurrence>, Diagnostic> {
    expand_series(
        rule,
        start,
        Window::parse(window_start, window_end)?,
        exclusions,
        overrides,
        &mut 1_000_000,
    )
}

#[derive(Clone, Copy)]
pub(crate) struct Window {
    pub lower: i64,
    pub upper: i64,
}

impl Window {
    pub fn parse(start: &str, end: &str) -> Result<Self, Diagnostic> {
        let lower = parse_stamp(start)?.at;
        let upper = parse_stamp(end)?.at;
        let first_day = lower.div_euclid(86400);
        let last_day = dates::shifted_day(first_day, 120, 0).unwrap_or(first_day);
        if lower >= upper || upper > last_day * 86400 + lower.rem_euclid(86400) {
            return Err(
                Site::START.error("Supply a nonempty recurrence window of at most ten years")
            );
        }
        Ok(Self { lower, upper })
    }
}

pub(crate) fn parse_stamp(text: &str) -> Result<Stamp, Diagnostic> {
    Stamp::parse(text).ok_or_else(|| Site::START.error("Supply a valid date or time"))
}

pub(crate) fn expand_series(
    rule: &RepeatRule,
    start: &str,
    window: Window,
    exclusions: &[&str],
    overrides: &[OccurrenceOverride<'_>],
    steps: &mut usize,
) -> Result<Vec<Occurrence>, Diagnostic> {
    let site = Site::START;
    let seed = parse_stamp(start)?;
    let Window { lower, upper } = window;
    let exceptions = exclusions.len().saturating_add(overrides.len());
    if exceptions > 10_000 {
        return Err(site.error("A repeat may supply at most 10,000 exceptions"));
    }
    *steps = steps
        .checked_sub(exceptions)
        .ok_or_else(|| site.error("The recurrence is too costly"))?;
    let mut excluded_days = BTreeSet::new();
    let mut excluded_times = BTreeSet::new();
    let mut changed = BTreeMap::new();
    for text in exclusions {
        let exclusion = parse_stamp(text)?;
        if exclusion.timed {
            excluded_times.insert(exclusion.at);
        } else {
            excluded_days.insert(exclusion.at.div_euclid(86400));
        }
    }
    for item in overrides {
        let original = parse_stamp(item.original)?;
        let current = parse_stamp(item.start)?;
        let mut slot = false;
        if original.timed == seed.timed {
            rule.search(&seed, original.at, original.at + 1, steps, site, |_| {
                slot = true;
                false
            })?;
        }
        if !slot {
            return Err(site.error("An override must name an original recurrence slot"));
        }
        if changed.insert(original.at, current).is_some() {
            return Err(site.error("A recurrence slot may have only one override"));
        }
    }
    const MAX_VISIBLE: usize = 1_000;
    let overflow = || site.error("The calendar window contains more than 1,000 occurrences");
    let in_window = lower..upper;
    let mut found = Vec::new();
    for (&original, moved) in &changed {
        if in_window.contains(&moved.at) {
            if found.len() == MAX_VISIBLE {
                return Err(overflow());
            }
            if let (Some(original), Some(start)) = (seed.value(original), moved.value(moved.at)) {
                found.push((moved.at, Occurrence { original, start }));
            }
        }
    }
    let mut full = false;
    rule.search(&seed, lower, upper, steps, site, |at| {
        if !changed.contains_key(&at)
            && !excluded_days.contains(&at.div_euclid(86400))
            && !excluded_times.contains(&at)
        {
            if found.len() == MAX_VISIBLE {
                full = true;
                return false;
            }
            if let Some(start) = seed.value(at) {
                found.push((
                    at,
                    Occurrence {
                        original: start.clone(),
                        start,
                    },
                ));
            }
        }
        true
    })?;
    if full {
        return Err(overflow());
    }
    found.sort_by_key(|(at, _)| *at);
    Ok(found.into_iter().map(|(_, item)| item).collect())
}

#[cfg(test)]
mod tests;
