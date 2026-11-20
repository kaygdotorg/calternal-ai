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
    package_store::commit_v2(
        data,
        id,
        &bytes,
        &crate::plugins::sha256_hex(&bytes),
        |_| Ok(()),
    )
    .unwrap();
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
    installed_v2.register_v2(v2(v2_reader), 1).unwrap();
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
fn local_app_ids_do_not_claim_host_operation_namespaces() {
    let mut catalog = Catalog::default();
    for prefix in [
        "spaces", "notes", "secrets", "plugins", "jobs", "machines", "pending", "drafts", "agent",
    ] {
        let mut plugin = bare(&format!("fixture-{prefix}"));
        plugin["apps"] = json!([app(prefix)]);
        catalog.register_v2(v2(plugin), 1).unwrap();
    }
    let legacy = legacy_operation("notes.takeover");
    let error = crate::plugins::check_operations(&legacy.manifest).unwrap_err();
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
    let candidate = package_store::prepare(path, package, checksum, false)
        .unwrap()
        .unwrap();
    let why = catalog(&restarted, "new-package", Some(candidate.plugin)).unwrap_err();
    assert!(why.contains("unavailable package damaged-legacy"), "{why}");
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

#[test]
fn delegated_threads_namespace_has_one_owner_and_preserves_exact_host_lookup() {
    let mut catalog = Catalog::default();
    let mut manifest = bare("intruder");
    manifest["apps"] = json!([app("threads")]);
    catalog.register_v2(v2(manifest), 1).unwrap();
    for (id, operation, reason) in [
        (
            "intruder",
            "threads.create",
            "reserved by calternal-orchestrator",
        ),
        (
            "calternal-orchestrator",
            "threads.lookup",
            "shadows a host operation",
        ),
    ] {
        let error = crate::plugins::check_operation(id, operation, "space.read").unwrap_err();
        assert!(error.contains(reason), "{error}");
    }
}

#[test]
fn colliding_and_invalid_rows_do_not_unseat_the_live_owner_after_update_or_restart() {
    let data = tempfile::tempdir().unwrap();
    let path = data.path();
    start_catalog(path);
    let mut broken = bare("broken-earlier");
    broken["kinds"] = json!([
        kind("owned-kind", "Owned Folder"),
        kind("reserved-kind", "Reserved Folder")
    ]);
    broken["obsolete_field"] = json!(true);
    stored_archive(path, "broken-earlier", broken);

    let owner = legacy_notes("winner", "owned-kind", "Owned Folder");
    let mut manifest = owner.manifest.clone();
    manifest
        .operations
        .push(legacy_operation("winner.read").manifest.operations[0].clone());
    let winner = package_store::Package {
        manifest,
        worker: None,
    };
    let archive = winner.archive().unwrap();
    package_store::commit_v2(
        path,
        "winner",
        &archive,
        &winner.checksum().unwrap(),
        |_| Ok(()),
    )
    .unwrap();

    let loser = legacy_notes("loser", "owned-kind", "Loser Folder");
    let loser = package_store::Package {
        manifest: loser.manifest.clone(),
        worker: None,
    };
    let archive = loser.archive().unwrap();
    package_store::commit_v2(path, "loser", &archive, &loser.checksum().unwrap(), |_| {
        Ok(())
    })
    .unwrap();
    let mut skipped_v2 = bare("loser-v2");
    skipped_v2["kinds"] = json!([kind("owned-kind", "Other Folder")]);
    stored_archive(path, "loser-v2", skipped_v2);

    let restarted = start_catalog(path);
    assert!(restarted.get("winner").is_some());
    assert!(restarted.get("loser").is_none());
    assert!(restarted.v2_manifest("loser-v2").is_none());
    assert!(restarted.v2_manifest("broken-earlier").is_none());

    let mut update = winner.manifest.clone();
    update.version = "0.1.1".into();
    // Narrowing authority discards retained revisions. The install order
    // still keeps this owner ahead of both collision losers.
    update.operations.clear();
    let update = package_store::Package {
        manifest: update,
        worker: None,
    };
    let approval = update.checksum().unwrap();
    let candidate = package_store::prepare(path, update, approval, true)
        .unwrap()
        .unwrap();
    let next = catalog(&restarted, "winner", Some(candidate.plugin.clone())).unwrap();
    assert_eq!(next.get("winner").unwrap().manifest.version, "0.1.1");
    package_store::commit(path, candidate).unwrap();

    for _ in 0..2 {
        let reopened = start_catalog(path);
        assert_eq!(reopened.get("winner").unwrap().manifest.version, "0.1.1");
        assert!(reopened.get("loser").is_none());
        assert!(reopened.v2_manifest("loser-v2").is_none());
        let mut intruder = bare("intruder");
        intruder["kinds"] = json!([kind("reserved-kind", "Unrelated Folder")]);
        refuse(
            &reopened,
            intruder,
            "reserved by unavailable package broken-earlier",
        );
    }
}

#[test]
fn startup_uses_install_order_across_versions() {
    let data = tempfile::tempdir().unwrap();
    let path = data.path();
    start_catalog(path);
    let mut first = bare("first-v2");
    first["kinds"] = json!([kind("owned-kind", "Owned Folder")]);
    stored_archive(path, "first-v2", first);
    let second = legacy_notes("later-v1", "owned-kind", "Other Folder");
    let second = package_store::Package {
        manifest: second.manifest.clone(),
        worker: None,
    };
    package_store::commit_v2(
        path,
        "later-v1",
        &second.archive().unwrap(),
        &second.checksum().unwrap(),
        |_| Ok(()),
    )
    .unwrap();
    for _ in 0..2 {
        let restarted = start_catalog(path);
        assert_eq!(restarted.v2_kind("owned-kind").unwrap().0.id(), "first-v2");
        assert!(restarted.get("later-v1").is_none());
    }
}

#[test]
fn startup_resolves_reads_against_all_winners_and_cascades_invalid_reads() {
    let data = tempfile::tempdir().unwrap();
    let path = data.path();
    start_catalog(path);

    let mut reader = bare("first-reader");
    reader["reads"] = json!(["later-kind"]);
    stored_archive(path, "first-reader", reader);
    let mut last_reader = bare("last-reader");
    last_reader["reads"] = json!(["middle-kind"]);
    stored_archive(path, "last-reader", last_reader);
    let mut middle = bare("middle-reader");
    middle["reads"] = json!(["unavailable-kind"]);
    middle["kinds"] = json!([kind("middle-kind", "Middle Folder")]);
    stored_archive(path, "middle-reader", middle);
    let mut absent = bare("absent-owner");
    absent["reads"] = json!(["never-installed"]);
    absent["kinds"] = json!([kind("unavailable-kind", "Unavailable Folder")]);
    stored_archive(path, "absent-owner", absent);

    let later = legacy_notes("later-owner", "later-kind", "Later Folder");
    let later = package_store::Package {
        manifest: later.manifest.clone(),
        worker: None,
    };
    package_store::commit_v2(
        path,
        "later-owner",
        &later.archive().unwrap(),
        &later.checksum().unwrap(),
        |_| Ok(()),
    )
    .unwrap();

    let catalog = start_catalog(path);
    assert!(catalog.v2_manifest("first-reader").is_some());
    for id in ["last-reader", "middle-reader", "absent-owner"] {
        assert!(catalog.v2_manifest(id).is_none(), "{id}");
    }
    for (name, expected) in [
        (
            "middle-kind",
            "unresolved read by unavailable package last-reader",
        ),
        (
            "unavailable-kind",
            "reserved by unavailable package absent-owner",
        ),
    ] {
        assert!(catalog.v2_kind(name).is_none());
        let mut candidate = bare("candidate");
        candidate["kinds"] = json!([kind(name, "Candidate Folder")]);
        refuse(&catalog, candidate, expected);
    }
}
