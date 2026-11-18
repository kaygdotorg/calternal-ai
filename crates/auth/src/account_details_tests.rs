use super::*;

#[test]
fn usernames_are_canonical_and_forged_identifiers_are_rejected() {
    assert_eq!(username("  Tien-Lan  ").unwrap(), "tien-lan");
    for invalid in [
        "xy",
        "-tien",
        "tien--lan",
        "admin",
        "tién",
        "01926fb448e37cfaa5eeece7d4e2fde6",
    ] {
        assert!(username(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn recovery_codes_are_distinct_and_only_hashes_are_stored() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = crate::db::open_conn(&dir.path().join("auth.db")).unwrap();
    let key = crate::invite_key::Key::open(&dir.path().join("invite.key"), || Ok(false)).unwrap();
    let tx = db.transaction().unwrap();
    let user = AccountId::new();
    crate::setup::insert_account(&tx, user, "Owner", "test-owner", 1).unwrap();
    let codes = rotate_codes(&tx, &key, user).unwrap();
    assert_eq!(codes.len(), 10);
    let hashes: Vec<Vec<u8>> = tx
        .prepare("SELECT hash FROM account_recovery_codes")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(hashes.len(), 10);
    for code in &codes {
        assert!(hashes.contains(&key.account_proof("recovery-code", code)));
        assert!(!hashes.contains(&code.as_bytes().to_vec()));
    }
    let replacement = rotate_codes(&tx, &key, user).unwrap();
    assert!(replacement.iter().all(|code| !codes.contains(code)));
    assert_eq!(
        tx.query_row("SELECT count(*) FROM account_recovery_codes", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        10
    );
}

#[test]
fn version_11_accounts_upgrade_without_rewriting_identity() {
    use calternal_store::migrations::{self, Schema};
    let baseline = Schema::new("auth.db", 11, &[include_str!("schema.sql")], &[]);
    let mut db = rusqlite::Connection::open_in_memory().unwrap();
    migrations::migrate(&db, baseline).unwrap();
    let user = AccountId::new();
    let tx = db.transaction().unwrap();
    crate::setup::insert_account(&tx, user, "Existing", "existing", 123).unwrap();
    tx.commit().unwrap();
    db.execute(
        "INSERT INTO passkeys(cred_id,user_id,passkey,created_at) VALUES(x'01',?1,'{}',123)",
        [user.to_string()],
    )
    .unwrap();
    migrations::migrate(&db, crate::db::MIGRATIONS).unwrap();
    assert_eq!(migrations::inspect(&db, crate::db::MIGRATIONS).unwrap(), 13);
    assert!(migrations::inspect(&db, baseline).is_err());
    assert_eq!(
        db.query_row(
            "SELECT username FROM users WHERE id=?1",
            [user.to_string()],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "existing"
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM passkeys WHERE user_id=?1",
            [user.to_string()],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn migration_failure_rolls_back_the_account_schema() {
    use calternal_store::migrations::{self, Schema};
    let baseline = Schema::new("auth.db", 11, &[include_str!("schema.sql")], &[]);
    let db = rusqlite::Connection::open_in_memory().unwrap();
    migrations::migrate(&db, baseline).unwrap();
    db.execute_batch("CREATE TABLE account_email_challenges (sentinel TEXT)")
        .unwrap();
    assert!(migrations::migrate(&db, crate::db::MIGRATIONS).is_err());
    assert_eq!(migrations::inspect(&db, baseline).unwrap(), 11);
    let changes: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='account_details')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!changes);
}
