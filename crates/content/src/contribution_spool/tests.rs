use super::*;
use calternal_types::ErrorCode;
use rusqlite::Connection;
use serde_json::json;
use std::path::Path;

const SCHEMA: &str = include_str!("test_schema.sql");

fn open_db(path: &Path) -> Connection {
    let db = Connection::open(path).unwrap();
    db.pragma_update(None, "foreign_keys", true).unwrap();
    db.pragma_update(None, "synchronous", "FULL").unwrap();
    db
}

fn setup(path: &Path) -> Connection {
    let db = open_db(path);
    let migration = calternal_store::migrations::Schema::new("test spool", 1, &[], &[(2, SCHEMA)]);
    calternal_store::migrations::migrate(&db, migration).unwrap();
    db.execute_batch("CREATE TABLE live(attempt TEXT PRIMARY KEY, generation INTEGER); INSERT INTO live VALUES('attempt',1);")
        .unwrap();
    db
}

fn checked<'t, 'c>(tx: &'t Transaction<'c>, generation: i64) -> Result<Spool<'t, 'c>> {
    Spool::checked(
        tx,
        &ExecutionRef("attempt".into()),
        generation,
        |tx, execution, generation| {
            tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM live WHERE attempt=?1 AND generation=?2)",
                params![execution.0, generation],
                |row| row.get(0),
            )
            .map_err(internal)
        },
    )
}

#[test]
fn scripted_call_and_lost_ack_replay_after_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("space.db");
    let mut db = setup(&path);
    let mut launches = 0;
    let first = json!({"seq":1,"kind":"output","data":{"text":"Reply to Send"}});
    let terminal = json!({"seq":2,"kind":"terminal","state":"ok"});
    let tx = db.transaction().unwrap();
    let spool = checked(&tx, 1).unwrap();
    let call = spool.reserve(5000).unwrap();
    launches += usize::from(spool.claim_launch(call).unwrap());
    assert!(spool.read(call, 0, 256 * 1024).unwrap().is_empty());
    assert_eq!(
        spool
            .ingest(call, &terminal, 5000, |_| true)
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    assert_eq!(
        spool
            .ingest(call, &first, 5000, |_| false)
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        spool.ingest(call, &first, 5000, |_| true).unwrap(),
        Ingest::Stored
    );
    tx.commit().unwrap();
    drop(db);

    let mut db = open_db(&path);
    let tx = db.transaction().unwrap();
    let spool = checked(&tx, 1).unwrap();
    launches += usize::from(spool.claim_launch(call).unwrap());
    assert_eq!(
        spool.read(call, 0, 256 * 1024).unwrap(),
        vec![first.clone()]
    );
    assert_eq!(
        spool.ingest(call, &first, 5000, |_| true).unwrap(),
        Ingest::Replay
    );
    let changed = json!({"seq":1,"kind":"output","data":{"text":"changed"}});
    assert_eq!(
        spool
            .ingest(call, &changed, 5000, |_| true)
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    assert_eq!(
        spool.ingest(call, &terminal, 5000, |_| true).unwrap(),
        Ingest::Stored
    );
    assert_eq!(spool.read(call, 1, 256 * 1024).unwrap(), vec![terminal]);
    assert_eq!(
        spool.output_text(call, 1, 2, "text").unwrap().as_deref(),
        Some("Reply to Send")
    );
    assert!(spool.output_text(call, 2, 2, "text").unwrap().is_none());
    assert!(spool.output_text(call, 1, 0, "text").unwrap().is_none());
    assert_eq!(launches, 1);
    tx.commit().unwrap();

    let tx = db.transaction().unwrap();
    tx.execute("DELETE FROM live", []).unwrap();
    assert!(checked(&tx, 1).is_err());
    tx.commit().unwrap();
}

#[test]
fn exhausting_frame_rate_reserves_one_terminal_across_reconnect() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("space.db");
    let mut db = setup(&path);
    let tx = db.transaction().unwrap();
    let spool = checked(&tx, 1).unwrap();
    let call = spool.reserve(5000).unwrap();
    assert!(spool.claim_launch(call).unwrap());
    for seq in 1..=64 {
        let frame = json!({"seq":seq,"kind":"output","data":true});
        assert_eq!(
            spool.ingest(call, &frame, 5000, |_| true).unwrap(),
            Ingest::Stored
        );
    }
    tx.commit().unwrap();
    drop(db);

    let mut db = open_db(&path);
    let tx = db.transaction().unwrap();
    let spool = checked(&tx, 1).unwrap();
    let excess = json!({"seq":65,"kind":"output","data":true});
    assert_eq!(spool.read(call, 0, 256 * 1024).unwrap().len(), 16);
    assert_eq!(spool.read(call, 0, 100).unwrap().len(), 2);
    assert_eq!(
        spool.read(call, 0, 16).unwrap_err().code,
        ErrorCode::TooLarge
    );
    assert_eq!(
        spool.ingest(call, &excess, 5000, |_| true).unwrap(),
        Ingest::Sealed
    );
    assert_eq!(
        spool.read(call, 64, 256 * 1024).unwrap(),
        vec![json!({"seq":65,"kind":"terminal","state":"failed"})]
    );
    tx.commit().unwrap();
}

#[test]
fn revoked_attempt_cannot_commit_terminal_and_expiry_releases_space_budget() {
    let directory = tempfile::tempdir().unwrap();
    let mut db = setup(&directory.path().join("space.db"));
    let tx = db.transaction().unwrap();
    let spool = checked(&tx, 1).unwrap();
    let call = spool.reserve(1000).unwrap();
    assert!(spool.claim_launch(call).unwrap());
    let output = json!({"seq":1,"kind":"output","data":{"text":"Already durable"}});
    assert_eq!(
        spool.ingest(call, &output, 1000, |_| true).unwrap(),
        Ingest::Stored
    );
    tx.commit().unwrap();

    let tx = db.transaction().unwrap();
    tx.execute("DELETE FROM live", []).unwrap();
    retire(
        &tx,
        &ExecutionRef("attempt".into()),
        1,
        2000,
        Duration::from_secs(60),
    )
    .unwrap();
    assert!(checked(&tx, 1).is_err());
    let count = |tx: &Transaction<'_>| {
        tx.query_row("SELECT count(*) FROM contribution_frames", [], |row| {
            row.get::<_, i32>(0)
        })
        .unwrap()
    };
    prune(&tx, 61999).unwrap();
    assert_eq!(count(&tx), 1);
    prune(&tx, 62000).unwrap();
    assert_eq!(count(&tx), 0);
    let remaining = budget(&tx).unwrap();
    assert_eq!((remaining.calls, remaining.bytes), (0, 0));
    tx.commit().unwrap();
}

#[test]
fn one_space_shares_ingest_credit_across_attempts() {
    let directory = tempfile::tempdir().unwrap();
    let mut db = setup(&directory.path().join("space.db"));
    let tx = db.transaction().unwrap();
    for attempt in 0..5 {
        let execution = ExecutionRef(format!("attempt-{attempt}"));
        let spool = Spool::checked(&tx, &execution, 1, |_, _, _| Ok(true)).unwrap();
        let call = spool.reserve(1000).unwrap();
        assert!(spool.claim_launch(call).unwrap());
        for seq in 1..=64 {
            let frame = json!({"seq":seq,"kind":"output","data":true});
            let outcome = spool.ingest(call, &frame, 1000, |_| true).unwrap();
            if attempt == 4 {
                assert_eq!(outcome, Ingest::Sealed);
                break;
            }
            assert_eq!(outcome, Ingest::Stored);
        }
    }
    tx.commit().unwrap();
}

#[test]
fn byte_budgets_keep_capacity_for_terminal() {
    let directory = tempfile::tempdir().unwrap();
    let mut db = setup(&directory.path().join("space.db"));
    let tx = db.transaction().unwrap();
    let frame = |seq| {
        let mut frame = json!({"seq":seq,"kind":"output","data":""});
        let envelope = serde_json::to_vec(&frame).unwrap().len() + 1;
        let limit = contribution_limits().encoded_frame_bytes as usize;
        frame["data"] = json!("x".repeat(limit - envelope));
        frame
    };
    for (attempt, frames, elapsed) in [("rate", 16, 0), ("retained", 63, 1000)] {
        let spool =
            Spool::checked(&tx, &ExecutionRef(attempt.into()), 1, |_, _, _| Ok(true)).unwrap();
        let call = spool.reserve(3000).unwrap();
        assert!(spool.claim_launch(call).unwrap());
        let start = if elapsed == 0 { 1000 } else { 3000 };
        for seq in 1..=frames {
            assert_eq!(
                spool
                    .ingest(call, &frame(seq), start + seq * elapsed, |_| true)
                    .unwrap(),
                Ingest::Stored
            );
        }
        assert_eq!(
            spool
                .ingest(
                    call,
                    &frame(frames + 1),
                    start + (frames + 1) * elapsed,
                    |_| true
                )
                .unwrap(),
            Ingest::Sealed
        );
        assert_eq!(
            spool.read(call, frames, 256 * 1024).unwrap(),
            vec![json!({"seq":frames+1,"kind":"terminal","state":"failed"})]
        );
    }
    tx.commit().unwrap();
}

#[test]
fn verified_exit_without_terminal_seals_the_call_once() {
    let directory = tempfile::tempdir().unwrap();
    let mut db = setup(&directory.path().join("space.db"));
    let tx = db.transaction().unwrap();
    let spool = checked(&tx, 1).unwrap();
    let call = spool.reserve(1000).unwrap();
    assert!(spool.claim_launch(call).unwrap());
    spool.failed_exit(call).unwrap();
    spool.failed_exit(call).unwrap();
    assert!(!spool.claim_launch(call).unwrap());
    assert_eq!(
        spool.read(call, 0, 256 * 1024).unwrap(),
        vec![json!({"seq":1,"kind":"terminal","state":"failed"})]
    );
    tx.commit().unwrap();
}
