//! Deterministic, account-zone calendar schedules for space automations.

use calternal_types::Error;
use jiff::{Timestamp, civil::Weekday};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Day {
    Daily,
    Weekdays,
    Weekly(Weekday),
    Monthly(u8),
}

const SHORT: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const LONG: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// One local wall-clock time on each date selected by a supported calendar rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schedule {
    day: Day,
    hour: u8,
    minute: u8,
}

impl Schedule {
    /// Parse a bounded English schedule phrase, consuming the entire phrase.
    /// Returns a caller-safe example when the phrase is unsupported.
    pub fn parse(words: &str) -> Result<Self, Error> {
        const EXAMPLE: &str =
            "Try every weekday at 9:00, every Monday at 8:30, or monthly on the 1st at 5pm.";
        if words.len() > 256 || words.chars().any(char::is_control) {
            return Err(Error::invalid_input(EXAMPLE));
        }
        let lowercase = words.to_ascii_lowercase();
        Self::parse_words(&lowercase.split_ascii_whitespace().collect::<Vec<_>>())
            .ok_or_else(|| Error::invalid_input(EXAMPLE))
    }

    fn parse_words(words: &[&str]) -> Option<Self> {
        let (day, rest) = match words {
            ["daily", rest @ ..] | ["every", "day", rest @ ..] => (Day::Daily, rest),
            ["weekdays" | "weekday", rest @ ..] | ["every", "weekday" | "weekdays", rest @ ..] => {
                (Day::Weekdays, rest)
            }
            ["monthly", "on", "the", ordinal, rest @ ..]
            | ["every", "month", "on", "the", ordinal, rest @ ..] => {
                (Day::Monthly(parse_ordinal(ordinal)?), rest)
            }
            ["monthly", rest @ ..] | ["every", "month", rest @ ..] => (Day::Monthly(1), rest),
            ["every", name, rest @ ..] => {
                let index = LONG.iter().zip(SHORT).position(|(long, short)| {
                    name.eq_ignore_ascii_case(long) || name.eq_ignore_ascii_case(short)
                })?;
                (
                    Day::Weekly(Weekday::from_monday_zero_offset(index as i8).ok()?),
                    rest,
                )
            }
            _ => return None,
        };
        let has_at = rest.first() == Some(&"at");
        let (clock, period) = match rest.strip_prefix(&["at"]).unwrap_or(rest) {
            [clock, "am"] => (*clock, Some("am")),
            [clock, "pm"] => (*clock, Some("pm")),
            [clock] => clock
                .strip_suffix("am")
                .map(|clock| (clock, Some("am")))
                .or_else(|| clock.strip_suffix("pm").map(|clock| (clock, Some("pm"))))
                .unwrap_or((clock, None)),
            _ => return None,
        };
        if !has_at && !clock.contains(':') && period.is_none() {
            return None;
        }
        let (hour, minute) = parse_clock(clock, period)?;
        Some(Self { day, hour, minute })
    }

    /// Return the canonical systemd OnCalendar expression, excluding the saved zone.
    pub fn expression(self) -> String {
        let clock = format!("{:02}:{:02}:00", self.hour, self.minute);
        match self.day {
            Day::Daily => format!("*-*-* {clock}"),
            Day::Weekdays => format!("Mon..Fri *-*-* {clock}"),
            Day::Weekly(day) => format!(
                "{} *-*-* {clock}",
                SHORT[day.to_monday_zero_offset() as usize]
            ),
            Day::Monthly(day) => format!("*-*-{day:02} {clock}"),
        }
    }

    /// Decode only canonical expressions emitted by this schedule version.
    /// An unknown expression is left to the caller's format error handling.
    pub fn from_expression(expression: &str) -> Option<Self> {
        let (rule, clock) = expression.rsplit_once(' ')?;
        let (hour, minute) = parse_clock(clock.strip_suffix(":00")?, None)?;
        let day = match rule {
            "*-*-*" => Day::Daily,
            "Mon..Fri *-*-*" => Day::Weekdays,
            rule if rule.starts_with("*-*-") => Day::Monthly(parse_number(&rule[4..], 1, 31)?),
            rule => {
                let prefix = rule.strip_suffix(" *-*-*")?;
                let index = SHORT.iter().position(|short| *short == prefix)?;
                Day::Weekly(Weekday::from_monday_zero_offset(index as i8).ok()?)
            }
        };
        let schedule = Self { day, hour, minute };
        (schedule.expression() == expression).then_some(schedule)
    }

    /// Describe the parsed expression in the same words as schedule confirmation.
    pub fn describe(self) -> String {
        let day = match self.day {
            Day::Daily => "every day".to_owned(),
            Day::Weekdays => "every weekday".to_owned(),
            Day::Weekly(day) => format!("every {}", LONG[day.to_monday_zero_offset() as usize]),
            Day::Monthly(day) => format!("every month on the {day}{}", ordinal_suffix(day)),
        };
        format!("{day} at {}:{:02}", self.hour, self.minute)
    }

    /// Return the first scheduled instant strictly after the supplied UTC
    /// milliseconds. Spring-forward gaps are skipped and folds use the first instant.
    pub fn next(self, zone: &str, after_ms: i64) -> Option<i64> {
        self.walk(zone, after_ms, 1, |candidate| candidate > after_ms)
    }

    /// Return the last scheduled instant at or before the supplied UTC milliseconds.
    /// The walk is calendar-bounded even when a machine was off for years.
    pub fn latest(self, zone: &str, at_ms: i64) -> Option<i64> {
        self.walk(zone, at_ms, -1, |candidate| candidate <= at_ms)
    }

    fn walk(
        self,
        zone: &str,
        from_ms: i64,
        step: i32,
        select: impl Fn(i64) -> bool,
    ) -> Option<i64> {
        let zone = crate::time_zone::named(zone)?;
        let mut date = Timestamp::from_millisecond(from_ms)
            .ok()?
            .to_zoned(zone.clone())
            .date();
        for _ in 0..=124 {
            let matches = match self.day {
                Day::Daily => true,
                Day::Weekdays => date.weekday().to_monday_zero_offset() < 5,
                Day::Weekly(day) => date.weekday() == day,
                Day::Monthly(day) => date.day() == day as i8,
            };
            if matches {
                let local = date.at(self.hour as i8, self.minute as i8, 0, 0);
                let candidate = zone.to_ambiguous_timestamp(local);
                if !matches!(candidate.offset(), jiff::tz::AmbiguousOffset::Gap { .. }) {
                    let instant = candidate.earlier().ok()?.as_millisecond();
                    if select(instant) {
                        return Some(instant);
                    }
                }
            }
            date = date.checked_add(jiff::Span::new().days(step)).ok()?;
        }
        None
    }
}

/// Compare two UTC millisecond instants by their civil day in the saved zone.
pub fn same_day(zone: &str, first_ms: i64, second_ms: i64) -> Option<bool> {
    let zone = crate::time_zone::named(zone)?;
    let date = |millis| {
        Some(
            Timestamp::from_millisecond(millis)
                .ok()?
                .to_zoned(zone.clone())
                .date(),
        )
    };
    Some(date(first_ms)? == date(second_ms)?)
}

fn parse_number(text: &str, minimum: u8, maximum: u8) -> Option<u8> {
    if !(1..=2).contains(&text.len()) || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let value = text.parse().ok()?;
    (minimum..=maximum).contains(&value).then_some(value)
}

fn parse_clock(clock: &str, period: Option<&str>) -> Option<(u8, u8)> {
    let (hour, minute) = match clock.split_once(':') {
        Some((hour, minute)) if minute.len() == 2 => (hour, parse_number(minute, 0, 59)?),
        Some(_) => return None,
        None => (clock, 0),
    };
    let hour = match period {
        Some(period) => parse_number(hour, 1, 12)? % 12 + if period == "pm" { 12 } else { 0 },
        None => parse_number(hour, 0, 23)?,
    };
    Some((hour, minute))
}

fn parse_ordinal(word: &str) -> Option<u8> {
    let digits = word.bytes().take_while(u8::is_ascii_digit).count();
    let day = parse_number(&word[..digits], 1, 31)?;
    (ordinal_suffix(day) == &word[digits..]).then_some(day)
}

fn ordinal_suffix(day: u8) -> &'static str {
    if matches!(day, 11..=13) {
        return "th";
    }
    match day % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    }
}

#[cfg(test)]
mod tests;
