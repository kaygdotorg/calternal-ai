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
