//! Temporary adapter for the format engine sketch. The host owns filesystem
//! safety and authority. This module owns interpretation of definition text.

use crate::internal;
use calternal_types::{Error, Result};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub(crate) type Fields = Map<String, Value>;

#[derive(Clone)]
pub(crate) struct Plugin {
    pub(crate) id: String,
    pub(crate) reads: Vec<String>,
    pub(crate) kinds: BTreeMap<String, Kind>,
    pub(crate) collections: BTreeMap<String, Collection>,
}

#[derive(Clone)]
pub(crate) struct Kind {
    pub(crate) id: String,
    pub(crate) folder: String,
    pub(crate) file: Option<String>,
    pub(crate) main: Option<String>,
    pub(crate) instructions: String,
    pub(crate) template: String,
    pub(crate) fields: BTreeMap<String, Field>,
    pub(crate) records: BTreeMap<String, Field>,
    pub(crate) state: BTreeMap<String, Field>,
    pub(crate) operations: BTreeMap<String, String>,
    pub(crate) outside: bool,
    pub(crate) filter: Option<String>,
}

#[derive(Clone)]
pub(crate) struct Field {
    specification: String,
    pub(crate) default: Option<Value>,
    pub(crate) computed: Option<String>,
}

#[derive(Clone)]
pub(crate) struct Collection {
    pub(crate) from: String,
    pub(crate) filter: Option<String>,
    pub(crate) sort: Vec<String>,
}

pub(crate) fn name(value: &str) -> bool {
    value.len() <= 64
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn invalid(path: &str, line: usize, reason: &str) -> Error {
    Error::invalid_input(format!("{path}:{line}: {reason}"))
}

fn split(source: &str) -> Option<(&str, &str)> {
    let source = source.strip_prefix("---\n")?;
    if let Some(body) = source.strip_prefix("---\n") {
        return Some(("", body));
    }
    source
        .split_once("\n---\n")
        .or_else(|| source.strip_suffix("\n---").map(|head| (head, "")))
}

struct Definition<'a> {
    keys: BTreeMap<&'a str, &'a str>,
    blocks: BTreeMap<&'a str, BTreeMap<String, String>>,
    body: &'a str,
}

fn definition<'a>(path: &str, source: &'a str) -> Result<Definition<'a>> {
    let (header, body) =
        split(source).ok_or_else(|| invalid(path, 1, "Expected a Markdown definition."))?;
    let mut keys = BTreeMap::new();
    let mut blocks = BTreeMap::<_, BTreeMap<_, _>>::new();
    let mut block = None;
    for (index, line) in header.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let line_number = index + 2;
        if let Some(line) = line.strip_prefix("  ") {
            let parent =
                block.ok_or_else(|| invalid(path, line_number, "Unexpected indentation."))?;
            let (key, value) = line
                .split_once(": ")
                .or_else(|| line.strip_suffix(':').map(|key| (key, "")))
                .unwrap_or((line, ""));
            if blocks
                .entry(parent)
                .or_default()
                .insert(key.to_owned(), value.to_owned())
                .is_some()
            {
                return Err(invalid(path, line_number, "Repeated definition line."));
            }
            continue;
        }
        let (key, value) = line
            .split_once(": ")
            .or_else(|| line.strip_suffix(':').map(|key| (key, "")))
            .ok_or_else(|| invalid(path, line_number, "Expected a definition key."))?;
        if keys.insert(key, value).is_some() {
            return Err(invalid(path, line_number, "Repeated definition key."));
        }
        block = value.is_empty().then_some(key);
    }
    if keys.get("format") != Some(&"1") {
        return Err(invalid(path, 1, "Expected format 1."));
    }
    Ok(Definition { keys, blocks, body })
}

impl Kind {
    pub(crate) fn filename_field(&self) -> Option<&str> {
        match &self.file {
            Some(pattern) => pattern.strip_prefix('{')?.strip_suffix('}'),
            None => self.fields.contains_key("title").then_some("title"),
        }
    }
}

impl Field {
    fn parse(specification: String) -> Self {
        let default = specification
            .split(", ")
            .find_map(|part| part.strip_prefix("default "))
            .map(|raw| match raw {
                "yes" => Value::Bool(true),
                "no" => Value::Bool(false),
                _ => serde_yaml::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_owned())),
            });
        let computed = specification.strip_prefix("= ").map(str::to_owned);
        Self {
            specification,
            default,
            computed,
        }
    }

    pub(crate) fn anchor(&self) -> bool {
        self.specification.split(", ").next() == Some("record anchor")
    }

    pub(crate) fn accepts(&self, value: &Value) -> bool {
        let ty = self.specification.split(", ").next().unwrap_or_default();
        match ty {
            "record anchor" => value.as_str().is_some_and(crate::records::valid_anchor),
            "yes/no" => value.is_boolean(),
            "people" => value
                .as_array()
                .is_some_and(|values| values.iter().all(Value::is_string)),
            "time" => value.as_str().is_some_and(|raw| {
                time::OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
                    .is_ok()
            }),
            "text" | "long text" => value.is_string(),
            "number" => value.is_number(),
            _ if ty.starts_with("links(") => value
                .as_array()
                .is_some_and(|values| values.iter().all(Value::is_string)),
            _ if ty.starts_with("link(") => value.is_string(),
            _ => false,
        }
    }
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
            .filter(|field| field.computed.is_none())
            .ok_or_else(|| Error::invalid_input("Field is undeclared or computed."))?;
        if value.is_null() {
            existing.remove(&key);
        } else if field.accepts(&value) {
            existing.insert(key, value);
        } else {
            return Err(Error::invalid_input("Field value has the wrong type."));
        }
    }
    for (key, field) in schema {
        if new && let Some(default) = &field.default {
            existing.entry(key).or_insert_with(|| default.clone());
        }
        if field
            .specification
            .split(", ")
            .any(|flag| flag == "required")
            && !existing.contains_key(key)
        {
            return Err(Error::invalid_input("A required field is missing."));
        }
    }
    Ok(())
}

pub(crate) fn parse_plugin(files: &BTreeMap<String, Vec<u8>>, official: bool) -> Result<Plugin> {
    let source = |path: &str| -> Result<&str> {
        let bytes = files
            .get(path)
            .ok_or_else(|| invalid(path, 1, "Definition is missing."))?;
        std::str::from_utf8(bytes).map_err(|_| invalid(path, 1, "Definition must be UTF-8."))
    };
    let manifest = definition("plugin.md", source("plugin.md")?)?;
    let id = manifest
        .keys
        .get("plugin")
        .copied()
        .filter(|id| name(id))
        .ok_or_else(|| invalid("plugin.md", 1, "Invalid plugin ID."))?;
    if matches!(
        id,
        "space" | "home" | "files" | "lines" | "calternal" | "outside"
    ) || (id.starts_with("calternal-") && !official)
    {
        return Err(Error::forbidden("That plugin ID is reserved."));
    }
    if manifest.keys.keys().any(|key| {
        !matches!(
            *key,
            "plugin"
                | "name"
                | "icon"
                | "version"
                | "format"
                | "sends out"
                | "slots"
                | "fills"
                | "reads"
        )
    }) || manifest
        .keys
        .get("sends out")
        .is_some_and(|value| *value != "nothing")
    {
        return Err(Error::forbidden(
            "This host slice cannot approve the plugin's declarations.",
        ));
    }
    let reads = manifest
        .keys
        .get("reads")
        .into_iter()
        .flat_map(|value| value.split(", "))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect();
    let mut plugin = Plugin {
        id: id.to_owned(),
        reads,
        kinds: BTreeMap::new(),
        collections: BTreeMap::new(),
    };
    for path in files.keys() {
        if let Some(key) = entry(path, "kinds/") {
            let mut def = definition(path, source(path)?)?;
            if def.keys.keys().any(|key| {
                !matches!(
                    *key,
                    "kind"
                        | "name"
                        | "format"
                        | "folder"
                        | "file"
                        | "item"
                        | "editable"
                        | "template"
                        | "fields"
                        | "records"
                        | "state"
                        | "row"
                        | "page"
                        | "operations"
                        | "default where"
                )
            }) {
                return Err(invalid(
                    path,
                    1,
                    "Kind key is outside the temporary format adapter.",
                ));
            }
            if def.keys.get("kind") != Some(&key) || !name(key) {
                return Err(invalid(path, 1, "Kind ID must match its filename."));
            }
            let folder = def
                .keys
                .get("folder")
                .filter(|v| !v.is_empty())
                .ok_or_else(|| invalid(path, 1, "A folder is required."))?
                .to_string();
            if calternal_fsio::beneath::plain_parts(&folder).is_err()
                || folder.split('/').any(|part| {
                    part.starts_with('.')
                        || matches!(
                            calternal_fsio::names::fold(part).as_str(),
                            "agents.md" | "claude.md" | "gemini.md" | ".cursorrules"
                        )
                        || part.contains(['{', '}', '\\'])
                })
            {
                return Err(invalid(
                    path,
                    1,
                    "Only literal, visible folder paths are supported.",
                ));
            }
            let file = def.keys.get("file").map(|value| value.to_string());
            let main = match def.keys.get("item").copied().unwrap_or("file") {
                "file" => None,
                raw => Some(
                    raw.strip_prefix("folder with ")
                        .filter(|v| {
                            v.ends_with(".md")
                                && !v.starts_with('.')
                                && !v.contains(['/', '\\'])
                                && !matches!(*v, "AGENTS.md" | "CLAUDE.md" | "GEMINI.md")
                        })
                        .ok_or_else(|| invalid(path, 1, "Unsupported item form."))?
                        .to_owned(),
                ),
            };
            let outside = match def.keys.get("editable") {
                Some(&"outside") => true,
                Some(&"operations") => false,
                _ => return Err(invalid(path, 1, "A valid editable mode is required.")),
            };
            let template = def
                .keys
                .get("template")
                .map(|template| source(&format!("templates/{template}.md")))
                .transpose()?
                .unwrap_or("")
                .to_owned();
            let mut fields = |section| {
                def.blocks
                    .remove(section)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(key, spec)| (key, Field::parse(spec)))
                    .collect()
            };
            let fields_map = fields("fields");
            let records = fields("records");
            let state = fields("state");
            plugin.kinds.insert(
                key.to_owned(),
                Kind {
                    id: format!("{id}.{key}"),
                    folder,
                    file,
                    main,
                    outside,
                    template,
                    instructions: def.body.to_owned(),
                    fields: fields_map,
                    records,
                    state,
                    operations: def.blocks.remove("operations").unwrap_or_default(),
                    filter: def.keys.get("default where").map(|value| value.to_string()),
                },
            );
        } else if let Some(key) = entry(path, "collections/") {
            let def = definition(path, source(path)?)?;
            if def.keys.get("collection") != Some(&key) || !name(key) {
                return Err(invalid(path, 1, "Collection ID must match its filename."));
            }
            let from = def
                .keys
                .get("from")
                .ok_or_else(|| invalid(path, 1, "Expected the collection source."))?;
            plugin.collections.insert(
                key.to_owned(),
                Collection {
                    from: if from.contains('.') {
                        from.to_string()
                    } else {
                        format!("{id}.{from}")
                    },
                    filter: def.keys.get("where").map(|value| value.to_string()),
                    sort: def
                        .keys
                        .get("sort")
                        .into_iter()
                        .flat_map(|value| value.split(", "))
                        .map(str::to_owned)
                        .collect(),
                },
            );
        } else if entry(path, "sidebars/").is_some() {
            definition(path, source(path)?)?;
        } else if path != "plugin.md"
            && path != "operations.ts"
            && entry(path, "templates/").is_none()
        {
            return Err(invalid(
                path,
                1,
                "This bundle entry is outside the host slice.",
            ));
        }
    }
    Ok(plugin)
}

fn entry<'a>(path: &'a str, directory: &str) -> Option<&'a str> {
    let name = path.strip_prefix(directory)?.strip_suffix(".md")?;
    (!name.contains('/')).then_some(name)
}

pub(crate) fn item_body(source: &str) -> Result<&str> {
    split(source)
        .map(|(_, body)| body)
        .ok_or_else(|| Error::invalid_input("Item needs YAML frontmatter."))
}

pub(crate) fn parse_item(source: &str) -> Result<(Fields, &str)> {
    let (yaml, body) =
        split(source).ok_or_else(|| Error::invalid_input("Item needs YAML frontmatter."))?;
    let fields = if yaml.is_empty() {
        Fields::new()
    } else {
        serde_yaml::from_str(yaml)
            .map_err(|_| Error::invalid_input("Item frontmatter is invalid."))?
    };
    Ok((fields, body))
}

pub(crate) fn write_item(fields: &Fields, body: &str) -> Result<String> {
    let yaml = if fields.is_empty() {
        String::new()
    } else {
        serde_yaml::to_string(fields).map_err(internal)?
    };
    Ok(format!("---\n{yaml}---\n{body}"))
}
