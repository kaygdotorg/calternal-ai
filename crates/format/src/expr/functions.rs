use crate::{Diagnostic, Value, dates, lines::Site};

pub(crate) fn domain(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority.rsplit('@').next()?;
    let host = if authority.starts_with('[') {
        authority.get(..=authority.find(']')?)?
    } else {
        authority.split(':').next()?
    };
    (!host.is_empty() && !host.contains([' ', '\t', '\n', '\r', '\\']))
        .then(|| host.to_ascii_lowercase())
}

pub(crate) fn format(value: Value, pattern: &str, site: Site) -> Result<Value, Diagnostic> {
    let text = match value {
        Value::Empty => return Ok(Value::Empty),
        Value::Number(number) | Value::Money { amount: number, .. } => {
            let places = if pattern == "0" {
                0
            } else {
                pattern
                    .strip_prefix("0.")
                    .filter(|part| {
                        !part.is_empty() && part.len() <= 18 && part.bytes().all(|c| c == b'0')
                    })
                    .ok_or_else(|| site.error("Use a number pattern such as 0.00"))?
                    .len()
            };
            let mut number = number.rounded(places as u32).to_string();
            if places > 0 {
                let fraction = match number.split_once('.') {
                    Some((_, fraction)) => fraction.len(),
                    None => {
                        number.push('.');
                        0
                    }
                };
                number.extend(std::iter::repeat_n('0', places - fraction));
            }
            number
        }
        Value::Date(date) | Value::Time(date) => {
            let date = date
                .get(..10)
                .filter(|date| dates::day(date).is_some())
                .ok_or_else(|| site.error("Use a valid date"))?;
            let mut rest = pattern;
            let mut output = String::new();
            while !rest.is_empty() {
                if let Some((token, value)) = [
                    (
                        "GGGG-[W]WW",
                        dates::period(date, "week").unwrap_or_default(),
                    ),
                    ("YYYY", date[..4].to_owned()),
                    ("MM", date[5..7].to_owned()),
                    ("DD", date[8..10].to_owned()),
                ]
                .into_iter()
                .find(|(token, _)| rest.starts_with(token))
                {
                    output.push_str(&value);
                    rest = &rest[token.len()..];
                } else {
                    let ch = rest.chars().next().unwrap();
                    output.push(ch);
                    rest = &rest[ch.len_utf8()..];
                }
            }
            output
        }
        _ => return Err(site.error("Format a date, time or number")),
    };
    Ok(Value::Text(text))
}

pub(crate) fn streak(records: Vec<Value>) -> Value {
    let mut count = 0;
    for record in records.iter().rev() {
        let Value::Map(fields) = record else { break };
        if fields
            .get("skipped")
            .is_some_and(|value| value != &Value::Empty)
            || fields
                .get("done")
                .is_none_or(|value| value == &Value::Empty)
        {
            break;
        }
        count += 1;
    }
    Value::Number(crate::Decimal::integer(count))
}

pub(crate) struct Repeat {
    frequency: Frequency,
    interval: i64,
    weekdays: Vec<i64>,
    month_days: Vec<i64>,
    until: Option<i64>,
}

enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

pub(crate) fn repeat(rule: &str) -> Option<Repeat> {
    let mut frequency = None;
    let mut interval = None;
    let mut weekdays = None;
    let mut month_days = None;
    let mut until = None;
    for clause in rule.strip_prefix("RRULE:").unwrap_or(rule).split(';') {
        let (key, value) = clause.split_once('=')?;
        match key {
            "FREQ" if frequency.is_none() => {
                frequency = Some(match value {
                    "DAILY" => Frequency::Daily,
                    "WEEKLY" => Frequency::Weekly,
                    "MONTHLY" => Frequency::Monthly,
                    "YEARLY" => Frequency::Yearly,
                    _ => return None,
                })
            }
            "BYDAY" if weekdays.is_none() => {
                weekdays = Some(
                    value
                        .split(',')
                        .map(|day| {
                            ["MO", "TU", "WE", "TH", "FR", "SA", "SU"]
                                .iter()
                                .position(|value| *value == day)
                                .map(|n| n as i64)
                        })
                        .collect::<Option<Vec<_>>>()?,
                )
            }
            "BYMONTHDAY" if month_days.is_none() => {
                month_days = Some(
                    value
                        .split(',')
                        .map(|day| {
                            day.parse::<i64>()
                                .ok()
                                .filter(|day| (-31..=31).contains(day) && *day != 0)
                        })
                        .collect::<Option<Vec<_>>>()?,
                )
            }
            "INTERVAL" if interval.is_none() => {
                interval = Some(value.parse::<i64>().ok().filter(|n| *n > 0)?)
            }
            "UNTIL" if until.is_none() => {
                let date = value.get(..8)?;
                if !date.bytes().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                until = Some(dates::day(&format!(
                    "{}-{}-{}",
                    &date[..4],
                    &date[4..6],
                    &date[6..]
                ))?);
            }
            _ => return None,
        }
    }
    Some(Repeat {
        frequency: frequency?,
        interval: interval.unwrap_or(1),
        weekdays: weekdays.unwrap_or_default(),
        month_days: month_days.unwrap_or_default(),
        until,
    })
}

pub(crate) fn next(
    rule: &str,
    after: i64,
    steps: &mut usize,
    site: Site,
) -> Result<Value, Diagnostic> {
    let rule = repeat(rule)
        .ok_or_else(|| site.error("This repeat uses an unsupported or invalid rule"))?;
    let max = dates::shifted_day(after, 120, 0).unwrap_or(after);
    let until = rule.until.unwrap_or(max).min(max);
    let interval = rule.interval;
    let weekday = |day: i64| (day + 3).rem_euclid(7);
    let matches = |day: i64| rule.weekdays.is_empty() || rule.weekdays.contains(&weekday(day));
    let matches_month = |day: i64| {
        if rule.month_days.is_empty() {
            return true;
        }
        let Some(date) = dates::render_day(day) else {
            return false;
        };
        let day_of_month = date[8..].parse::<i64>().unwrap_or(0);
        let last = (day..=day + 31)
            .take_while(|n| dates::render_day(*n).is_some_and(|current| current[..7] == date[..7]))
            .count() as i64
            + day_of_month;
        rule.month_days.contains(&day_of_month) || rule.month_days.contains(&(day_of_month - last))
    };
    let mut advance = |amount: i64| -> Result<(), Diagnostic> {
        *steps = steps
            .checked_sub(amount as usize)
            .ok_or_else(|| site.error("The repeat is too costly"))?;
        Ok(())
    };
    let result = match rule.frequency {
        Frequency::Daily | Frequency::Weekly => {
            let mut result = None;
            for day in after + 1..=until {
                advance(1)?;
                let phase = if matches!(rule.frequency, Frequency::Daily) {
                    day - after
                } else if rule.weekdays.is_empty() {
                    if weekday(day) != weekday(after) {
                        continue;
                    }
                    (day - after) / 7
                } else {
                    (day + weekday(after) - after).div_euclid(7)
                };
                if phase % interval == 0 && matches(day) && matches_month(day) {
                    result = Some(day);
                    break;
                }
            }
            result
        }
        Frequency::Monthly | Frequency::Yearly => {
            let unit = if matches!(rule.frequency, Frequency::Monthly) {
                1
            } else {
                12
            };
            let mut result = None;
            for offset in 1..=120 / unit {
                advance(1)?;
                if i64::from(offset) % interval != 0 {
                    continue;
                }
                let Some(day) =
                    dates::shifted_day(after, unit * offset, 0).filter(|day| *day <= until)
                else {
                    break;
                };
                if matches(day) && matches_month(day) {
                    result = Some(day);
                    break;
                }
            }
            result
        }
    };
    Ok(result
        .and_then(dates::render_day)
        .map_or(Value::Empty, Value::Date))
}
