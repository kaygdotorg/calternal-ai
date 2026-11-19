use super::*;

#[test]
fn account_security_authority_is_published_and_erased() {
    let f = Fixture::new();
    let user = f.member.to_string();
    let c = f.connection();
    c.execute(
        "INSERT INTO account_details(user_id,email,verified_at) VALUES(?1,'member@example.test',10)",
        [&user],
    ).unwrap();
    c.execute(
        "INSERT INTO account_recovery_codes VALUES(zeroblob(32),?1)",
        [&user],
    )
    .unwrap();
    c.execute("INSERT INTO account_security_events VALUES('security-event',?1,'code','started','account',11)",
        [&user]).unwrap();
    c.execute("INSERT INTO oidc_providers VALUES('test','https://issuer.example.test','client','{}',x'01','r1',1)", []).unwrap();
    c.execute(
        "INSERT INTO oidc_links VALUES('test','provider-subject',?1)",
        [&user],
    )
    .unwrap();

    let baseline = f.baseline();
    for table in [
        "account_details",
        "account_recovery_codes",
        "account_security_events",
        "oidc_links",
        "oidc_providers",
    ] {
        assert_eq!(baseline.tables[table].rows.len(), 1, "{table}");
    }

    let coordinator = f.coordinator();
    let mut c = f.connection();
    coordinator.reconcile(&c).unwrap();
    let tx = c.transaction().unwrap();
    tx.execute("DELETE FROM space_memberships WHERE account_id=?1", [&user])
        .unwrap();
    tx.execute("DELETE FROM users WHERE id=?1", [user]).unwrap();
    let change = coordinator.prepare(&tx).unwrap();
    tx.commit().unwrap();
    coordinator.complete(&c, &change).unwrap();
    coordinator.validate_against_witness(&c).unwrap();
    let erased = coordinator.validate_and_read().unwrap();
    for table in [
        "account_details",
        "account_recovery_codes",
        "account_security_events",
        "oidc_links",
    ] {
        assert!(erased.tables[table].rows.is_empty(), "{table}");
    }
    assert_eq!(erased.tables["oidc_providers"].rows.len(), 1);
}

#[test]
fn protected_upsert_publishes_when_the_owner_is_already_dirty() {
    let f = Fixture::new();
    f.baseline();
    let coordinator = f.coordinator();
    let mut c = f.connection();
    coordinator.reconcile(&c).unwrap();
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    let user = f.member.to_string();
    tx.execute(
        "INSERT INTO account_details(user_id,email,verified_at) VALUES(?1,'old@example.test',10)",
        [&user],
    )
    .unwrap();

    assert_eq!(
        tx.execute(
            "INSERT INTO account_details(user_id,email,verified_at) VALUES(?1,'new@example.test',20)
             ON CONFLICT(user_id) DO UPDATE SET email=excluded.email,verified_at=excluded.verified_at",
            [&user],
        )
        .unwrap(),
        1
    );
    let change = coordinator.prepare(&tx).unwrap();
    tx.commit().unwrap();
    coordinator.complete(&c, &change).unwrap();
    coordinator.validate_against_witness(&c).unwrap();
    let snapshot = coordinator.validate_and_read().unwrap();
    let rows = &snapshot.tables["account_details"].rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], Cell::Text(user));
    assert_eq!(rows[0][1], Cell::Text("new@example.test".into()));
}

#[test]
fn registered_future_auth_rows_survive_old_writes_compaction_and_witness_replay() {
    let f = Fixture::new();
    f.baseline();
    let mut c = f.connection();
    c.execute_batch(
        "CREATE TABLE future_preferences(
           user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
           preference TEXT NOT NULL) STRICT;
         INSERT INTO security_recovery_inventory VALUES('future_preferences','1','user_id');
         ALTER TABLE users ADD COLUMN future_label TEXT;",
    )
    .unwrap();
    let user = f.member.to_string();
    c.execute("INSERT INTO future_preferences VALUES(?1,'keep')", [&user])
        .unwrap();
    c.execute(
        "UPDATE users SET future_label='new value' WHERE id=?1",
        [&user],
    )
    .unwrap();
    f.coordinator().refresh_after_migration(&c).unwrap();

    let older = f.coordinator();
    older.reconcile(&c).unwrap();
    let tx = c.transaction().unwrap();
    tx.execute("UPDATE users SET name='old writer' WHERE id=?1", [&user])
        .unwrap();
    let proposal = older.prepare(&tx).unwrap();
    tx.commit().unwrap();
    // Simulate a restart before publishing the committed resolution.
    drop(older);
    let older = f.coordinator();
    older.reconcile(&c).unwrap();
    older.complete(&c, &proposal).unwrap();
    older.compact_erasure(&c).unwrap();
    older.validate_against_witness(&c).unwrap();
    let snapshot = older
        .validate_and_read_at(&older.checkpoint().unwrap())
        .unwrap();
    assert_eq!(
        snapshot.tables["future_preferences"].rows,
        vec![vec![Cell::Text(user.clone()), Cell::Text("keep".into())]]
    );
    let users = &snapshot.tables["users"];
    let future = users
        .columns
        .iter()
        .position(|name| name == "future_label")
        .unwrap();
    assert!(
        users
            .rows
            .iter()
            .any(|row| row[future] == Cell::Text("new value".into()))
    );

    let tx = c.transaction().unwrap();
    tx.execute("DELETE FROM space_memberships WHERE account_id=?1", [&user])
        .unwrap();
    tx.execute("DELETE FROM users WHERE id=?1", [&user])
        .unwrap();
    let proposal = older.prepare(&tx).unwrap();
    tx.commit().unwrap();
    older.complete(&c, &proposal).unwrap();
    older.compact_erasure(&c).unwrap();
    older.validate_against_witness(&c).unwrap();
    assert!(
        older.validate_and_read().unwrap().tables["future_preferences"]
            .rows
            .is_empty()
    );
}

#[test]
fn unregistered_auth_table_still_refuses_startup() {
    let f = Fixture::new();
    f.baseline();
    let c = f.connection();
    c.execute_batch("CREATE TABLE unclassified (id TEXT PRIMARY KEY) STRICT;")
        .unwrap();
    assert!(
        f.coordinator()
            .reconcile(&c)
            .unwrap_err()
            .to_string()
            .contains("unclassified")
    );
}

#[test]
fn registered_owner_table_requires_erasure_cascade() {
    let f = Fixture::new();
    f.baseline();
    let c = f.connection();
    c.execute_batch(
        "CREATE TABLE future_preferences(user_id TEXT PRIMARY KEY REFERENCES users(id)) STRICT;
         INSERT INTO security_recovery_inventory VALUES('future_preferences','1','user_id');",
    )
    .unwrap();
    assert!(
        f.coordinator()
            .reconcile(&c)
            .unwrap_err()
            .to_string()
            .contains("owner deletion cascade")
    );
}
