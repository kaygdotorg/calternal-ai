use super::*;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

#[test]
fn fresh_catalog_uses_operator_distribution_entries() {
    let appdata = tempfile::tempdir().unwrap();
    let distribution = tempfile::tempdir().unwrap();
    let root = distribution.path();
    fs::write(
        root.join("trusted-plugins.json"),
        br#"{"plugins":[{"id":"calternal-notes","version":"0.1.0","manifest":"notes.json","worker":null,"worker_sha256":null,"default_enabled":true}]}"#,
    )
    .unwrap();
    fs::write(
        root.join("notes.json"),
        include_bytes!("../../../../../plugins/calternal-notes/manifest.json"),
    )
    .unwrap();
    let registry = load(appdata.path(), Some(root), None, false).unwrap();
    let catalog = registry.snapshot();
    assert_eq!(catalog.plugins().len(), 1);
    assert!(
        catalog
            .get("calternal-notes")
            .unwrap()
            .entry
            .default_enabled
    );
}

#[test]
fn installed_package_v2_opts_in_only_the_original_seed_and_preserves_rollback() {
    use calternal_store::migrations::{Schema, inspect, migrate_release};

    let v1 = Schema::new("installed-plugins.sqlite3", 1, &[PACKAGE_SCHEMA], &[]);
    let v2 = Schema::new(
        "installed-plugins.sqlite3",
        1,
        &[PACKAGE_SCHEMA],
        &[bundled_notes::LIVE_EDITING],
    );
    let seeded = include_bytes!("notes_before_live_editing.calplugin").as_slice();
    let mut custom = decode(seeded.to_vec(), super::super::sha256_hex(seeded)).unwrap();
    let id = custom.manifest.id.clone();
    custom.manifest.name = "Personal Notes".into();
    let custom = custom.archive().unwrap();
    for (original, approval, live) in [(seeded, 0, true), (seeded, 9, false), (&custom, 0, false)] {
        let db = Connection::open_in_memory().unwrap();
        migrate_release(&db, v1, false).unwrap();
        let checksum = super::super::sha256_hex(original);
        db.execute(
            "INSERT INTO installed_plugins VALUES (2,?1,?2,?3,0,?4)",
            params![id, original, checksum, approval],
        )
        .unwrap();
        db.execute(
            "INSERT INTO retained_plugins
             SELECT 1,id,archive,checksum,default_enabled,approval FROM installed_plugins",
            [],
        )
        .unwrap();
        migrate_release(&db, v2, true).unwrap();
        assert_eq!(inspect(&db, v2).unwrap(), 2);
        assert_eq!(inspect(&db, v1).unwrap(), 2);
        let (archive, checksum) = db
            .query_row(
                "SELECT archive,checksum FROM installed_plugins
                 WHERE revision=2 AND default_enabled=0 AND approval=?1",
                [approval],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap();
        let package = decode(archive.clone(), checksum).unwrap();
        assert_eq!(package.manifest.kinds[0].live_editing(), live);
        assert_eq!(archive == original, !live);
        let retained: Vec<u8> = db
            .query_row("SELECT archive FROM retained_plugins", [], |row| row.get(0))
            .unwrap();
        assert_eq!(retained, original);
    }
}

#[test]
fn startup_reuses_verified_current_and_retained_workers_and_prunes_stale_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let package = Package {
        manifest: host::parse_manifest(include_bytes!(
            "../../../../../plugins/calternal-notes-insights/manifest.json"
        ))
        .unwrap(),
        worker: Some("export default {};".into()),
    };
    let id = &package.manifest.id;
    let archive = package.archive().unwrap();
    let checksum = package.checksum().unwrap();
    let db = open(root).unwrap();
    db.execute("INSERT INTO plugin_seed VALUES (1)", [])
        .unwrap();
    db.execute(
        "INSERT INTO installed_plugins VALUES (2, ?1, ?2, ?3, 1, 2)",
        params![id, archive, checksum],
    )
    .unwrap();
    db.execute(
        "INSERT INTO retained_plugins VALUES (1, ?1, ?2, ?3, 1, 1)",
        params![id, archive, checksum],
    )
    .unwrap();
    drop(db);
    load(root, None, None, false).unwrap();

    let cache = root.join("installed-plugin-workers");
    let current = cache.join(format!("{id}.2.js"));
    let retained = cache.join(format!("{id}.1.js"));
    let stale = cache.join(format!("{id}.3.js"));
    // Keep the old descriptors open so a replacement cannot reuse their inodes.
    let active_file = fs::File::open(&current).unwrap();
    let retained_file = fs::File::open(&retained).unwrap();
    fs::write(&stale, b"stale").unwrap();
    load(root, None, None, false).unwrap();
    assert_eq!(
        fs::metadata(&current).unwrap().ino(),
        active_file.metadata().unwrap().ino()
    );
    assert_eq!(
        fs::metadata(&retained).unwrap().ino(),
        retained_file.metadata().unwrap().ino()
    );
    assert!(!stale.exists());

    let expected = package.worker.unwrap();
    fs::set_permissions(&current, fs::Permissions::from_mode(0o644)).unwrap();
    fs::write(&retained, b"corrupt cached worker").unwrap();
    load(root, None, None, false).unwrap();
    for path in [&current, &retained] {
        assert_eq!(fs::read_to_string(path).unwrap(), expected);
        assert_eq!(fs::metadata(path).unwrap().mode() & 0o7777, 0o600);
    }
    assert_ne!(
        fs::metadata(&current).unwrap().ino(),
        active_file.metadata().unwrap().ino()
    );

    let outside = root.join("outside.js");
    fs::write(&outside, expected.as_bytes()).unwrap();
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).unwrap();
    fs::remove_file(&current).unwrap();
    symlink(&outside, &current).unwrap();
    load(root, None, None, false).unwrap();
    assert!(fs::symlink_metadata(&current).unwrap().is_file());
    assert_eq!(fs::read_to_string(&outside).unwrap(), expected);

    fs::remove_file(&current).unwrap();
    fs::hard_link(&outside, &current).unwrap();
    load(root, None, None, false).unwrap();
    assert_ne!(
        fs::metadata(&current).unwrap().ino(),
        fs::metadata(&outside).unwrap().ino()
    );
    assert_eq!(fs::read_to_string(outside).unwrap(), expected);
}

#[test]
fn installed_package_v3_registers_the_provider_without_changing_its_archive() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("installed-plugins.sqlite3");
    let db = Connection::open(path).unwrap();
    let baseline = calternal_store::migrations::Schema::new(
        "installed-plugins.sqlite3",
        1,
        &[PACKAGE_SCHEMA],
        &[bundled_notes::LIVE_EDITING],
    );
    calternal_store::migrations::migrate(&db, baseline).unwrap();
    let package = Package {
        manifest: host::parse_manifest(include_bytes!(
            "../../../../../plugins/calternal-claude-code/manifest.json"
        ))
        .unwrap(),
        worker: None,
    };
    let archive = package.archive().unwrap();
    let checksum = package.checksum().unwrap();
    db.execute(
        "INSERT INTO installed_plugins VALUES (7, ?1, ?2, ?3, 1, 4)",
        params![package.manifest.id, &archive, &checksum],
    )
    .unwrap();
    db.execute("INSERT INTO plugin_seed VALUES (1)", [])
        .unwrap();
    calternal_store::migrations::migrate_release(&db, UPGRADE_SCHEMA, true).unwrap();
    let distribution = tempfile::tempdir().unwrap();
    fs::write(distribution.path().join("trusted-plugins.json"), b"invalid").unwrap();
    assert!(
        load(dir.path(), Some(distribution.path()), None, false)
            .unwrap()
            .snapshot()
            .get(&package.manifest.id)
            .is_some()
    );

    let registration: (String, String) = db
        .query_row(
            "SELECT id, login_command FROM provider_runtime",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        registration,
        (package.manifest.id, "claude auth login".into())
    );
    let policy = provider_runtime::SignInPolicy::read(dir.path(), &registration.0)
        .unwrap()
        .unwrap();
    assert!(policy.allows("https://claude.ai/oauth/authorize?state=fixture", true));
    assert!(!policy.allows("https://claude.ai/oauth/authorize?state=fixture", false));
    assert!(!policy.allows("http://claude.ai/oauth/authorize", true));
    assert!(!policy.allows("https://claude.ai.example.org/oauth/authorize", true));
    assert!(!policy.allows("https://claude.ai/oauth/authorize/other", true));

    let installed: (i64, Vec<u8>, String, bool, i64) = db
        .query_row(
            "SELECT revision, archive, checksum, default_enabled, approval FROM installed_plugins",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(installed, (7, archive, checksum, true, 4));
    assert_eq!(
        calternal_store::migrations::inspect(&db, baseline).unwrap(),
        3
    );
}
