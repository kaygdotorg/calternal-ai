//! Shared validation for Inbox questions, parked jobs and author tooling.
use calternal_types::Error;
use serde_json::{Value, json};

/// Validate an action-free Inbox question and return its display label.
/// Uses the bounded GenUI compiler and evaluator without I/O. A rejected form
/// includes source diagnostics when the compiler or evaluator reports them.
pub fn label(form: &calternal_core::GenUiView) -> Result<String, Error> {
    calternal_core::validate_genui(form)?;
    let limits = calternal_genui::Budgets::default();
    let compiled = calternal_genui::compile(&form.source, limits);
    let program = compiled
        .program
        .ok_or_else(|| diagnostics(&form.source, compiled.diagnostics))?;
    let evaluation = calternal_genui::evaluate(&program, None, limits);
    if !evaluation.diagnostics.is_empty() {
        return Err(diagnostics(&form.source, evaluation.diagnostics));
    }
    let [root] = evaluation.roots.as_slice() else {
        return Err(Error::invalid_input("question must contain one Form"));
    };
    if root["type"] != "Form" {
        return Err(Error::invalid_input("question must contain one valid Form"));
    }
    let mut pending = vec![root];
    while let Some(value) = pending.pop() {
        match value {
            Value::Array(items) => pending.extend(items),
            Value::Object(fields) => {
                if value["kind"] == "action" {
                    return Err(Error::invalid_input("question form cannot contain actions"));
                }
                pending.extend(fields.values());
            }
            _ => (),
        }
    }
    root["props"]["label"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| Error::invalid_input("question form needs a label"))
}

fn diagnostics(source: &str, errors: Vec<calternal_genui::Diagnostic>) -> Error {
    let rows: Vec<_> = errors
        .into_iter()
        .map(|error| {
            let line = source
                .lines()
                .nth(error.line.saturating_sub(1))
                .unwrap_or("");
            let mut column = 1;
            let mut offset = 0;
            for ch in line.chars() {
                if column >= error.column {
                    break;
                }
                column += ch.len_utf16();
                offset += ch.len_utf8();
            }
            let remainder = &line[offset..];
            let mut chars = remainder.trim_start().chars();
            let got = match chars.next() {
                Some(first) if first.is_alphanumeric() || first == '_' => std::iter::once(first)
                    .chain(chars.take_while(|ch| ch.is_alphanumeric() || *ch == '_'))
                    .take(48)
                    .collect(),
                Some(first) => first.escape_debug().to_string(),
                None => "end of input".into(),
            };
            json!({
                "path": "/wait/source/form/source",
                "line": error.line,
                "column": error.column,
                "expected": error.message.chars().take(256).collect::<String>(),
                "got": got
            })
        })
        .collect();
    Error::invalid_input("question form has GenUI diagnostics")
        .with_details(json!({"diagnostics": rows}))
}
