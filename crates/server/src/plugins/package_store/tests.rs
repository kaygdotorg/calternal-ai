use super::*;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

#[test]
fn installed_package_v2_opts_in_only_the_original_seed_and_preserves_rollback() {
    use calternal_store::migrations::{Schema, inspect, migrate_release};

    let v1 = Schema::new("installed-plugins.sqlite3", 1, &[PACKAGE_SCHEMA], &[]);
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
        migrate_release(&db, UPGRADE_SCHEMA, true).unwrap();
        assert_eq!(inspect(&db, UPGRADE_SCHEMA).unwrap(), 2);
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
