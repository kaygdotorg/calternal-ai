//! Exercise persisted skips, namespace conflicts and update dependency fences.
use super::*;

fn start_catalog(data: &Path) -> Catalog {
    let distribution = tempfile::tempdir().unwrap();
    package_store::load(data, Some(distribution.path()), None, false)
        .unwrap()
        .snapshot()
        .as_ref()
        .clone()
}

fn stored_archive(data: &Path, id: &str, manifest: Value) {
    let bytes = serde_json::to_vec(&json!({"manifest": manifest, "assets": {}})).unwrap();
    package_store::commit_v2(data, id, &bytes, &crate::plugins::sha256_hex(&bytes)).unwrap();
}

fn legacy_operation(id: &str) -> Arc<crate::plugins::Plugin> {
    let notes = legacy_notes("legacy-reader", "legacy-item", "Legacy Items");
    let runner = calternal_plugin_host::parse_manifest(include_bytes!(
        "../../../../../../plugins/calternal-orchestrator/manifest.json"
    ))
    .unwrap();
    let mut manifest = notes.manifest.clone();
    let mut operation = runner.operations.into_iter().next().unwrap();
    operation.id = id.into();
    manifest.operations.push(operation);
    Arc::new(crate::plugins::Plugin::new(notes.entry.clone(), manifest, None).unwrap())
}

#[test]
fn v1_and_v2_reserve_prefixes_and_report_exact_operation_collisions() {
    let legacy = legacy_operation("reader.open");
    let legacy_catalog = Catalog::from_plugins(vec![legacy.clone()]);
    assert!(legacy_catalog.owner_of_operation("reader.open").is_some());

    let mut v2_reader = bare("reader");
    with_op(&mut v2_reader, "reader.delete");
    refuse(&legacy_catalog, v2_reader.clone(), "namespace reader");
    with_op(&mut v2_reader, "reader.open");
    refuse(&legacy_catalog, v2_reader.clone(), "Operation reader.open");

    let mut installed_v2 = Catalog::default();
    installed_v2.register_v2(v2(v2_reader)).unwrap();
    for (operation, expected) in [
        ("reader.delete", "namespace reader"),
        ("reader.open", "Operation reader.open"),
    ] {
        let legacy = legacy_operation(operation);
        let error = catalog(&installed_v2, &legacy.manifest.id.clone(), Some(legacy)).unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn host_namespaces_cannot_be_claimed_by_apps_or_legacy_operations() {
    let catalog = Catalog::default();
    for prefix in [
        "spaces", "notes", "secrets", "plugins", "jobs", "machines", "pending", "drafts", "agent",
    ] {
        let mut plugin = bare("intruder");
        plugin["apps"] = json!([app(prefix)]);
        refuse(
            &catalog,
            plugin,
            &format!("namespace {prefix} is reserved by the host"),
        );
    }
    let legacy = legacy_operation("notes.takeover");
    let error = crate::plugins::check_operations(&legacy).unwrap_err();
    assert!(
        error.contains("namespace notes is reserved by the host"),
        "{error}"
    );
}

#[test]
fn skipped_manifest_and_its_dependant_reserve_claims_across_restart_and_removal() {
    let data = tempfile::tempdir().unwrap();
    let path = data.path();
    start_catalog(path);
    let mut owner = bare("unavailable");
    owner["kinds"] = json!([kind("private-kind", "Private Folder")]);
    with_op(&mut owner, "unavailable.open");
    owner["obsolete_manifest_field"] = json!(true);
    stored_archive(path, "unavailable", owner);

    let mut dependent = bare("dependant");
    dependent["reads"] = json!(["private-kind"]);
    dependent["kinds"] = json!([kind("dependent-kind", "Dependent Folder")]);
    stored_archive(path, "dependant", dependent);

    stored_archive(path, "unrelated", bare("unrelated"));
    let restarted = start_catalog(path);
    assert!(restarted.v2_manifest("unrelated").is_some());
    for id in ["unavailable", "dependant"] {
        assert!(restarted.v2_manifest(id).is_none());
    }
    assert!(restarted.v2_kind("private-kind").is_none());
    assert!(restarted.v2_operation("unavailable.open").is_none());

    for (id, folder, expected) in [
        ("private-kind", "Elsewhere", "Kind private-kind"),
        ("other-kind", "PRIVATE FOLDER", "Storage folder"),
        ("dependent-kind", "Elsewhere", "Kind dependent-kind"),
        ("other-kind", "DEPENDENT FOLDER", "Storage folder"),
    ] {
        let mut plugin = bare("intruder");
        plugin["kinds"] = json!([kind(id, folder)]);
        refuse(&restarted, plugin, expected);
    }
    let legacy = legacy_operation("unavailable.open");
    let error = catalog(&restarted, &legacy.manifest.id.clone(), Some(legacy)).unwrap_err();
    assert!(
        error.contains("Operation unavailable.open is reserved"),
        "{error}"
    );

    let after_remove = catalog(&restarted, "unavailable", None).unwrap();
    package_store::remove(path, "unavailable").unwrap();
    let mut newcomer = bare("newcomer");
    newcomer["kinds"] = json!([kind("private-kind", "Private Folder")]);
    // A dependant's reservation survives removal of its original owner.
    let mut still_reserved = bare("intruder");
    still_reserved["kinds"] = json!([kind("dependent-kind", "Elsewhere")]);
    refuse(
        &after_remove,
        still_reserved.clone(),
        "unavailable package dependant",
    );
    refuse(
        &start_catalog(path),
        still_reserved,
        "unavailable package dependant",
    );
    // Removing the original owner cannot cause a queued dependant to bind
    // to someone else's item storage on the next restart.
    let bytes = serde_json::to_vec(&json!({"manifest": newcomer, "assets": {}})).unwrap();
    let approval = crate::plugins::sha256_hex(&bytes);
    let error = install_v2(path, &after_remove, &bytes, &approval).unwrap_err();
    assert!(
        error.contains("unresolved read by unavailable package dependant"),
        "{error}"
    );

    let after_both = catalog(&after_remove, "dependant", None).unwrap();
    package_store::remove(path, "dependant").unwrap();
    install_v2(path, &after_both, &bytes, &approval).unwrap();
    assert_eq!(
        start_catalog(path).v2_kind("private-kind").unwrap().0.id(),
        "newcomer"
    );
}

#[test]
fn unreadable_v1_archive_is_skipped_with_actionable_admission_error() {
    let data = tempfile::tempdir().unwrap();
    let path = data.path();
    start_catalog(path);
    let legacy = legacy_notes("damaged-legacy", "legacy-kind", "Legacy Folder");
    let package = package_store::Package {
        manifest: legacy.manifest.clone(),
        worker: None,
    };
    let checksum = package.checksum().unwrap();
    let candidate = package_store::prepare(path, package, checksum, false)
        .unwrap()
        .unwrap();
    package_store::commit(path, candidate).unwrap();
    stored_archive(path, "unrelated", bare("unrelated"));

    let db = rusqlite::Connection::open(path.join("installed-plugins.sqlite3")).unwrap();
    db.execute(
        "UPDATE installed_plugins SET archive=?1 WHERE id='damaged-legacy'",
        [b"corrupt".as_slice()],
    )
    .unwrap();
    drop(db);
    let restarted = start_catalog(path);
    assert!(restarted.get("damaged-legacy").is_none());
    assert!(restarted.v2_manifest("unrelated").is_some());

    let other = legacy_notes("new-package", "new-kind", "New Folder");
    let package = package_store::Package {
        manifest: other.manifest.clone(),
        worker: None,
    };
    let checksum = package.checksum().unwrap();
    let why = package_store::prepare(path, package, checksum, false)
        .err()
        .unwrap();
    assert!(
        why.contains("Installed package damaged-legacy is unavailable"),
        "{why}"
    );
    assert!(
        why.contains("calternal plugin remove damaged-legacy"),
        "{why}"
    );

    let bytes = serde_json::to_vec(&json!({"manifest":bare("new-package"),"assets":{}})).unwrap();
    let approval = crate::plugins::sha256_hex(&bytes);
    let why = install_v2(path, &restarted, &bytes, &approval).unwrap_err();
    assert!(
        why.contains("calternal plugin remove damaged-legacy"),
        "{why}"
    );
    let after_remove = catalog(&restarted, "damaged-legacy", None).unwrap();
    package_store::remove(path, "damaged-legacy").unwrap();
    install_v2(path, &after_remove, &bytes, &approval).unwrap();
}

#[test]
fn replacing_legacy_package_cannot_drop_a_kind_read_by_v2() {
    let data = tempfile::tempdir().unwrap();
    let path = data.path();
    start_catalog(path);
    let legacy = legacy_notes("original", "original-kind", "Original Folder");
    let package = package_store::Package {
        manifest: legacy.manifest.clone(),
        worker: None,
    };
    let checksum = package.checksum().unwrap();
    let candidate = package_store::prepare(path, package, checksum, false)
        .unwrap()
        .unwrap();
    package_store::commit(path, candidate).unwrap();

    let mut dependent = bare("v2-dependent");
    dependent["reads"] = json!(["original-kind"]);
    let bytes = serde_json::to_vec(&json!({"manifest":dependent,"assets":{}})).unwrap();
    install_v2(
        path,
        &start_catalog(path),
        &bytes,
        &crate::plugins::sha256_hex(&bytes),
    )
    .unwrap();

    let mut changed = legacy.manifest.clone();
    changed.kinds.clear();
    let replacement = package_store::Package {
        manifest: changed.clone(),
        worker: None,
    };
    let approval = replacement.checksum().unwrap();
    let why = package_store::prepare(path, replacement, approval, true)
        .err()
        .unwrap();
    assert!(why.contains("update changes stored kinds"), "{why}");

    let plugin =
        Arc::new(crate::plugins::Plugin::new(legacy.entry.clone(), changed, None).unwrap());
    let restart = start_catalog(path);
    let why = catalog(&restart, "original", Some(plugin)).unwrap_err();
    assert!(
        why.contains("Package v2-dependent depends on the removed kind"),
        "{why}"
    );
    assert!(restart.v2_manifest("v2-dependent").is_some());
    assert!(
        restart
            .get("original")
            .unwrap()
            .manifest
            .kinds
            .iter()
            .any(|item| item.id == "original-kind")
    );
}
