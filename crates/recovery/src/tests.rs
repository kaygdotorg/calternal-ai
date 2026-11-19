use super::*;
use calternal_auth::spaces::Spaces;
use calternal_types::{AccountId, SpaceId};
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
            "secret-key" => {
                c.execute(
                    "INSERT INTO owner_secret_keys VALUES('space',?1,?2,0)",
                    rusqlite::params![self.space.to_string(), &[42u8; 32][..]],
                )
                .unwrap();
            }
            "secret-erase" => {
                c.execute("UPDATE owner_secret_keys SET erasing=1", [])
                    .unwrap();
            }
            "receipt-delete" | "receipt-mixed" => {
                c.execute("DELETE FROM space_receipts", []).unwrap();
                if mutation == "receipt-mixed" {
                    self.mutate(c, "revoke");
                }
            }
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
    fn seed_receipt(&self) {
        self.connection().execute(
            "INSERT INTO space_receipts VALUES(?1,?2,'receipt:東京','spaces.set_role','test','{}',42)",
            rusqlite::params![self.owner.to_string(), self.space.to_string()],
        ).unwrap();
    }
}

#[test]
fn attachment_seeds_receipts_and_reopen_repairs_a_missed_key() {
    let f = Fixture::new();
    let db = f.connection();
    let request = "request:opaque/東京";
    db.execute(
        "INSERT INTO space_receipts VALUES(?1,?2,?3,'spaces.set_role','seed','{}',42)",
        rusqlite::params![f.owner.to_string(), f.space.to_string(), request],
    )
    .unwrap();
    let coordinator = f.coordinator();
    coordinator.reconcile(&db).unwrap();
    let before = coordinator.validate_and_read().unwrap();
    assert_eq!(before.tables["space_receipts"].rows.len(), 1);
    drop(coordinator);

    // A capture defect can leave the database changed with its last good witness.
    db.execute_batch("DROP TRIGGER security_recovery_space_receipts_UPDATE")
        .unwrap();
    db.execute(
        "UPDATE space_receipts SET fingerprint='missed' WHERE request_id=?1",
        [request],
    )
    .unwrap();
    let reopened = f.coordinator();
    reopened.reconcile(&db).unwrap();
    let repaired = reopened.validate_and_read().unwrap();
    assert!(repaired.sequence > before.sequence);
    assert_eq!(
        repaired.tables,
        capture(&db, f.install, repaired.sequence).unwrap().tables
    );
    reopened.validate_against_witness(&db).unwrap();
}

const STEPS: &[Step] = &[
    Step::BeforeIntent,
    Step::BeforeAppend,
    Step::AfterPrefix,
    Step::AfterAppend,
    Step::AfterIntent,
    Step::BeforeCommit,
    Step::AfterCommit,
    Step::BeforeCompletion,
    Step::AfterCompletion,
];

#[test]
fn v3_snapshot_is_refused_without_rewriting_it() {
    let f = Fixture::new();
    let coordinator = f.coordinator();
    let mut image = coordinator.load().unwrap().image;
    assert_eq!(image.version, CURRENT);
    image.version = 3;
    let obsolete = encode(&image).unwrap();
    coordinator.store.snapshot(&obsolete).unwrap();

    assert!(Coordinator::open(&f.root, f.install).is_err());
    assert_eq!(coordinator.store.read("snapshot.json").unwrap(), obsolete);
}

#[test]
fn receipt_retirement_deadline_survives_reopen_and_compacts() {
    let f = Fixture::new();
    f.seed_receipt();
    f.baseline();
    let c = f.coordinator();
    let mut db = f.connection();
    let tx = db.transaction().unwrap();
    f.mutate(&tx, "receipt-delete");
    let id = c.prepare(&tx).unwrap();
    tx.commit().unwrap();
    c.complete(&db, &id).unwrap();
    let reopened = f.coordinator();
    let mut state = reopened.load().unwrap();
    assert!(state.image.retired_receipt_ms.is_some());
    let old = state.image.segment.clone();
    state.image.retired_receipt_ms = Some(now_ms() - RETIRED_RECEIPT_COMPACT_MS);
    reopened.compact(&mut state, false).unwrap();
    assert!(!f.root.join(old).exists());
    assert_eq!(reopened.load().unwrap().image.retired_receipt_ms, None);
    assert!(
        reopened.validate_and_read().unwrap().tables["space_receipts"]
            .rows
            .is_empty()
    );
}

#[test]
fn space_erasure_retires_cross_actor_receipt_bytes_and_retains_its_own_reply() {
    let f = Fixture::new();
    let db = f.connection();
    let obsolete = "erased-cross-actor-result-東京";
    db.execute(
        "INSERT INTO space_receipts VALUES(?1,?2,'member-reply','spaces.set_role','fp',?3,?4)",
        rusqlite::params![
            f.member.to_string(),
            f.space.to_string(),
            obsolete,
            now_ms()
        ],
    )
    .unwrap();
    let c = Arc::new(f.coordinator());
    let spaces = Spaces::open(&f.db).unwrap();
    spaces.configure_recovery(c.clone()).unwrap();
    let reply = spaces
        .delete_fenced(f.owner, f.space, ("delete", "spaces.delete", "fp"), |_| {})
        .unwrap();
    spaces
        .finish_erasure(vec![calternal_types::Owner::Space(f.space)])
        .unwrap();
    let recovered = c.validate_and_read().unwrap();
    assert_eq!(recovered.tables, capture(&db, f.install, 1).unwrap().tables);
    assert_eq!(recovered.tables["space_receipts"].rows.len(), 1);
    for entry in std::fs::read_dir(&f.root).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        assert!(
            !bytes
                .windows(obsolete.len())
                .any(|part| part == obsolete.as_bytes())
        );
    }
    assert_eq!(
        spaces
            .replay_space_delete(f.owner, f.space, "delete".into(), "fp".into())
            .unwrap(),
        Some((reply, false))
    );
    assert!(
        spaces
            .replay_space_delete(f.owner, f.space, "delete".into(), "different".into())
            .is_err()
    );
    assert!(
        spaces
            .replay_space_delete(f.member, f.space, "delete".into(), "fp".into())
            .unwrap()
            .is_none()
    );

    // Reconstitute the relevant logical rows into a fresh schema, with no
    // surviving content store or journal for the erased scope.
    let rebuilt = f._dir.path().join("rebuilt-auth.db");
    let restored = calternal_auth::db::open_conn(&rebuilt).unwrap();
    restored
        .execute(
            "INSERT INTO users(id,name,username,created_at) VALUES(?1,'owner','owner',1)",
            [f.owner.to_string()],
        )
        .unwrap();
    let [
        Cell::Text(actor),
        Cell::Text(scope),
        Cell::Text(request),
        Cell::Text(operation),
        Cell::Text(fingerprint),
        Cell::Text(result),
        Cell::Integer(at),
    ] = recovered.tables["space_receipts"].rows[0].as_slice()
    else {
        panic!("untyped receipt")
    };
    restored
        .execute(
            "INSERT INTO space_receipts VALUES(?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![actor, scope, request, operation, fingerprint, result, at],
        )
        .unwrap();
    let [Cell::Text(kind), Cell::Text(id), Cell::Integer(pending)] =
        recovered.tables["owner_erasures"].rows[0].as_slice()
    else {
        panic!("untyped tombstone")
    };
    restored
        .execute(
            "INSERT INTO owner_erasures VALUES(?1,?2,?3)",
            rusqlite::params![kind, id, pending],
        )
        .unwrap();
    let restored = Spaces::open(&rebuilt).unwrap();
    assert_eq!(
        restored
            .replay_space_delete(f.owner, f.space, "delete".into(), "fp".into())
            .unwrap(),
        Some((serde_json::from_str(result).unwrap(), false))
    );
}

#[test]
fn expiry_then_reuse_recovers_only_the_new_reply() {
    let f = Fixture::new();
    let db = f.connection();
    db.execute(
        "INSERT INTO space_receipts VALUES(?1,?1,'reused','spaces.create','old',?2,0)",
        rusqlite::params![f.owner.to_string(), r#"{"obsolete":true}"#],
    )
    .unwrap();
    let c = Arc::new(f.coordinator());
    let spaces = Spaces::open(&f.db).unwrap();
    spaces.configure_recovery(c.clone()).unwrap();
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(spaces.maintain_receipts())
        .unwrap();
    let reply = spaces
        .change_with_receipt(
            f.owner,
            None,
            calternal_auth::spaces::SpaceChange::Create {
                name: "Created after expiry".into(),
            },
            Some(("reused", "spaces.create", "new")),
        )
        .unwrap();
    let snapshot = f.coordinator().validate_and_read().unwrap();
    assert_eq!(snapshot.tables, capture(&db, f.install, 1).unwrap().tables);
    let row = &snapshot.tables["space_receipts"].rows[0];
    assert_eq!(row[4], Cell::Text("new".into()));
    assert_eq!(row[5], Cell::Text(reply.to_string()));
    assert_eq!(snapshot.tables["space_receipts"].rows.len(), 1);
}
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
    for mutation in [
        "grant",
        "revoke",
        "role",
        "transfer",
        "passkey",
        "secret-key",
        "secret-erase",
        "receipt-delete",
        "receipt-mixed",
    ] {
        for &step in STEPS {
            let f = Fixture::new();
            if mutation.starts_with("receipt-") {
                f.seed_receipt();
            }
            if mutation == "secret-erase" {
                f.connection()
                    .execute(
                        "INSERT INTO owner_secret_keys VALUES('space',?1,?2,0)",
                        rusqlite::params![f.space.to_string(), &[42u8; 32][..]],
                    )
                    .unwrap();
            }
            let old = f.baseline();
            let coordinator = injected(&f.root, f.install, step);
            let mut db = f.connection();
            let tx = db.transaction().unwrap();
            f.mutate(&tx, mutation);
            let committed = match coordinator.prepare(&tx) {
                Err(_) => {
                    drop(tx);
                    false
                }
                Ok(id) => {
                    tx.commit().unwrap();
                    assert!(coordinator.complete(&db, &id).is_err(), "{step:?}");
                    true
                }
            };
            let replay = f.coordinator();
            replay.replay(&db).unwrap();
            replay.replay(&db).unwrap();
            let snapshot = replay.validate_and_read().unwrap();
            assert_eq!(
                snapshot.tables,
                capture(&db, f.install, 1).unwrap().tables,
                "{mutation} {step:?}"
            );
            assert_eq!(snapshot.tables == old.tables, !committed);
        }
    }
}

#[test]
fn crashes_at_each_replay_publication_boundary_are_idempotent() {
    for mutation in [
        "grant",
        "revoke",
        "role",
        "transfer",
        "passkey",
        "secret-key",
        "secret-erase",
        "receipt-delete",
        "receipt-mixed",
    ] {
        for committed in [false, true] {
            for step in [
                Step::Replay,
                Step::BeforeCompletion,
                Step::BeforeAppend,
                Step::AfterPrefix,
                Step::AfterAppend,
                Step::FileSynced,
                Step::AfterCompletion,
            ] {
                let f = Fixture::new();
                if mutation.starts_with("receipt-") {
                    f.seed_receipt();
                }
                if mutation == "secret-erase" {
                    f.connection()
                        .execute(
                            "INSERT INTO owner_secret_keys VALUES('space',?1,?2,0)",
                            rusqlite::params![f.space.to_string(), &[42u8; 32][..]],
                        )
                        .unwrap();
                }
                let old = f.baseline();
                let c = f.coordinator();
                let mut db = f.connection();
                let tx = db.transaction().unwrap();
                f.mutate(&tx, mutation);
                c.prepare(&tx).unwrap();
                if committed {
                    tx.commit().unwrap();
                } else {
                    drop(tx);
                }
                assert!(
                    injected(&f.root, f.install, step).replay(&db).is_err(),
                    "{step:?}"
                );
                let replay = f.coordinator();
                replay.replay(&db).unwrap();
                replay.replay(&db).unwrap();
                let snapshot = replay.validate_and_read().unwrap();
                assert_eq!(snapshot.tables, capture(&db, f.install, 1).unwrap().tables);
                assert_eq!(snapshot.tables == old.tables, !committed);
            }
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
        f.baseline();
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
        "truncate-log",
        "flip-log",
        "missing-log",
        "truncate-snapshot",
        "flip-snapshot",
        "missing-snapshot",
    ] {
        let f = Fixture::new();
        f.baseline();
        let c = f.coordinator();
        if damage == "installation" {
            assert!(Coordinator::open(&f.root, Uuid::now_v7()).is_err());
            continue;
        }
        let path = if damage.ends_with("log") {
            log_path(&f)
        } else {
            f.root.join("snapshot.json")
        };
        if damage.starts_with("missing") {
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
        assert!(c.validate_and_read().is_err(), "{damage}");
    }
}

#[test]
fn modes_symlinks_and_space_records_keep_credentials_private() {
    let f = Fixture::new();
    f.baseline();
    let c = f.coordinator();
    for e in std::fs::read_dir(&f.root).unwrap() {
        assert_eq!(
            e.unwrap().metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let s = c.load().unwrap();
    let space = &s.image.records[&format!("2{}", f.space.as_uuid().simple())];
    let text = serde_json::to_string(space).unwrap();
    for secret in [
        "PRIVATE-PASSKEY",
        "passkeys",
        "token_hash",
        "setup_tokens",
        "installations",
    ] {
        assert!(!text.contains(secret));
    }
    let link = f._dir.path().join("link");
    std::os::unix::fs::symlink(&f.root, &link).unwrap();
    assert!(Coordinator::open(&link, f.install).is_err());
    let path = log_path(&f);
    let held = f._dir.path().join("held");
    std::fs::rename(&path, &held).unwrap();
    std::os::unix::fs::symlink(&held, &path).unwrap();
    assert!(c.validate_and_read().is_err());
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
    let _id = coordinator.prepare(&tx).unwrap();
    tx.commit().unwrap();
    c.execute(
        "UPDATE security_recovery_commits SET digest='wrong' WHERE singleton=1",
        [],
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
    let spaces = auth.spaces();
    let account = created.id;
    let listed = tokio::task::spawn_blocking(move || spaces.list(account).unwrap())
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    let personal = &listed[0];
    assert!(personal.personal);
    assert_eq!(personal.owner, created.id);
    assert_ne!(personal.id.as_uuid(), created.id.as_uuid());
    let signup = coordinator.validate_and_read().unwrap();
    assert!(
        ids(&signup, "spaces", "id")
            .unwrap()
            .contains(&personal.id.as_uuid())
    );
    assert_eq!(signup.tables, capture(&c, f.install, 1).unwrap().tables);

    assert!(
        ids(&coordinator.validate_and_read().unwrap(), "users", "id")
            .unwrap()
            .contains(&created.id.as_uuid())
    );
    for cred in [vec![1_u8], vec![2_u8]] {
        c.execute("INSERT INTO passkeys(cred_id,user_id,passkey,created_at) VALUES(?1,?2,'protected passkey fixture',1)", rusqlite::params![cred, created.id.to_string()]).unwrap();
    }
    let installation = calternal_types::InstallationId::new();
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
    let old = f.baseline();
    let c = injected(&f.root, f.install, Step::AfterPrefix);
    let mut db = f.connection();
    let tx = db.transaction().unwrap();
    f.mutate(&tx, "revoke");
    f.mutate(&tx, "passkey");
    assert!(c.prepare(&tx).is_err());
    drop(tx);
    assert!(c.validate_and_read().is_err());
    c.replay(&db).unwrap();
    assert_eq!(c.validate_and_read().unwrap().tables, old.tables);
    let c = injected(&f.root, f.install, Step::BeforeCompletion);
    let tx = db.transaction().unwrap();
    f.mutate(&tx, "revoke");
    f.mutate(&tx, "passkey");
    let id = c.prepare(&tx).unwrap();
    tx.commit().unwrap();
    assert!(c.complete(&db, &id).is_err());
    assert!(c.validate_and_read().is_err());
    c.replay(&db).unwrap();
    assert_eq!(
        c.validate_and_read().unwrap().tables,
        capture(&db, f.install, 1).unwrap().tables
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
    let installation = calternal_types::InstallationId::new();
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
    f.coordinator().reconcile(&c).unwrap();
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
    assert_eq!(coordinator.validate_and_read().unwrap().tables, old.tables);
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
    for name in ["snapshot", "segment"] {
        let f = Fixture::new();
        f.baseline();
        let c = f.coordinator();
        let path = if name == "snapshot" {
            f.root.join("snapshot.json")
        } else {
            log_path(&f)
        };
        std::fs::remove_file(&path).unwrap();
        assert!(c.validate_and_read().is_err());
        assert!(c.replay(&f.connection()).is_err());
        assert!(Coordinator::open(&f.root, f.install).is_err());
        assert!(!path.exists());
    }
}

// Independent review regressions: these deliberately express the required safety
// contract; docs/reviews/recovery-review-2026-11-17.md records their original failures before implementation.
#[test]
fn deleting_newest_generation_must_not_resurrect_revoked_membership() {
    let f = Fixture::new();
    f.baseline();
    let c = f.coordinator();
    let old = std::fs::read(log_path(&f)).unwrap();
    let mut db = f.connection();
    let tx = db.transaction().unwrap();
    f.mutate(&tx, "revoke");
    let id = c.prepare(&tx).unwrap();
    tx.commit().unwrap();
    c.complete(&db, &id).unwrap();
    let pin = c.checkpoint().unwrap();
    assert!(
        !ids(
            &c.validate_and_read().unwrap(),
            "space_memberships",
            "account_id"
        )
        .unwrap()
        .contains(&f.member.as_uuid())
    );
    fsio::write_atomic_mode(&log_path(&f), &old, 0o600).unwrap();
    // A valid prefix is indistinguishable without independent freshness evidence.
    assert!(c.validate_and_read_at(&pin).is_err());
    assert!(c.replay(&db).is_err());
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
fn completed_transaction_retention_is_bounded() {
    let f = Fixture::new();
    f.baseline();
    let c = Arc::new(
        Coordinator::with_compaction_threshold(&f.root, f.install, 1, Arc::new(|_| Ok(())))
            .unwrap(),
    );
    let spaces = Spaces::open(&f.db).unwrap();
    spaces.configure_recovery(c.clone()).unwrap();
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
    assert_eq!(std::fs::read_dir(&f.root).unwrap().count(), 2);
    assert_eq!(std::fs::metadata(log_path(&f)).unwrap().len(), 0);
    assert_eq!(
        c.validate_and_read().unwrap().tables,
        capture(&f.connection(), f.install, 1).unwrap().tables
    );
}

fn log_path(f: &Fixture) -> PathBuf {
    let image: Image = decode(&std::fs::read(f.root.join("snapshot.json")).unwrap()).unwrap();
    f.root.join(image.segment)
}

#[test]
fn telemetry_changes_do_not_publish() {
    let f = Fixture::new();
    let installation = calternal_types::InstallationId::new();
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
    c.reconcile(&db).unwrap();
    calls.store(0, Ordering::SeqCst);
    let tx = db.transaction().unwrap();
    tx.execute(
        "UPDATE installations SET last_seen_at=50,expires_at=150 WHERE id=?1",
        [installation.to_string()],
    )
    .unwrap();
    tx.execute("UPDATE passkeys SET last_used_at=50", [])
        .unwrap();
    let id = c.prepare(&tx).unwrap();
    assert_eq!(id, "");
    tx.commit().unwrap();
    c.complete(&db, &id).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let snapshot = c.validate_and_read_at(&pin).unwrap();
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
fn whole_tree_rollback_requires_a_surviving_database_or_external_checkpoint() {
    let f = Fixture::new();
    f.baseline();
    fn files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        std::fs::read_dir(root)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (
                    PathBuf::from(e.file_name()),
                    std::fs::read(e.path()).unwrap(),
                )
            })
            .collect()
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
            .contains(&f.member.as_uuid())
    );
    assert!(
        ids(&snapshot, "spaces", "id")
            .unwrap()
            .contains(&created.id.as_uuid())
    );
    assert_eq!(
        snapshot.tables,
        capture(&f.connection(), f.install, 1).unwrap().tables
    );
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
    count.store(0, Ordering::SeqCst);
    let first = apply().unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2);
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

/// Machine authority (codes, enrollment, revocation, pool grants) is protected
/// state: each change is published (two file fsyncs) before it returns, account-
/// owned rows land in the owner's record and pool rows in the global record, and a
/// refused publication leaves the authority unchanged.
#[test]
fn machine_authority_is_published_before_acknowledgement() {
    let f = Fixture::new();
    f.baseline();
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (observed, failing) = (count.clone(), fail.clone());
    let c = Arc::new(
        Coordinator::with_observer(
            &f.root,
            f.install,
            Arc::new(move |s| {
                if matches!(s, Step::BeforeIntent) && failing.swap(false, Ordering::SeqCst) {
                    return Err(err("injected crash before the intent"));
                }
                if matches!(s, Step::FileSynced) {
                    observed.fetch_add(1, Ordering::SeqCst);
                }
                Ok(())
            }),
        )
        .unwrap(),
    );
    let spaces = Spaces::open(&f.db).unwrap();
    spaces.configure_recovery(c.clone()).unwrap();
    let machines = spaces.machines();
    // A publication that fails before commit leaves no authority behind.
    fail.store(true, Ordering::SeqCst);
    assert!(machines.set_pool_grant(f.outsider, true).is_err());
    assert!(machines.pool_grants().unwrap().is_empty());
    let published = |label: &str, run: &dyn Fn()| {
        count.store(0, Ordering::SeqCst);
        run();
        assert_eq!(count.load(Ordering::SeqCst), 2, "{label} was not published");
    };
    count.store(0, Ordering::SeqCst);
    let (code, _) = machines
        .issue_code(Some(f.owner), std::time::Duration::from_secs(60))
        .unwrap();
    assert_eq!(
        count.load(Ordering::SeqCst),
        2,
        "an owner's code was not published"
    );
    let canon = calternal_auth::token::normalize_setup_token(&code).unwrap();
    count.store(0, Ordering::SeqCst);
    let machine = machines
        .redeem_code(
            calternal_auth::token::hash32(&canon),
            vec![7; 32],
            "laptop".into(),
            "linux".into(),
            "x86_64".into(),
            false,
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        count.load(Ordering::SeqCst),
        2,
        "enrollment was not published"
    );
    published("a pool code", &|| {
        machines
            .issue_code(None, std::time::Duration::from_secs(60))
            .unwrap();
    });
    published("a pool grant", &|| {
        machines.set_pool_grant(f.member, true).unwrap()
    });
    published("a revocation", &|| {
        assert!(machines.revoke(machine.id).unwrap());
    });
    let snapshot = c.validate_and_read().unwrap();
    assert_eq!(snapshot.tables["machines"].rows.len(), 1);
    assert_eq!(snapshot.tables["machine_pool_grants"].rows.len(), 1);
    assert_eq!(snapshot.tables["machine_codes"].rows.len(), 2);
    assert_eq!(
        snapshot.tables,
        capture(&f.connection(), f.install, snapshot.sequence)
            .unwrap()
            .tables
    );
}

#[test]
fn every_change_kind_has_two_file_fsyncs_and_no_directory_fsyncs() {
    for extra in [0, 12] {
        for mutation in [
            "grant",
            "revoke",
            "role",
            "transfer",
            "passkey",
            "account",
            "new-account",
            "create",
            "delete",
            "delete-account",
            "both",
            "noop",
        ] {
            let f = Fixture::new();
            for n in 0..extra {
                let id = AccountId::new();
                f.connection()
                    .execute(
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
            let counts = Arc::new(Mutex::new((0, 0)));
            let seen = counts.clone();
            let c = Coordinator::with_observer(
                &f.root,
                f.install,
                Arc::new(move |step| {
                    let mut n = seen.lock().unwrap();
                    if step == Step::FileSynced {
                        n.0 += 1;
                    }
                    if step == Step::DirectorySynced {
                        n.1 += 1;
                    }
                    Ok(())
                }),
            )
            .unwrap();
            let mut db = f.connection();
            c.reconcile(&db).unwrap();
            *counts.lock().unwrap() = (0, 0);
            let tx = db.transaction().unwrap();
            match mutation {
                "new-account" => {
                    tx.execute(
                        "INSERT INTO users(id,name,username,created_at) VALUES(?1,'new','new',1)",
                        [AccountId::new().to_string()],
                    )
                    .unwrap();
                }
                "account" => {
                    tx.execute(
                        "UPDATE users SET name='changed' WHERE id=?1",
                        [f.owner.to_string()],
                    )
                    .unwrap();
                }
                "create" => {
                    let id = SpaceId::new();
                    tx.execute(
                        "INSERT INTO spaces(id,owner_account,name,created) VALUES(?1,?2,'new',1)",
                        rusqlite::params![id.to_string(), f.owner.to_string()],
                    )
                    .unwrap();
                    tx.execute(
                        "INSERT INTO space_memberships VALUES(?1,?2,'manager')",
                        rusqlite::params![id.to_string(), f.owner.to_string()],
                    )
                    .unwrap();
                }
                "delete" => {
                    tx.execute("DELETE FROM spaces WHERE id=?1", [f.space.to_string()])
                        .unwrap();
                }
                "delete-account" => {
                    tx.execute("DELETE FROM users WHERE id=?1", [f.outsider.to_string()])
                        .unwrap();
                }
                "both" => {
                    f.mutate(&tx, "revoke");
                    f.mutate(&tx, "passkey");
                }
                "noop" => {}
                _ => f.mutate(&tx, mutation),
            }
            let before = std::fs::metadata(log_path(&f)).unwrap().len() as usize;
            let id = c.prepare(&tx).unwrap();
            if mutation != "noop" {
                let bytes = std::fs::read(log_path(&f)).unwrap();
                let proposed: Entry = decode(&bytes[before + 8..]).unwrap();
                let Body::Proposed { delta, .. } = proposed.body else {
                    panic!("expected delta")
                };
                assert_eq!(
                    delta.len(),
                    if matches!(mutation, "both" | "delete" | "delete-account") {
                        2
                    } else {
                        1
                    },
                    "{mutation}, unrelated={extra}"
                );
            } else {
                assert_eq!(
                    std::fs::metadata(log_path(&f)).unwrap().len(),
                    before as u64
                );
            }
            tx.commit().unwrap();
            c.complete(&db, &id).unwrap();
            assert_eq!(
                *counts.lock().unwrap(),
                (if mutation == "noop" { 0 } else { 2 }, 0),
                "{mutation}, unrelated={extra}"
            );
            assert_eq!(
                c.validate_and_read().unwrap().tables,
                capture(&db, f.install, 1).unwrap().tables
            );
            if matches!(mutation, "delete" | "delete-account") {
                let kind = if mutation == "delete" {
                    "space"
                } else {
                    "account"
                };
                let owner = if mutation == "delete" {
                    f.space.as_uuid()
                } else {
                    f.outsider.as_uuid()
                };
                c.compact_erasure(&db).unwrap();
                let recovered = f.coordinator().validate_and_read().unwrap();
                assert_eq!(
                    ids(&recovered, "owner_erasures", "id").unwrap(),
                    vec![owner]
                );
                assert_eq!(
                    recovered.tables["owner_erasures"].rows,
                    vec![vec![
                        Cell::Text(kind.into()),
                        Cell::Text(owner.to_string()),
                        Cell::Integer(1)
                    ]],
                );
            }
        }
    }
}

#[test]
fn all_compaction_boundaries_preserve_the_acknowledged_state() {
    use std::sync::atomic::AtomicUsize;
    for mutation in ["revoke", "receipt-delete", "receipt-mixed"] {
        for (step, positions) in [
            (Step::BeforeSegment, 1),
            (Step::AfterSegment, 1),
            (Step::BeforeSnapshot, 1),
            (Step::AfterSnapshot, 1),
            (Step::BeforeCompact, 1),
            (Step::AfterCompact, 1),
            (Step::BeforeCompactSync, 2),
            (Step::AfterCompactSync, 2),
            (Step::FileSynced, 4),
            (Step::DirectorySynced, 3),
        ] {
            for position in 0..positions {
                let f = Fixture::new();
                if mutation.starts_with("receipt-") {
                    f.seed_receipt();
                }
                f.baseline();
                let armed = Arc::new(AtomicBool::new(false));
                let enabled = armed.clone();
                let hits = AtomicUsize::new(0);
                let c = Coordinator::with_compaction_threshold(
                    &f.root,
                    f.install,
                    1,
                    Arc::new(move |s| {
                        if enabled.load(Ordering::SeqCst)
                            && s == step
                            && hits.fetch_add(1, Ordering::SeqCst) == position
                        {
                            return Err(err("compaction crash"));
                        }
                        Ok(())
                    }),
                )
                .unwrap();
                let mut db = f.connection();
                c.reconcile(&db).unwrap();
                armed.store(true, Ordering::SeqCst);
                let tx = db.transaction().unwrap();
                f.mutate(&tx, mutation);
                match c.prepare(&tx) {
                    Err(_) => {
                        drop(tx);
                    }
                    Ok(id) => {
                        tx.commit().unwrap();
                        assert!(c.complete(&db, &id).is_err(), "{step:?} {position}");
                    }
                }
                let replay = f.coordinator();
                replay.reconcile(&db).unwrap();
                replay.replay(&db).unwrap();
                assert_eq!(
                    replay.validate_and_read().unwrap().tables,
                    capture(&db, f.install, 1).unwrap().tables,
                    "{step:?} {position}"
                );
                assert_eq!(std::fs::read_dir(&f.root).unwrap().count(), 2);
            }
        }
    }
}

#[test]
fn compaction_adds_exactly_two_file_and_three_directory_fsyncs() {
    let f = Fixture::new();
    f.baseline();
    let counts = Arc::new(Mutex::new((0, 0)));
    let seen = counts.clone();
    let c = Coordinator::with_compaction_threshold(
        &f.root,
        f.install,
        1,
        Arc::new(move |s| {
            let mut n = seen.lock().unwrap();
            if s == Step::FileSynced {
                n.0 += 1;
            }
            if s == Step::DirectorySynced {
                n.1 += 1;
            }
            Ok(())
        }),
    )
    .unwrap();
    let mut db = f.connection();
    c.reconcile(&db).unwrap();
    *counts.lock().unwrap() = (0, 0);
    let pin = c.checkpoint().unwrap();
    let tx = db.transaction().unwrap();
    f.mutate(&tx, "revoke");
    let id = c.prepare(&tx).unwrap();
    tx.commit().unwrap();
    c.complete(&db, &id).unwrap();
    assert_eq!(*counts.lock().unwrap(), (4, 3));
    assert!(c.validate_and_read_at(&pin).is_err());
    let pin = c.checkpoint().unwrap();
    let s = c.load().unwrap();
    assert_eq!(s.length, 0);
    assert_eq!(s.image.head, pin);
    c.replay(&db).unwrap();
}

#[test]
fn frame_deletion_substitution_reordering_and_sequence_gaps_refuse() {
    for damage in [
        "delete",
        "substitute",
        "reorder",
        "gap",
        "wrong-installation",
    ] {
        let f = Fixture::new();
        f.baseline();
        let c = f.coordinator();
        let path = log_path(&f);
        let old = std::fs::read(&path).unwrap();
        let mut db = f.connection();
        let tx = db.transaction().unwrap();
        f.mutate(&tx, "revoke");
        let id = c.prepare(&tx).unwrap();
        tx.commit().unwrap();
        c.complete(&db, &id).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let mut frames = Vec::new();
        let mut pos = 0;
        while pos < bytes.len() {
            let end =
                pos + 8 + u64::from_le_bytes(bytes[pos..pos + 8].try_into().unwrap()) as usize;
            frames.push(bytes[pos..end].to_vec());
            pos = end;
        }
        match damage {
            "delete" => {
                frames.remove(1);
            }
            "substitute" => {
                frames[2] = frames[0].clone();
            }
            "reorder" => {
                frames.swap(0, 2);
            }
            _ => {
                let mut e: Entry = decode(&frames[2][8..]).unwrap();
                if damage == "gap" {
                    e.sequence += 1;
                } else {
                    e.installation = Uuid::now_v7();
                }
                frames[2] = frame(&e).unwrap();
            }
        }
        assert!(old.len() < bytes.len());
        fsio::write_atomic_mode(&path, &frames.concat(), 0o600).unwrap();
        assert!(c.validate_and_read().is_err(), "{damage}");
        assert!(c.replay(&db).is_err(), "{damage}");
    }
}

#[test]
fn torn_marker_prefixes_repair_only_from_the_exact_committed_witness() {
    for committed in [false, true] {
        for cut in [0, 1, 7, 8, 9, 33, 100] {
            let f = Fixture::new();
            let old = f.baseline();
            let c = f.coordinator();
            let mut db = f.connection();
            let tx = db.transaction().unwrap();
            f.mutate(&tx, "grant");
            c.prepare(&tx).unwrap();
            if committed {
                tx.commit().unwrap();
            } else {
                drop(tx);
            }
            let s = c.load().unwrap();
            let marker = s.resolution(committed).unwrap().0;
            assert!(cut < marker.len());
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(log_path(&f))
                .unwrap();
            use std::io::Write;
            file.write_all(&marker[..cut]).unwrap();
            drop(file);
            assert!(c.validate_and_read().is_err());
            let replay = f.coordinator();
            replay.replay(&db).unwrap();
            assert_eq!(
                replay.validate_and_read().unwrap().tables,
                capture(&db, f.install, 1).unwrap().tables
            );
            assert_eq!(
                replay.validate_and_read().unwrap().tables == old.tables,
                !committed
            );
        }
    }
}

#[test]
fn corrupt_resolution_tail_is_preserved_and_refused() {
    let f = Fixture::new();
    f.baseline();
    let c = f.coordinator();
    let mut db = f.connection();
    let tx = db.transaction().unwrap();
    f.mutate(&tx, "revoke");
    c.prepare(&tx).unwrap();
    tx.commit().unwrap();
    use std::io::Write;
    let path = log_path(&f);
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&[255; 8])
        .unwrap();
    let damaged = std::fs::read(&path).unwrap();
    assert!(c.replay(&db).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), damaged);
    assert!(c.validate_and_read().is_err());
}

#[test]
fn tail_repair_can_be_interrupted_without_recovering_an_aborted_proposal() {
    for mutation in ["grant", "receipt-delete", "receipt-mixed"] {
        for step in [Step::BeforeTailRepair, Step::AfterTailRepair] {
            let f = Fixture::new();
            if mutation.starts_with("receipt-") {
                f.seed_receipt();
            }
            let old = f.baseline();
            let c = injected(&f.root, f.install, Step::AfterPrefix);
            let mut db = f.connection();
            let tx = db.transaction().unwrap();
            f.mutate(&tx, mutation);
            assert!(c.prepare(&tx).is_err());
            drop(tx);
            assert!(injected(&f.root, f.install, step).replay(&db).is_err());
            let c = f.coordinator();
            c.replay(&db).unwrap();
            assert_eq!(
                c.validate_and_read().unwrap().tables,
                old.tables,
                "{mutation} {step:?}"
            );
        }
    }
}

#[test]
fn cached_writer_detects_snapshot_or_segment_replacement() {
    let f = Fixture::new();
    f.baseline();
    let c = f.coordinator();
    let mut db = f.connection();
    c.replay(&db).unwrap();
    let old = std::fs::read(log_path(&f)).unwrap();
    let tx = db.transaction().unwrap();
    f.mutate(&tx, "revoke");
    let id = c.prepare(&tx).unwrap();
    tx.commit().unwrap();
    c.complete(&db, &id).unwrap();
    fsio::write_atomic_mode(&log_path(&f), &old, 0o600).unwrap();
    let tx = db.transaction().unwrap();
    f.mutate(&tx, "grant");
    assert!(c.prepare(&tx).is_err());
}

#[test]
fn missing_compacted_segment_never_falls_back_to_the_old_segment() {
    let f = Fixture::new();
    f.baseline();
    let fired = AtomicBool::new(false);
    let c = Coordinator::with_compaction_threshold(
        &f.root,
        f.install,
        1,
        Arc::new(move |s| {
            if s == Step::AfterSnapshot && !fired.swap(true, Ordering::SeqCst) {
                return Err(err("snapshot published"));
            }
            Ok(())
        }),
    )
    .unwrap();
    assert!(c.replay(&f.connection()).is_err());
    assert_eq!(std::fs::read_dir(&f.root).unwrap().count(), 3);
    std::fs::remove_file(log_path(&f)).unwrap();
    assert!(c.validate_and_read().is_err());
    assert!(c.replay(&f.connection()).is_err());
    assert!(Coordinator::open(&f.root, f.install).is_err());
}

#[test]
fn compaction_preserves_the_exact_external_checkpoint() {
    let f = Fixture::new();
    let expected = f.baseline();
    let pin = f.coordinator().checkpoint().unwrap();
    let c = Coordinator::with_compaction_threshold(&f.root, f.install, 1, Arc::new(|_| Ok(())))
        .unwrap();
    c.replay(&f.connection()).unwrap();
    assert_eq!(c.checkpoint().unwrap(), pin);
    assert_eq!(c.validate_and_read_at(&pin).unwrap(), expected);
}

#[test]
fn failed_prepare_reconciles_only_after_the_authority_transaction_rolls_back() {
    let f = Fixture::new();
    let old = f.baseline();
    let spaces = Spaces::open(&f.db).unwrap();
    let sink = Arc::new(injected(&f.root, f.install, Step::AfterIntent));
    spaces.configure_recovery(sink.clone()).unwrap();
    assert!(spaces.remove(f.owner, f.space, f.member).is_err());
    assert_eq!(sink.validate_and_read().unwrap().tables, old.tables);
    spaces.remove(f.owner, f.space, f.member).unwrap();
    assert_eq!(
        sink.validate_and_read().unwrap().tables,
        capture(&f.connection(), f.install, 1).unwrap().tables
    );
}

#[test]
fn secret_key_lifecycle_is_protected_idempotent_and_cannot_revive() {
    let f = Fixture::new();
    f.baseline();
    let coordinator = Arc::new(f.coordinator());
    let spaces = Spaces::open(&f.db).unwrap();
    spaces.configure_recovery(coordinator.clone()).unwrap();
    let id = f.space.as_uuid();
    for erasing in [false, false, true, true] {
        spaces
            .record_secret_key("space", id, [42; 32], erasing)
            .unwrap();
    }
    assert!(
        spaces
            .record_secret_key("space", id, [42; 32], false)
            .is_err()
    );
    assert!(
        spaces
            .record_secret_key("space", id, [43; 32], true)
            .is_err()
    );
    let snapshot = coordinator.validate_and_read().unwrap();
    let table = &snapshot.tables["owner_secret_keys"];
    let erasing = table.columns.iter().position(|c| c == "erasing").unwrap();
    assert_eq!(table.rows[0][erasing], Cell::Integer(1));
    spaces.delete(f.owner, f.space).unwrap();
    spaces
        .record_secret_key("space", id, [42; 32], true)
        .unwrap();
    assert_eq!(
        coordinator.validate_and_read().unwrap().tables["owner_secret_keys"].rows[0][erasing],
        Cell::Integer(1)
    );
}

#[path = "startup_tests.rs"]
mod startup_tests;

#[path = "account_tests.rs"]
mod account_tests;
