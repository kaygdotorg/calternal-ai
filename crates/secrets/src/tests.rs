use crate::*;
use calternal_types::{AccountId, Actor, SpaceId, Via};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const CANARY: &[u8] = b"sk-canary-7f3a9c1e-never-on-disk";

fn value(v: &[u8]) -> SecretValue {
    SecretValue::new(v.to_vec()).unwrap()
}
fn name(n: &str) -> SecretName {
    SecretName::new(n).unwrap()
}
fn actor(account: AccountId) -> Actor {
    Actor::account(account)
}
fn env() -> Consumer {
    Consumer::SandboxEnv {
        thread: uuid::Uuid::now_v7(),
        variable: "OPENAI_API_KEY".into(),
    }
}
fn account() -> Owner {
    Owner::Account(AccountId::new())
}
fn used(store: &LocalStore, handle: &Handle) -> Result<Vec<u8>> {
    let me = actor(AccountId::new());
    store
        .use_value(handle, &me, &env())
        .map(|v| v.expose().to_vec())
}
fn db(dir: &Path, owner: &Owner) -> std::path::PathBuf {
    let kind = if matches!(owner, Owner::Account(_)) {
        "accounts"
    } else {
        "spaces"
    };
    dir.join(kind)
        .join(owner.uuid().to_string())
        .join("secrets.db")
}

#[test]
fn put_use_round_trip_audits_without_value() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let owner = account();
    let me = actor(AccountId::new());
    let handle = store
        .put(&owner, &name("openai"), "OpenAI key", value(CANARY), &me)
        .unwrap();
    assert_eq!(handle.version, 1);
    let token = store.token(&handle).unwrap();
    assert!(token.as_str().starts_with("sh-") && token.as_str().len() == 43);
    assert_eq!(
        store
            .resolve(&owner, &HandleToken::parse(token.as_str()).unwrap())
            .unwrap(),
        handle
    );
    assert_eq!(used(&store, &handle).unwrap(), CANARY);
    let audit = store.audit(&owner, 10).unwrap();
    assert_eq!(audit[0].action, Action::Use);
    assert!(
        audit[0]
            .consumer
            .as_deref()
            .unwrap()
            .starts_with("env OPENAI_API_KEY in thread ")
    );
    assert_eq!(audit[1].action, Action::Put);
    assert_eq!(
        store
            .put(&owner, &name("openai"), "x", value(b"y"), &me)
            .unwrap_err(),
        Error::Conflict
    );
}

#[test]
fn rotate_destroys_old_version_and_its_handle() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let owner = account();
    let me = actor(AccountId::new());
    let old = store
        .put(&owner, &name("k"), "K", value(b"one"), &me)
        .unwrap();
    let old_token = store.token(&old).unwrap();
    let new = store
        .rotate(&owner, &name("k"), value(b"two"), &me)
        .unwrap();
    assert_eq!(new.version, 2);
    assert_eq!(used(&store, &old).unwrap_err(), Error::NotFound);
    assert_eq!(
        store.resolve(&owner, &old_token).unwrap_err(),
        Error::NotFound
    );
    assert_eq!(used(&store, &new).unwrap(), b"two");
    assert_eq!(store.handle(&owner, &name("k")).unwrap(), new);
    assert_eq!(
        store
            .rotate(&owner, &name("absent"), value(b"x"), &me)
            .unwrap_err(),
        Error::NotFound
    );
}

#[test]
fn wrong_owner_and_copied_rows_fail() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let (a, b) = (account(), account());
    let me = actor(AccountId::new());
    let ha = store
        .put(&a, &name("k"), "K", value(b"alpha"), &me)
        .unwrap();
    store
        .put(&b, &name("other"), "O", value(b"beta"), &me)
        .unwrap();
    // A's handle presented as B's: no such row, and the token does not resolve.
    let forged = Handle {
        owner: b,
        ..ha.clone()
    };
    assert_eq!(used(&store, &forged).unwrap_err(), Error::NotFound);
    assert_eq!(
        store.resolve(&b, &store.token(&ha).unwrap()).unwrap_err(),
        Error::NotFound
    );
    // Copy A's whole database over B's: the token MAC and the AEAD binding refuse it.
    std::fs::copy(db(dir.path(), &a), db(dir.path(), &b)).unwrap();
    assert_eq!(
        store.resolve(&b, &store.token(&ha).unwrap()).unwrap_err(),
        Error::KeyUnusable
    );
    assert_eq!(used(&store, &forged).unwrap_err(), Error::KeyUnusable);
    // A row renamed inside its own owner fails too (name is bound).
    let c = rusqlite::Connection::open(db(dir.path(), &a)).unwrap();
    c.execute("UPDATE secrets SET name='renamed'", []).unwrap();
    let renamed = Handle {
        owner: a,
        name: name("renamed"),
        version: 1,
    };
    assert_eq!(used(&store, &renamed).unwrap_err(), Error::NotFound);
}

#[test]
fn tampered_ciphertext_is_refused_and_not_audited_as_use() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let owner = Owner::Space(SpaceId::new());
    let me = actor(AccountId::new());
    let h = store
        .put(&owner, &name("k"), "K", value(b"payload"), &me)
        .unwrap();
    let c = rusqlite::Connection::open(db(dir.path(), &owner)).unwrap();
    for offset in [0usize, 13, 30] {
        let mut sealed: Vec<u8> = c
            .query_row("SELECT sealed FROM secrets", [], |r| r.get(0))
            .unwrap();
        let original = sealed.clone();
        sealed[offset] ^= 1;
        c.execute("UPDATE secrets SET sealed=?1", [&sealed])
            .unwrap();
        assert_eq!(used(&store, &h).unwrap_err(), Error::NotFound);
        c.execute("UPDATE secrets SET sealed=?1", [&original])
            .unwrap();
    }
    c.execute("UPDATE secrets SET sealed=x'0102'", []).unwrap();
    assert_eq!(used(&store, &h).unwrap_err(), Error::NotFound);
    assert!(
        store
            .audit(&owner, 10)
            .unwrap()
            .iter()
            .all(|e| e.action != Action::Use)
    );
}

#[test]
fn missing_or_wrong_key_is_refused_once_secrets_exist() {
    let dir = tempfile::tempdir().unwrap();
    let owner = account();
    let key = dir
        .path()
        .join("accounts")
        .join(owner.uuid().to_string())
        .join("secret.key");
    let store = LocalStore::open(dir.path()).unwrap();
    store
        .put(
            &owner,
            &name("k"),
            "K",
            value(b"v"),
            &actor(AccountId::new()),
        )
        .unwrap();
    assert_eq!(
        std::fs::metadata(&key).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let saved = std::fs::read(&key).unwrap();
    std::fs::remove_file(&key).unwrap();
    assert_eq!(store.list(&owner).err(), Some(Error::KeyMissing));
    assert!(
        !key.exists(),
        "a refused open must not mint a replacement key"
    );
    // A different key of the right shape is refused by fingerprint.
    std::fs::write(&key, [7u8; 32]).unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(store.list(&owner).err(), Some(Error::KeyUnusable));
    std::fs::write(&key, &saved).unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(store.list(&owner).err(), Some(Error::KeyUnusable));
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(store.list(&owner).is_ok());
}

#[test]
fn accounts_and_spaces_are_isolated() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let me = actor(AccountId::new());
    let owners = [
        account(),
        account(),
        Owner::Space(SpaceId::new()),
        Owner::Space(SpaceId::new()),
    ];
    for (i, owner) in owners.iter().enumerate() {
        store
            .put(
                owner,
                &name("shared-name"),
                "same name",
                value(format!("value-{i}").as_bytes()),
                &me,
            )
            .unwrap();
    }
    for (i, owner) in owners.iter().enumerate() {
        let h = store.handle(owner, &name("shared-name")).unwrap();
        assert_eq!(used(&store, &h).unwrap(), format!("value-{i}").as_bytes());
        assert_eq!(store.list(owner).unwrap().len(), 1);
        for other in owners.iter().filter(|o| *o != owner) {
            assert_eq!(
                store.resolve(other, &store.token(&h).unwrap()).unwrap_err(),
                Error::NotFound
            );
        }
    }
    // Same UUID as account and as space are distinct owners.
    let id = uuid::Uuid::now_v7();
    store
        .put(
            &Owner::Account(AccountId::from_uuid(id).unwrap()),
            &name("x"),
            "X",
            value(b"acct"),
            &me,
        )
        .unwrap();
    assert!(
        store
            .list(&Owner::Space(SpaceId::from_uuid(id).unwrap()))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn request_provide_and_cancel() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let owner = account();
    let agent = Actor {
        account: AccountId::new(),
        via: Some(Via::Agent {
            thread: uuid::Uuid::now_v7(),
        }),
    };
    let id = store
        .request(
            &owner,
            &name("github"),
            "GitHub token",
            "push the branch",
            &agent,
        )
        .unwrap();
    let pending = store.pending(&owner).unwrap();
    assert_eq!((pending.len(), pending[0].id), (1, id.0));
    let h = store
        .provide(&owner, &id, value(b"ghp"), &actor(agent.account))
        .unwrap();
    assert_eq!(h.version, 1);
    assert!(store.pending(&owner).unwrap().is_empty());
    assert_eq!(
        store
            .provide(&owner, &id, value(b"again"), &actor(agent.account))
            .unwrap_err(),
        Error::NotFound
    );
    // A second request for an existing name rotates it.
    let id = store
        .request(&owner, &name("github"), "GitHub token", "expired", &agent)
        .unwrap();
    assert_eq!(
        store
            .provide(&owner, &id, value(b"ghp2"), &actor(agent.account))
            .unwrap()
            .version,
        2
    );
    let id = store
        .request(&owner, &name("other"), "Other", "why", &agent)
        .unwrap();
    store.cancel(&owner, &id, &actor(agent.account)).unwrap();
    assert!(store.pending(&owner).unwrap().is_empty());
    assert_eq!(
        store
            .request(&owner, &name("x"), "", "why", &agent)
            .unwrap_err(),
        Error::InvalidInput
    );
    assert_eq!(
        store
            .request(&owner, &name("x"), "l", "a\nb", &agent)
            .unwrap_err(),
        Error::InvalidInput
    );
    // A request from another owner cannot be answered here.
    assert_eq!(
        store
            .provide(&account(), &id, value(b"v"), &actor(agent.account))
            .unwrap_err(),
        Error::NotFound
    );
}

#[test]
fn inputs_are_validated() {
    assert!(
        SecretName::new("Upper").is_err()
            && SecretName::new("").is_err()
            && SecretName::new("-x").is_err()
    );
    assert_eq!(
        SecretName::for_plugin("mail", "imap").unwrap().as_str(),
        "plugin:mail:imap"
    );
    assert!(SecretName::for_plugin("a:b", "c").is_err());
    assert!(
        SecretValue::new(Vec::new()).is_err()
            && SecretValue::new(vec![0; SecretValue::MAX + 1]).is_err()
    );
    assert!(
        HandleToken::parse("sh-XYZ").is_err()
            && HandleToken::parse(&format!("sh-{}", "a".repeat(41))).is_err()
    );
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let owner = account();
    let h = store
        .put(
            &owner,
            &name("k"),
            "K",
            value(b"v"),
            &actor(AccountId::new()),
        )
        .unwrap();
    for bad in [
        Consumer::SandboxEnv {
            thread: uuid::Uuid::nil(),
            variable: "lower".into(),
        },
        Consumer::SandboxEnv {
            thread: uuid::Uuid::nil(),
            variable: "1X".into(),
        },
        Consumer::RequestHeader {
            origin: "http://plain".into(),
            header: "Authorization".into(),
        },
        Consumer::RequestHeader {
            origin: "https://a/b".into(),
            header: "Authorization".into(),
        },
        Consumer::RequestHeader {
            origin: "https://a".into(),
            header: "Bad Header".into(),
        },
    ] {
        assert_eq!(
            store
                .use_value(&h, &actor(AccountId::new()), &bad)
                .unwrap_err(),
            Error::InvalidInput
        );
    }
    let ok = Consumer::RequestHeader {
        origin: "https://api.example.com".into(),
        header: "Authorization".into(),
    };
    assert!(store.use_value(&h, &actor(AccountId::new()), &ok).is_ok());
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn scan(dir: &Path, needle: &[u8]) -> Vec<std::path::PathBuf> {
    let mut hits = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            hits.extend(scan(&path, needle));
        } else if contains(&std::fs::read(&path).unwrap(), needle) {
            hits.push(path);
        }
    }
    hits
}

#[test]
fn value_never_reaches_disk_audit_metadata_errors_or_debug() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let owner = account();
    let me = actor(AccountId::new());
    let id = store
        .request(&owner, &name("leak"), "Leak test", "scan", &me)
        .unwrap();
    let h = store.provide(&owner, &id, value(CANARY), &me).unwrap();
    let used = store.use_value(&h, &me, &env()).unwrap();
    let h2 = store
        .rotate(&owner, &name("leak"), value(CANARY), &me)
        .unwrap();
    store.use_value(&h2, &me, &env()).unwrap();
    let mut text = format!(
        "{used:?} {h:?} {:?} {:?} {:?} {:?}",
        store.list(&owner).unwrap(),
        store.audit(&owner, 100).unwrap(),
        store.pending(&owner).unwrap(),
        store.token(&h2).unwrap(),
    );
    for e in [
        Error::NotFound,
        Error::InvalidInput,
        Error::Conflict,
        Error::Limit,
        Error::KeyMissing,
        Error::KeyUnusable,
        Error::Internal,
    ] {
        text.push_str(&format!("{e} {e:?}"));
    }
    assert!(!contains(text.as_bytes(), CANARY));
    assert!(text.contains("SecretValue(..)"));
    // Every file (databases, WAL, shared memory, key) before and after checkpoint.
    assert_eq!(scan(dir.path(), CANARY), Vec::<std::path::PathBuf>::new());
    store.delete(&owner, &name("leak"), &me).unwrap();
    drop(store);
    assert_eq!(scan(dir.path(), CANARY), Vec::<std::path::PathBuf>::new());
}

/// Child half of the crash tests: runs only when the parent sets the variables,
/// then aborts inside the transaction just before commit.
#[test]
fn crash_child() {
    let Ok(dir) = std::env::var("CALTERNAL_SECRETS_CHILD_DIR") else {
        return;
    };
    let owner: uuid::Uuid = std::env::var("CALTERNAL_SECRETS_CHILD_OWNER")
        .unwrap()
        .parse()
        .unwrap();
    let owner = Owner::Account(AccountId::from_uuid(owner).unwrap());
    let store = LocalStore::open(Path::new(&dir)).unwrap();
    let me = actor(AccountId::new());
    match std::env::var("CALTERNAL_SECRETS_CRASH").unwrap().as_str() {
        "key-published" | "key-recorded" => {
            drop(store.put(&owner, &name("new"), "N", value(b"new"), &me))
        }
        "erase-recorded" | "erase-unlinked" => drop(store.erase_owner(&owner)),
        "put" => drop(store.put(&owner, &name("crashing"), "C", value(b"new"), &me)),
        "rotate" => drop(store.rotate(&owner, &name("existing"), value(b"new"), &me)),
        _ => unreachable!(),
    }
    unreachable!("crash point did not fire");
}

fn crash(dir: &Path, owner: &Owner, point: &str) {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "tests::crash_child",
            "--test-threads=1",
            "--nocapture",
        ])
        .env("CALTERNAL_SECRETS_CHILD_DIR", dir)
        .env("CALTERNAL_SECRETS_CHILD_OWNER", owner.uuid().to_string())
        .env("CALTERNAL_SECRETS_CRASH", point)
        .stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success(), "child must die at the crash point");
}

#[test]
fn crash_during_put_and_rotate_leaves_old_state() {
    let dir = tempfile::tempdir().unwrap();
    let owner = account();
    let me = actor(AccountId::new());
    let store = LocalStore::open(dir.path()).unwrap();
    store
        .put(&owner, &name("existing"), "E", value(b"old"), &me)
        .unwrap();
    drop(store);

    crash(dir.path(), &owner, "put");
    let store = LocalStore::open(dir.path()).unwrap();
    assert_eq!(
        store.handle(&owner, &name("crashing")).unwrap_err(),
        Error::NotFound
    );
    assert_eq!(store.list(&owner).unwrap().len(), 1);
    store
        .put(&owner, &name("crashing"), "C", value(b"after"), &me)
        .unwrap();
    drop(store);

    crash(dir.path(), &owner, "rotate");
    let store = LocalStore::open(dir.path()).unwrap();
    let h = store.handle(&owner, &name("existing")).unwrap();
    assert_eq!(h.version, 1);
    assert_eq!(used(&store, &h).unwrap(), b"old");
    assert!(
        store
            .audit(&owner, 100)
            .unwrap()
            .iter()
            .all(|e| e.action != Action::Rotate)
    );
}

#[test]
fn pending_requests_are_bounded_per_owner() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let (owner, me) = (account(), actor(AccountId::new()));
    for i in 0..LIMITS.pending {
        store
            .request(&owner, &name(&format!("n{i}")), "L", "R", &me)
            .unwrap();
    }
    assert_eq!(
        store
            .request(&owner, &name("over"), "L", "R", &me)
            .unwrap_err(),
        Error::Limit
    );
    // Another owner is unaffected.
    store
        .request(&account(), &name("over"), "L", "R", &me)
        .unwrap();
}

mod keys;
