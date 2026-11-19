//! Offline validation for the versioned v2 declaration and its descriptions.
use std::{collections::BTreeSet, sync::OnceLock};

use serde::{Deserialize, Deserializer, de};
use serde_json::{Map, Value};

const MANIFEST: &str = include_str!("../../../contract/plugin-manifest-v2.schema.json");
const V1: &str = include_str!("../../../contract/plugin-manifest-v1.schema.json");
const DESCRIPTION: &str = include_str!("../../../contract/description-v2.schema.json");
const CATALOGUE: &str = include_str!("../../../contract/host-catalogue-v2.json");
const CONTRACT: &str = "https://calternal.cloud/contract/";

/// Validated installer declaration. Its fields are data, not runtime permission.
#[derive(Clone, Debug)]
pub struct ManifestV2(Value);

impl ManifestV2 {
    /// Parse bounded UTF-8 JSON with unique keys, the pinned schema and local
    /// reference checks. Does not read package assets or contact the network.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > crate::MAX_MANIFEST_BYTES {
            return Err("manifest.json exceeds 256 KiB".into());
        }
        let manifest = strict_json(bytes).map_err(|e| format!("manifest.json: {e}"))?;
        unsupported(&manifest)?;
        static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
        let validator = VALIDATOR.get_or_init(|| {
            validator(&contract_json(MANIFEST)).expect("pinned v2 manifest schema compiles")
        });
        validate(validator, &manifest, "v2 manifest")?;
        check_declarations(&manifest)?;
        Ok(Self(manifest))
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
            .prepare()
            .expect("pinned schemas resolve offline")
    });
    let options = jsonschema::options().with_registry(registry);
    let options = if schema.get("$schema").is_none() {
        options.with_draft(jsonschema::Draft::Draft7)
    } else {
        options
    };
    options
        .build(schema)
        .map_err(|_| "schema is invalid or uses an unregistered reference".into())
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

fn check_declarations(manifest: &Value) -> Result<(), String> {
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
        let metadata = validator(&kind["schema"])?;
        check_fields(kind, id)?;
        if let Some(create) = kind.get("host_create") {
            validate(&metadata, &create["defaults"], "host_create defaults")?;
            if let Some(after) = create.get("after") {
                let op_id = if let Some(job_id) = after.get("job") {
                    entries(manifest, "jobs")
                        .find(|job| &job["id"] == job_id)
                        .map(|job| &job["run_operation"])
                        .ok_or("host_create refers to an undeclared job")?
                } else {
                    &after["operation"]
                };
                let op = operation(op_id).ok_or("host_create refers to an undeclared operation")?;
                let field = str_field(&op["target"], "field");
                let input = &op["input_schema"];
                if field.is_empty()
                    || op["target"]["kind"] != id
                    || input["type"] != "object"
                    || input["additionalProperties"] != false
                    || input["required"] != serde_json::json!([field])
                    || !input["properties"]
                        .as_object()
                        .is_some_and(|props| props.len() == 1)
                    || !matches!(
                        input["properties"][field]["$ref"].as_str(),
                        Some(
                            "https://calternal.cloud/contract/host-catalogue-v2.json#/$defs/ItemRef"
                        )
                    )
                {
                    return Err(
                        "host_create follow-up must accept only the created kind's ItemRef".into(),
                    );
                }
            }
        }
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
        validator(&op["input_schema"])?;
        validator(&op["output_schema"])?;
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
            &validator(&setting["value_schema"])?,
            &setting["default"],
            "Setting default",
        )?;
    }
    for route in entries(manifest, "credential_routes") {
        validator(&route["input_schema"])?;
        validator(&route["output_schema"])?;
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
    Ok(())
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
    for name in ["id", "updated", "trashed", "trashed_at"] {
        if properties.get(name).is_some() {
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
fn strict_json(bytes: &[u8]) -> Result<Value, serde_json::Error> {
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

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Unique, A::Error> {
                let mut object = Map::new();
                while let Some((key, Unique(value))) = map.next_entry::<String, Unique>()? {
                    if object.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate object key"));
                    }
                }
                Ok(Unique(Value::Object(object)))
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
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!(
        "../../../contract/fixtures/description-v2/manifests/reading-list-manifest-v2-app.json"
    );
    const TEMPLATE: &[u8] = include_bytes!(
        "../../../contract/fixtures/description-v2/reading-list-manifest-v2-app.json"
    );

    #[test]
    fn installer_reads_sdk_v2_reading_list() {
        let mut manifest = strict_json(FIXTURE).unwrap();
        manifest["id"] = "calternal-reading-list".into();
        let installed = ManifestV2::parse(&serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert_eq!(installed.id(), "calternal-reading-list");
        assert!(installed.reads("reading-item"));
        installed.check_description(TEMPLATE).unwrap();
    }

    #[test]
    fn installer_checks_host_create_defaults_and_indexed_field_types() {
        let mut fixture = strict_json(FIXTURE).unwrap();
        let kind = &mut fixture["kinds"][0];
        kind["host_create"] = serde_json::json!({"defaults":{"title":"New item","read":false}});
        let installed = ManifestV2::parse(&serde_json::to_vec(&fixture).unwrap()).unwrap();
        assert_eq!(
            installed.declaration()["kinds"][0]["host_create"]["defaults"]["read"],
            false
        );
        fixture["kinds"][0]["host_create"]["defaults"]["read"] = "false".into();
        assert!(
            ManifestV2::parse(&serde_json::to_vec(&fixture).unwrap())
                .unwrap_err()
                .contains("host_create defaults")
        );
        fixture["kinds"][0]["indexed_fields"]["read"]["type"] = "Text".into();
        assert!(
            ManifestV2::parse(&serde_json::to_vec(&fixture).unwrap())
                .unwrap_err()
                .contains("Indexed field read")
        );
    }

    #[test]
    fn installer_refuses_malformed_and_unsupported_v2_manifests() {
        for (bytes, error) in [
            (b"{}".as_slice(), "pinned schema"),
            (b"{".as_slice(), "manifest.json"),
            (
                br#"{"api_version":2,"api_version":2}"#.as_slice(),
                "duplicate object key",
            ),
            (br#"{"scratch":{}}"#.as_slice(), "Private scratch disks"),
            (
                br#"{"kinds":[{"conversation":true}]}"#.as_slice(),
                "Conversation kinds",
            ),
            (
                br#"{"jobs":[{"placement":"heavy"}]}"#.as_slice(),
                "Heavy jobs",
            ),
        ] {
            let result = ManifestV2::parse(bytes).unwrap_err();
            assert!(result.contains(error), "{result}");
        }
    }

    #[test]
    fn installer_refuses_a_reserved_field_and_undeclared_bind() {
        let mut manifest = strict_json(FIXTURE).unwrap();
        manifest["kinds"][0]["indexed_fields"]["updated"] =
            serde_json::json!({"type":"Timestamp","audience":"member_item"});
        let bytes = serde_json::to_vec(&manifest).unwrap();
        assert!(
            ManifestV2::parse(&bytes)
                .unwrap_err()
                .contains("reserved generic indexed field updated")
        );

        let mut manifest = strict_json(FIXTURE).unwrap();
        manifest["reads"] = serde_json::json!(["reading-item"]);
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let manifest = ManifestV2::parse(&bytes).unwrap();
        assert!(
            manifest
                .check_description(TEMPLATE)
                .unwrap_err()
                .contains("undeclared kind reading-reminder")
        );
    }
}
