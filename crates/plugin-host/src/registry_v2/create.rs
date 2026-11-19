//! Semantic checks for the 13.18 host-create declaration, after pinned shape validation.
use super::*;
use percent_encoding::percent_decode_str;

const ANNOTATIONS: &[&str] = &[
    "title",
    "description",
    "$comment",
    "default",
    "examples",
    "deprecated",
    "readOnly",
    "writeOnly",
];

pub(super) fn check(
    manifest: &Value,
    kind: &Value,
    metadata: &jsonschema::Validator,
) -> Result<(), String> {
    let Some(create) = kind.get("host_create") else {
        return Ok(());
    };
    validate(metadata, &create["defaults"], "host_create defaults")?;
    let Some(after) = create.get("after") else {
        return Ok(());
    };
    let id = if let Some(job) = after.get("job") {
        entries(manifest, "jobs")
            .find(|entry| &entry["id"] == job)
            .map(|job| &job["run_operation"])
    } else {
        after.get("operation")
    };
    let operation =
        id.and_then(|id| entries(manifest, "operations").find(|entry| &entry["id"] == id));
    let accepted = operation.is_some_and(|op| {
        let field = str_field(&op["target"], "field");
        op["target"]["kind"] == kind["id"]
            && !field.is_empty()
            && accepts_created_ref(&op["input_schema"], field)
    });
    if !accepted {
        return Err(format!(
            "host_create.after must accept only the created ItemRef of {} at target.field",
            str_field(kind, "id")
        ));
    }
    Ok(())
}

fn accepts_created_ref(input: &Value, field: &str) -> bool {
    let Some(root) = input.as_object() else {
        return false;
    };
    let allowed_root = [
        "type",
        "additionalProperties",
        "required",
        "properties",
        "definitions",
        "$defs",
    ];
    if root
        .keys()
        .any(|key| !allowed_root.contains(&key.as_str()) && !ANNOTATIONS.contains(&key.as_str()))
        || input["type"] != "object"
        || input["additionalProperties"] != false
        || input["required"] != serde_json::json!([field])
        || !input["properties"]
            .as_object()
            .is_some_and(|properties| properties.len() == 1)
    {
        return false;
    }

    let mut current = &input["properties"][field];
    let mut visited = BTreeSet::new();
    while let Some(reference) = current["$ref"].as_str() {
        if !visited.insert(reference)
            || current.as_object().is_none_or(|node| {
                node.keys()
                    .any(|key| key != "$ref" && !ANNOTATIONS.contains(&key.as_str()))
            })
        {
            return false;
        }
        let Some((document, fragment)) = reference.split_once('#') else {
            return false;
        };
        let Ok(pointer) = percent_decode_str(fragment).decode_utf8() else {
            return false;
        };
        match document {
            "" => {
                let Some(target) = input.pointer(&pointer) else {
                    return false;
                };
                current = target;
            }
            "host-catalogue-v2.json"
            | "https://calternal.cloud/contract/host-catalogue-v2.json" => {
                return pointer == "/$defs/ItemRef";
            }
            _ => return false,
        }
    }
    false
}
