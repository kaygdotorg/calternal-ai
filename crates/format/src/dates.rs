//! Civil arithmetic adapted from the engine spike's dates module.

use crate::Value;

pub(crate) fn day(text: &str) -> Option<i64> {
    let (year, month, date) = date_parts(text)?;
    Some(day_number(year, month, date))
}

fn date_parts(text: &str) -> Option<(i64, i64, i64)> {
    let bytes = text.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    {
        return None;
    }
    let year = text[..4].parse().ok()?;
    let month = text[5..7].parse().ok()?;
    let date = text[8..].parse().ok()?;
    ((1900..=2200).contains(&year) && (1..=days_in_month(year, month)).contains(&date))
        .then_some((year, month, date))
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 => 28 + i64::from(year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)),
        1..=12 => 31,
        _ => 0,
    }
}

fn day_number(year: i64, month: i64, date: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_in_era = year.rem_euclid(400);
    let march_month = month + if month > 2 { -3 } else { 9 };
    let date_in_year = (153 * march_month + 2) / 5 + date - 1;
    era * 146097 + year_in_era * 365 + year_in_era / 4 - year_in_era / 100 + date_in_year - 719468
}

fn civil(days: i64) -> (i64, i64, i64) {
    let days = days + 719468;
    let era = days.div_euclid(146097);
    let date_in_era = days.rem_euclid(146097);
    let year_in_era =
        (date_in_era - date_in_era / 1460 + date_in_era / 36524 - date_in_era / 146096) / 365;
    let date_in_year = date_in_era - (365 * year_in_era + year_in_era / 4 - year_in_era / 100);
    let march_month = (5 * date_in_year + 2) / 153;
    let date = date_in_year - (153 * march_month + 2) / 5 + 1;
    let month = march_month + if march_month < 10 { 3 } else { -9 };
    (year_in_era + era * 400 + i64::from(month <= 2), month, date)
}

pub(crate) fn render_day(days: i64) -> Option<String> {
    if !(day_number(1900, 1, 1)..=day_number(2200, 12, 31)).contains(&days) {
        return None;
    }
    let (year, month, date) = civil(days);
    Some(format!("{year:04}-{month:02}-{date:02}"))
}

pub(crate) fn shifted_day(days: i64, months: i32, seconds: i64) -> Option<i64> {
    let (year, month, date) = civil(days);
    let months = 12 * year + month - 1 + i64::from(months);
    let year = months.div_euclid(12);
    let month = months.rem_euclid(12) + 1;
    day_number(year, month, date.min(days_in_month(year, month)))
        .checked_add(seconds.div_euclid(86400))
}

pub(crate) fn time(text: &str) -> Option<(i64, i32)> {
    let date = day(text.get(..10)?)?;
    let rest = text
        .get(10..)?
        .strip_prefix('T')
        .or_else(|| text.get(10..)?.strip_prefix(' '))?;
    let hours = component(rest.get(..2)?, 23)?;
    let minutes = component(rest.get(2..5)?.strip_prefix(':')?, 59)?;
    let mut tail = rest.get(5..)?;
    let seconds = if let Some(after) = tail.strip_prefix(':') {
        tail = after.get(2..)?;
        component(after.get(..2)?, 59)?
    } else {
        0
    };
    let offset = match tail.as_bytes().first() {
        None | Some(b'Z') if matches!(tail, "" | "Z") => 0,
        Some(sign @ (b'+' | b'-')) if tail.len() == 6 => {
            let hour = component(tail.get(1..3)?, 23)?;
            let minute = component(tail.get(3..6)?.strip_prefix(':')?, 59)?;
            (hour * 3600 + minute * 60) * if *sign == b'-' { -1 } else { 1 }
        }
        _ => return None,
    };
    Some((
        date * 86400 + i64::from(hours * 3600 + minutes * 60 + seconds),
        offset,
    ))
}

fn component(text: &str, max: i32) -> Option<i32> {
    if text.len() != 2 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let number = text.parse().ok()?;
    (number <= max).then_some(number)
}

pub(crate) fn shifted_time(text: &str, months: i32, seconds: i64) -> Option<String> {
    let (local, _) = time(text)?;
    let date = shifted_day(local.div_euclid(86400), months, 0)?;
    let local = date
        .checked_mul(86400)?
        .checked_add(local.rem_euclid(86400))?
        .checked_add(seconds)?;
    let day = render_day(local.div_euclid(86400))?;
    let time = local.rem_euclid(86400);
    let offset = text[11..]
        .find(['Z', '+', '-'])
        .map_or("", |at| &text[11 + at..]);
    Some(format!(
        "{day}T{:02}:{:02}:{:02}{offset}",
        time / 3600,
        time / 60 % 60,
        time % 60
    ))
}

pub(crate) fn period(date: &str, unit: &str) -> Option<String> {
    let (year, month, _) = date_parts(date)?;
    match unit {
        "year" => Some(format!("{year:04}")),
        "month" => Some(format!("{year:04}-{month:02}")),
        "week" => {
            let day = day(date)?;
            let thursday = day + 3 - (day + 3).rem_euclid(7);
            let (year, _, _) = civil(thursday);
            let week = (thursday - day_number(year, 1, 1)) / 7 + 1;
            Some(format!("{year:04}-W{week:02}"))
        }
        _ => None,
    }
}

pub(crate) fn duration(text: &str) -> Option<Value> {
    if let Some(iso) = text.strip_prefix('P') {
        return iso_duration(iso);
    }
    let end = text.find(|ch: char| !(ch.is_ascii_digit() || ch == '-'))?;
    duration_part(text[..end].parse().ok()?, &text[end..])
}

fn duration_part(amount: i64, unit: &str) -> Option<Value> {
    let mut months = 0;
    let mut seconds = 0;
    match unit {
        "y" => months = i32::try_from(amount.checked_mul(12)?).ok()?,
        "mo" => months = i32::try_from(amount).ok()?,
        "w" => seconds = amount.checked_mul(604800)?,
        "d" => seconds = amount.checked_mul(86400)?,
        "h" => seconds = amount.checked_mul(3600)?,
        "min" => seconds = amount.checked_mul(60)?,
        "s" => seconds = amount,
        _ => return None,
    }
    Some(Value::Duration { months, seconds })
}

fn iso_duration(text: &str) -> Option<Value> {
    let mut time = false;
    let mut amount = "";
    let mut months = 0_i32;
    let mut seconds = 0_i64;
    let mut start = 0;
    let mut count = 0;
    for (at, ch) in text.char_indices() {
        if ch.is_ascii_digit() {
            continue;
        }
        if ch == 'T' && !time && start == at {
            time = true;
            start = at + 1;
            amount = "T";
            continue;
        }
        let unit = match (time, ch) {
            (false, 'Y') => "y",
            (false, 'M') => "mo",
            (false, 'W') => "w",
            (false, 'D') => "d",
            (true, 'H') => "h",
            (true, 'M') => "min",
            (true, 'S') => "s",
            _ => return None,
        };
        let Value::Duration {
            months: m,
            seconds: s,
        } = duration_part(text[start..at].parse().ok()?, unit)?
        else {
            return None;
        };
        months = months.checked_add(m)?;
        seconds = seconds.checked_add(s)?;
        start = at + 1;
        count += 1;
        amount = "";
    }
    (count > 0 && start == text.len() && amount.is_empty())
        .then_some(Value::Duration { months, seconds })
}
