use super::*;
use calternal_auth::{
    AccountId,
    spaces::{SpaceId, Spaces},
};
use std::{
    os::unix::fs::PermissionsExt,
    sync::atomic::{AtomicBool, Ordering},
};

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    db: PathBuf,
    install: Uuid,
    owner: AccountId,
    member: AccountId,
    outsider: AccountId,
    space: SpaceId,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("protected");
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let db = dir.path().join("auth.db");
        let c = calternal_auth::db::open_conn(&db).unwrap();
        let owner = AccountId::new();
        let member = AccountId::new();
        let outsider = AccountId::new();
        for (id, name) in [(owner, "owner"), (member, "member"), (outsider, "outsider")] {
            c.execute(
                "INSERT INTO users(id,name,username,created_at) VALUES(?1,?2,?2,1)",
                rusqlite::params![id.to_string(), name],
            )
            .unwrap();
        }
        c.execute("INSERT INTO passkeys(cred_id,user_id,passkey,created_at) VALUES(x'01',?1,'PRIVATE-PASSKEY-ONE',1),(x'02',?1,'PRIVATE-PASSKEY-TWO',1)", [owner.to_string()]).unwrap();
        let spaces = Spaces::open(&db).unwrap();
        let space = spaces.create(owner, "test space").unwrap().id;
        c.execute(
            "INSERT INTO space_memberships VALUES(?1,?2,'editor')",
            rusqlite::params![space.to_string(), member.to_string()],
        )
        .unwrap();
        Self {
            _dir: dir,
            root,
            db,
            install: Uuid::now_v7(),
            owner,
            member,
            outsider,
            space,
        }
    }
    fn coordinator(&self) -> Coordinator {
        Coordinator::open(&self.root, self.install).unwrap()
    }
    fn connection(&self) -> Connection {
        calternal_auth::db::open_conn(&self.db).unwrap()
    }
    fn baseline(&self) -> Snapshot {
        let coordinator = self.coordinator();
        let mut c = self.connection();
        coordinator.reconcile(&c).unwrap();
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        let id = coordinator.prepare(&tx).unwrap();
        tx.commit().unwrap();
        coordinator.complete(&c, &id).unwrap();
        coordinator.validate_and_read().unwrap()
    }
    fn mutate(&self, c: &Connection, mutation: &str) {
        match mutation {
            "grant" => {
                c.execute(
                    "INSERT INTO space_memberships VALUES(?1,?2,'reader')",
                    rusqlite::params![self.space.to_string(), self.outsider.to_string()],
                )
                .unwrap();
            }
            "revoke" => {
                c.execute(
                    "DELETE FROM space_memberships WHERE space_id=?1 AND account_id=?2",
                    rusqlite::params![self.space.to_string(), self.member.to_string()],
                )
                .unwrap();
            }
            "role" => {
                c.execute("UPDATE space_memberships SET role='reader' WHERE space_id=?1 AND account_id=?2", rusqlite::params![self.space.to_string(),self.member.to_string()]).unwrap();
            }
            "transfer" => {
                c.execute(
                    "UPDATE spaces SET owner_account=?1 WHERE id=?2",
                    rusqlite::params![self.member.to_string(), self.space.to_string()],
                )
                .unwrap();
            }
            "passkey" => {
                c.execute("DELETE FROM passkeys WHERE cred_id=x'01'", [])
                    .unwrap();
            }
            _ => unreachable!(),
        }
    }
}
const STEPS: &[Step] = &[
    Step::BeforeIntent,
    Step::AfterIntent,
    Step::BeforePendingHead,
    Step::AfterPendingHead,
    Step::BeforeCommit,
    Step::AfterCommit,
    Step::BeforeIndex,
    Step::AfterIndex,
    Step::BeforeRecord,
    Step::AfterRecord,
    Step::BeforeDirectorySync,
    Step::AfterDirectorySync,
    Step::BeforeCompletion,
    Step::AfterCompletion,
];
fn injected(root: &Path, install: Uuid, target: Step) -> Coordinator {
    let fired = AtomicBool::new(false);
    Coordinator::with_observer(
        root,
        install,
        Arc::new(move |step| {
            if step == target && !fired.swap(true, Ordering::SeqCst) {
                Err(err("simulated crash"))
            } else {
                Ok(())
            }
        }),
    )
    .unwrap()
}

#[test]
fn every_crash_boundary_preserves_the_transaction_decision() {
    for mutation in ["grant", "revoke", "role", "transfer", "passkey"] {
        for &step in STEPS {
            let f = Fixture::new();
            let old = f.baseline();
            let coordinator = injected(&f.root, f.install, step);
            let mut c = f.connection();
            let tx = c
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .unwrap();
            f.mutate(&tx, mutation);
            let prepare = coordinator.prepare(&tx);
            let committed = match prepare {
                Err(_) => {
                    drop(tx);
                    false
                }
                Ok(id) => {
                    tx.commit().unwrap();
                    assert!(
                        coordinator.complete(&c, &id).is_err(),
                        "{mutation} {step:?}"
                    );
                    true
                }
            };
            // A pending intent cannot be used for state-loss recovery, even if
            // the last complete version was otherwise readable.
            if step != Step::BeforeIntent
                && step != Step::BeforeDirectorySync
                && step != Step::AfterCompletion
            {
                assert!(
                    f.coordinator().validate_and_read().is_err(),
                    "{mutation} {step:?}"
                );
            }
            drop(c);
            let c = f.connection();
            // Crash once in replay, then restart it twice. Both the decision
            // point and each publication/completion step must be repeatable.
            if step != Step::BeforeIntent
                && step != Step::BeforeDirectorySync
                && step != Step::AfterCompletion
            {
                assert!(
                    injected(&f.root, f.install, Step::Replay)
                        .replay(&c)
                        .is_err()
                );
            }
            let recovery = f.coordinator();
            recovery.replay(&c).unwrap();
            recovery.replay(&c).unwrap();
            let recovered = recovery.validate_and_read().unwrap();
            let current = capture(&c, f.install, 999).unwrap();
            assert_eq!(recovered.tables, current.tables, "{mutation} {step:?}");
            if committed {
                assert_ne!(recovered.tables, old.tables);
            } else {
                assert_eq!(recovered.tables, old.tables);
            }
        }
    }
}

#[test]
fn crashes_at_each_replay_publication_boundary_are_idempotent() {
    for mutation in ["grant", "revoke", "role", "transfer", "passkey"] {
        for step in [
            Step::Replay,
            Step::BeforeRecord,
            Step::AfterRecord,
            Step::BeforeDirectorySync,
            Step::AfterDirectorySync,
            Step::BeforeCompletion,
            Step::AfterCompletion,
        ] {
            let f = Fixture::new();
            f.baseline();
            let coordinator = f.coordinator();
            let mut c = f.connection();
            let tx = c
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .unwrap();
            f.mutate(&tx, mutation);
            coordinator.prepare(&tx).unwrap();
            tx.commit().unwrap();
            assert!(
                injected(&f.root, f.install, step).replay(&c).is_err(),
                "{mutation} {step:?}"
            );
            let replay = f.coordinator();
            replay.replay(&c).unwrap();
            replay.replay(&c).unwrap();
            assert_eq!(
                replay.validate_and_read().unwrap().tables,
                capture(&c, f.install, 1).unwrap().tables
            );
        }
    }
}

#[test]
fn real_space_api_waits_for_recovery_and_records_transfer_and_deletion() {
    let f = Fixture::new();
    let coordinator = Arc::new(f.coordinator());
    let spaces = Spaces::open(&f.db).unwrap();
    spaces.configure_recovery(coordinator.clone()).unwrap();
    spaces
        .set_role(f.owner, f.space, f.member, "reader")
        .unwrap();
    assert_eq!(
        coordinator.validate_and_read().unwrap().tables,
        capture(&f.connection(), f.install, 1).unwrap().tables
    );
    spaces.transfer(f.owner, f.space, f.member).unwrap();
    spaces.delete(f.member, f.space).unwrap();
    let snapshot = coordinator.validate_and_read().unwrap();
    assert!(ids(&snapshot, "spaces", "id").unwrap().is_empty());
    assert_eq!(
        snapshot.tables,
        capture(&f.connection(), f.install, 1).unwrap().tables
    );
}

#[test]
fn space_api_never_acknowledges_failed_intent_or_completion() {
    // Exercise the hook registry and the bounded worker path.
    for target in [Step::BeforeIntent, Step::BeforeCompletion] {
        let f = Fixture::new();
        let spaces = Spaces::open(&f.db).unwrap();
        let coordinator = Arc::new(injected(&f.root, f.install, target));
        spaces.configure_recovery(coordinator).unwrap();
        assert!(spaces.remove(f.owner, f.space, f.member).is_err());
        let c = f.connection();
        let count: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM space_memberships WHERE account_id=?1",
                [f.member.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, if target == Step::BeforeIntent { 1 } else { 0 });
        let replay = f.coordinator();
        replay.replay(&c).unwrap();
        if target == Step::BeforeCompletion {
            assert_eq!(
                replay.validate_and_read().unwrap().tables,
                capture(&c, f.install, 1).unwrap().tables
            );
        }
    }
}

#[test]
fn wrong_installation_truncation_bit_flip_and_missing_record_refuse() {
    for damage in [
        "installation",
        "truncate-intent",
        "flip-intent",
        "truncate-completion",
        "flip-record",
        "flip-space",
        "missing-record",
        "missing-intent",
    ] {
        let f = Fixture::new();
        f.baseline();
        let coordinator = f.coordinator();
        if damage == "installation" {
            assert!(Coordinator::open(&f.root, Uuid::now_v7()).is_err());
            continue;
        }
        let intent = coordinator.intents().unwrap().pop().unwrap();
        let record = record_path(&f, &format!("1{}", f.owner.0.simple()));
        let path = match damage {
            "truncate-intent" | "flip-intent" | "missing-intent" => {
                f.root.join("transactions").join(&intent)
            }
            "truncate-completion" => f.root.join("head.json"),
            "flip-space" => record_path(&f, &format!("2{}", f.space.0.simple())),
            _ => record,
        };
        if damage.starts_with("missing-") {
            std::fs::remove_file(path).unwrap();
        } else {
            let mut bytes = std::fs::read(&path).unwrap();
            if damage.starts_with("truncate") {
                bytes.truncate(bytes.len() / 2);
            } else {
                let i = bytes.len() / 2;
                bytes[i] ^= 1;
            }
            fsio::write_atomic_mode(&path, &bytes, 0o600).unwrap();
        }
        assert!(coordinator.validate_and_read().is_err(), "{damage}");
    }
}

#[test]
fn modes_symlinks_and_space_records_keep_credentials_private() {
    let f = Fixture::new();
    f.baseline();
    for sub in ["accounts", "spaces", "transactions", "index"] {
        for entry in std::fs::read_dir(f.root.join(sub)).unwrap() {
            let path = entry.unwrap().path();
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            if sub == "spaces" {
                let bytes = std::fs::read(path).unwrap();
                let env: Envelope = serde_json::from_slice(&bytes).unwrap();
                let text = String::from_utf8(env.bytes).unwrap();
                for secret in [
                    "PRIVATE-PASSKEY",
                    "passkeys",
                    "token_hash",
                    "setup_tokens",
                    "installations",
                ] {
                    assert!(!text.contains(secret));
                }
                assert!(text.contains(&f.space.to_string()));
            }
        }
    }
    // Refuse final root symlinks and insecure parent permissions.
    let link = f._dir.path().join("link");
    std::os::unix::fs::symlink(&f.root, &link).unwrap();
    assert!(Coordinator::open(&link, f.install).is_err());
    let account = record_path(&f, &format!("1{}", f.owner.0.simple()));
    let held = f._dir.path().join("held");
    std::fs::rename(&account, &held).unwrap();
    std::os::unix::fs::symlink(&held, &account).unwrap();
    assert!(f.coordinator().validate_and_read().is_err());
    std::fs::set_permissions(&f.root, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(Coordinator::open(&f.root, f.install).is_err());
}

#[test]
fn losing_state_with_an_unfinished_intent_cannot_adopt_a_new_database() {
    let f = Fixture::new();
    f.baseline();
    let coordinator = f.coordinator();
    let mut c = f.connection();
    let tx = c.transaction().unwrap();
    f.mutate(&tx, "revoke");
    coordinator.prepare(&tx).unwrap();
    tx.commit().unwrap();
    let empty = calternal_auth::db::open_conn(&f._dir.path().join("empty.db")).unwrap();
    assert!(coordinator.reconcile(&empty).is_err());
    assert!(coordinator.validate_and_read().is_err());
}

#[test]
fn equal_account_and_space_ids_and_changed_witnesses_refuse() {
    let f = Fixture::new();
    let mut snapshot = f.baseline();
    let table = snapshot.tables.get_mut("spaces").unwrap();
    let i = table.columns.iter().position(|c| c == "id").unwrap();
    table.rows[0][i] = Cell::Text(f.owner.to_string());
    assert!(validate(&snapshot, f.install).is_err());
    let mut c = f.connection();
    let coordinator = f.coordinator();
    let tx = c.transaction().unwrap();
    f.mutate(&tx, "grant");
    let id = coordinator.prepare(&tx).unwrap();
    tx.commit().unwrap();
    c.execute(
        "UPDATE security_recovery_commits SET digest='wrong' WHERE id=?1",
        [id],
    )
    .unwrap();
    assert!(coordinator.replay(&c).is_err());
}

#[tokio::test]
async fn account_creation_passkey_removal_and_session_revocation_use_the_barrier() {
    let f = tokio::task::spawn_blocking(Fixture::new).await.unwrap();
    let auth = calternal_auth::Auth::open(
        f._dir.path(),
        calternal_auth::AuthConfig::new("http://localhost:8080").unwrap(),
    )
    .unwrap();
    // Auth::open's documented system/auth.db path; seed that independent fixture
    // using the same migrated schema and keep all bytes in this disposable tree.
    let path = f._dir.path().join("system/auth.db");
    let c = calternal_auth::db::open_conn(&path).unwrap();
    let coordinator = Arc::new(f.coordinator());
    auth.configure_recovery(coordinator.clone()).await.unwrap();
    let created = auth.create("new account").await.unwrap();
    assert!(
        ids(&coordinator.validate_and_read().unwrap(), "users", "id")
            .unwrap()
            .contains(&created.id.0)
    );
    for cred in [vec![1_u8], vec![2_u8]] {
        c.execute("INSERT INTO passkeys(cred_id,user_id,passkey,created_at) VALUES(?1,?2,'protected passkey fixture',1)", rusqlite::params![cred, created.id.to_string()]).unwrap();
    }
    let installation = calternal_auth::InstallationId::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    c.execute("INSERT INTO installations(id,user_id,kind,name,token_hash,created_at,last_seen_at,assertion_at) VALUES(?1,?2,'browser','test',x'0102',?3,?3,?3)",rusqlite::params![installation.to_string(),created.id.to_string(),now]).unwrap();
    auth.remove_passkey(created.id, installation, "AQ")
        .await
        .unwrap();
    let snapshot = coordinator.validate_and_read().unwrap();
    assert_eq!(snapshot.tables["passkeys"].rows.len(), 1);
    assert_eq!(snapshot.tables, capture(&c, f.install, 1).unwrap().tables);
    assert_eq!(
        auth.remove_passkey(created.id, installation, "Ag")
            .await
            .unwrap_err()
            .code(),
        calternal_auth::ErrorCode::LastPasskey
    );
    auth.revoke(created.id, installation).await.unwrap();
    let snapshot = coordinator.validate_and_read().unwrap();
    assert_eq!(snapshot.tables, capture(&c, f.install, 1).unwrap().tables);
    let table = &snapshot.tables["installations"];
    let column = table
        .columns
        .iter()
        .position(|c| c == "revoked_at")
        .unwrap();
    assert!(matches!(table.rows[0][column], Cell::Integer(_)));
}

#[test]
fn second_record_failure_never_exposes_a_partial_snapshot() {
    let f = Fixture::new();
    f.baseline();
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let coordinator = Coordinator::with_observer(
        &f.root,
        f.install,
        Arc::new(move |step| {
            if step == Step::AfterRecord && calls.fetch_add(1, Ordering::SeqCst) == 1 {
                Err(err("crash after second record"))
            } else {
                Ok(())
            }
        }),
    )
    .unwrap();
    let mut c = f.connection();
    let tx = c.transaction().unwrap();
    f.mutate(&tx, "revoke");
    f.mutate(&tx, "passkey");
    let intent = coordinator.prepare(&tx).unwrap();
    tx.commit().unwrap();
    assert!(coordinator.complete(&c, &intent).is_err());
    assert!(coordinator.validate_and_read().is_err());
    f.coordinator().replay(&c).unwrap();
    assert_eq!(
        f.coordinator().validate_and_read().unwrap().tables,
        capture(&c, f.install, 1).unwrap().tables
    );
}

#[tokio::test]
async fn failed_recovery_completion_still_fences_a_committed_session_revocation() {
    let f = tokio::task::spawn_blocking(Fixture::new).await.unwrap();
    let auth = calternal_auth::Auth::open(
        f._dir.path(),
        calternal_auth::AuthConfig::new("http://localhost:8080").unwrap(),
    )
    .unwrap();
    let created = auth.create("revoke test").await.unwrap();
    let c = calternal_auth::db::open_conn(&f._dir.path().join("system/auth.db")).unwrap();
    let installation = calternal_auth::InstallationId::new();
    let token = calternal_auth::token::new_session_token().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    c.execute("INSERT INTO installations(id,user_id,kind,name,token_hash,created_at,last_seen_at,expires_at) VALUES(?1,?2,'browser','test',?3,?4,?4,9223372036854775807)",rusqlite::params![installation.to_string(), created.id.to_string(), calternal_auth::token::hash(&token), now]).unwrap();
    assert_eq!(
        auth.identify(&token).await,
        Some((created.id, installation))
    );
    auth.configure_recovery(Arc::new(injected(
        &f.root,
        f.install,
        Step::BeforeCompletion,
    )))
    .await
    .unwrap();
    assert!(auth.revoke(created.id, installation).await.is_err());
    assert_eq!(auth.identify(&token).await, None);
    f.coordinator().replay(&c).unwrap();
    assert_eq!(
        f.coordinator().validate_and_read().unwrap().tables,
        capture(&c, f.install, 1).unwrap().tables
    );
}

#[test]
fn complete_table_inventory_is_required_even_for_empty_tables() {
    let f = Fixture::new();
    let snapshot = f.baseline();
    for name in ["passkeys", "space_memberships", "groups", "invites"] {
        let mut partial = snapshot.clone();
        partial.tables.remove(name);
        assert!(validate(&partial, f.install).is_err());
    }
}

#[test]
fn replay_and_completion_cannot_publish_an_uncommitted_witness() {
    let f = Fixture::new();
    let old = f.baseline();
    let coordinator = f.coordinator();
    let mut c = f.connection();
    let tx = c.transaction().unwrap();
    f.mutate(&tx, "grant");
    let intent = coordinator.prepare(&tx).unwrap();
    assert!(coordinator.complete(&tx, &intent).is_err());
    assert!(coordinator.replay(&tx).is_err());
    assert!(coordinator.reconcile(&tx).is_err());
    assert!(coordinator.validate_and_read().is_err());
    drop(tx);
    coordinator.replay(&c).unwrap();
    assert_eq!(coordinator.validate_and_read().unwrap(), old);
    assert!(coordinator.complete(&c, &intent).is_err());
    assert!(coordinator.complete(&c, "not-a-transaction-id").is_err());
    assert!(
        coordinator
            .complete(&c, &Uuid::now_v7().to_string())
            .is_err()
    );
}

#[test]
fn missing_versions_and_partial_directory_trees_refuse_without_repair() {
    let f = Fixture::new();
    f.baseline();
    let coordinator = f.coordinator();
    let mut c = f.connection();
    let tx = c.transaction().unwrap();
    f.mutate(&tx, "revoke");
    let intent = coordinator.prepare(&tx).unwrap();
    tx.commit().unwrap();
    coordinator.complete(&c, &intent).unwrap();
    let latest = coordinator.validate_and_read().unwrap();
    let stem = format!("{:020}-{}", latest.sequence, latest.transaction);
    for extension in ["intent"] {
        std::fs::remove_file(
            f.root
                .join("transactions")
                .join(format!("{stem}.{extension}")),
        )
        .unwrap();
    }
    assert!(coordinator.validate_and_read().is_err());
    assert!(coordinator.replay(&c).is_err());
    std::fs::remove_dir_all(f.root.join("accounts")).unwrap();
    assert!(Coordinator::open(&f.root, f.install).is_err());
    assert!(!f.root.join("accounts").exists());
}

// Independent review regressions: these deliberately express the required safety
// contract; REVIEW.md records their original failures before implementation.
#[test]
fn deleting_newest_generation_must_not_resurrect_revoked_membership() {
    let f = Fixture::new();
    f.baseline();
    let coordinator = f.coordinator();
    let mut c = f.connection();
    let tx = c.transaction().unwrap();
    f.mutate(&tx, "revoke");
    let intent = coordinator.prepare(&tx).unwrap();
    tx.commit().unwrap();
    coordinator.complete(&c, &intent).unwrap();
    let latest = coordinator.validate_and_read().unwrap();
    assert!(
        !ids(&latest, "space_memberships", "account_id")
            .unwrap()
            .contains(&f.member.0)
    );
    let stem = format!("{:020}-{}", latest.sequence, latest.transaction);
    for extension in ["intent"] {
        std::fs::remove_file(
            f.root
                .join("transactions")
                .join(format!("{stem}.{extension}")),
        )
        .unwrap();
    }
    assert!(
        coordinator.validate_and_read().is_err(),
        "deleting an acknowledged revocation's newest files must refuse recovery"
    );
}

#[test]
fn repeated_prepare_must_not_publish_a_rolled_back_grant() {
    let f = Fixture::new();
    let old = f.baseline();
    let coordinator = f.coordinator();
    let mut c = f.connection();
    let tx = c.transaction().unwrap();
    f.mutate(&tx, "grant");
    coordinator.prepare(&tx).unwrap();
    // A second prepare must either refuse or preserve the first intent as pending.
    // Both calls are sequential and use the same still-uncommitted transaction.
    let _ = coordinator.prepare(&tx);
    drop(tx);
    coordinator.replay(&c).unwrap();
    let recovered = coordinator.validate_and_read().unwrap();
    assert_eq!(
        recovered.tables, old.tables,
        "an aborted transaction must never publish its proposed grant"
    );
}

#[tokio::test]
async fn acknowledged_passkey_label_must_reach_recovery() {
    let f = tokio::task::spawn_blocking(Fixture::new).await.unwrap();
    let auth = calternal_auth::Auth::open(
        f._dir.path(),
        calternal_auth::AuthConfig::new("http://localhost:8080").unwrap(),
    )
    .unwrap();
    let made = auth.create("label test").await.unwrap();
    let mut c = calternal_auth::db::open_conn(&f._dir.path().join("system/auth.db")).unwrap();
    c.execute(
        "INSERT INTO passkeys(cred_id,user_id,passkey,created_at,name) VALUES(x'01',?1,'private fixture',1,'Before')",
        [made.id.to_string()],
    ).unwrap();
    let coordinator = Arc::new(f.coordinator());
    auth.configure_recovery(coordinator.clone()).await.unwrap();
    let tx = c.transaction().unwrap();
    let intent = coordinator.prepare(&tx).unwrap();
    tx.commit().unwrap();
    coordinator.complete(&c, &intent).unwrap();
    assert_eq!(
        auth.rename_passkey(made.id, "AQ", "After").await.unwrap(),
        "After"
    );
    let table = &coordinator.validate_and_read().unwrap().tables["passkeys"];
    let column = table
        .columns
        .iter()
        .position(|name| name == "name")
        .unwrap();
    assert_eq!(table.rows[0][column], Cell::Text("After".into()));
}

#[test]
fn publication_record_cost_is_independent_of_other_accounts_and_spaces() {
    let mut costs = Vec::new();
    for extra in [0, 12] {
        let f = Fixture::new();
        let c = f.connection();
        for n in 0..extra {
            let id = AccountId::new();
            c.execute(
                "INSERT INTO users(id,name,username,created_at) VALUES(?1,?2,?2,1)",
                rusqlite::params![id.to_string(), format!("other-{n}")],
            )
            .unwrap();
            Spaces::open(&f.db)
                .unwrap()
                .create(id, "unrelated")
                .unwrap();
        }
        f.baseline();
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = count.clone();
        let coordinator = Coordinator::with_observer(
            &f.root,
            f.install,
            Arc::new(move |step| {
                if step == Step::AfterRecord {
                    seen.fetch_add(1, Ordering::SeqCst);
                }
                Ok(())
            }),
        )
        .unwrap();
        let mut c = f.connection();
        let tx = c.transaction().unwrap();
        f.mutate(&tx, "role");
        let id = coordinator.prepare(&tx).unwrap();
        tx.commit().unwrap();
        coordinator.complete(&c, &id).unwrap();
        costs.push(count.load(Ordering::SeqCst));
    }
    assert_eq!(
        costs,
        vec![1, 1],
        "only the changed space record is materialized"
    );
}

#[test]
fn completed_transaction_retention_is_bounded() {
    let f = Fixture::new();
    f.baseline();
    let spaces = Spaces::open(&f.db).unwrap();
    spaces
        .configure_recovery(Arc::new(f.coordinator()))
        .unwrap();
    for n in 0..20 {
        spaces
            .set_role(
                f.owner,
                f.space,
                f.member,
                if n % 2 == 0 { "reader" } else { "editor" },
            )
            .unwrap();
    }
    assert!(
        std::fs::read_dir(f.root.join("transactions"))
            .unwrap()
            .count()
            <= 2,
        "completed intents must not accumulate with installation history"
    );
}

fn record_path(f: &Fixture, key: &str) -> PathBuf {
    let c = f.coordinator();
    let h = c.head().unwrap();
    let r = index::get(&c.store, h.root.as_deref(), key)
        .unwrap()
        .unwrap();
    f.root.join(r.path())
}

#[test]
fn fsync_cost_is_constant_for_one_account_or_space_replacement() {
    for mutation in ["role", "passkey"] {
        let mut costs = Vec::new();
        for extra in [0, 12] {
            let f = Fixture::new();
            for n in 0..extra {
                let id = AccountId::new();
                f.connection()
                    .execute(
                        "INSERT INTO users(id,name,username,created_at) VALUES(?1,?2,?2,1)",
                        rusqlite::params![id.to_string(), format!("unrelated-{n}")],
                    )
                    .unwrap();
                Spaces::open(&f.db)
                    .unwrap()
                    .create(id, "unrelated")
                    .unwrap();
            }
            f.baseline();
            let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let observed = count.clone();
            let c = Coordinator::with_observer(
                &f.root,
                f.install,
                Arc::new(move |step| {
                    if matches!(step, Step::FileSynced | Step::DirectorySynced) {
                        observed.fetch_add(1, Ordering::SeqCst);
                    }
                    Ok(())
                }),
            )
            .unwrap();
            let mut db = f.connection();
            let tx = db.transaction().unwrap();
            f.mutate(&tx, mutation);
            let id = c.prepare(&tx).unwrap();
            tx.commit().unwrap();
            c.complete(&db, &id).unwrap();
            costs.push(count.load(Ordering::SeqCst));
        }
        assert_eq!(
            costs,
            vec![81, 81],
            "filesystem durability cost for {mutation}"
        );
    }
}

#[test]
fn every_index_and_compaction_boundary_can_be_interrupted() {
    for (step, count) in [
        (Step::BeforeIndex, 34),
        (Step::AfterIndex, 34),
        (Step::BeforeCompact, 36),
        (Step::AfterCompact, 36),
        (Step::BeforeCompactSync, 3),
        (Step::AfterCompactSync, 3),
    ] {
        for fail_at in 0..count {
            let f = Fixture::new();
            f.baseline();
            let calls = std::sync::atomic::AtomicUsize::new(0);
            let c = Coordinator::with_observer(
                &f.root,
                f.install,
                Arc::new(move |s| {
                    if s == step && calls.fetch_add(1, Ordering::SeqCst) == fail_at {
                        Err(err("interrupted publication/collection"))
                    } else {
                        Ok(())
                    }
                }),
            )
            .unwrap();
            let mut db = f.connection();
            let tx = db.transaction().unwrap();
            f.mutate(&tx, "revoke");
            let id = c.prepare(&tx).unwrap();
            tx.commit().unwrap();
            assert!(c.complete(&db, &id).is_err(), "{step:?} {fail_at}");
            if matches!(step, Step::BeforeIndex | Step::AfterIndex) {
                assert!(c.validate_and_read().is_err());
            }
            let recovery = f.coordinator();
            recovery.replay(&db).unwrap();
            recovery.replay(&db).unwrap();
            assert!(
                !ids(
                    &recovery.validate_and_read().unwrap(),
                    "space_memberships",
                    "account_id"
                )
                .unwrap()
                .contains(&f.member.0)
            );
            assert_eq!(
                std::fs::read_dir(f.root.join("transactions"))
                    .unwrap()
                    .count(),
                1
            );
            // One global record, three account records, one space record.
            assert_eq!(
                std::fs::read_dir(f.root.join("accounts")).unwrap().count()
                    + std::fs::read_dir(f.root.join("spaces")).unwrap().count(),
                5
            );
            assert!(std::fs::read_dir(f.root.join("index")).unwrap().count() <= 5 * 34);
        }
    }
}

#[test]
fn telemetry_and_sharing_delivery_changes_do_not_publish() {
    let f = Fixture::new();
    let installation = calternal_auth::InstallationId::new();
    f.connection().execute("INSERT INTO installations(id,user_id,kind,name,token_hash,created_at,last_seen_at,expires_at) VALUES(?1,?2,'browser','test',x'0123',1,1,100)",rusqlite::params![installation.to_string(),f.owner.to_string()]).unwrap();
    f.baseline();
    let pin = f.coordinator().checkpoint().unwrap();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let c = Coordinator::with_observer(
        &f.root,
        f.install,
        Arc::new(move |s| {
            if matches!(s, Step::FileSynced | Step::DirectorySynced) {
                observed.fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        }),
    )
    .unwrap();
    let mut db = f.connection();
    let tx = db.transaction().unwrap();
    tx.execute(
        "UPDATE installations SET last_seen_at=50,expires_at=150 WHERE id=?1",
        [installation.to_string()],
    )
    .unwrap();
    tx.execute("UPDATE passkeys SET last_used_at=50", [])
        .unwrap();
    tx.execute(
        "INSERT INTO sharing_installation_resets VALUES(?1,50)",
        [installation.to_string()],
    )
    .unwrap();
    let id = c.prepare(&tx).unwrap();
    assert_eq!(id, "");
    tx.commit().unwrap();
    c.complete(&db, &id).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let snapshot = c.validate_and_read_at(&pin).unwrap();
    assert!(!snapshot.tables.contains_key("sharing_installation_resets"));
    assert!(!snapshot.tables.contains_key("share_fanout"));
    let expiry = snapshot.tables["installations"]
        .columns
        .iter()
        .position(|n| n == "expires_at")
        .unwrap();
    assert_eq!(
        snapshot.tables["installations"].rows[0][expiry],
        Cell::Integer(100)
    );
}

#[test]
fn stale_record_and_index_bytes_cannot_satisfy_the_current_head() {
    for damage in ["record", "index", "head"] {
        let f = Fixture::new();
        f.baseline();
        let c = f.coordinator();
        let old_head = c.head().unwrap();
        let old_path = record_path(&f, &format!("2{}", f.space.0.simple()));
        let old_record = std::fs::read(old_path).unwrap();
        let old_index = c
            .store
            .read("index", &format!("{}.json", old_head.root.unwrap()))
            .unwrap();
        let mut db = f.connection();
        let tx = db.transaction().unwrap();
        f.mutate(&tx, "revoke");
        let id = c.prepare(&tx).unwrap();
        tx.commit().unwrap();
        c.complete(&db, &id).unwrap();
        let path = match damage {
            "record" => record_path(&f, &format!("2{}", f.space.0.simple())),
            "index" => f
                .root
                .join("index")
                .join(format!("{}.json", c.head().unwrap().root.unwrap())),
            _ => f.root.join("head.json"),
        };
        if damage == "head" {
            std::fs::remove_file(path).unwrap();
            assert!(Coordinator::open(&f.root, f.install).is_err());
        } else {
            fsio::write_atomic_mode(
                &path,
                if damage == "record" {
                    &old_record
                } else {
                    &old_index
                },
                0o600,
            )
            .unwrap();
            assert!(c.validate_and_read().is_err());
        }
    }
}

#[test]
fn whole_tree_rollback_requires_a_surviving_database_or_external_checkpoint() {
    let f = Fixture::new();
    f.baseline();
    fn files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut result = vec![(
            PathBuf::from("head.json"),
            std::fs::read(root.join("head.json")).unwrap(),
        )];
        for sub in ["accounts", "spaces", "index", "transactions"] {
            for e in std::fs::read_dir(root.join(sub)).unwrap() {
                let e = e.unwrap();
                result.push((
                    PathBuf::from(sub).join(e.file_name()),
                    std::fs::read(e.path()).unwrap(),
                ));
            }
        }
        result
    }
    let old = files(&f.root);
    let c = f.coordinator();
    let mut db = f.connection();
    let tx = db.transaction().unwrap();
    f.mutate(&tx, "revoke");
    let id = c.prepare(&tx).unwrap();
    tx.commit().unwrap();
    c.complete(&db, &id).unwrap();
    let latest = c.checkpoint().unwrap();
    for (path, _) in files(&f.root) {
        std::fs::remove_file(f.root.join(path)).unwrap();
    }
    for (path, bytes) in old {
        fsio::write_atomic_mode(&f.root.join(path), &bytes, 0o600).unwrap();
    }
    assert!(
        c.validate_and_read().is_ok(),
        "a complete old image has no local freshness evidence"
    );
    assert!(
        c.validate_and_read_at(&latest).is_err(),
        "external pin detects complete-tree rollback"
    );
    assert!(
        c.replay(&db).is_err(),
        "surviving committed SQLite witness detects tree rollback"
    );
}

#[test]
fn concurrent_space_mutation_pairs_never_publish_a_mixed_snapshot() {
    for a in 0..5 {
        for b in 0..5 {
            let f = Fixture::new();
            f.baseline();
            let sink = Arc::new(f.coordinator());
            let first = Spaces::open(&f.db).unwrap();
            let second = Spaces::open(&f.db).unwrap();
            first.configure_recovery(sink.clone()).unwrap();
            second.configure_recovery(sink.clone()).unwrap();
            let start = std::sync::Barrier::new(2);
            std::thread::scope(|scope| {
                for (which, spaces) in [(a, first), (b, second)] {
                    let f = &f;
                    let start = &start;
                    scope.spawn(move || {
                        start.wait();
                        let result = match which {
                            0 => spaces.create(f.owner, "parallel").map(|_| ()),
                            1 => spaces.remove(f.owner, f.space, f.member),
                            2 => spaces.set_role(f.owner, f.space, f.member, "reader"),
                            3 => spaces.transfer(f.owner, f.space, f.member),
                            _ => spaces.delete(f.owner, f.space),
                        };
                        if let Err(error) = result {
                            assert!(matches!(
                                error.code(),
                                calternal_auth::ErrorCode::Forbidden
                                    | calternal_auth::ErrorCode::NotFound
                                    | calternal_auth::ErrorCode::InvalidInput
                            ));
                        }
                    });
                }
            });
            assert_eq!(
                sink.validate_and_read().unwrap().tables,
                capture(&f.connection(), f.install, 1).unwrap().tables,
                "pair {a},{b}"
            );
        }
    }
}

#[test]
fn next_request_finishes_a_committed_revocation_before_publishing_another_change() {
    let f = Fixture::new();
    f.baseline();
    let spaces = Spaces::open(&f.db).unwrap();
    let coordinator = Arc::new(injected(&f.root, f.install, Step::AfterCommit));
    spaces.configure_recovery(coordinator.clone()).unwrap();
    assert!(spaces.remove(f.owner, f.space, f.member).is_err());
    assert!(coordinator.validate_and_read().is_err());
    // The same sink's one-shot fault has fired. This second request must first
    // publish the already committed revocation, then publish its new space.
    let created = spaces.create(f.owner, "next request").unwrap();
    let snapshot = coordinator.validate_and_read().unwrap();
    assert!(
        !ids(&snapshot, "space_memberships", "account_id")
            .unwrap()
            .contains(&f.member.0)
    );
    assert!(
        ids(&snapshot, "spaces", "id")
            .unwrap()
            .contains(&created.id.0)
    );
    assert_eq!(
        snapshot.tables,
        capture(&f.connection(), f.install, 1).unwrap().tables
    );
}

#[test]
fn aborted_intent_collection_can_be_interrupted_without_publishing_its_grant() {
    for step in [
        Step::BeforeCompact,
        Step::AfterCompact,
        Step::BeforeCompactSync,
        Step::AfterCompactSync,
    ] {
        let f = Fixture::new();
        let old = f.baseline();
        let c = f.coordinator();
        let mut db = f.connection();
        let tx = db.transaction().unwrap();
        f.mutate(&tx, "grant");
        c.prepare(&tx).unwrap();
        drop(tx);
        assert!(injected(&f.root, f.install, step).replay(&db).is_err());
        c.replay(&db).unwrap();
        c.replay(&db).unwrap();
        assert_eq!(c.validate_and_read().unwrap().tables, old.tables);
    }
}

#[test]
fn account_receipt_and_space_change_publish_together_and_retry_has_no_fsyncs() {
    let f = Fixture::new();
    f.baseline();
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = count.clone();
    let c = Arc::new(
        Coordinator::with_observer(
            &f.root,
            f.install,
            Arc::new(move |s| {
                if matches!(s, Step::FileSynced | Step::DirectorySynced) {
                    observed.fetch_add(1, Ordering::SeqCst);
                }
                Ok(())
            }),
        )
        .unwrap(),
    );
    let spaces = Spaces::open(&f.db).unwrap();
    spaces.configure_recovery(c.clone()).unwrap();
    let apply = || {
        spaces.change_with_receipt(
            f.owner,
            Some(f.space),
            calternal_auth::spaces::SpaceChange::SetRole {
                target: f.member,
                role: "reader".into(),
            },
            Some(("receipt-1", "spaces.role", "fixed-fingerprint")),
        )
    };
    let first = apply().unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 150);
    let snapshot = c.validate_and_read().unwrap();
    assert_eq!(snapshot.tables["space_receipts"].rows.len(), 1);
    assert_eq!(
        snapshot.tables,
        capture(&f.connection(), f.install, 1).unwrap().tables
    );
    count.store(0, Ordering::SeqCst);
    assert_eq!(apply().unwrap(), first);
    assert_eq!(count.load(Ordering::SeqCst), 0);
}
