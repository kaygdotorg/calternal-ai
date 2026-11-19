use crate::plugins::package_store::InstallPackage;
use serde_json::{Value, json};
use std::{fs, path::Path};

use super::*;

const MANIFEST: &[u8] = include_bytes!(
    "../../../../../contract/fixtures/description-v2/manifests/reading-list-manifest-v2-app.json"
);
const TEMPLATE: &[u8] = include_bytes!(
    "../../../../../contract/fixtures/description-v2/reading-list-manifest-v2-app.json"
);
const WORKER: &[u8] = b"throw new Error('registration must never start this worker');";

fn source(path: &Path) -> Value {
    let mut manifest: Value = serde_json::from_slice(MANIFEST).unwrap();
    manifest["id"] = json!("calternal-reading-list");
    manifest["worker"] = json!({"path":"worker.mjs", "sha256": crate::plugins::sha256_hex(WORKER)});
    let template = manifest["apps"][0]["description"]["template"]["path"]
        .as_str()
        .unwrap();
    fs::write(path.join(template), TEMPLATE).unwrap();
    fs::write(path.join("worker.mjs"), WORKER).unwrap();
    fs::write(
        path.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    manifest
}

fn read_package(path: &Path) -> PackageV2 {
    match InstallPackage::read(path).unwrap() {
        InstallPackage::V2(package, _) => package,
        InstallPackage::V1(_) => panic!("expected v2"),
    }
}

fn read_error(path: &Path, expected: &str) {
    let error = InstallPackage::read(path)
        .err()
        .expect("package was refused");
    assert!(error.contains(expected), "{error}");
}

#[test]
fn v2_installer_registers_reading_list_and_reloads_it_without_a_worker() {
    let appdata = tempfile::tempdir().unwrap();
    let distribution = tempfile::tempdir().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    fs::write(
        distribution.path().join("trusted-plugins.json"),
        r#"{"plugins":[]}"#,
    )
    .unwrap();
    source(fixture.path());

    let archive = fixture.path().join("reading-list.calplugin");
    fs::write(
        &archive,
        InstallPackage::read(fixture.path())
            .unwrap()
            .archive()
            .unwrap(),
    )
    .unwrap();
    let package = read_package(&archive);
    let approved = package.checksum().unwrap();
    let open =
        || package_store::load(appdata.path(), Some(distribution.path()), None, false).unwrap();

    let base = open().snapshot();
    let wire =
        serde_json::to_vec(&Change::install_v2(&package, approved.clone()).unwrap()).unwrap();
    let Change::InstallV2 { archive, approval } = serde_json::from_slice(&wire).unwrap() else {
        panic!("expected v2 install");
    };
    assert_eq!(approval, approved);
    assert!(
        install_v2(appdata.path(), &base, archive.as_bytes(), "wrong")
            .unwrap_err()
            .contains("approval checksum")
    );
    assert!(open().snapshot().v2_kind("reading-item").is_none());

    let installed = install_v2(appdata.path(), &base, archive.as_bytes(), &approval).unwrap();
    let restarted = open().snapshot();
    for catalogue in [&installed, restarted.as_ref()] {
        let (owner, kind) = catalogue.v2_kind("reading-item").unwrap();
        assert_eq!(owner.id(), "calternal-reading-list");
        assert!(owner.reads("reading-reminder"));
        assert_eq!(owner.declaration()["network"], "none");
        assert_eq!(kind["claimed_text"], true);
        assert_eq!(kind["searchable"], true);
        assert_eq!(
            kind["indexed_fields"]["read"],
            json!({"type":"Boolean","audience":"member_item"})
        );
        assert_eq!(
            catalogue.v2_operation("reading.add").unwrap().0.id(),
            owner.id()
        );
        assert!(catalogue.owner_of_operation("reading.add").is_none());
        assert!(catalogue.get(owner.id()).is_none());
    }
    assert_eq!(
        fs::read_dir(appdata.path().join("installed-plugin-workers"))
            .unwrap()
            .count(),
        0
    );

    let id = "calternal-reading-list";
    let removed = catalog(&installed, id, None).unwrap();
    package_store::remove(appdata.path(), id).unwrap();
    assert!(removed.v2_kind("reading-item").is_none());
    assert!(open().snapshot().v2_manifest(id).is_none());

    // The same version selector still writes the exact legacy archive.
    let v1 = fixture.path().join("legacy");
    fs::create_dir(&v1).unwrap();
    fs::write(
        v1.join("manifest.json"),
        include_bytes!("../../../../../plugins/calternal-notes/manifest.json"),
    )
    .unwrap();
    let original = crate::plugins::package_store::Package::read(&v1)
        .unwrap()
        .archive()
        .unwrap();
    let selected = InstallPackage::read(&v1).unwrap();
    assert!(matches!(&selected, InstallPackage::V1(_)));
    assert_eq!(selected.archive().unwrap(), original);
}

#[test]
fn v2_installer_rejects_digest_mismatch_and_bad_declarations() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path();
    let original = source(path);
    fs::write(path.join("worker.mjs"), b"changed").unwrap();
    read_error(path, "SHA-256 digest mismatch");
    fs::remove_file(path.join("worker.mjs")).unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    fs::write(outside.path(), WORKER).unwrap();
    std::os::unix::fs::symlink(outside.path(), path.join("worker.mjs")).unwrap();
    read_error(path, "Pinned asset worker.mjs");
    fs::remove_file(path.join("worker.mjs")).unwrap();
    fs::write(path.join("worker.mjs"), WORKER).unwrap();

    let manifest_path = path.join("manifest.json");
    fs::write(&manifest_path, b"{}").unwrap();
    read_error(path, "api_version");
    fs::write(&manifest_path, br#"{"api_version":2}"#).unwrap();
    read_error(path, "pinned schema");

    let mut manifest = original.clone();
    manifest["kinds"][0]["indexed_fields"]["title"] =
        json!({"type":"Text","audience":"member_item"});
    fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    read_error(path, "reserved generic indexed field title");

    let mut manifest = original;
    manifest["reads"] = json!(["reading-item"]);
    fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    read_error(path, "reads undeclared kind reading-reminder");
}
