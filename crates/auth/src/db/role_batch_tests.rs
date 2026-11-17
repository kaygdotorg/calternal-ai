use super::*;
use crate::{
    ErrorCode,
    spaces::{RecoverySink, Spaces},
};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Instant;

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

#[test]
fn queued_role_changes_share_one_recovery_publication() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("auth.db")).unwrap();
    let spaces = Spaces::from_db(db.clone(), crate::spaces::DEFAULT_RECEIPT_RETENTION);
    let ids = spaces.seed_development().unwrap();
    let sink = Arc::new(CountingSink::default());
    db.configure_recovery(sink.clone()).unwrap();

    let (entered, running) = std::sync::mpsc::channel();
    let (release, wait) = std::sync::mpsc::channel();
    let (observed, read_result) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            db.run_blocking(move |_| {
                entered.send(()).unwrap();
                wait.recv_timeout(Duration::from_secs(10)).unwrap();
                Ok(())
            })
            .unwrap();
        });
        running.recv_timeout(Duration::from_secs(5)).unwrap();

        let calls = 8;
        let mut observed = Some(observed);
        let mut joins = Vec::new();
        for i in 0..calls {
            if i == 1 {
                assert!(wait_for_queued(&db, 1));
                let read_ids = ids.clone();
                let observed = observed.take().unwrap();
                db.worker
                    .blocking_send(Job::Read(Box::new(move |c| {
                        let role: String = c.query_row(
                        "SELECT role FROM space_memberships WHERE space_id=?1 AND account_id=?2",
                        rusqlite::params![read_ids.shared.to_string(), read_ids.guest.to_string()],
                        |row| row.get(0),
                    ).unwrap();
                        observed.send(role).unwrap();
                    })))
                    .map_err(|_| "send read job")
                    .unwrap();
            }
            let spaces = spaces.clone();
            let ids = ids.clone();
            joins.push(scope.spawn(move || {
                let role = if i == 0 { "undefined-role" } else { "editor" };
                spaces.set_role(ids.owner, ids.shared, ids.guest, role)
            }));
        }
        let queued = wait_for_queued(&db, calls + 1);
        let before = sink.completed.load(Ordering::SeqCst);
        release.send(()).unwrap();
        assert!(queued);
        assert_eq!(before, 0);
        for (i, join) in joins.into_iter().enumerate() {
            let result = join.join().unwrap();
            if i == 0 {
                assert_eq!(result.unwrap_err().code(), ErrorCode::InvalidInput);
            } else {
                result.unwrap();
            }
        }
    });

    assert_eq!(sink.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(sink.completed.load(Ordering::SeqCst), 1);
    assert_eq!(
        read_result.recv_timeout(Duration::from_secs(5)).unwrap(),
        "editor"
    );
    let page = spaces.members(ids.owner, ids.shared, None).unwrap();
    assert_eq!(
        page.members
            .iter()
            .find(|m| m.account_id == ids.guest)
            .unwrap()
            .role,
        "editor"
    );
}

fn wait_for_queued(db: &Db, count: usize) -> bool {
    let target = db.worker.max_capacity() - count;
    let deadline = Instant::now() + Duration::from_secs(5);
    while db.worker.capacity() > target && Instant::now() < deadline {
        std::thread::yield_now();
    }
    db.worker.capacity() == target
}
