use std::collections::BTreeSet;

use super::RepeatRule;
use crate::{Diagnostic, Value, dates, lines::Site};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Frequency {
    Secondly,
    Minutely,
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Clone, Debug)]
pub(super) struct Stamp {
    pub at: i64,
    pub timed: bool,
    pub offset: Option<i32>,
}

impl Stamp {
    pub fn parse(text: &str) -> Option<Self> {
        if let Some(day) = dates::day(text) {
            return Some(Self {
                at: day * 86400,
                timed: false,
                offset: None,
            });
        }
        let (at, offset) = dates::time(text)?;
        let offset = text[11..].contains(['Z', '+', '-']).then_some(offset);
        Some(Self {
            at,
            timed: true,
            offset,
        })
    }

    fn ical(text: &str) -> Option<Self> {
        let raw = text.strip_suffix('Z').unwrap_or(text);
        let bytes = raw.as_bytes();
        if !bytes
            .iter()
            .enumerate()
            .all(|(index, c)| c.is_ascii_digit() || (index == 8 && *c == b'T'))
        {
            return None;
        }
        let date = raw.get(..8)?;
        let date = format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..]);
        match raw.len() {
            8 if raw.len() == text.len() => Self::parse(&date),
            15 if bytes[8] == b'T' => {
                let tz = &text[raw.len()..];
                Self::parse(&format!(
                    "{date}T{}:{}:{}{tz}",
                    &raw[9..11],
                    &raw[11..13],
                    &raw[13..15]
                ))
            }
            _ => None,
        }
    }

    pub fn value(&self, at: i64) -> Option<Value> {
        let date = dates::render_day(at.div_euclid(86400))?;
        if !self.timed {
            return Some(Value::Date(date));
        }
        let second = at.rem_euclid(86400);
        let offset = match self.offset {
            None => String::new(),
            Some(0) => "Z".into(),
            Some(offset) => {
                let sign = if offset < 0 { '-' } else { '+' };
                let seconds = offset.abs();
                format!("{sign}{:02}:{:02}", seconds / 3600, seconds % 3600 / 60)
            }
        };
        Some(Value::Time(format!(
            "{date}T{:02}:{:02}:{:02}{offset}",
            second / 3600,
            second % 3600 / 60,
            second % 60
        )))
    }
}

pub(super) fn parse_at(text: &str, site: Site) -> Result<RepeatRule, Diagnostic> {
    let reject = || site.error("Use a valid iCalendar recurrence rule");
    if text.len() > 4096 || !text.is_ascii() {
        return Err(reject());
    }
    let upper = text.to_ascii_uppercase();
    let text = upper.strip_prefix("RRULE:").unwrap_or(&upper);
    let mut freq = None;
    let mut rule = RepeatRule {
        frequency: Frequency::Daily,
        interval: 1,
        count: None,
        until: None,
        week_start: 0,
        weekdays: Vec::new(),
        month_days: Vec::new(),
        months: Vec::new(),
        year_days: Vec::new(),
        week_numbers: Vec::new(),
        hours: Vec::new(),
        minutes: Vec::new(),
        seconds: Vec::new(),
        positions: Vec::new(),
    };
    let mut seen = BTreeSet::new();
    for token in text.split(';') {
        let (key, value) = token.split_once('=').ok_or_else(reject)?;
        if !seen.insert(key) {
            return Err(reject());
        }
        let numeric = |min, max, signed| numbers(value, min, max, signed).ok_or_else(reject);
        match key {
            "FREQ" => {
                freq = Some(match value {
                    "SECONDLY" => Frequency::Secondly,
                    "MINUTELY" => Frequency::Minutely,
                    "HOURLY" => Frequency::Hourly,
                    "DAILY" => Frequency::Daily,
                    "WEEKLY" => Frequency::Weekly,
                    "MONTHLY" => Frequency::Monthly,
                    "YEARLY" => Frequency::Yearly,
                    _ => return Err(reject()),
                })
            }
            "INTERVAL" => rule.interval = positive(value).ok_or_else(reject)?,
            "COUNT" => rule.count = Some(positive(value).ok_or_else(reject)?),
            "UNTIL" => rule.until = Some(Stamp::ical(value).ok_or_else(reject)?),
            "WKST" => rule.week_start = weekday(value).ok_or_else(reject)?,
            "BYDAY" => {
                rule.weekdays = value
                    .split(',')
                    .map(|token| {
                        let (ordinal, name) =
                            token.split_at_checked(token.len().checked_sub(2)?)?;
                        let ordinal = if ordinal.is_empty() {
                            0
                        } else {
                            ordinal
                                .parse::<i64>()
                                .ok()
                                .filter(|n| (-53..=53).contains(n) && *n != 0)?
                        };
                        Some((weekday(name)?, ordinal))
                    })
                    .collect::<Option<_>>()
                    .ok_or_else(reject)?;
            }
            "BYMONTHDAY" => rule.month_days = numeric(-31, 31, true)?,
            "BYMONTH" => rule.months = numeric(1, 12, false)?,
            "BYYEARDAY" => rule.year_days = numeric(-366, 366, true)?,
            "BYWEEKNO" => rule.week_numbers = numeric(-53, 53, true)?,
            "BYHOUR" => rule.hours = numeric(0, 23, false)?,
            "BYMINUTE" => rule.minutes = numeric(0, 59, false)?,
            "BYSECOND" => rule.seconds = numeric(0, 59, false)?,
            "BYSETPOS" => rule.positions = numeric(-366, 366, true)?,
            _ => return Err(reject()),
        }
    }
    rule.frequency = freq.ok_or_else(reject)?;
    let ordinal = rule.weekdays.iter().any(|&(_, ordinal)| ordinal != 0);
    if rule.count.is_some() && rule.until.is_some()
        || ordinal
            && (!matches!(rule.frequency, Frequency::Monthly | Frequency::Yearly)
                || !rule.week_numbers.is_empty())
        || !rule.month_days.is_empty() && rule.frequency == Frequency::Weekly
        || !rule.year_days.is_empty()
            && matches!(
                rule.frequency,
                Frequency::Daily | Frequency::Weekly | Frequency::Monthly
            )
        || !rule.week_numbers.is_empty() && rule.frequency != Frequency::Yearly
        || !rule.positions.is_empty()
            && !seen
                .iter()
                .any(|key| key.starts_with("BY") && *key != "BYSETPOS")
    {
        return Err(reject());
    }
    Ok(rule)
}

fn positive(text: &str) -> Option<i64> {
    text.parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .map(i64::from)
}

fn numbers(text: &str, min: i64, max: i64, signed: bool) -> Option<Vec<i64>> {
    let result = text
        .split(',')
        .map(|part| {
            if !signed && !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            part.parse::<i64>()
                .ok()
                .filter(|n| (min..=max).contains(n) && (!signed || *n != 0))
        })
        .collect::<Option<BTreeSet<_>>>()?;
    Some(result.into_iter().collect())
}

fn weekday(text: &str) -> Option<i64> {
    ["MO", "TU", "WE", "TH", "FR", "SA", "SU"]
        .iter()
        .position(|day| *day == text)
        .map(|n| n as i64)
}
