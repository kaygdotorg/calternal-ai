use super::*;
use crate::db::Db;
use crate::test_support::TempDir;

#[test]
fn usernames() {
    assert_eq!(username_for("K Gopal Krishna"), "k-gopal-krishna");
    assert_eq!(username_for("  kayg!! "), "kayg");
    assert_eq!(username_for("स्वेता"), "owner");
    assert_eq!(username_for(&"a".repeat(100)).len(), 32);
}

#[test]
fn names() {
    assert_eq!(clean_person_name("  kayg  "), Some("kayg".into()));
    assert_eq!(clean_person_name("   "), None);
    assert_eq!(clean_person_name(&"x".repeat(81)), None);
}

#[test]
fn names_reject_invisible_and_normalize_nfc() {
    for name in [
        "ka\u{202e}yg",
        "ka\u{200d}yg",
        "ka\u{200b}yg",
        "ka\u{00ad}yg",
        "ka\n yg",
        "ka\u{034f}yg",
        "ka\u{fe0f}yg",
    ] {
        assert_eq!(clean_person_name(name), None, "{name:?}");
    }
    assert_eq!(clean_person_name("  Cafe\u{301} "), Some("Café".into()));
}

#[test]
fn recovery_requires_exact_username_or_uuid_never_display_name() {
    let dir = TempDir::new();
    let path = dir.path().join("system/auth.db");
    drop(Db::open(&path).unwrap());
    let mut c = open_conn(&path).unwrap();
    let owner = AccountId::new().to_string();
    let other = AccountId::new().to_string();
    for (id, username) in [(&owner, "owner-login"), (&other, "other-login")] {
        c.execute(
            "INSERT INTO users (id,name,username,created_at) VALUES (?1,'Kay',?2,0)",
            params![id, username],
        )
        .unwrap();
    }
    for selector in ["Kay", "OWNER-LOGIN", " owner-login "] {
        assert_eq!(
            recover_in(&mut c, selector, 1000).unwrap_err().code(),
            ErrorCode::NotFound
        );
    }
    let tokens: i64 = c
        .query_row("SELECT count(*) FROM setup_tokens", [], |r| r.get(0))
        .unwrap();
    assert_eq!(tokens, 0, "refused selectors must not issue any token");
    assert_eq!(find_in(&c, "owner-login").unwrap().0, owner);
    assert_eq!(find_in(&c, &other).unwrap().0, other);
    recover_in(&mut c, &owner, 1000).unwrap();
    // A 32-character hexadecimal username is also valid UUID syntax.
    let hex_username = AccountId::new().as_uuid().simple().to_string();
    c.execute(
        "UPDATE users SET username=?2 WHERE id=?1",
        params![other, hex_username],
    )
    .unwrap();
    assert_eq!(find_in(&c, &hex_username).unwrap().0, other);
    // Refuse legacy data where two accepted identifiers collide.
    c.execute(
        "UPDATE users SET username=?2 WHERE id=?1",
        params![other, owner],
    )
    .unwrap();
    assert_eq!(
        recover_in(&mut c, &owner, 1000).err().unwrap().code(),
        ErrorCode::InvalidInput
    );
    let tokens: i64 = c
        .query_row("SELECT count(*) FROM setup_tokens", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        tokens, 1,
        "ambiguous selectors must not replace or issue tokens"
    );
}

#[test]
fn cli_creation_refuses_admin_name_confusables() {
    let cases: &[(&str, &[&str])] = &[
        ("Kay", &["KAY", "Ｋａｙ", "Kаy", "Kay"]),
        ("Owner", &["0wner"]),
        ("Ivy", &["lvy"]),
        ("Straße", &["STRASSE"]),
        ("Café", &["Cafe\u{301}"]),
    ];
    let mut failures = Vec::new();
    for (admin, impostors) in cases {
        let dir = TempDir::new();
        let path = dir.path().join("system/auth.db");
        drop(Db::open(&path).unwrap());
        let mut c = open_conn(&path).unwrap();
        create_in(&mut c, admin, 1000).unwrap();
        for name in *impostors {
            if create_in(&mut c, name, 1000).is_ok() {
                failures.push(format!("{name:?} impersonates {admin:?}"));
            }
        }
        let count: i64 = c
            .query_row("SELECT count(*) FROM users", [], |r| r.get(0))
            .unwrap();
        if count != 1 {
            failures.push(format!("{admin:?}: expected one user, found {count}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

#[test]
fn recover_needs_a_known_person() {
    let dir = TempDir::new();
    let path = dir.path().join("system/auth.db");
    drop(Db::open(&path).unwrap());
    let mut c = open_conn(&path).unwrap();
    let err = recover_in(&mut c, "nobody", 1000).unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);
    c.execute(
        "INSERT INTO users (id, name, username, created_at) VALUES (?1, 'Kayg', 'kayg', 0)",
        [AccountId::new().to_string()],
    )
    .unwrap();
    let (name, t1) = recover_in(&mut c, "kayg", 1000).unwrap();
    assert_eq!(name, "Kayg");
    let (_, t2) = recover_in(&mut c, "kayg", 1000).unwrap();
    assert_ne!(t1, t2);
    let live: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM setup_tokens WHERE used_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(live, 1, "a new recovery token replaces the old one");
}
