use super::*;
use serde_json::json;

fn package() -> (Value, BTreeMap<String, String>) {
    (
        json!({"api_version":2,"id":"fixture","name":"Fixture","version":"1.0.0",
            "icon":"book","kinds":[],"apps":[],"operations":[],"network":"none"}),
        BTreeMap::new(),
    )
}

fn pin(files: &mut BTreeMap<String, String>, path: &str, bytes: &[u8]) -> Value {
    files.insert(path.into(), STANDARD.encode(bytes));
    json!({"path":path,"sha256":crate::plugins::sha256_hex(bytes)})
}

fn validation(manifest: Value, assets: BTreeMap<String, String>) -> Result<ManifestV2, String> {
    PackageV2 {
        manifest: RawValue::from_string(manifest.to_string()).unwrap(),
        assets,
    }
    .validate()
}

#[test]
fn v2_installer_checks_harness_entry_bytes() {
    let (mut manifest, mut assets) = package();
    manifest["worker"] = pin(&mut assets, "worker.mjs", b"worker");
    manifest["operations"] = json!([{
        "id":"fixture.run", "title":"Run", "required_capability":"plugins.use",
        "risk":"external", "idempotency":"request_id", "methods":[],
        "input_schema":{}, "output_schema":{}
    }]);
    manifest["contributions"] = json!([{
        "app":{"package":"owner","app":"home"}, "point":"runner", "version":1,
        "id":"fixture", "kind":"harness", "operation":"fixture.run",
        "harness":{"image":format!("sha256:{}", "1".repeat(64)),
            "entry":pin(&mut assets, "entry.mjs", b"entry"), "name":"Fixture"}
    }]);
    assert!(validation(manifest.clone(), assets.clone()).is_ok());
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::write(root.join("manifest.json"), manifest.to_string()).unwrap();
    std::fs::write(root.join("worker.mjs"), b"worker").unwrap();
    std::fs::write(root.join("entry.mjs"), b"entry").unwrap();
    assert!(InstallPackage::read(root).is_ok());
    std::fs::write(root.join("entry.mjs"), b"changed").unwrap();
    assert!(
        InstallPackage::read(root)
            .err()
            .unwrap()
            .contains("digest mismatch")
    );
    assets.insert("entry.mjs".into(), STANDARD.encode(b"changed"));
    assert!(
        validation(manifest.clone(), assets.clone())
            .unwrap_err()
            .contains("digest mismatch")
    );
    assets.remove("entry.mjs");
    assert!(
        validation(manifest, assets)
            .unwrap_err()
            .contains("entry.mjs is missing")
    );
}

#[test]
fn v2_installer_rejects_extra_invalid_oversized_and_non_image_assets() {
    let (manifest, mut assets) = package();
    assets.insert("extra".into(), STANDARD.encode(b"hello"));
    assert!(
        validation(manifest, assets)
            .unwrap_err()
            .contains("unpinned asset")
    );

    let (mut manifest, mut assets) = package();
    manifest["worker"] = pin(&mut assets, "worker", b"text");
    assets.insert("worker".into(), "#".into());
    assert!(
        validation(manifest.clone(), assets)
            .unwrap_err()
            .contains("invalid base64")
    );

    let mut assets = BTreeMap::new();
    manifest["worker"] = pin(&mut assets, "worker", &vec![1; MAX_WORKER + 1]);
    assert!(
        validation(manifest, assets)
            .unwrap_err()
            .contains("byte budget")
    );

    let (mut manifest, mut assets) = package();
    manifest["icon"] = json!({"image":pin(&mut assets,"icon",b"not an image")});
    assert!(
        validation(manifest, assets)
            .unwrap_err()
            .contains("must be PNG or JPEG")
    );
}

#[test]
fn v2_installer_enforces_the_combined_asset_budget() {
    let (mut manifest, mut assets) = package();
    let length = MAX_ICON;
    let mut image = vec![0; length];
    image[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    for i in 0..=(MAX_BINARY_ASSETS / length) {
        let pinned = pin(&mut assets, &format!("icon-{i}"), &image);
        manifest["kinds"].as_array_mut().unwrap().push(json!({
            "id":format!("kind-{i}"), "icon":{"image":pinned}, "title_field":"title",
            "claimed_text":true,"storage":{"hidden":true},
            "schema":{"type":"object","properties":{"title":{"type":"string"}}}
        }));
    }
    assert!(
        validation(manifest, assets)
            .unwrap_err()
            .contains("package assets exceed the byte budget")
    );
}
