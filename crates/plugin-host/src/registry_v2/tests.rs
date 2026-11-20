use super::*;

const FIXTURE: &[u8] = include_bytes!(
    "../../../../contract/fixtures/description-v2/manifests/reading-list-installer-v2-app.json"
);
const TEMPLATE: &[u8] = include_bytes!(
    "../../../../contract/fixtures/description-v2/reading-list-installer-v2-app.json"
);

#[test]
fn installer_reads_sdk_v2_reading_list() {
    let installed = ManifestV2::parse(FIXTURE).unwrap();
    assert_eq!(installed.id(), "calternal-reading-list");
    assert!(installed.reads("reading-item"));
    installed.check_description(TEMPLATE).unwrap();
}

#[test]
fn installer_checks_host_create_defaults_and_pinned_follow_up_vectors() {
    let mut fixture = strict_json(FIXTURE).unwrap();
    fixture["kinds"][0]["host_create"] = serde_json::json!({"defaults":{"title":"Created"}});
    ManifestV2::parse(&serde_json::to_vec(&fixture).unwrap()).unwrap();
    fixture["kinds"][0]["host_create"]["defaults"]["title"] = false.into();
    refuse(&fixture, "host_create defaults");
    fixture["kinds"][0]
        .as_object_mut()
        .unwrap()
        .remove("host_create");
    fixture["kinds"][0]["indexed_fields"]["read"]["type"] = "Text".into();
    refuse(&fixture, "Indexed field read");

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contract/fixtures/description-v2/manifests/install");
    for name in [
        "host-create-after-item-ref-annotations",
        "host-create-after-item-ref-array-pointer",
        "host-create-after-item-ref-escaped-pointer",
        "host-create-after-item-ref-percent-encoded-pointer",
        "invalid-host-create-after-item-ref-array-pointer-out-of-range",
        "invalid-host-create-after-job-input",
    ] {
        let bytes = std::fs::read(root.join(format!("{name}.json"))).unwrap();
        let result = ManifestV2::parse(&bytes);
        if name.starts_with("invalid-") {
            let error = result.unwrap_err();
            assert!(
                error.contains("host_create.after") || error.contains("unregistered reference"),
                "{name}: {error}"
            );
        } else {
            assert!(result.is_ok(), "{name}: {result:?}");
        }
    }
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

fn refuse(fixture: &Value, expected: &str) {
    let error = ManifestV2::parse(&serde_json::to_vec(fixture).unwrap()).unwrap_err();
    assert!(error.contains(expected), "{error}");
}

#[test]
fn rejects_reserved_kinds_namespaces_and_duplicate_declarations() {
    let fixture = strict_json(FIXTURE).unwrap();
    for id in ["requests", "collections", "automations", "skill-example"] {
        let mut manifest = fixture.clone();
        manifest["kinds"][0]["id"] = id.into();
        refuse(&manifest, "reserved by the host");
    }
    for id in ["host.take", "turns.take", "foreign.take"] {
        let mut manifest = fixture.clone();
        manifest["operations"][0]["id"] = id.into();
        refuse(
            &manifest,
            "must use the package or one of its app namespaces",
        );
    }
    for list in ["apps", "kinds", "operations"] {
        let mut manifest = fixture.clone();
        let duplicate = manifest[list][0].clone();
        manifest[list].as_array_mut().unwrap().push(duplicate);
        refuse(&manifest, "Duplicate v2 declaration");
    }
    let mut manifest = fixture;
    manifest["version"] = "not-semver".into();
    refuse(&manifest, "semantic version");
}

#[test]
fn rejects_unsafe_disclosures_and_other_schema_drafts() {
    let fixture = strict_json(FIXTURE).unwrap();
    for value in [
        "Names\x00",
        "Names\u{0085}",
        "Names\u{202e}",
        "Names\u{2068}",
    ] {
        let mut manifest = fixture.clone();
        manifest["name"] = value.into();
        refuse(&manifest, "control and bidirectional");
        manifest["name"] = fixture["name"].clone();
        manifest["operations"][0]["material_inputs"] = serde_json::json!({"url":value});
        refuse(&manifest, "control and bidirectional");
        manifest["operations"][0]
            .as_object_mut()
            .unwrap()
            .remove("material_inputs");
        manifest["secrets"] = serde_json::json!([{"name":"login","label":"Login","purpose":value,"scope":"personal","required":true}]);
        refuse(&manifest, "control and bidirectional");
    }
    for schema in [
        "https://json-schema.org/draft/2020-12/schema",
        "http://json-schema.org/draft-04/schema#",
    ] {
        let mut manifest = fixture.clone();
        manifest["operations"][0]["input_schema"]["$schema"] = schema.into();
        refuse(&manifest, "draft-07");
    }
    let mut nested = fixture.clone();
    nested["kinds"][0]["schema"]["definitions"] = serde_json::json!({
        "other":{"$id":"urn:other","$schema":"https://json-schema.org/draft/2020-12/schema","type":"string"}
    });
    refuse(&nested, "draft-07");
    let mut allowed = fixture;
    allowed["kinds"][0]["schema"]["$schema"] = "http://json-schema.org/draft-07/schema#".into();
    ManifestV2::parse(&serde_json::to_vec(&allowed).unwrap()).unwrap();
}

#[test]
fn checks_template_action_ownership_and_credential_bindings() {
    let mut manifest = strict_json(FIXTURE).unwrap();
    let checked = ManifestV2::parse(FIXTURE).unwrap();
    for op in ["foreign.add", "host.not-an-action"] {
        let template = std::str::from_utf8(TEMPLATE)
            .unwrap()
            .replace("reading.add", op);
        assert!(
            checked
                .check_description(template.as_bytes())
                .unwrap_err()
                .contains(op)
        );
    }
    manifest["network"] = serde_json::json!({"fixed_sites":["read.example.org"]});
    manifest["secrets"] = serde_json::json!([{"name":"login","label":"Login","purpose":"Check the listing","scope":"personal","required":true}]);
    manifest["credential_routes"] = serde_json::json!([{
        "id":"read","connection":{"fixed_site":"foreign.example.org"},"profile":"source",
        "secret":"login","input_schema":{},"output_schema":{}
    }]);
    refuse(&manifest, "undeclared network site");
    manifest["credential_routes"][0]["connection"]["fixed_site"] = "read.example.org".into();
    manifest["credential_routes"][0]["secret"] = "unknown".into();
    refuse(&manifest, "undeclared secret");
    manifest["credential_routes"][0]["secret"] = "login".into();
    ManifestV2::parse(&serde_json::to_vec(&manifest).unwrap()).unwrap();
}
