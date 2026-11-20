//! Storage halves of the running-execution qualification cases A–E: exact
//! reattachment, Stop on the exact old attempt, steer ledger and held Sends.
//! Each reopens the real SQLite file where a server restart is modelled.
use super::tests::{pin, seed_item, upgrade};
use super::*;
use rusqlite::Connection;
use tempfile::tempdir;

fn running(tx: &Transaction<'_>, pin: &PinnedExecution) -> (Admission, Attempt) {
    let admitted = admit(tx, pin, 64).unwrap();
    claim(tx, &admitted.job, |_| Ok(true)).unwrap().unwrap();
    let attempt = start(tx, &admitted.job, &pin.harness, |_| Ok(true))
        .unwrap()
        .attempt;
    (admitted, attempt)
}

fn binding(pin: &PinnedExecution) -> LaunchBinding {
    LaunchBinding {
        reservation: Uuid::now_v7(),
        machine: pin.machine,
        boot: Uuid::now_v7(),
        ticket: Uuid::now_v7(),
        call: "call-1".into(),
    }
}

fn attempts(db: &Connection) -> i64 {
    db.query_row("SELECT count(*) FROM execution_attempts", [], |r| r.get(0))
        .unwrap()
}

/// Cases A and D: the committed binding survives a server kill (reopen of
/// the file) and the same reservation on the same boot resumes the same
/// attempt; forged, foreign and rewinding offers change nothing.
#[test]
fn reattach_resumes_only_the_committed_reservation_on_the_same_boot() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("content.db");
    let mut db = Connection::open(&path).unwrap();
    upgrade(&db);
    let item = ItemRef(Uuid::now_v7().to_string());
    seed_item(&db, &item);
    let pin = pin(item, AccountId::new());
    let launch = binding(&pin);
    let tx = db.transaction().unwrap();
    let (_, attempt) = running(&tx, &pin);
    let mut wrong_machine = launch.clone();
    wrong_machine.machine = Uuid::now_v7();
    assert!(bind_launch(&tx, &attempt, &wrong_machine, |_| Ok(true)).is_err());
    assert!(bind_launch(&tx, &attempt, &launch, |_| Ok(false)).is_err());
    bind_launch(&tx, &attempt, &launch, |_| Ok(true)).unwrap();
    bind_launch(&tx, &attempt, &launch, |_| Ok(true)).unwrap();
    let mut other = launch.clone();
    other.ticket = Uuid::now_v7();
    assert!(bind_launch(&tx, &attempt, &other, |_| Ok(true)).is_err());
    tx.commit().unwrap();
    drop(db);

    // Server restart: a fresh connection on the same file.
    let mut db = Connection::open(&path).unwrap();
    db.pragma_update(None, "foreign_keys", "ON").unwrap();
    let tx = db.transaction().unwrap();
    let r = launch.reservation;
    assert_eq!(
        reattach(&tx, &Uuid::now_v7(), &launch.machine, &launch.boot, |_| {
            panic!("forged reservation reached authority")
        })
        .unwrap(),
        None
    );
    assert_eq!(
        reattach(&tx, &r, &Uuid::now_v7(), &launch.boot, |_| panic!(
            "foreign"
        ))
        .unwrap(),
        None
    );
    match reattach(&tx, &r, &launch.machine, &launch.boot, |_| Ok(true)).unwrap() {
        Some(Reattach::Resume {
            attempt: resumed,
            binding: bound,
            ..
        }) => {
            assert_eq!(resumed, attempt);
            assert_eq!(bound, launch);
        }
        other => panic!("expected resume, got {other:?}"),
    }
    let boot = launch.boot;
    assert!(verify_attached(&tx, &r, &boot, &Uuid::now_v7(), 3, 1).is_err());
    assert!(verify_attached(&tx, &r, &Uuid::now_v7(), &r, 3, 1).is_err());
    assert!(verify_attached(&tx, &r, &boot, &r, 0, 1).is_err());
    assert_eq!(
        verify_attached(&tx, &r, &boot, &r, 3, 1).unwrap(),
        Attached::Live(attempt.clone())
    );
    assert_eq!(
        verify_attached(&tx, &r, &boot, &r, 1, 1).unwrap(),
        Attached::Live(attempt.clone())
    );
    assert!(is_current(&tx, &attempt, |_| Ok(true)).unwrap());
    assert_eq!(attempts(&tx), 1);
}

/// Cases A/D proof-loss arms and case "new boot": a new boot commits
/// interruption with an exact Stop and starts nothing; lost authority on
/// reconnect gives the cleanup-only path.
#[test]
fn new_boot_or_lost_authority_retires_without_a_replacement() {
    let mut db = Connection::open_in_memory().unwrap();
    upgrade(&db);
    let item = ItemRef(Uuid::now_v7().to_string());
    seed_item(&db, &item);
    let pin = pin(item.clone(), AccountId::new());
    let tx = db.transaction().unwrap();
    let (_, attempt) = running(&tx, &pin);
    let launch = binding(&pin);
    bind_launch(&tx, &attempt, &launch, |_| Ok(true)).unwrap();
    let rebooted = Uuid::now_v7();
    assert_eq!(
        reattach(&tx, &launch.reservation, &launch.machine, &rebooted, |_| {
            panic!("a new boot is never re-authorized")
        })
        .unwrap(),
        Some(Reattach::Failed {
            attempt: attempt.clone()
        })
    );
    assert!(!is_current(&tx, &attempt, |_| Ok(true)).unwrap());
    assert_eq!(cleanup(&tx, 8).unwrap(), [attempt.clone()]);
    let stop = outstanding(&tx, &attempt).unwrap();
    assert_eq!(stop.len(), 1);
    assert_eq!(stop[0].kind, ControlKind::Stop);
    // The old runner may still attach for cleanup; it never resumes.
    assert!(matches!(
        reattach(
            &tx,
            &launch.reservation,
            &launch.machine,
            &launch.boot,
            |_| Ok(true)
        )
        .unwrap(),
        Some(Reattach::CleanupOnly { .. })
    ));
    assert_eq!(attempts(&tx), 1);

    // A second item whose authority is gone at reconnect.
    let other = ItemRef(Uuid::now_v7().to_string());
    tx.execute(
        "INSERT INTO items(id,kind,metadata,revision,seq,updated_at,path,folded_path,mat_state)
         VALUES(?1,'example','{}','initial',2,'today','other.md','other.md','ok')",
        [&other.0],
    )
    .unwrap();
    let second = super::tests::pin(other, AccountId::new());
    let (_, revoked) = running(&tx, &second);
    let launch = binding(&second);
    bind_launch(&tx, &revoked, &launch, |_| Ok(true)).unwrap();
    assert!(matches!(
        reattach(&tx, &launch.reservation, &launch.machine, &launch.boot, |_| Ok(false)).unwrap(),
        Some(Reattach::CleanupOnly { attempt, .. }) if attempt == revoked
    ));
    assert_eq!(
        verify_attached(
            &tx,
            &launch.reservation,
            &launch.boot,
            &launch.reservation,
            2,
            2
        )
        .unwrap(),
        Attached::CleanupOnly(revoked)
    );
}

/// Case C: a committed Stop survives restart and is delivered only to the
/// exact old attempt; the successor never sees it, and a delayed duplicate
/// or forged ACK changes nothing.
#[test]
fn committed_stop_reaches_only_its_old_attempt_after_restart() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("content.db");
    let mut db = Connection::open(&path).unwrap();
    upgrade(&db);
    let item = ItemRef(Uuid::now_v7().to_string());
    seed_item(&db, &item);
    let first = pin(item.clone(), AccountId::new());
    let second = pin(item.clone(), AccountId::new());
    let tx = db.transaction().unwrap();
    let (_, old) = running(&tx, &first);
    let launch = binding(&first);
    bind_launch(&tx, &old, &launch, |_| Ok(true)).unwrap();
    assert_eq!(steer(&tx, &old, "steer-sent", "go on").unwrap(), 1);
    assert_eq!(outstanding(&tx, &old).unwrap().len(), 1);
    assert_eq!(steer(&tx, &old, "steer-unsent", "and more").unwrap(), 2);
    let successor = admit(&tx, &second, 64).unwrap();
    stop(&tx, &old, "stop-a", |_, _| Ok(())).unwrap();
    assert!(stop(&tx, &old, "stop-a", |_, _| Ok(())).is_err());
    tx.commit().unwrap();
    drop(db);

    let mut db = Connection::open(&path).unwrap();
    db.pragma_update(None, "foreign_keys", "ON").unwrap();
    let tx = db.transaction().unwrap();
    let Some(Reattach::CleanupOnly { attempt, .. }) = reattach(
        &tx,
        &launch.reservation,
        &launch.machine,
        &launch.boot,
        |_| panic!("retired attempt re-authorized"),
    )
    .unwrap() else {
        panic!("expected the cleanup-only path");
    };
    assert_eq!(attempt, old);
    let deliveries = outstanding(&tx, &old).unwrap();
    assert_eq!(deliveries.len(), 1, "only the Stop; steers are fenced");
    assert_eq!(deliveries[0].kind, ControlKind::Stop);
    assert_eq!(deliveries[0].request_id, "stop-a");
    let state: String = tx
        .query_row(
            "SELECT state FROM execution_controls WHERE request_id='steer-unsent'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(state, "undelivered");
    let stop_seq = deliveries[0].seq;
    assert!(
        !ack_control(
            &tx,
            &old.execution,
            stop_seq,
            "forged",
            ControlState::Applied
        )
        .unwrap()
    );
    assert!(
        ack_control(
            &tx,
            &old.execution,
            stop_seq,
            "stop-a",
            ControlState::Applied
        )
        .unwrap()
    );
    assert!(
        !ack_control(
            &tx,
            &old.execution,
            stop_seq,
            "stop-a",
            ControlState::Applied
        )
        .unwrap()
    );
    assert!(settle(&tx, &old).unwrap());
    assert_eq!(
        promote(&tx, &item, |_| Ok(true)).unwrap(),
        Some(successor.job.clone())
    );
    claim(&tx, &successor.job, |_| Ok(true)).unwrap().unwrap();
    let new = start(&tx, &successor.job, &second.harness, |_| Ok(true))
        .unwrap()
        .attempt;
    // The delayed duplicate of A's Stop targets the old exact attempt only.
    assert!(stop(&tx, &old, "stop-a", |_, _| Ok(())).is_err());
    assert!(
        !ack_control(
            &tx,
            &old.execution,
            stop_seq,
            "stop-a",
            ControlState::Applied
        )
        .unwrap()
    );
    assert!(is_current(&tx, &new, |_| Ok(true)).unwrap());
    assert!(outstanding(&tx, &new).unwrap().is_empty());
}

/// Steer ledger bounds and deduplication by request ID.
#[test]
fn steers_are_bounded_deduplicated_and_retired_with_the_attempt() {
    let mut db = Connection::open_in_memory().unwrap();
    upgrade(&db);
    let item = ItemRef(Uuid::now_v7().to_string());
    seed_item(&db, &item);
    let pin = pin(item, AccountId::new());
    let tx = db.transaction().unwrap();
    let (_, attempt) = running(&tx, &pin);
    assert!(steer(&tx, &attempt, "empty", "").is_err());
    let largest = "x".repeat(ENCODED_STEER_BYTES - 1);
    assert!(steer(&tx, &attempt, "too-large", &format!("{largest}x")).is_err());
    assert_eq!(steer(&tx, &attempt, "s-0", &largest).unwrap(), 1);
    assert_eq!(steer(&tx, &attempt, "s-0", &largest).unwrap(), 1);
    assert_eq!(
        steer(&tx, &attempt, "s-0", "changed").unwrap_err(),
        Error::request_reused()
    );
    for n in 1..PENDING_STEERS_PER_ATTEMPT {
        steer(&tx, &attempt, &format!("s-{n}"), "more").unwrap();
    }
    assert_eq!(
        steer(&tx, &attempt, "overflow", "more")
            .unwrap_err()
            .details,
        Some(json!({"reason": "pending_limit"}))
    );
    let delivered = outstanding(&tx, &attempt).unwrap();
    assert_eq!(delivered.len(), 32);
    assert!(ack_control(&tx, &attempt.execution, 1, "s-0", ControlState::Applied).unwrap());
    steer(&tx, &attempt, "after-ack", "more").unwrap();
    // The reserved Stop slot is independent of the full steer budget.
    stop(&tx, &attempt, "stop", |_, _| Ok(())).unwrap();
    assert!(steer(&tx, &attempt, "late", "more").is_err());
    let after = outstanding(&tx, &attempt).unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].kind, ControlKind::Stop);
}

/// Cases B and E: Sends held while admission is closed keep their receipt
/// identities, survive restart, and are released once in original order
/// under live checks; ordinary queue limits still apply.
#[test]
fn held_sends_survive_restart_and_release_once_in_order() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("content.db");
    let mut db = Connection::open(&path).unwrap();
    upgrade(&db);
    let item = ItemRef(Uuid::now_v7().to_string());
    seed_item(&db, &item);
    let a = pin(item.clone(), AccountId::new());
    let b = pin(item.clone(), AccountId::new());
    let c = pin(item.clone(), AccountId::new());
    let revoked = pin(item.clone(), AccountId::new());
    let tx = db.transaction().unwrap();
    let held: Vec<Admission> = [&a, &revoked, &b, &c]
        .into_iter()
        .map(|p| hold(&tx, p, 64).unwrap())
        .collect();
    assert_eq!(unsettled(&tx).unwrap(), 4);
    assert_eq!(held_count(&tx).unwrap(), 4);
    assert!(
        claim(&tx, &held[0].job, |_| panic!("held work claimed"))
            .unwrap()
            .is_none()
    );
    let targets = archive_targets(&tx, &item).unwrap();
    assert_eq!(targets.len(), 4);
    assert!(targets.iter().all(|t| t.queued && t.attempt.is_none()));
    tx.commit().unwrap();
    drop(db);

    let mut db = Connection::open(&path).unwrap();
    db.pragma_update(None, "foreign_keys", "ON").unwrap();
    let tx = db.transaction().unwrap();
    let released = release_held(&tx, 64, |p| Ok(p.actor != revoked.actor)).unwrap();
    assert_eq!(
        released,
        [
            Released::Admitted(Admission {
                job: held[0].job.clone(),
                intent: held[0].intent.clone()
            }),
            Released::Refused(held[1].job.clone()),
            Released::Admitted(Admission {
                job: held[2].job.clone(),
                intent: held[2].intent.clone()
            }),
            Released::Refused(held[3].job.clone()),
        ]
    );
    assert!(
        release_held(&tx, 64, |_| panic!("second release"))
            .unwrap()
            .is_empty()
    );
    let head = claim(&tx, &held[0].job, |_| Ok(true)).unwrap().unwrap();
    assert_eq!(head.pin, a);
    assert!(claim(&tx, &held[2].job, |_| Ok(true)).unwrap().is_none());
    assert_eq!(unsettled(&tx).unwrap(), 2);
}

/// A held Send is cancellable by its exact Stop before admission.
#[test]
fn held_send_can_be_stopped_before_release() {
    let mut db = Connection::open_in_memory().unwrap();
    upgrade(&db);
    let item = ItemRef(Uuid::now_v7().to_string());
    seed_item(&db, &item);
    let a = pin(item.clone(), AccountId::new());
    let tx = db.transaction().unwrap();
    let held = hold(&tx, &a, 64).unwrap();
    stop_pending(&tx, &held.intent, |target, starter| {
        assert_eq!(target, &item);
        assert_eq!(starter, a.actor);
        Ok(())
    })
    .unwrap();
    assert!(stop_pending(&tx, &held.intent, |_, _| Ok(())).is_err());
    assert!(
        release_held(&tx, 64, |_| panic!("stopped Send released"))
            .unwrap()
            .is_empty()
    );
    assert_eq!(unsettled(&tx).unwrap(), 0);
}
