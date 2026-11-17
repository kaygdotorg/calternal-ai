//! An old-schema copy with a real passkey and session remains usable.
mod common;
use calternal_types::AccountId;
use common::*;

#[tokio::test]
async fn schema_two_copy_keeps_real_credentials_sessions_and_recovery() {
    let source = app();
    let (signed, mut device) = first_owner(&source, "Owner").await;
    let owner = signed.0;
    let member = source.auth.create("Member 雪").await.unwrap();
    let (name, recovery) = source.auth.recover("owner").await.unwrap();
    assert_eq!(name, "Owner");
    let copy = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(copy.path().join("system")).unwrap();
    let path = copy.path().join("system/auth.db");
    let db = rusqlite::Connection::open(source.dir.path().join("system/auth.db")).unwrap();
    db.execute("VACUUM INTO ?1", [path.to_str().unwrap()])
        .unwrap();
    let fixture = rusqlite::Connection::open(&path).unwrap();
    // Build the existing v2 schema with real rows from a software passkey
    // enrollment. No migration code runs on the source or on a real server.
    remove_v11_sharing_schema(&fixture).unwrap();
    fixture
        .execute_batch(
            "PRAGMA foreign_keys=OFF;
        DROP TRIGGER account_identity_separate; DROP TABLE space_receipts; DROP TABLE development_directory;
        DROP TABLE space_memberships; DROP TABLE spaces; DROP TABLE space_role_capabilities; DROP TABLE space_roles;
        DROP TRIGGER installation_scopes_insert; DROP TRIGGER installation_scopes_update; ALTER TABLE installations DROP COLUMN registration_session; ALTER TABLE installations DROP COLUMN assertion_at; ALTER TABLE installations DROP COLUMN scopes;
        DROP TRIGGER first_person_admin;
        DROP TRIGGER user_role_insert;
        DROP TRIGGER user_role_update;
        DROP TRIGGER role_in_use;
        DROP TABLE invite_groups;
        DROP TABLE invites;
        DROP TABLE group_members;
        DROP TABLE groups;
        ALTER TABLE users DROP COLUMN quota_bytes;
        ALTER TABLE users DROP COLUMN role;
        DROP TABLE role_capabilities;
        DROP TABLE roles;
        PRAGMA user_version=2;",
        )
        .unwrap();
    let credential: String = fixture
        .query_row("SELECT passkey FROM passkeys", [], |r| r.get(0))
        .unwrap();
    let session: Vec<u8> = fixture
        .query_row("SELECT token_hash FROM installations", [], |r| r.get(0))
        .unwrap();
    drop(fixture);
    let restored = app_in(copy, |_| {});
    let fixture = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        fixture
            .query_row("SELECT passkey FROM passkeys", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        credential
    );
    assert_eq!(
        fixture
            .query_row("SELECT token_hash FROM installations", [], |r| r
                .get::<_, Vec<u8>>(0))
            .unwrap(),
        session
    );
    assert_eq!(
        restored.auth.identify(&signed.2).await,
        Some((owner, signed.1))
    );
    assert_eq!(login(&restored, owner, &mut device).await.unwrap().0, owner);
    assert!(
        restored
            .auth
            .start_setup(&recovery, "", "local")
            .await
            .is_ok()
    );
    assert!(
        restored
            .auth
            .start_setup(&member.token, "", "local")
            .await
            .is_ok()
    );
    // people.list is outside the library; inspect exactly the persisted rows
    // its source assertions tested, retaining role/quota migration coverage.
    let mut st = fixture
        .prepare("SELECT id,role,quota_bytes FROM users")
        .unwrap();
    let rows: Vec<(String, String, Option<i64>)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .any(|p| p.0 == owner.to_string() && p.1 == "admin")
    );
    assert!(
        rows.iter()
            .any(|p| p.0 == member.id.to_string() && p.1 == "member" && p.2.is_none())
    );
    let version: i64 = fixture
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 16);
    assert_eq!(
        fixture
            .query_row(
                "SELECT count(*) FROM sqlite_master
                 WHERE type='table' AND name='sharing_reset_cursors'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
    assert_eq!(
        fixture
            .query_row(
                "SELECT count(*) FROM sqlite_master
                 WHERE type='table' AND name='share_fanout_deliveries'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn origin_dev_schema_migrates_idempotently_and_rolls_back_partial_failure() {
    let source = app();
    let (_signed, mut device) = first_owner(&source, "Existing").await;
    let (uid, cred_id, credential): (String, Vec<u8>, String) = db(&source)
        .query_row("SELECT user_id,cred_id,passkey FROM passkeys", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("system")).unwrap();
    let path = d.path().join("system/auth.db");
    let c = rusqlite::Connection::open(&path).unwrap();
    c.execute_batch(include_str!("fixtures/auth-v2.sql"))
        .unwrap();
    c.execute(
        "INSERT INTO users VALUES (?1,'Existing','existing',1)",
        [&uid],
    )
    .unwrap();
    c.execute(
        "INSERT INTO users VALUES (?1,'Second','second',2)",
        [AccountId::new().to_string()],
    )
    .unwrap();
    c.execute(
        "INSERT INTO passkeys VALUES (?1,?2,?3,1,NULL,'Phone')",
        rusqlite::params![cred_id, uid, credential],
    )
    .unwrap();
    c.execute("CREATE TABLE groups (collision TEXT)", [])
        .unwrap();
    let open =
        || calternal_auth::Auth::open(d.path(), calternal_auth::AuthConfig::new(ORIGIN).unwrap());
    assert!(open().is_err());
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='roles'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    c.execute("DROP TABLE groups", []).unwrap();
    drop(open().unwrap());
    drop(open().unwrap());
    assert_eq!(
        c.query_row("SELECT role FROM users WHERE id=?1", [&uid], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "admin"
    );
    assert_eq!(
        c.query_row("SELECT passkey FROM passkeys", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        credential
    );
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        16
    );
    drop(c);
    let restored = app_in(d, |_| {});
    assert_eq!(
        login(&restored, AccountId::parse(&uid).unwrap(), &mut device)
            .await
            .unwrap()
            .0
            .to_string(),
        uid
    );
}
