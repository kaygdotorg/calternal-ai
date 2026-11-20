use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_types::PackageId;

const TEMPLATE: &[u8] = br#"{"version":2,"kind":"app","revision":1}"#;
const ENTRY: &[u8] = b"scripted entry";

fn input_schema() -> Value {
    json!({
        "type":"object", "additionalProperties":false, "required":["text"],
        "properties":{"text":{"type":"string", "minLength":1, "maxLength":16384}}
    })
}

fn output_schema() -> Value {
    json!({
        "type":"object", "additionalProperties":false, "required":["text","log"],
        "properties":{
            "text":{"type":"string", "minLength":1, "maxLength":16384},
            "log":{"$ref":"host-catalogue-v2.json#/$defs/ItemLocator"}
        }
    })
}

fn owner(package: &str, input: Value) -> Value {
    let mut manifest = bare(package);
    let mut app = app("home");
    app["description"]["template"]["sha256"] = json!(crate::plugins::sha256_hex(TEMPLATE));
    app["extension_points"] = json!([{
        "id":"runner", "kind":"harness", "version":1.0,
        "input_schema":input, "output_schema":output_schema()
    }]);
    manifest["apps"] = json!([app]);
    manifest
}

fn contributor() -> Value {
    let mut manifest = bare("helper");
    with_op(&mut manifest, "helper.run");
    manifest["worker"]["sha256"] = json!(crate::plugins::sha256_hex(WORKER));
    manifest["operations"][0]["input_schema"] = input_schema();
    manifest["operations"][0]["output_schema"] = output_schema();
    manifest["contributions"] = json!([{
        "id":"script", "app":{"package":"alpha","app":"home"}, "point":"runner",
        "version":1, "kind":"harness", "operation":"helper.run",
        "harness":{"name":"Script","image":format!("sha256:{}", "1".repeat(64)),
            "entry":{"path":"entry.mjs","sha256":crate::plugins::sha256_hex(ENTRY)}}
    }]);
    manifest
}

#[test]
fn v2_installer_resolves_qualified_harness_interfaces() {
    let appdata = tempfile::tempdir().unwrap();
    let distribution = tempfile::tempdir().unwrap();
    fs::write(
        distribution.path().join("trusted-plugins.json"),
        r#"{"plugins":[]}"#,
    )
    .unwrap();
    let open = || {
        package_store::load(appdata.path(), Some(distribution.path()), None, false)
            .unwrap()
            .snapshot()
    };
    let install = |base: &Catalog, manifest: &Value| {
        let assets = if manifest["id"] == "helper" {
            json!({"worker.mjs":STANDARD.encode(WORKER), "entry.mjs":STANDARD.encode(ENTRY)})
        } else {
            json!({"app.json":STANDARD.encode(TEMPLATE)})
        };
        let bytes = serde_json::to_vec(&json!({"manifest":manifest, "assets":assets})).unwrap();
        let approval = crate::plugins::sha256_hex(&bytes);
        install_v2(appdata.path(), base, &bytes, &approval)
    };
    let catalogue = install(&open(), &owner("alpha", input_schema())).unwrap();
    let catalogue = install(&catalogue, &owner("beta", json!({"type":"integer"}))).unwrap();
    let package = contributor();

    for (path, substitute, error) in [
        (
            "/contributions/0/app/package",
            json!("beta"),
            "input schema",
        ),
        ("/contributions/0/version", json!(2), "version"),
        (
            "/contributions/0/operation",
            json!("alpha.run"),
            "owned operation",
        ),
        (
            "/operations/0/output_schema",
            json!({"type":"boolean"}),
            "output schema",
        ),
    ] {
        let mut invalid = package.clone();
        *invalid.pointer_mut(path).unwrap() = substitute;
        let message = install(&catalogue, &invalid).unwrap_err();
        assert!(message.contains(error), "{message}");
        assert!(open().v2_manifest("helper").is_none());
    }

    let catalogue = install(&catalogue, &package).unwrap();
    let owner = PackageId("alpha".into());
    let restarted = open();
    for catalogue in [&catalogue, restarted.as_ref()] {
        let mut selected =
            catalogue.checked_contributions(&owner, "home", "runner", 1, "harness", |_| true);
        let choice = selected.next().unwrap();
        assert_eq!(choice.identity.contribution.0, "script");
        assert_eq!(choice.declaration["operation"], "helper.run");
        assert!(choice.interface.accepts_input(&json!({"text":"Approved"})));
        assert!(!choice.interface.accepts_input(&json!(42)));
        assert!(choice.interface.accepts_output(
            &json!({"text":"Reply to Approved","log":{"space":"space","id":"log"}})
        ));
        assert!(
            !choice
                .interface
                .accepts_output(&json!({"text":"Reply to Approved"}))
        );
        assert!(selected.next().is_none());
        assert_eq!(
            catalogue
                .checked_contributions(
                    &PackageId("beta".into()),
                    "home",
                    "runner",
                    1,
                    "harness",
                    |_| true
                )
                .count(),
            0
        );
        assert_eq!(
            catalogue
                .checked_contributions(&owner, "home", "runner", 1, "harness", |package| package
                    == &owner)
                .count(),
            0
        );
    }

    let original = catalogue
        .checked_contributions(&owner, "home", "runner", 1, "harness", |_| true)
        .next()
        .unwrap();
    let removed = catalog(&catalogue, "helper", None).unwrap();
    package_store::remove(appdata.path(), "helper").unwrap();
    assert_eq!(
        removed
            .checked_contributions(&owner, "home", "runner", 1, "harness", |_| true)
            .count(),
        0
    );
    let reinstalled = install(&removed, &package).unwrap();
    let replacement = reinstalled
        .checked_contributions(&owner, "home", "runner", 1, "harness", |_| true)
        .next()
        .unwrap();
    assert_eq!(replacement.identity, original.identity);
    assert_eq!(replacement.owner_revision, original.owner_revision);
    assert!(replacement.contributor_revision > original.contributor_revision);
}
