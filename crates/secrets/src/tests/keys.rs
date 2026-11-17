use super::*;

#[test]
fn owner_keys_are_independent_even_for_the_same_aead_binding() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let (a, b) = (account(), account());
    let me = actor(AccountId::new());
    for o in [a, b] {
        store
            .put(&o, &name("k"), "K", value(b"value"), &me)
            .unwrap();
    }
    let ca = rusqlite::Connection::open(db(dir.path(), &a)).unwrap();
    let cb = rusqlite::Connection::open(db(dir.path(), &b)).unwrap();
    let ka =
        crate::key::MasterKey::open(db(dir.path(), &a).parent().unwrap(), &ca, false, &a, None)
            .unwrap();
    let kb =
        crate::key::MasterKey::open(db(dir.path(), &b).parent().unwrap(), &cb, false, &b, None)
            .unwrap();
    let stored_b: Vec<u8> = cb
        .query_row("SELECT sealed FROM secrets WHERE name='k'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        ka.open_value(&b, "k", 1, &stored_b).unwrap_err(),
        Error::NotFound
    );
    let sealed = ka.seal(&b, "k", 1, b"value").unwrap();
    assert_eq!(
        kb.open_value(&b, "k", 1, &sealed).unwrap_err(),
        Error::NotFound
    );
    assert!(!dir.path().join("system/secret.key").exists());
}

#[test]
fn erased_key_cannot_open_an_old_database_copy_or_a_cached_handle() {
    let dir = tempfile::tempdir().unwrap();
    let owner = account();
    let store = LocalStore::open(dir.path()).unwrap();
    let h = store
        .put(
            &owner,
            &name("k"),
            "K",
            value(b"value"),
            &actor(AccountId::new()),
        )
        .unwrap();
    let backup = dir.path().join("old.db");
    std::fs::copy(db(dir.path(), &owner), &backup).unwrap();
    store.erase_owner(&owner).unwrap();
    store.erase_owner(&owner).unwrap();
    assert_eq!(used(&store, &h).unwrap_err(), Error::NotFound);
    std::fs::copy(&backup, db(dir.path(), &owner)).unwrap();
    assert_eq!(used(&store, &h).unwrap_err(), Error::KeyMissing);
    assert_eq!(
        store
            .put(
                &owner,
                &name("new"),
                "K",
                value(b"replacement"),
                &actor(AccountId::new())
            )
            .unwrap_err(),
        Error::KeyMissing
    );
}

#[test]
fn creation_and_erasure_crashes_never_release_or_replace_a_key() {
    for point in ["key-published", "key-recorded"] {
        let dir = tempfile::tempdir().unwrap();
        let owner = account();
        crash(dir.path(), &owner, point);
        let path = db(dir.path(), &owner).with_file_name("secret.key");
        let original = std::fs::read(&path).unwrap();
        let store = LocalStore::open(dir.path()).unwrap();
        store
            .put(
                &owner,
                &name("new"),
                "N",
                value(b"after"),
                &actor(AccountId::new()),
            )
            .unwrap();
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
    for point in ["erase-recorded", "erase-unlinked"] {
        let dir = tempfile::tempdir().unwrap();
        let owner = account();
        let store = LocalStore::open(dir.path()).unwrap();
        let h = store
            .put(
                &owner,
                &name("k"),
                "K",
                value(b"before"),
                &actor(AccountId::new()),
            )
            .unwrap();
        crash(dir.path(), &owner, point);
        // With no host log, pre-unlink interruption has no committed erasure;
        // post-unlink interruption refuses missing key. Retrying finishes both.
        if point == "erase-unlinked" {
            assert_eq!(used(&store, &h).unwrap_err(), Error::KeyMissing);
        }
        store.erase_owner(&owner).unwrap();
        assert!(used(&store, &h).is_err());
    }
}

#[test]
fn requests_expire_after_fifteen_minutes_and_can_be_asked_again() {
    let dir = tempfile::tempdir().unwrap();
    let owner = account();
    let store = LocalStore::open(dir.path()).unwrap();
    let actor = actor(AccountId::new());
    let id = store
        .request(&owner, &name("k"), "K", "reason", &actor)
        .unwrap();
    let c = rusqlite::Connection::open(db(dir.path(), &owner)).unwrap();
    c.execute(
        "UPDATE requests SET created_at=?1",
        [calternal_types::now_ms() - 15 * 60 * 1000],
    )
    .unwrap();
    assert!(store.pending(&owner).unwrap().is_empty());
    assert_eq!(
        store
            .provide(&owner, &id, value(b"late"), &actor)
            .unwrap_err(),
        Error::NotFound
    );
    let next = store
        .request(&owner, &name("k"), "K", "reason", &actor)
        .unwrap();
    assert_ne!(id, next);
    assert_eq!(store.pending(&owner).unwrap().len(), 1);
}

#[test]
fn refused_lifecycle_publication_never_writes_ciphertext_or_unlinks_a_live_key() {
    struct Refuse;
    impl KeyLifecycle for Refuse {
        fn record(&self, _: &Owner, _: &[u8; 32], _: bool) -> Result<()> {
            Err(Error::Internal)
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let owner = account();
    let actor = actor(AccountId::new());
    let store = LocalStore::open(dir.path()).unwrap();
    store
        .configure_lifecycle(std::sync::Arc::new(Refuse))
        .unwrap();
    assert_eq!(
        store
            .put(&owner, &name("k"), "K", value(b"secret"), &actor)
            .unwrap_err(),
        Error::Internal
    );
    let c = rusqlite::Connection::open(db(dir.path(), &owner)).unwrap();
    assert_eq!(
        c.query_row("SELECT count(*) FROM secrets", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    // Complete creation with a standalone store, then attach a refusing barrier.
    let standalone = LocalStore::open(dir.path()).unwrap();
    standalone
        .put(&owner, &name("k"), "K", value(b"secret"), &actor)
        .unwrap();
    let before = std::fs::read(db(dir.path(), &owner).with_file_name("secret.key")).unwrap();
    assert_eq!(store.erase_owner(&owner), Err(Error::Internal));
    assert_eq!(
        std::fs::read(db(dir.path(), &owner).with_file_name("secret.key")).unwrap(),
        before
    );
}

#[test]
fn deletion_finishes_an_interrupted_first_key_creation() {
    let dir = tempfile::tempdir().unwrap();
    let owner = account();
    crash(dir.path(), &owner, "key-published");
    let store = LocalStore::open(dir.path()).unwrap();
    store.erase_owner(&owner).unwrap();
    assert!(!db(dir.path(), &owner).with_file_name("secret.key").exists());
    assert_eq!(
        store
            .put(
                &owner,
                &name("new"),
                "N",
                value(b"new"),
                &actor(AccountId::new())
            )
            .unwrap_err(),
        Error::NotFound
    );
}
