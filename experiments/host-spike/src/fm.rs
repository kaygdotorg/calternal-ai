//! Frontmatter as `key: value` lines, read and edited line by line.
//!
//! Editing replaces only the lines of the keys it sets, so unknown keys,
//! comments and untouched values survive (ADR 0068). Values are written as
//! YAML scalars a person can read: `true`, `12`, `null`, or a quoted string.
use serde_json::Value;

/// Render a new file: frontmatter with `id`, `kind` and the fields, then the body.
pub fn render(id: &str, kind: &str, fields: &[(String, Value)], body: &str) -> String {
    let mut out = format!("---\nid: {id}\nkind: {kind}\n");
    for (k, v) in fields {
        out.push_str(&format!("{k}: {}\n", scalar(v)));
    }
    out.push_str("---\n");
    if !body.is_empty() {
        out.push('\n');
        out.push_str(body.trim_end_matches('\n'));
        out.push('\n');
    }
    out
}

/// A value as one YAML scalar. Strings are always quoted (JSON escaping is valid YAML).
pub fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => Value::String(s.clone()).to_string(),
        Value::Null => "null".into(),
        other => other.to_string(),
    }
}

/// One frontmatter line. `id` and `kind` are host-written identifiers and stay unquoted.
fn line_for(key: &str, v: &Value) -> String {
    match (key, v) {
        ("id" | "kind", Value::String(s)) => format!("{key}: {s}\n"),
        _ => format!("{key}: {}\n", scalar(v)),
    }
}

/// Read a scalar back: `true`/`false`/`null`, a number, a quoted string, else the plain text.
pub fn parse_scalar(raw: &str) -> Value {
    let raw = raw.trim();
    match raw {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        "null" | "~" | "" => return Value::Null,
        _ => {}
    }
    if raw.starts_with('"') {
        if let Ok(v @ Value::String(_)) = serde_json::from_str(raw) {
            return v;
        }
    } else if let Ok(v @ Value::Number(_)) = serde_json::from_str(raw) {
        return v;
    }
    Value::String(raw.trim_matches('\'').to_owned())
}

/// The frontmatter's byte length (through the closing `---\n`), or `None` when
/// the text does not start with a frontmatter block.
pub fn end(text: &str) -> Option<usize> {
    let rest = text.strip_prefix("---\n")?;
    let mut at = 4;
    for line in rest.split_inclusive('\n') {
        at += line.len();
        if line == "---\n" || line == "---" {
            return Some(at);
        }
    }
    None
}

/// The frontmatter's keys and values, in file order. Lines that are not
/// `key: value` (comments, nested YAML) are skipped, not refused.
pub fn fields(text: &str) -> Vec<(String, Value)> {
    let Some(end) = end(text) else {
        return Vec::new();
    };
    text[4..end]
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with(' ') && *l != "---")
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_owned(), parse_scalar(v)))
        .collect()
}

/// Set keys in the frontmatter, replacing their lines or adding them before
/// the closing line. Everything else is kept byte for byte.
pub fn set(text: &str, updates: &[(String, Value)]) -> Option<String> {
    let end = end(text)?;
    let mut done = vec![false; updates.len()];
    let mut out = String::from("---\n");
    let body_lines: Vec<&str> = text[4..end].split_inclusive('\n').collect();
    let (closing, lines) = body_lines.split_last()?;
    for line in lines {
        let key = line.split_once(':').map(|(k, _)| k.trim());
        match updates.iter().position(|(k, _)| Some(k.as_str()) == key) {
            Some(i) if !line.starts_with(' ') && !line.starts_with('#') => {
                out.push_str(&line_for(&updates[i].0, &updates[i].1));
                done[i] = true;
            }
            _ => out.push_str(line),
        }
    }
    for (i, (k, v)) in updates.iter().enumerate() {
        if !done[i] {
            out.push_str(&line_for(k, v));
        }
    }
    out.push_str(closing);
    out.push_str(&text[end..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn set_keeps_everything_it_does_not_touch() {
        let text = "---\nid: x\n# a comment\nread: false\nextra: kept\n---\n\nbody\n";
        let out = set(
            text,
            &[
                ("read".into(), json!(true)),
                ("title".into(), json!("A: b")),
            ],
        )
        .unwrap();
        assert_eq!(
            out,
            "---\nid: x\n# a comment\nread: true\nextra: kept\ntitle: \"A: b\"\n---\n\nbody\n"
        );
        let f = fields(&out);
        assert!(f.contains(&("title".into(), json!("A: b"))));
        assert!(f.contains(&("read".into(), json!(true))));
    }
}
