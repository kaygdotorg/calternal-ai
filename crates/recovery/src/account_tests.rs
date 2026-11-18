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
