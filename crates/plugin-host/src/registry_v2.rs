//! Offline validation for the versioned v2 declaration and its descriptions.
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, OnceLock},
};

use serde::{Deserialize, Deserializer, de};
use serde_json::Value;
mod create;
mod interfaces;
pub use interfaces::ContributionInterface;

const MANIFEST: &str = include_str!("../../../contract/plugin-manifest-v2.schema.json");
const V1: &str = include_str!("../../../contract/plugin-manifest-v1.schema.json");
const DESCRIPTION: &str = include_str!("../../../contract/description-v2.schema.json");
const CATALOGUE: &str = include_str!("../../../contract/host-catalogue-v2.json");
const CONTRACT: &str = "https://calternal.cloud/contract/";

/// Validated installer declaration. Its fields are data, not runtime permission.
#[derive(Clone, Debug)]
pub struct ManifestV2(Value, Arc<BTreeMap<String, jsonschema::Validator>>);

impl ManifestV2 {
    /// Parse bounded UTF-8 JSON with unique keys, the pinned schema and local
    /// reference checks. Does not read package assets or contact the network.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > crate::MAX_MANIFEST_BYTES {
            return Err("manifest.json exceeds 256 KiB".into());
        }
        let mut manifest = strict_json(bytes).map_err(|e| format!("manifest.json: {e}"))?;
        unsupported(&manifest)?;
        static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
        let validator = VALIDATOR.get_or_init(|| {
            validator(&contract_json(MANIFEST)).expect("pinned v2 manifest schema compiles")
        });
        validate(validator, &manifest, "v2 manifest")?;
        check_display_labels(&manifest)?;
        interfaces::check_declarations(&mut manifest)?;
        let metadata = check_declarations(&manifest)?;
        Ok(Self(manifest, Arc::new(metadata)))
    }

    /// Package ID from the checked manifest. Cost: one field lookup.
    pub fn id(&self) -> &str {
        str_field(&self.0, "id")
    }

    /// The checked declaration in the pinned schema's shape. Cost: O(1).
    pub fn declaration(&self) -> &Value {
        &self.0
    }

    /// Whether the package requests this kind as a Bind source. No viewer
    /// authorization is implied. Cost: at most 64 string comparisons.
    pub fn reads(&self, kind: &str) -> bool {
        entries(&self.0, "reads").any(|entry| entry == kind)
    }

    /// Validate an owned item's complete metadata with its cached install
    /// schema. Refuses unknown kinds. No package I/O or schema compilation.
    pub fn check_item_metadata(&self, kind: &str, value: &Value) -> Result<(), String> {
        let schema = self
            .1
            .get(kind)
            .ok_or_else(|| "undeclared item kind".to_owned())?;
        validate(schema, value, "item metadata")
    }

    /// Validate a broker item operand against the pinned v2 catalogue. The
    /// process compiles the bounded set of exchange validators once.
    pub fn check_item_operand(method: &str, value: &Value) -> Result<(), String> {
        static OPERANDS: OnceLock<BTreeMap<String, jsonschema::Validator>> = OnceLock::new();
        let validators = OPERANDS.get_or_init(|| {
            let root = catalogue();
            let rules = root["item_engine"].as_object().expect("pinned item engine");
            rules
                .iter()
                .chain(std::iter::once((
                    &"item_append".to_owned(),
                    &root["file_append"],
                )))
                .filter_map(|(method, rule)| {
                    let reference = rule["input"]["$ref"].as_str()?;
                    Some((
                        method.clone(),
                        validator(&serde_json::json!({
                            "$ref": format!("{CONTRACT}host-catalogue-v2.json{reference}")
                        }))
                        .expect("pinned item operand schema"),
                    ))
                })
                .collect()
        });
        let schema = validators
            .get(method)
            .ok_or_else(|| "unknown item method".to_owned())?;
        validate(schema, value, "item operand")
    }

    /// Check a pinned template against the companion description schema and
    /// the package's declared Bind reads. Cost: one bounded JSON validation.
    pub fn check_description(&self, bytes: &[u8]) -> Result<(), String> {
        let description = strict_json(bytes).map_err(|e| format!("app description: {e}"))?;
        static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
        let validator = VALIDATOR.get_or_init(|| {
            validator(&contract_json(DESCRIPTION)).expect("pinned v2 description schema compiles")
        });
        validate(validator, &description, "app description")?;
        for bind in entries(&description, "binds") {
            if let Some(kind) = bind["source"]["kind"].as_str()
                && !self.reads(kind)
            {
                return Err(format!(
                    "Bind {} reads undeclared kind {kind}",
                    str_field(bind, "id")
                ));
            }
        }
        check_template_operations(&description, &self.0)?;
        Ok(())
    }

    /// The pinned host kind declarations, shared by installers validating
    /// foreign Bind sources. Cost: one lookup in a process-local constant.
    pub fn host_owns_kind(kind: &str) -> bool {
        catalogue()["kinds"].get(kind).is_some()
    }
}

fn catalogue() -> &'static Value {
    static CATALOGUE_VALUE: OnceLock<Value> = OnceLock::new();
    CATALOGUE_VALUE.get_or_init(|| contract_json(CATALOGUE))
}

fn contract_json(bytes: &str) -> Value {
    serde_json::from_str(bytes).expect("pinned contract is JSON")
}

fn validator(schema: &Value) -> Result<jsonschema::Validator, String> {
    static REGISTRY: OnceLock<jsonschema::Registry<'static>> = OnceLock::new();
    let registry = REGISTRY.get_or_init(|| {
        jsonschema::Registry::new()
            .add(
                format!("{CONTRACT}plugin-manifest-v1.schema.json"),
                contract_json(V1),
            )
            .expect("pinned v1 schema URI")
            .add(
                format!("{CONTRACT}host-catalogue-v2.json"),
                catalogue().clone(),
            )
            .expect("pinned catalogue URI")
            .add(
                format!("{CONTRACT}description-v2.schema.json"),
                contract_json(DESCRIPTION),
            )
            .expect("pinned description schema URI")
            .prepare()
            .expect("pinned schemas resolve offline")
    });
    let options = jsonschema::options()
        .with_registry(registry)
        .with_base_uri(format!("{CONTRACT}__embedded__.json"));
    let options = if schema.get("$schema").is_none() {
        options.with_draft(jsonschema::Draft::Draft7)
    } else {
        options
    };
    options
        .build(schema)
        .map_err(|_| "schema is invalid or uses an unregistered reference".into())
}

fn embedded_schema(schema: &Value) -> Result<jsonschema::Validator, String> {
    fn draft_07(value: &Value) -> bool {
        value == "http://json-schema.org/draft-07/schema#"
            || value == "http://json-schema.org/draft-07/schema"
    }

    fn mixed_drafts(value: &Value) -> bool {
        match value {
            Value::Object(fields) => {
                // A property named $schema may itself be a schema object.
                // Only URI strings can choose a draft in a nested resource.
                fields
                    .get("$schema")
                    .is_some_and(|uri| uri.is_string() && !draft_07(uri))
                    || fields.values().any(mixed_drafts)
            }
            Value::Array(items) => items.iter().any(mixed_drafts),
            _ => false,
        }
    }

    if schema.get("$schema").is_some_and(|draft| !draft_07(draft)) || mixed_drafts(schema) {
        return Err("Embedded schemas must use JSON Schema draft-07".into());
    }
    // The registry's URI resolver classifies fragments before it decodes
    // their pointer. Canonicalize an encoded leading slash, preserving the
    // rest of the URI and the exact signed manifest.
    fn normalize_fragment(value: &mut Value) {
        match value {
            Value::Object(fields) => {
                if let Some(reference) = fields.get_mut("$ref")
                    && let Some((document, fragment)) =
                        reference.as_str().and_then(|r| r.split_once('#'))
                    && let Some(rest) = fragment
                        .strip_prefix("%2F")
                        .or_else(|| fragment.strip_prefix("%2f"))
                {
                    *reference = Value::String(format!("{document}#/{rest}"));
                }
                for (key, child) in fields {
                    if !matches!(key.as_str(), "default" | "const" | "enum" | "examples") {
                        normalize_fragment(child);
                    }
                }
            }
            Value::Array(items) => items.iter_mut().for_each(normalize_fragment),
            _ => {}
        }
    }

    let mut normalized = schema.clone();
    normalize_fragment(&mut normalized);
    validator(&normalized)
}

fn check_display_labels(value: &Value) -> Result<(), String> {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if key == "schema" || key.ends_with("_schema") || key == "default" {
                    continue;
                }
                let valid = |text: &str| {
                    !text.chars().any(|ch| {
                        ch.is_control()
                            || matches!(ch, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
                    })
                };
                if matches!(
                    key.as_str(),
                    "label" | "name" | "title" | "purpose" | "description"
                ) && value.as_str().is_some_and(|text| !valid(text))
                    || key == "material_inputs"
                        && value.as_object().is_some_and(|fields| {
                            fields
                                .values()
                                .filter_map(Value::as_str)
                                .any(|text| !valid(text))
                        })
                {
                    return Err("Manifest labels, names and purposes must omit control and bidirectional override characters".into());
                }
                check_display_labels(value)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                check_display_labels(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_template_operations(value: &Value, manifest: &Value) -> Result<(), String> {
    match value {
        Value::Object(fields) => {
            if let Some(operation) = value["operation"].as_str() {
                let host = &catalogue()["operations"][operation];
                if !entries(manifest, "operations").any(|entry| entry["id"] == operation)
                    && !(host["generic_invoke"] == true && host["available"] != false)
                {
                    return Err(format!(
                        "Template action invokes undeclared or unavailable operation {operation}"
                    ));
                }
            }
            for (key, child) in fields {
                // Inputs may contain arbitrary literal operation-shaped data.
                if key != "inputs" {
                    check_template_operations(child, manifest)?;
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                check_template_operations(item, manifest)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate(validator: &jsonschema::Validator, value: &Value, label: &str) -> Result<(), String> {
    validator.validate(value).map_err(|error| {
        let path = error.instance_path().to_string();
        format!(
            "{label} does not match the pinned schema at {}",
            if path.is_empty() { "/" } else { &path }
        )
    })
}

fn entries<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value[key].as_array().into_iter().flatten()
}

fn str_field<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field].as_str().unwrap_or_default()
}

fn unsupported(manifest: &Value) -> Result<(), String> {
    if manifest.get("scratch").is_some()
        || entries(manifest, "kinds").any(|kind| kind.get("scratch").is_some())
    {
        return Err(
            "Private scratch disks are unavailable for v2 packages in this release.".into(),
        );
    }
    if entries(manifest, "kinds").any(|kind| {
        kind.get("conversation").is_some()
            || entries(kind, "capabilities").any(|capability| capability == "conversation")
            || kind["capabilities"].get("conversation").is_some()
    }) {
        return Err("Conversation kinds are unavailable for v2 packages until revision 14.".into());
    }
    if entries(manifest, "jobs").any(|job| job["placement"] == "heavy") {
        return Err("Heavy jobs are unavailable for v2 packages in this release.".into());
    }
    Ok(())
}

fn check_declarations(manifest: &Value) -> Result<BTreeMap<String, jsonschema::Validator>, String> {
    let mut validators = BTreeMap::new();
    semver::Version::parse(str_field(manifest, "version"))
        .map_err(|_| "v2 package version must be a semantic version")?;
    for (list, key) in [
        ("apps", "id"),
        ("kinds", "id"),
        ("operations", "id"),
        ("providers", "id"),
        ("jobs", "id"),
        ("schedule_eligibility", "id"),
        ("wait_webhooks", "id"),
        ("settings", "key"),
        ("secrets", "name"),
        ("credential_routes", "id"),
    ] {
        let mut ids = BTreeSet::new();
        for entry in entries(manifest, list) {
            if !ids.insert(str_field(entry, key)) {
                return Err(format!(
                    "Duplicate v2 declaration in {list}: {}",
                    str_field(entry, key)
                ));
            }
        }
    }
    let owns = |kind: &Value| entries(manifest, "kinds").any(|entry| &entry["id"] == kind);
    let readable = |kind: &Value| entries(manifest, "reads").any(|entry| entry == kind);
    let operation = |id: &Value| entries(manifest, "operations").find(|op| &op["id"] == id);
    for kind in entries(manifest, "kinds") {
        let id = str_field(kind, "id");
        if ManifestV2::host_owns_kind(id) || id == "automations" || id.starts_with("skill-") {
            return Err(format!("Kind {id} is reserved by the host"));
        }
        let storage = serde_json::from_value(kind["storage"].clone())
            .map_err(|_| format!("Kind {id} has an invalid storage declaration"))?;
        if !crate::registry::valid_storage(&storage) {
            return Err(format!(
                "Kind {id} must use a safe, non-reserved storage path"
            ));
        }
        let metadata = embedded_schema(&kind["schema"])?;
        check_fields(kind, id)?;
        create::check(manifest, kind, &metadata)?;
        validators.insert(id.to_owned(), metadata);
    }
    for op in entries(manifest, "operations") {
        let id = str_field(op, "id");
        let (prefix, _) = id.split_once('.').expect("schema checked operation ID");
        if entries(catalogue(), "reserved_operation_prefixes")
            .any(|p| p.as_str().and_then(|p| p.strip_suffix('.')) == Some(prefix))
            || (prefix != str_field(manifest, "id")
                && !entries(manifest, "apps").any(|app| app["id"] == prefix))
        {
            return Err(format!(
                "Operation {id} must use the package or one of its app namespaces"
            ));
        }
        embedded_schema(&op["input_schema"])?;
        embedded_schema(&op["output_schema"])?;
        if let Some(target) = op.get("target")
            && !owns(&target["kind"])
            && !readable(&target["kind"])
        {
            return Err(format!("Operation {id} targets an undeclared kind"));
        }
        if let Some(kind) = op.get("creates")
            && !owns(kind)
        {
            return Err(format!(
                "Operation {id} declares creation of a foreign kind"
            ));
        }
        if entries(op, "credential_routes")
            .any(|route| !entries(manifest, "credential_routes").any(|entry| &entry["id"] == route))
        {
            return Err(format!(
                "Operation {id} references an undeclared credential route"
            ));
        }
    }
    for (list, key) in [
        ("jobs", "run_operation"),
        ("schedule_eligibility", "operation"),
        ("wait_webhooks", "schema_operation"),
    ] {
        if entries(manifest, list).any(|entry| operation(&entry[key]).is_none()) {
            return Err(format!("{list} references an undeclared operation"));
        }
    }
    for setting in entries(manifest, "settings") {
        validate(
            &embedded_schema(&setting["value_schema"])?,
            &setting["default"],
            "Setting default",
        )?;
    }
    for route in entries(manifest, "credential_routes") {
        embedded_schema(&route["input_schema"])?;
        embedded_schema(&route["output_schema"])?;
        if !entries(manifest, "secrets").any(|secret| secret["name"] == route["secret"]) {
            return Err("Credential route refers to an undeclared secret".into());
        }
        if let Some(host) = route["connection"].get("fixed_site")
            && !entries(&manifest["network"], "fixed_sites").any(|site| site == host)
        {
            return Err("Credential route refers to an undeclared network site".into());
        }
        if route["connection"].get("user_server").is_some()
            && manifest["network"]["user_servers"] != true
        {
            return Err("Credential route requires access to servers the person adds".into());
        }
    }
    Ok(validators)
}

fn check_fields(kind: &Value, id: &str) -> Result<(), String> {
    let properties = &kind["schema"]["properties"];
    let title = str_field(kind, "title_field");
    if !properties
        .get(title)
        .is_some_and(|field| declared_type(field, "string"))
    {
        return Err(format!(
            "Kind {id} must declare its title field as a string"
        ));
    }
    for name in catalogue()["generic_item_fields"]
        .as_object()
        .expect("pinned generic fields")
        .keys()
    {
        // The title field is the declared metadata source of the generated title.
        if !(name == "title" && title == name) && properties.get(name).is_some() {
            return Err(format!(
                "Kind {id} uses reserved generic metadata field {name}"
            ));
        }
    }
    if let Some(fields) = kind["indexed_fields"].as_object() {
        for (field, declaration) in fields {
            if catalogue()["generic_item_fields"].get(field).is_some() {
                return Err(format!(
                    "Kind {id} uses reserved generic indexed field {field}"
                ));
            }
            if declaration["source"] == "body_excerpt" || declaration["type"] == "RowImageRef" {
                continue;
            }
            let ty = match str_field(declaration, "type") {
                "Number" => "number",
                "Boolean" => "boolean",
                _ => "string",
            };
            if !properties
                .get(field)
                .is_some_and(|schema| declared_type(schema, ty))
            {
                return Err(format!(
                    "Indexed field {field} does not match kind {id}'s metadata schema"
                ));
            }
        }
    }
    Ok(())
}

fn declared_type(schema: &Value, expected: &str) -> bool {
    let matches = |ty: &Value| ty == expected || (expected == "number" && ty == "integer");
    let ty = &schema["type"];
    matches(ty)
        || ty.as_array().is_some_and(|types| {
            types.iter().any(matches) && types.iter().all(|ty| matches(ty) || ty == "null")
        })
}

// serde_json::Value accepts the last occurrence of a duplicate key. This visitor
// applies the same unique-key rule at every object in an untrusted declaration.
/// Decode a manifest or stored claim source without normalizing duplicate keys.
pub(crate) fn strict_json(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    serde_json::from_slice::<Unique>(bytes).map(|value| value.0)
}

struct Unique(Value);

impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Unique;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("JSON with unique object keys")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<Unique, A::Error> {
                let object =
                    calternal_types::unique_json::collect_unique_members::<_, Unique>(map)?;
                Ok(Unique(Value::Object(
                    object
                        .into_iter()
                        .map(|(key, value)| (key, value.0))
                        .collect(),
                )))
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Unique, A::Error> {
                let mut array = Vec::new();
                while let Some(Unique(value)) = seq.next_element()? {
                    array.push(value);
                }
                Ok(Unique(Value::Array(array)))
            }

            fn visit_bool<E: de::Error>(self, value: bool) -> Result<Unique, E> {
                Ok(Unique(value.into()))
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Unique, E> {
                Ok(Unique(value.into()))
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Unique, E> {
                Ok(Unique(value.into()))
            }

            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Unique, E> {
                Ok(Unique(value.into()))
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Unique, E> {
                Ok(Unique(value.into()))
            }

            fn visit_unit<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[cfg(test)]
mod tests;
