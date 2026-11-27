//! Source-preserving rendering for item field overlays.

use super::check_key;
use crate::{Diagnostic, Value, lines::Site};

pub(in crate::item) fn quoted(text: &str) -> String {
    let mut output = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\r' => output.push_str("\\r"),
            '\n' => output.push_str("\\n"),
            '\t' => output.push_str("\\t"),
            control if control.is_control() => {
                output.push_str(&format!("\\u{:04x}", control as u32))
            }
            _ => output.push(ch),
        }
    }
    output.push('"');
    output
}

pub(in crate::item) fn render_entry(
    key: &str,
    value: &Value,
    comment: Option<&str>,
    ending: &str,
) -> Result<String, Diagnostic> {
    let mut output = String::new();
    render(key, value, comment, ending, 0, &mut output)?;
    Ok(output)
}

fn render(
    key: &str,
    value: &Value,
    comment: Option<&str>,
    ending: &str,
    depth: usize,
    result: &mut String,
) -> Result<(), Diagnostic> {
    if depth >= 64 {
        return Err(Site::START.error("Keep frontmatter nesting within 64 levels"));
    }
    result.extend(std::iter::repeat_n(' ', depth * 2));
    result.push_str(key);
    let scalar = match value {
        Value::Map(fields) if !fields.is_empty() => None,
        Value::Map(_) => {
            return Err(Site::START.error("An empty map has no supported item representation"));
        }
        Value::List(values) if !values.is_empty() => None,
        Value::List(_) => Some("[]".into()),
        Value::Empty => Some("null".into()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Money { amount, currency } => Some(quoted(&format!("{amount} {currency}"))),
        Value::Text(value) => Some(quoted(value)),
        Value::Date(value) if crate::dates::day(value).is_some() => Some(value.clone()),
        Value::Time(value) if crate::dates::time(value).is_some() => Some(value.clone()),
        Value::Date(_) | Value::Time(_) => {
            return Err(Site::START.error("Use a valid civil date or time"));
        }
        Value::Duration { months: 0, seconds } => Some(quoted(&format!("{seconds}s"))),
        Value::Duration { months, seconds: 0 } => Some(quoted(&format!("{months}mo"))),
        Value::Duration { months, seconds } if *months >= 0 && *seconds >= 0 => {
            Some(quoted(&format!("P{months}MT{seconds}S")))
        }
        Value::Duration { .. } => {
            return Err(Site::START.error("This mixed-sign duration cannot be written to an item"));
        }
    };
    if let Some(scalar) = scalar {
        result.push(' ');
        result.push_str(&scalar);
    }
    if let Some(comment) = comment {
        result.push(' ');
        result.push_str(comment);
    }
    result.push_str(ending);
    match value {
        Value::Map(fields) => {
            for (key, value) in fields {
                check_key(key, Site::START)?;
                render(
                    &format!("{}:", quoted(key)),
                    value,
                    None,
                    ending,
                    depth + 1,
                    result,
                )?;
            }
        }
        Value::List(values) => {
            for value in values {
                render("-", value, None, ending, depth + 1, result)?;
            }
        }
        _ => {}
    }
    Ok(())
}
