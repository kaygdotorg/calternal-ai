use super::*;
use crate::{
    ErrorCode,
    spaces::{RecoverySink, Spaces},
};
use calternal_types::{AccountId, SpaceId};
use serde_json::{Value, json};
use std::sync::mpsc::{self, Sender};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::oneshot;

#[derive(Default)]
struct CountingSink {
    gate: Mutex<()>,
    prepared: AtomicUsize,
    completed: AtomicUsize,
    publication: Option<Publication>,
}

struct Publication {
    entered: std::sync::mpsc::Sender<()>,
    fail: Mutex<std::sync::mpsc::Receiver<()>>,
}

impl RecoverySink for CountingSink {
    fn reconcile(&self, _: &Connection) -> Result<()> {
        Ok(())
    }

    fn prepare(&self, _: &Connection) -> Result<String> {
        self.prepared.fetch_add(1, Ordering::SeqCst);
        Ok("test-batch".into())
    }

    fn complete(&self, _: &Connection, _: &str) -> Result<()> {
        self.completed.fetch_add(1, Ordering::SeqCst);
        if let Some(publication) = &self.publication {
            publication.entered.send(()).unwrap();
            publication
                .fail
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            return Err(Error::internal("injected recovery publication failure"));
        }
        Ok(())
    }

    fn serial(&self) -> &Mutex<()> {
        &self.gate
    }
}

#[test]
fn committed_batch_waits_for_recovery_and_refuses_success_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("auth.db")).unwrap();
    let (entered, published) = std::sync::mpsc::channel();
    let (fail, wait) = std::sync::mpsc::channel();
    let sink = Arc::new(CountingSink {
        publication: Some(Publication {
            entered,
            fail: Mutex::new(wait),
        }),
        ..CountingSink::default()
    });
    db.configure_recovery(sink.clone()).unwrap();

    let (reply, mut result) = tokio::sync::oneshot::channel();
    db.worker
        .blocking_send(Job::Role(role_batch::RoleWrite {
            apply: Box::new(|_| Ok(serde_json::json!("success"))),
            reply,
        }))
        .map_err(|_| "send role job")
        .unwrap();
    published.recv_timeout(Duration::from_secs(5)).unwrap();
    let before = result.try_recv();
    fail.send(()).unwrap();

    assert!(matches!(
        before,
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
    assert_eq!(
        result.blocking_recv().unwrap().unwrap_err().code(),
        ErrorCode::Internal
    );
}

fn fixture() -> (tempfile::TempDir, Db, SpaceId, AccountId, Arc<CountingSink>) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("auth.db")).unwrap();
    let spaces = Spaces::from_db(db.clone(), crate::spaces::DEFAULT_RECEIPT_RETENTION);
    let ids = spaces.seed_development().unwrap();
    let sink = Arc::new(CountingSink::default());
    db.configure_recovery(sink.clone()).unwrap();
    (dir, db, ids.shared, ids.guest, sink)
}

fn send(db: &Db, job: Job) {
    db.worker
        .blocking_send(job)
        .map_err(|_| "send auth job")
        .unwrap();
}

/// Place an ordinary write boundary in the mailbox, and hold the worker there
/// until the caller has enqueued the exact order to exercise.
fn pause(
    db: &Db,
    at_boundary: impl FnOnce(&Connection) + Send + 'static,
) -> (mpsc::Receiver<()>, Sender<()>) {
    let (entered, waiting) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    send(
        db,
        Job::Single(Box::new(move |c| {
            at_boundary(c);
            entered.send(()).unwrap();
            resume.recv_timeout(Duration::from_secs(5)).unwrap();
        })),
    );
    (waiting, release)
}

fn role(c: &Connection, space: SpaceId, member: AccountId) -> String {
    c.query_row(
        "SELECT role FROM space_memberships WHERE space_id=?1 AND account_id=?2",
        rusqlite::params![space.to_string(), member.to_string()],
        |row| row.get(0),
    )
    .unwrap()
}

/// This mailbox probe returns the value from its own savepoint. A rejected
/// probe changes a row before failing to exercise rollback of only that write.
fn role_job(
    space: SpaceId,
    member: AccountId,
    next: &'static str,
    reject: bool,
) -> (Job, oneshot::Receiver<Result<Value>>) {
    let (reply, result) = oneshot::channel();
    (
        Job::Role(role_batch::RoleWrite {
            apply: Box::new(move |c| {
                c.execute(
                    "UPDATE space_memberships SET role=?3 WHERE space_id=?1 AND account_id=?2",
                    rusqlite::params![space.to_string(), member.to_string(), next],
                )
                .map_err(sql)?;
                if reject {
                    return Err(Error::invalid("injected role refusal"));
                }
                Ok(json!(role(c, space, member)))
            }),
            reply,
        }),
        result,
    )
}

fn observe(db: &Db, space: SpaceId, member: AccountId) -> oneshot::Receiver<String> {
    let (reply, result) = oneshot::channel();
    send(
        db,
        Job::Read(Box::new(move |c| {
            reply.send(role(c, space, member)).unwrap();
        })),
    );
    result
}

fn applied(result: oneshot::Receiver<Result<Value>>, expected: &str) {
    assert_eq!(result.blocking_recv().unwrap().unwrap(), json!(expected));
}

#[test]
fn queued_role_changes_share_one_recovery_publication() {
    let (_dir, db, space, member, sink) = fixture();
    let (waiting, release) = pause(&db, |_| {});
    waiting.recv_timeout(Duration::from_secs(5)).unwrap();

    let (first, first_reply) = role_job(space, member, "editor", false);
    send(&db, first);
    let read = observe(&db, space, member);
    let (rejected, rejected_reply) = role_job(space, member, "reader", true);
    send(&db, rejected);
    let (reply, last_reply) = oneshot::channel();
    send(
        &db,
        Job::Role(role_batch::RoleWrite {
            apply: Box::new(move |c| {
                assert_eq!(role(c, space, member), "editor");
                c.execute(
                "UPDATE space_memberships SET role='manager' WHERE space_id=?1 AND account_id=?2",
                rusqlite::params![space.to_string(), member.to_string()],
            ).map_err(sql)?;
                Ok(json!(role(c, space, member)))
            }),
            reply,
        }),
    );
    assert_eq!(sink.completed.load(Ordering::SeqCst), 0);
    release.send(()).unwrap();

    applied(first_reply, "editor");
    assert_eq!(
        rejected_reply.blocking_recv().unwrap().unwrap_err().code(),
        ErrorCode::InvalidInput
    );
    applied(last_reply, "manager");
    assert_eq!(read.blocking_recv().unwrap(), "manager");
    assert_eq!(
        observe(&db, space, member).blocking_recv().unwrap(),
        "manager"
    );
    assert_eq!(sink.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(sink.completed.load(Ordering::SeqCst), 1);
}

#[test]
fn same_member_changes_stay_in_order_across_batches() {
    let (_dir, db, space, member, sink) = fixture();
    let (waiting, release) = pause(&db, |_| {});
    waiting.recv_timeout(Duration::from_secs(5)).unwrap();

    let (first, first_reply) = role_job(space, member, "editor", false);
    send(&db, first);
    let (middle, continue_worker) = pause(&db, move |c| {
        assert_eq!(role(c, space, member), "editor");
    });
    let (last, mut last_reply) = role_job(space, member, "reader", false);
    send(&db, last);
    release.send(()).unwrap();
    middle.recv_timeout(Duration::from_secs(5)).unwrap();

    applied(first_reply, "editor");
    assert!(matches!(
        last_reply.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    assert_eq!(sink.completed.load(Ordering::SeqCst), 1);

    continue_worker.send(()).unwrap();
    applied(last_reply, "reader");
    assert_eq!(
        observe(&db, space, member).blocking_recv().unwrap(),
        "reader"
    );
    assert_eq!(sink.prepared.load(Ordering::SeqCst), 2);
    assert_eq!(sink.completed.load(Ordering::SeqCst), 2);
}

#[test]
fn reserved_mailbox_capacity_does_not_mean_a_role_change_is_queued() {
    let (_dir, db, space, member, sink) = fixture();
    let (waiting, release) = pause(&db, |_| {});
    waiting.recv_timeout(Duration::from_secs(5)).unwrap();

    let (rejected, rejected_reply) = role_job(space, member, "manager", true);
    send(&db, rejected);
    let read = observe(&db, space, member);
    // Reproduce the sporadic schedule precisely. The old test counted this
    // reservation as a queued editor write while its sender was still paused.
    let permit = db.worker.clone().try_reserve_owned().unwrap();
    assert_eq!(db.worker.capacity(), db.worker.max_capacity() - 3);
    release.send(()).unwrap();
    assert_eq!(read.blocking_recv().unwrap(), "reader");
    assert_eq!(
        rejected_reply.blocking_recv().unwrap().unwrap_err().code(),
        ErrorCode::InvalidInput
    );
    assert_eq!(sink.completed.load(Ordering::SeqCst), 0);

    let (later, reply) = role_job(space, member, "editor", false);
    permit.send(later);
    applied(reply, "editor");
    assert_eq!(
        observe(&db, space, member).blocking_recv().unwrap(),
        "editor"
    );
    assert_eq!(sink.completed.load(Ordering::SeqCst), 1);
}
