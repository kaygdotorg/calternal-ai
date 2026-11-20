use crate::plugins::package_store::InstallPackage;
use serde_json::{Value, json};
use std::{fs, path::Path};

use super::*;

const MANIFEST: &[u8] = include_bytes!(
    "../../../../../contract/fixtures/description-v2/manifests/reading-list-installer-v2-app.json"
);
const TEMPLATE: &[u8] = include_bytes!(
    "../../../../../contract/fixtures/description-v2/reading-list-installer-v2-app.json"
);
const WORKER: &[u8] = b"throw new Error('registration must never start this worker');";

fn source(path: &Path) -> Value {
    let mut manifest: Value = serde_json::from_slice(MANIFEST).unwrap();
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

    // Install a legacy package through the normal prepare/commit path in
    // the mixed catalog, then rebuild the catalog from its stored archive.
    let v1 = fixture.path().join("legacy");
    fs::create_dir(&v1).unwrap();
    fs::write(
        v1.join("manifest.json"),
        include_bytes!("../../../../../plugins/calternal-notes/manifest.json"),
    )
    .unwrap();
    let InstallPackage::V1(legacy) = InstallPackage::read(&v1).unwrap() else {
        panic!("expected original v1 reader");
    };
    // Reinstall v2 before writing v1 to cover both decoders on restart.
    let base = open().snapshot();
    install_v2(appdata.path(), &base, archive.as_bytes(), &approved).unwrap();
    let checksum = legacy.checksum().unwrap();
    let candidate = package_store::prepare(appdata.path(), legacy, checksum, false)
        .unwrap()
        .unwrap();
    package_store::commit(appdata.path(), candidate).unwrap();
    let mixed = open().snapshot();
    assert_eq!(
        mixed.get("calternal-notes").unwrap().manifest.api_version,
        1
    );
    assert_eq!(mixed.v2_manifest(id).unwrap().id(), id);
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

fn bare(id: &str) -> Value {
    json!({"api_version":2,"id":id,"name":"Installer test","version":"1.0.0","icon":"book",
        "apps":[],"kinds":[],"operations":[],"network":"none"})
}

fn app(id: &str) -> Value {
    json!({"id":id,"name":"Installer test","icon":"book",
        "description":{"id":"app","template":{"path":"app.json","sha256":"0".repeat(64)}}})
}

fn kind(id: &str, folder: &str) -> Value {
    json!({"id":id,"icon":"book","title_field":"title","claimed_text":true,
        "storage":{"folder":folder},"schema":{"type":"object","properties":{"title":{"type":"string"}}}})
}

fn provider(id: &str) -> Value {
    json!({"id":id,"name":"Provider","engine":"turns","adapter":{"path":"adapter.json","sha256":"0".repeat(64)}})
}

fn op(id: &str) -> Value {
    json!({"id":id,"title":"Read","input_schema":{},"output_schema":{},"required_capability":"space.read",
        "risk":"read","idempotency":"read_only","methods":[]})
}

fn with_op(manifest: &mut Value, id: &str) {
    manifest["operations"] = json!([op(id)]);
    manifest["worker"] = json!({"path":"worker.mjs","sha256":"0".repeat(64)});
}

fn v2(manifest: Value) -> Arc<calternal_plugin_host::ManifestV2> {
    Arc::new(
        calternal_plugin_host::ManifestV2::parse(&serde_json::to_vec(&manifest).unwrap()).unwrap(),
    )
}

fn refuse(catalogue: &Catalog, manifest: Value, message: &str) {
    let mut catalogue = catalogue.clone();
    let error = catalogue.register_v2(v2(manifest), 1).unwrap_err();
    assert!(error.contains(message), "{error}");
}

fn legacy_notes(id: &str, kind: &str, folder: &str) -> Arc<crate::plugins::Plugin> {
    let mut manifest = calternal_plugin_host::parse_manifest(include_bytes!(
        "../../../../../plugins/calternal-notes/manifest.json"
    ))
    .unwrap();
    manifest.id = id.into();
    manifest.kinds[0].id = kind.into();
    manifest.kinds[0].storage = calternal_core::ManifestPlacement::Visible {
        folder: folder.into(),
    };
    let package = package_store::Package {
        manifest,
        worker: None,
    };
    Arc::new(crate::plugins::Plugin::new(package.entry(false), package.manifest, None).unwrap())
}

#[test]
fn v2_installer_rejects_registration_collisions_and_dangling_reads() {
    let mut catalog_v1 =
        Catalog::from_plugins(vec![legacy_notes("calternal-notes", "notes", "Notes")]);
    let mut name = bare("intruder");
    name["apps"] = json!([app("calternal-notes")]);
    with_op(&mut name, "calternal-notes.export");
    refuse(&catalog_v1, name, "namespace calternal-notes");
    refuse(
        &catalog_v1,
        bare("calternal-notes"),
        "namespace calternal-notes",
    );
    let mut candidate = bare("intruder");
    candidate["kinds"] = json!([kind("notes", "Elsewhere")]);
    refuse(&catalog_v1, candidate.clone(), "Kind notes");
    candidate["kinds"] = json!([kind("different", "NOTES")]);
    refuse(&catalog_v1, candidate, "Storage folder");

    // Provider declarations from the sealed v2 draft cannot claim an ID
    // under the current manifest. Harnesses use qualified contributions.
    let native_manifest = bare("intruder");
    v2(native_manifest.clone());
    for provider_id in ["calternal-claude-code", "owner-provider"] {
        let mut candidate = native_manifest.clone();
        candidate["providers"] = json!([provider(provider_id)]);
        let error =
            calternal_plugin_host::ManifestV2::parse(&serde_json::to_vec(&candidate).unwrap())
                .unwrap_err();
        assert_eq!(error, "v2 manifest does not match the pinned schema at /");
    }

    let mut catalog_v2 = Catalog::default();
    let mut owner = bare("owner-package");
    owner["apps"] = json!([app("owner-app")]);
    owner["kinds"] = json!([kind("owner-kind", "Owner Folder")]);
    with_op(&mut owner, "owner-app.read");
    catalog_v2.register_v2(v2(owner.clone()), 1).unwrap();
    refuse(&catalog_v2, owner, "Operation owner-app.read");
    refuse(&catalog_v2, bare("owner-app"), "namespace owner-app");
    let mut candidate = bare("app-only-name");
    candidate["apps"] = json!([app("owner-package")]);
    catalog_v2.register_v2(v2(candidate), 1).unwrap();
    let mut candidate = bare("other-package");
    candidate["apps"] = json!([app("owner-app")]);
    with_op(&mut candidate, "other-package.read");
    catalog_v2.register_v2(v2(candidate), 1).unwrap();
    let mut candidate = bare("operation-conflict");
    candidate["apps"] = json!([app("owner-app")]);
    with_op(&mut candidate, "owner-app.read");
    refuse(&catalog_v2, candidate, "Operation owner-app.read");
    for (entry, value, error) in [
        ("kinds", kind("owner-kind", "New Folder"), "Kind owner-kind"),
        ("kinds", kind("new-kind", "OWNER FOLDER"), "Storage folder"),
    ] {
        let mut candidate = bare("intruder");
        candidate[entry] = json!([value]);
        refuse(&catalog_v2, candidate, error);
    }
    for (id, kind, folder, error) in [
        ("owner-package", "new", "New", "namespace owner-package"),
        ("owner-app", "new", "New", "namespace owner-app"),
        ("legacy", "owner-kind", "New", "Kind owner-kind"),
        ("legacy", "new", "OWNER FOLDER", "Storage folder"),
    ] {
        let v1 = legacy_notes(id, kind, folder);
        let failure = catalog(&catalog_v2, id, Some(v1)).unwrap_err();
        assert!(failure.contains(error), "{failure}");
    }
    for bad_site in ["127.0.0.1", "localhost"] {
        let mut candidate = bare("intruder");
        candidate["network"] = json!({"fixed_sites":[bad_site]});
        refuse(&catalog_v2, candidate, "exact HTTPS hostname");
    }
    let mut candidate = bare("intruder");
    with_op(&mut candidate, "intruder.read");
    candidate["operations"][0]["required_capability"] = json!("spaces.list");
    refuse(&catalog_v2, candidate, "needs a non-space capability");
    let mut dependent = bare("dependent");
    dependent["reads"] = json!(["missing"]);
    refuse(
        &catalog_v2,
        dependent.clone(),
        "Declared read kind missing is not installed",
    );
    dependent["reads"] = json!(["owner-kind"]);
    catalog_v2.register_v2(v2(dependent.clone()), 1).unwrap();
    assert!(
        catalog(&catalog_v2, "owner-package", None)
            .unwrap_err()
            .contains("depends on the removed kind")
    );
    dependent["reads"] = json!(["notes"]);
    catalog_v1.register_v2(v2(dependent), 1).unwrap();
    assert!(
        catalog(&catalog_v1, "calternal-notes", None)
            .unwrap_err()
            .contains("depends on the removed kind")
    );
}

#[test]
fn v2_installer_skips_invalid_or_colliding_stored_rows_on_startup() {
    let data = tempfile::tempdir().unwrap();
    let distribution = tempfile::tempdir().unwrap();
    fs::write(
        distribution.path().join("trusted-plugins.json"),
        r#"{"plugins":[]}"#,
    )
    .unwrap();
    let open = || package_store::load(data.path(), Some(distribution.path()), None, false).unwrap();
    let base = open().snapshot();
    let mut bad = bare("bad-package");
    bad["unknown_after_schema_change"] = json!(true);
    let stored = |manifest| serde_json::to_vec(&json!({"manifest":manifest,"assets":{}})).unwrap();
    let inject = |id, bytes: &[u8]| {
        package_store::commit_v2(
            data.path(),
            id,
            bytes,
            &crate::plugins::sha256_hex(bytes),
            |_| Ok(()),
        )
        .unwrap()
    };
    inject("bad-package", &stored(bad));
    let good = bare("good-package");
    let good = stored(good);
    let approved = crate::plugins::sha256_hex(&good);
    let now = install_v2(data.path(), &base, &good, &approved).unwrap();
    assert!(
        install_v2(data.path(), &now, &good, &approved)
            .unwrap_err()
            .contains("is already registered")
    );
    assert!(
        install_v2(data.path(), &base, &good, &approved)
            .unwrap_err()
            .contains("already installed")
    );

    let legacy = legacy_notes("calternal-notes", "notes", "Notes");
    let package = package_store::Package {
        manifest: legacy.manifest.clone(),
        worker: None,
    };
    let approved = package.checksum().unwrap();
    let legacy = package_store::prepare(data.path(), package, approved, false)
        .unwrap()
        .unwrap();
    package_store::commit(data.path(), legacy).unwrap();
    let mut collision = bare("conflict");
    collision["kinds"] = json!([kind("notes", "Elsewhere")]);
    inject("conflict", &stored(collision));

    let restarted = open().snapshot();
    assert!(restarted.v2_manifest("bad-package").is_none());
    assert!(restarted.v2_manifest("conflict").is_none());
    assert!(restarted.v2_manifest("good-package").is_some());
    assert!(restarted.get("calternal-notes").is_some());
}

mod reservations;

mod contributions;
