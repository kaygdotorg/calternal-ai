//! Adapter from verified bundle bytes and JSON values to the pure format engine.
pub(crate) mod browser;

use calternal_format as format;
use calternal_types::{AccountId, Error, Result};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::ops::Deref;

pub(crate) type Fields = Map<String, Value>;
pub(crate) use format::{Collection, Field, FieldType, OperationForm, Plugin};

pub(crate) struct Kind<'a> {
    pub(crate) id: &'a str,
    pub(crate) folder: &'a str,
    pub(crate) main: Option<&'a str>,
    pub(crate) template: &'a str,
    pub(crate) compiled: &'a format::Kind,
}

impl Deref for Kind<'_> {
    type Target = format::Kind;

    fn deref(&self) -> &Self::Target {
        self.compiled
    }
}

impl Kind<'_> {
    pub(crate) fn filename_field(&self) -> Option<&str> {
        self.file.strip_prefix('{')?.strip_suffix('}')
    }

    pub(crate) fn outside(&self) -> bool {
        self.editable == format::Editability::Outside
    }
}

pub(crate) fn invalid(error: format::Diagnostic) -> Error {
    Error::invalid_input(error.to_string())
}

pub(crate) fn view_kind<'a>(id: &'a str, plugin: &'a Plugin, key: &str) -> Option<Kind<'a>> {
    let compiled = plugin.kinds.get(key)?;
    Some(Kind {
        id,
        folder: compiled.folder.as_deref()?,
        main: match &compiled.item {
            format::ItemForm::Folder(main) => Some(main),
            _ => None,
        },
        template: compiled
            .template
            .as_ref()
            .and_then(|template| plugin.templates.get(template))
            .map_or("", String::as_str),
        compiled,
    })
}

pub(crate) fn parse_plugin(
    entries: &BTreeMap<String, Vec<u8>>,
    official: bool,
    expected_id: Option<&str>,
    installed: &[Plugin],
) -> Result<Plugin> {
    let files = entries
        .iter()
        .map(|(path, data)| format::PluginFile {
            path,
            text: path
                .ends_with(".md")
                .then(|| std::str::from_utf8(data).ok())
                .flatten(),
        })
        .collect::<Vec<_>>();
    // The operator-approved local source is the developer path. Extract its
    // candidate pair for the engine policy, which validates the manifest once.
    let current = files
        .iter()
        .find(|file| file.path == "plugin.md")
        .and_then(|file| file.text);
    let header_value = |key: &str| {
        current?
            .split("\n---")
            .next()?
            .lines()
            .find_map(|line| line.strip_prefix(key))
    };
    let mut trusted = installed
        .iter()
        .filter(|plugin| plugin.manifest.id.starts_with("calternal-"))
        .map(|plugin| (plugin.manifest.id.as_str(), plugin.manifest.name.as_str()))
        .collect::<Vec<_>>();
    if official && let (Some(id), Some(name)) = (header_value("plugin: "), header_value("name: ")) {
        trusted.push((id, name));
    }
    let plugin = format::parse_plugin(
        &files,
        &format::InstallOptions {
            expected_id,
            official: &trusted,
            installed,
        },
    )
    .map_err(invalid)?;
    let manifest = &plugin.manifest;
    // Program and destination declarations feed the scope-checked run service;
    // they do not grant a container ambient network or filesystem access.
    if manifest.directives.iter().any(|key| {
        matches!(
            key.key.as_str(),
            "extends"
                | "outside"
                | "notifications"
                | "sign-ins"
                | "secrets"
                | "listens"
                | "refuses"
        )
    }) {
        return Err(Error::forbidden(
            "This host slice cannot grant the plugin's declarations.",
        ));
    }
    for kind in plugin.kinds.values() {
        if !kind.rights.read.is_empty()
            || kind.plus_marked_folders
            || kind.unique.is_some()
            || (!kind.hidden && kind.keep.is_some())
            || matches!(kind.item, format::ItemForm::Binary { .. })
            || (!kind.hidden && kind.folder.as_ref().is_none_or(|name| !plain_folder(name)))
            || matches!(&kind.item, format::ItemForm::Folder(main) if reserved_name(main) || main.contains('/'))
            || (kind.file.contains(['{', '}'])
                && kind
                    .file
                    .strip_prefix('{')
                    .and_then(|s| s.strip_suffix('}'))
                    .is_none_or(|name| name.contains(['{', '}', ':'])))
        {
            return Err(Error::forbidden(
                "The kind declares storage or authority outside this host slice.",
            ));
        }
    }
    Ok(plugin)
}

fn plain_folder(name: &str) -> bool {
    calternal_fsio::beneath::plain_parts(name).is_ok()
        && name.split('/').all(|part| {
            !part.starts_with('.') && !part.contains(['{', '}']) && !reserved_name(part)
        })
}

fn reserved_name(value: &str) -> bool {
    matches!(
        calternal_fsio::names::fold(value).as_str(),
        "agents.md" | "claude.md" | "gemini.md" | ".cursorrules"
    )
}

pub(crate) fn apply_fields(
    schema: &BTreeMap<String, Field>,
    existing: &mut Fields,
    fields: Fields,
    new: bool,
) -> Result<()> {
    for (key, value) in fields {
        let field = schema
            .get(&key)
            .filter(|field| !field.from_metadata && field.sidecar.is_none())
            .and_then(|field| field.field_type.as_ref())
            .ok_or_else(|| Error::invalid_input("Field is undeclared or read-only."))?;
        if value.is_null() {
            existing.remove(&key);
        } else if accepts(field, &value) {
            existing.insert(key, value);
        } else {
            return Err(Error::invalid_input("Field value has the wrong type."));
        }
    }
    for (key, field) in schema {
        if new
            && !existing.contains_key(key)
            && let Some(default) = field_default(field)?
        {
            existing.insert(key.clone(), default);
        }
        if field.required && !existing.contains_key(key) {
            return Err(Error::invalid_input("A required field is missing."));
        }
    }
    Ok(())
}

pub(crate) fn accepts(field: &FieldType, value: &Value) -> bool {
    match field {
        FieldType::Bool => value.is_boolean(),
        FieldType::Text => value.as_str().is_some_and(|v| !v.contains(['\r', '\n'])),
        FieldType::LongText | FieldType::Link(_) => value.is_string(),
        FieldType::Number(units) => units.is_empty() && value.is_number(),
        FieldType::Person => value
            .as_str()
            .is_some_and(|id| AccountId::parse(id).is_ok()),
        FieldType::Time => value.as_str().is_some_and(|raw| {
            time::OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339).is_ok()
        }),
        FieldType::Anchor => value.as_str().is_some_and(crate::records::valid_anchor),
        FieldType::Choice(options) => value
            .as_str()
            .is_some_and(|selected| options.iter().any(|option| option == selected)),
        FieldType::List(item) => value
            .as_array()
            .is_some_and(|items| items.iter().all(|value| accepts(item, value))),
        _ => false,
    }
}

pub(crate) fn field_default(field: &Field) -> Result<Option<Value>> {
    let Some(default) = &field.default else {
        return Ok(None);
    };
    let empty = format::Fields::new();
    let (today, now) = clock()?;
    let context = format::EvalContext {
        today: Some(&today),
        now: Some(&now),
        offset_seconds: Some(0),
        ..format::EvalContext::default()
    };
    let result = format::evaluate(
        default,
        format::Row {
            fields: &empty,
            metadata: &empty,
        },
        &context,
    )
    .map_err(invalid)?;
    Ok(Some(as_json(&result)))
}

pub(crate) fn clock() -> Result<(String, String)> {
    let now = time::OffsetDateTime::now_utc();
    let today = now.date().to_string();
    let now = now
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(crate::internal)?;
    Ok((today, now))
}

pub(crate) fn as_fields(fields: &Fields) -> Result<format::Fields> {
    fields
        .iter()
        .map(|(key, value)| Ok((key.clone(), as_format(value)?)))
        .collect()
}

pub(crate) fn as_format(value: &Value) -> Result<format::Value> {
    use format::Value as F;
    Ok(match value {
        Value::Null => F::Empty,
        Value::Bool(value) => F::Bool(*value),
        Value::Number(value) => F::Number(
            value
                .to_string()
                .parse()
                .map_err(|_| Error::invalid_input("Number is outside the format decimal range."))?,
        ),
        Value::String(value) => F::Text(value.clone()),
        Value::Array(items) => F::List(items.iter().map(as_format).collect::<Result<_>>()?),
        Value::Object(values) => F::Map(as_fields(values)?),
    })
}

pub(crate) fn as_json(value: &format::Value) -> Value {
    use format::Value as F;
    match value {
        F::Empty => Value::Null,
        F::Bool(value) => Value::Bool(*value),
        F::Number(value) => serde_json::from_str(&value.to_string())
            .unwrap_or_else(|_| Value::String(value.to_string())),
        F::Text(value) | F::Date(value) | F::Time(value) => Value::String(value.clone()),
        F::List(items) => Value::Array(items.iter().map(as_json).collect()),
        F::Map(items) => Value::Object(
            items
                .iter()
                .map(|(key, value)| (key.clone(), as_json(value)))
                .collect(),
        ),
        F::Money { amount, currency } => Value::String(format!("{amount} {currency}")),
        F::Duration { months, seconds } => serde_json::json!({"months":months,"seconds":seconds}),
    }
}

pub(crate) fn parse_item(source: &str) -> Result<(Fields, &str)> {
    let item = format::parse_item(source).map_err(invalid)?;
    let fields = item
        .fields()
        .iter()
        .filter(|(name, _)| {
            !matches!(
                name.as_str(),
                "id" | "kind" | "created" | "edited" | "at" | "author" | "anchor"
            )
        })
        .map(|(name, value)| (name.clone(), as_json(value)))
        .collect();
    Ok((fields, item.body()))
}

pub(crate) fn item_body(source: &str) -> Result<&str> {
    // The indexed prefix has already been parsed and authenticated.
    if let Some(rest) = source.strip_prefix("---\n") {
        return Ok(rest
            .strip_prefix("---\n")
            .or_else(|| rest.split_once("\n---\n").map(|(_, body)| body))
            .unwrap_or(""));
    }
    Ok(format::parse_item(source).map_err(invalid)?.body())
}

pub(crate) fn write_item(
    source: &str,
    patch: &Fields,
    body: Option<&str>,
    kind: &Kind<'_>,
) -> Result<String> {
    let original = format::parse_item(source).map_err(invalid)?;
    let mut markdown = format::write_item(&original, &as_fields(patch)?).map_err(invalid)?;
    if let Some(body) = body {
        markdown.truncate(markdown.len() - original.body().len());
        markdown.push_str(body);
    }
    if !markdown.ends_with('\n') {
        markdown.push('\n');
    }
    let updated = format::parse_item(&markdown).map_err(invalid)?;
    let (today, now) = clock()?;
    format::check_item(
        kind.compiled,
        &updated,
        &format::EvalContext {
            today: Some(&today),
            now: Some(&now),
            offset_seconds: Some(0),
            ..format::EvalContext::default()
        },
    )
    .map_err(invalid)?;
    Ok(markdown)
}

#[cfg(test)]
mod tests {
    #[test]
    fn exact_json_decimals_survive_the_format_boundary() {
        let raw = "0.100000000000000001";
        let value = serde_json::from_str(raw).unwrap();
        let typed = super::as_format(&value).unwrap();
        assert_eq!(super::as_json(&typed).to_string(), raw);
    }
}
