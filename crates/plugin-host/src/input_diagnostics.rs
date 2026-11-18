//! Convert schema failures into diagnostics that never echo submitted values.
use calternal_types::Error;
use jsonschema::error::{TypeKind, ValidationError, ValidationErrorKind};
use serde_json::Value;

pub(crate) fn invalid_input(error: &ValidationError<'_>, schema: &Value) -> Error {
    let mut path = error.instance_path().to_string();
    let child = |path: &mut String, key: &str| {
        path.push('/');
        for ch in key.chars().take(160) {
            match ch {
                '~' => path.push_str("~0"),
                '/' => path.push_str("~1"),
                ch => path.push(ch),
            }
        }
    };
    let expectation = match error.kind() {
        ValidationErrorKind::Type {
            kind: TypeKind::Single(kind),
        } => format!("expected {kind}"),
        ValidationErrorKind::Type {
            kind: TypeKind::Multiple(kinds),
        } => {
            format!(
                "expected {}",
                kinds
                    .iter()
                    .map(|kind| kind.to_string())
                    .collect::<Vec<_>>()
                    .join(" or ")
            )
        }
        ValidationErrorKind::Required { property } => {
            let key = property.as_str().unwrap_or_default();
            child(&mut path, key);
            let schema_path = error.schema_path().to_string();
            let types = schema_path
                .strip_suffix("/required")
                .and_then(|parent| schema.pointer(parent))
                .and_then(|object| object.get("properties")?.get(key)?.get("type"));
            expected_required(types)
        }
        ValidationErrorKind::AdditionalProperties { unexpected } => {
            if let Some(key) = unexpected.first() {
                child(&mut path, key);
            }
            "field is not accepted by the operation schema".to_owned()
        }
        rule => format!(
            "must satisfy the operation schema’s {} rule",
            rule.keyword()
        ),
    };
    Error::input_field(&path, &expectation)
}

fn expected_required(types: Option<&Value>) -> String {
    // A schema may itself supply arbitrary strings. Only the closed JSON Schema
    // type vocabulary belongs in a caller-visible diagnostic.
    let allowed = [
        "array", "boolean", "integer", "null", "number", "object", "string",
    ];
    let expected: Vec<_> = match types {
        Some(Value::Array(types)) => types.iter().filter_map(Value::as_str).collect(),
        Some(Value::String(kind)) => vec![kind.as_str()],
        _ => Vec::new(),
    };
    let expected: Vec<_> = expected
        .into_iter()
        .filter(|kind| allowed.contains(kind))
        .take(7)
        .collect();
    if expected.is_empty() {
        "required field is missing".into()
    } else {
        format!(
            "required field is missing, expected {}",
            expected.join(" or ")
        )
    }
}

/// Point into declared output fields without reflecting worker-supplied keys.
/// Cost: one bounded diagnostic allocation on validation failure.
pub(crate) fn output_path(error: &ValidationError<'_>) -> String {
    let schema_path = error.schema_path().to_string();
    let path = if let ValidationErrorKind::Required { property } = error.kind()
        && let Some(parent) = schema_path.strip_suffix("/required")
        && let Some(key) = property.as_str()
    {
        format!(
            "{parent}/properties/{}",
            key.replace('~', "~0").replace('/', "~1")
        )
    } else {
        schema_path
    };
    if path.is_empty() {
        return "/".into();
    }
    path.chars()
        .take(160)
        .flat_map(char::escape_debug)
        .collect()
}
