use super::*;
use crate::{DEFAULT_RECEIPT_RETENTION, Request, journal::lock};
use calternal_types::SpaceId;
use serde_json::{Value, json};
use std::sync::{Mutex, mpsc};

const REQUEST_ID: &str = "checkpoint:resume";

struct Fixture {
    dir: tempfile::TempDir,
    roots: calternal_roots::Roots,
    space: SpaceId,
    actor: AccountId,
    reply: Value,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let roots = calternal_roots::Roots {
            appdata: dir.path().join("appdata"),
            userdata_ssd: dir.path().join("ssd"),
            userdata_hdd: dir.path().join("hdd"),
        };
        Self {
            dir,
            roots,
            space: SpaceId::new(),
            actor: AccountId::new(),
            reply: json!({"append":[{"text":"checkpoint"}]}),
        }
    }

    fn open(&self, fault: fn(crate::FaultPoint)) -> Content {
        Content::open(
            self.roots.clone(),
            self.dir.path().join("secrets"),
            Duration::from_secs(60),
            DEFAULT_RECEIPT_RETENTION,
            fault,
        )
        .unwrap()
    }

    fn write(&self, content: &Content) -> Result<Value> {
        content.set_control(
            self.space,
            Request {
                account: self.actor,
                via: None,
                id: REQUEST_ID,
                operation: "threads.checkpoint",
                fingerprint: "original",
            },
            "checkpoint",
            |_| Ok((json!(true), self.reply.clone())),
        )
    }

    fn index(&self) -> rusqlite::Connection {
        let path =
            crate::db::space_path(&self.roots.appdata, self.space).with_file_name("index.db");
        rusqlite::Connection::open(path).unwrap()
    }
}

#[test]
fn a_forward_boot_clock_jump_keeps_the_pending_checkpoint_reply() {
    let f = Fixture::new();
    let content = f.open(|_| {});
    content
        .with_space(f.space, |space| {
            space
                .conn
                .execute_batch(
                    "CREATE TRIGGER stop_retirement BEFORE DELETE ON outbox
             BEGIN SELECT RAISE(FAIL, 'interrupted after publication'); END;",
                )
                .map_err(internal)
        })
        .unwrap();
    f.write(&content).unwrap_err();
    content
        .with_space(f.space, |space| {
            space
                .conn
                .execute_batch("DROP TRIGGER stop_retirement")
                .map_err(internal)
        })
        .unwrap();
    drop(content);

    let restarted = f.open(|_| {});
    let store = restarted
        .journals
        .open_store(f.space, restarted.lifecycle())
        .unwrap();
    let mut space = Space::open(&f.roots.appdata, &f.roots.userdata_ssd, f.space, store).unwrap();
    let jumped =
        now_ms() + (DEFAULT_RECEIPT_RETENTION + Duration::from_secs(60)).as_millis() as i64;
    // Same cold recovery path and DB order, with the boot wall clock advanced.
    restarted.recover_space_at(&mut space, jumped).unwrap();
    let reply = restarted
        .journals
        .receipt(&space.journal, f.actor, REQUEST_ID)
        .unwrap()
        .unwrap();
    assert_eq!(reply.result, f.reply);
    let entries = restarted.journals.entries(&space.journal).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].data["append"], f.reply["append"]);
}

#[test]
fn unreadable_outbox_skips_pruning() {
    let f = Fixture::new();
    let content = f.open(|_| {});
    f.write(&content).unwrap();
    content
        .with_space(f.space, |space| {
            space
                .conn
                .execute_batch("ALTER TABLE outbox RENAME TO unavailable_outbox")
                .map_err(internal)?;
            let cutoff =
                now_ms() + (DEFAULT_RECEIPT_RETENTION + Duration::from_secs(60)).as_millis() as i64;
            space.prune_receipts_at(cutoff);
            let saved = content
                .journals
                .receipt(&space.journal, f.actor, REQUEST_ID)?
                .unwrap();
            assert_eq!(saved.result, f.reply);
            Ok(())
        })
        .unwrap();
}

type Pause = (mpsc::Sender<()>, mpsc::Receiver<()>);
static RETIREMENT_PAUSE: Mutex<Option<Pause>> = Mutex::new(None);

fn pause_retirement(point: crate::FaultPoint) {
    if point == crate::FaultPoint::AfterJournalCommit
        && let Some((entered, release)) = lock(&RETIREMENT_PAUSE).take()
    {
        entered.send(()).unwrap();
        release.recv().unwrap();
    }
}

#[test]
fn maintenance_and_retry_serialize_with_retirement() {
    let f = Fixture::new();
    let content = f.open(pause_retirement);
    let journal = content.live_journal(f.space).unwrap();
    let slot = lock(&content.spaces).get(&f.space).unwrap().clone();
    lock(&slot.space).as_mut().unwrap().last_receipt_prune =
        Instant::now() - RECEIPT_PRUNE_INTERVAL;
    let _worker = lock(&content.root_guard);
    let (entered, checkpoint) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    *lock(&RETIREMENT_PAUSE) = Some((entered, resume));

    std::thread::scope(|scope| {
        let mutation = scope.spawn(|| f.write(&content));
        if let Err(error) = checkpoint.recv_timeout(Duration::from_secs(5)) {
            drop(release);
            panic!("mutation did not reach the journal boundary: {error}");
        }
        let aged = f.index().execute("UPDATE idempotency SET at_ms=0", []);
        let retry = scope.spawn(|| content.receipt(f.space, f.actor, REQUEST_ID));
        let deferred = crate::lifecycle::place_pending(&slot, |_| {});
        let still_present = content.journals.receipt(&journal, f.actor, REQUEST_ID);
        release.send(()).unwrap();
        aged.unwrap();
        assert!(deferred);
        assert!(still_present.unwrap().is_some());
        assert_eq!(mutation.join().unwrap().unwrap(), f.reply);
        assert_eq!(retry.join().unwrap().unwrap().unwrap().result, f.reply);
    });

    assert_eq!(content.journal(f.space).unwrap().len(), 1);
    assert!(crate::lifecycle::place_pending(&slot, |_| {}));
    assert!(!crate::lifecycle::place_pending(&slot, |_| {}));
    assert!(
        content
            .journals
            .receipt(&journal, f.actor, REQUEST_ID)
            .unwrap()
            .is_none()
    );
}
