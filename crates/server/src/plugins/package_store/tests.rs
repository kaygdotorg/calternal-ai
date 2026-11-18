use super::*;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

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
    load(root, None, None).unwrap();

    let cache = root.join("installed-plugin-workers");
    let current = cache.join(format!("{id}.2.js"));
    let retained = cache.join(format!("{id}.1.js"));
    let stale = cache.join(format!("{id}.3.js"));
    // Keep the old descriptors open so a replacement cannot reuse their inodes.
    let active_file = fs::File::open(&current).unwrap();
    let retained_file = fs::File::open(&retained).unwrap();
    fs::write(&stale, b"stale").unwrap();
    load(root, None, None).unwrap();
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
    load(root, None, None).unwrap();
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
    load(root, None, None).unwrap();
    assert!(fs::symlink_metadata(&current).unwrap().is_file());
    assert_eq!(fs::read_to_string(&outside).unwrap(), expected);

    fs::remove_file(&current).unwrap();
    fs::hard_link(&outside, &current).unwrap();
    load(root, None, None).unwrap();
    assert_ne!(
        fs::metadata(&current).unwrap().ino(),
        fs::metadata(&outside).unwrap().ino()
    );
    assert_eq!(fs::read_to_string(outside).unwrap(), expected);
}

#[test]
fn provider_registration_migrates_without_changing_installed_package() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("installed-plugins.sqlite3");
    let db = Connection::open(path).unwrap();
    let baseline = calternal_store::migrations::Schema::new(
        "installed-plugins.sqlite3",
        1,
        &[PACKAGE_SCHEMA],
        &[],
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
    calternal_store::migrations::migrate_release(&db, UPGRADE_SCHEMA, true).unwrap();

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
        2
    );
}
