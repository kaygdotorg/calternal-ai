//! Windowed occurrence expansion for a prepared collection.

use crate::{
    Collection, Diagnostic, Occurrence, OccurrenceOverride, RepeatRule, Value,
    lines::Site,
    repeat::{Window, expand_series, parse_stamp},
};

/// A visible one-off or series selected for a collection window.
#[derive(Clone, Copy, Debug)]
pub struct CalendarSeries<'a> {
    /// Index of the source item in the authorized rows supplied by the host.
    pub index: usize,
    /// Start date or timestamp, using the original seed for a series.
    pub start: &'a str,
    /// Optional checked repeat rule.
    pub rule: Option<&'a RepeatRule>,
    /// Authorized EXDATE values for this series.
    pub exclusions: &'a [&'a str],
    /// Authorized moved instances, including any whose new start enters the window.
    pub overrides: &'a [OccurrenceOverride<'a>],
}

/// One calendar entry with a reference back to its visible source item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalendarOccurrence {
    /// Source item index from the series input.
    pub index: usize,
    /// Original slot and effective start in the calendar's civil timeline.
    pub occurrence: Occurrence,
}

/// Expand visible rows for a calendar view or a collection with expand: repeat.
/// Supply the pre-page authorized matches and a half-open civil start window.
/// Returns up to 1,000 entries in effective start order with one shared budget.
pub fn expand_calendar_window(
    collection: &Collection,
    series: &[CalendarSeries<'_>],
    start: &str,
    end: &str,
) -> Result<Vec<CalendarOccurrence>, Diagnostic> {
    if !collection.expand_repeat
        && !collection
            .compiled_views
            .iter()
            .any(|view| view.kind == "calendar")
    {
        return Err(collection
            .site
            .error("Use a calendar view or expand: repeat"));
    }
    let window = Window::parse(start, end)?;
    let invalid = || Site::START.error("A calendar occurrence must have a civil date or time");
    let mut output = Vec::new();
    let mut steps: usize = 1_000_000;
    for item in series {
        steps = steps
            .checked_sub(1)
            .ok_or_else(|| collection.site.error("The calendar window is too costly"))?;
        let occurrences = if let Some(rule) = item.rule {
            expand_series(
                rule,
                item.start,
                window,
                item.exclusions,
                item.overrides,
                &mut steps,
            )?
        } else {
            let stamp = parse_stamp(item.start)?;
            if (window.lower..window.upper).contains(&stamp.at) {
                let start = stamp.value(stamp.at).ok_or_else(invalid)?;
                vec![Occurrence {
                    original: start.clone(),
                    start,
                }]
            } else {
                vec![]
            }
        };
        if output.len().saturating_add(occurrences.len()) > 1_000 {
            return Err(collection
                .site
                .error("The calendar window contains more than 1,000 occurrences"));
        }
        for occurrence in occurrences {
            let text = match &occurrence.start {
                Value::Date(text) | Value::Time(text) => text,
                _ => return Err(invalid()),
            };
            let at = parse_stamp(text)?.at;
            output.push((
                at,
                CalendarOccurrence {
                    index: item.index,
                    occurrence,
                },
            ));
        }
    }
    output.sort_by_key(|(at, _)| *at);
    Ok(output
        .into_iter()
        .map(|(_, occurrence)| occurrence)
        .collect())
}
