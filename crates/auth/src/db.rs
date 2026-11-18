//! `auth.db`: users, passkeys, setup tokens and Installations.
//!
//! One global SQLite file in WAL mode with `synchronous=FULL`: a signed-in
//! session or a used-up setup token must survive a power cut. The auth worker
//! owns its SQLite connection and orders authority checks and writes against
//! acknowledged recovery publication. Queueing or disk latency can delay other
//! auth work without blocking the async executor.

use crate::{Error, Result};
use rusqlite::Connection;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

mod role_batch;

/// The one schema version this build reads; [`migrate`] creates it on an empty file.
const VERSION: i64 = 10;

/// A handle to the dedicated auth.db worker. Its bounded mailbox serializes
/// authority reads, authentication writes and recovery publication for this
/// installation. A read sees the state before or after the worker's protected
/// commit. The worker owns its connection directly. A batch takes role writes
/// from at most 32 queued jobs and uses one savepoint per request. Pure access
/// checks queued between those writes run immediately after publication. The
/// recovery sink lock covers the proposal, SQLite commit, in-memory revoke
/// fence and publication. The worker holds no lock during fsync outside that
/// commit or while waiting for or replying to another thread. Space stores
/// own their content and journal connections.
#[derive(Clone)]
pub struct Db {
    worker: tokio::sync::mpsc::Sender<Job>,
    sink: Arc<OnceLock<Sink>>,
    changes: Arc<tokio::sync::watch::Sender<u64>>,
}

type Sink = Arc<dyn crate::spaces::RecoverySink>;

thread_local! {
    /// The recovery sink of the [`Db`] whose worker runs on this thread; set before
    /// each job, read by [`crate::spaces::commit_with`]. Other threads have none.
    static ACTIVE_SINK: std::cell::RefCell<Option<Sink>> = const { std::cell::RefCell::new(None) };
}

/// The recovery sink of the database this worker thread serves, if configured.
/// Cost: one thread-local read and an Arc clone.
pub(crate) fn active_sink() -> Option<Sink> {
    ACTIVE_SINK.with(|s| s.borrow().clone())
}

enum Job {
    Single(Box<dyn FnOnce(&mut Connection) + Send>),
    Read(Box<dyn FnOnce(&Connection) + Send>),
    Role(role_batch::RoleWrite),
}

impl Db {
    /// Opens or creates the database and brings its schema up to date.
    ///
    /// Blocking startup operation: creates parent directories, configures
    /// WAL/FULL/foreign keys and a 5 s busy timeout, then applies migrations
    /// atomically. Call outside async request workers. Returns [`Error::Internal`]
    /// on filesystem, connection or migration failure. Creates the requested
    /// parent as 0700 and database as 0600 before SQLite can create sidecars.
    /// Does not validate root pairing or recover lost installation state;
    /// callers must apply those guards before allowing database creation.
    pub fn open(path: &Path) -> Result<Self> {
        let mut connection = open_conn(path)?;
        let (worker, mut inbox) = tokio::sync::mpsc::channel::<Job>(64);
        let db = Self {
            worker,
            sink: Arc::new(OnceLock::new()),
            changes: Arc::new(tokio::sync::watch::channel(0).0),
        };
        let sink = db.sink.clone();
        let changes = db.changes.clone();
        std::thread::Builder::new()
            .name("auth-db".into())
            .spawn(move || {
                let mut pending = VecDeque::new();
                while let Some(job) = pending.pop_front().or_else(|| inbox.blocking_recv()) {
                    ACTIVE_SINK.with(|s| *s.borrow_mut() = sink.get().cloned());
                    let before = connection.total_changes();
                    match job {
                        Job::Single(job) => job(&mut connection),
                        Job::Read(read) => read(&connection),
                        Job::Role(first) => {
                            let mut batch = vec![first];
                            for _ in 1..role_batch::MAX_BATCH {
                                match inbox.try_recv() {
                                    Ok(Job::Role(write)) => batch.push(write),
                                    Ok(read @ Job::Read(_)) => pending.push_back(read),
                                    Ok(single @ Job::Single(_)) => {
                                        pending.push_back(single);
                                        break;
                                    }
                                    Err(_) => break,
                                }
                            }
                            role_batch::execute(&mut connection, batch);
                        }
                    }
                    if connection.total_changes() != before {
                        changes.send_modify(|n| *n = n.wrapping_add(1));
                    }
                }
            })
            .map_err(Error::internal)?;
        Ok(db)
    }

    /// The job that attaches `sink`: reconcile pending proposals on this database's
    /// worker, then make every later commit on this handle publish through it.
    /// Attaching the same sink again is a no-op; a different one is refused.
    fn attach(&self, sink: Sink) -> impl FnOnce(&mut Connection) -> Result<()> + Send + 'static {
        let cell = self.sink.clone();
        move |c| {
            if let Some(existing) = cell.get() {
                return if Arc::ptr_eq(existing, &sink) {
                    Ok(())
                } else {
                    Err(Error::internal("recovery already configured"))
                };
            }
            sink.reconcile(c)?;
            cell.set(sink)
                .map_err(|_| Error::internal("recovery already configured"))
        }
    }

    /// Attaches the protected recovery barrier from a blocking thread; call before
    /// serving requests, under the root locks. Fails closed if reconciliation fails.
    /// Cost: one worker round trip plus the sink's reconciliation.
    pub fn configure_recovery(&self, sink: Sink) -> Result<()> {
        self.run_blocking(self.attach(sink))
    }

    /// [`Self::configure_recovery`] from async code.
    pub async fn configure_recovery_async(&self, sink: Sink) -> Result<()> {
        self.run(self.attach(sink)).await
    }

    /// Runs `f` with the connection on a dedicated blocking thread.
    ///
    /// One call allocates a boxed closure and a oneshot result channel, sends
    /// once through a bounded MPSC queue, and receives once through the oneshot.
    /// Admission is O(1); latency includes waiting for capacity, earlier jobs,
    /// and `f` itself (including SQLite's 5 s busy timeout).
    /// The already-open worker is shared by all handle clones.
    /// At most 64 jobs are queued, plus one executing job or role batch.
    /// Waiting caller futures retain their own closures outside the queue.
    /// No general Tokio blocking
    /// pool slot is used. Use [`Self::try_run`] for immediate overload refusal.
    /// Dropping the future after admission does not cancel `f`.
    ///
    /// Returns an internal error if the worker or result channel stops.
    pub async fn run<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    {
        let worker = &self.worker;
        let (tx, rx) = tokio::sync::oneshot::channel();
        worker
            .send(Job::Single(Box::new(move |c| {
                let _ = tx.send(f(c));
            })))
            .await
            .map_err(|_| Error::internal("auth database worker stopped"))?;
        rx.await.map_err(Error::internal)?
    }

    /// Read current authority asynchronously on the same worker as its writes.
    /// The callback must be read-only. Concurrent role writes can pass a queued
    /// read within one bounded batch. The read runs after the commit and
    /// recovery publication attempt, including the existing failure handling.
    pub(crate) async fn read<T: Send + 'static>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (reply, result) = tokio::sync::oneshot::channel();
        self.worker
            .send(Job::Read(Box::new(move |c| {
                let _ = reply.send(f(c));
            })))
            .await
            .map_err(|_| Error::internal("auth database worker stopped"))?;
        result.await.map_err(Error::internal)?
    }

    /// Runs `f` on the same worker, refusing admission immediately if all 64
    /// queued slots are occupied. The currently executing job is separate.
    /// Returns [`Error::QueueFull`] without executing `f` on saturation, or
    /// [`Error::Internal`] if the worker stops. Once admitted, cancellation
    /// does not cancel the database operation. Costs match [`Self::run`],
    /// except there is no queue-capacity wait.
    pub async fn try_run<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.worker
            .try_send(Job::Single(Box::new(move |c| {
                let _ = tx.send(f(c));
            })))
            .map_err(|e| match e {
                tokio::sync::mpsc::error::TrySendError::Full(_) => Error::QueueFull,
                tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                    Error::internal("auth database worker stopped")
                }
            })?;
        rx.await.map_err(Error::internal)?
    }

    /// Runs a job synchronously on the same bounded dedicated worker.
    /// Host adapters with synchronous directory traits must call this only from
    /// blocking threads, never a Tokio async worker. Admission waits for capacity
    /// and preserves the source worker's 64 queued-job bound and ordering.
    /// Cancellation after admission does not cancel the job.
    pub fn run_blocking<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.worker
            .blocking_send(Job::Single(Box::new(move |c| {
                let _ = tx.send(f(c));
            })))
            .map_err(|_| Error::internal("auth database worker stopped"))?;
        rx.blocking_recv().map_err(Error::internal)?
    }

    /// Blocking counterpart of [`Self::read`], for the synchronous directory.
    pub(crate) fn read_blocking<T: Send + 'static>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (reply, result) = tokio::sync::oneshot::channel();
        self.worker
            .blocking_send(Job::Read(Box::new(move |c| {
                let _ = reply.send(f(c));
            })))
            .map_err(|_| Error::internal("auth database worker stopped"))?;
        result.blocking_recv().map_err(Error::internal)?
    }

    /// Enqueue a role mutation that can run inside another transaction's savepoint.
    /// The callback must perform no cross-thread work, independent commits, or
    /// external side effects. Reports success only after the shared commit publishes.
    pub(crate) fn run_role_write_blocking(
        &self,
        apply: impl FnOnce(&Connection) -> Result<serde_json::Value> + Send + 'static,
    ) -> Result<serde_json::Value> {
        let (reply, result) = tokio::sync::oneshot::channel();
        self.worker
            .blocking_send(Job::Role(role_batch::RoleWrite {
                apply: Box::new(apply),
                reply,
            }))
            .map_err(|_| Error::internal("auth database worker stopped"))?;
        result.blocking_recv().map_err(Error::internal)?
    }

    /// Watches worker mutations, conservatively including rolled-back writes.
    /// Notifications are hints: always recheck
    /// authority before delivery. One watch value per existing database handle;
    /// constant memory and no SQLite or additional worker on subscription.
    pub fn changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.changes.subscribe()
    }
}

/// Opens a configured, migrated connection for synchronous host/CLI work.
///
/// Uses the same blocking creation/migration behavior and errors as [`Db::open`],
/// without starting a dedicated worker. Callers own serialization and must keep
/// SQLite work off async request workers. Root validation remains the caller's
/// responsibility, including refusal to recreate unexpectedly missing auth state.
pub fn open_conn(path: &Path) -> Result<Connection> {
    if let Some(dir) = path.parent() {
        create_parent(dir)?;
    }
    calternal_fsio::private::file(path).map_err(Error::internal)?;
    let conn = Connection::open(path).map_err(Error::internal)?;
    configure(&conn).map_err(Error::internal)?;
    migrate(&conn).map_err(Error::internal)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> rusqlite::Result<()> {
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "FULL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

/// Creates the current schema ([`include_str!`] of `schema.sql`) in an empty
/// database; an existing database must already be at [`VERSION`]. There is no
/// migration chain: preview data is disposable.
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    // Read the version after taking the write lock: a CLI command may open
    // the same database while the server is starting.
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
    let have: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
    match have {
        0 => {
            tx.execute_batch(include_str!("schema.sql"))?;
            tx.pragma_update(None, "user_version", VERSION)?;
        }
        VERSION => {}
        _ => {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISMATCH),
                Some(format!(
                    "auth.db schema version {have} is not {VERSION}; use fresh disposable data"
                )),
            ));
        }
    }
    tx.commit()
}

// Owner-only creation and fsio directory durability; not a root guard.
fn create_parent(dir: &Path) -> Result<()> {
    calternal_fsio::private::directory(dir).map_err(Error::internal)
}

#[cfg(test)]
mod port_tests {
    use super::*;
    use crate::test_support::TempDir;
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    #[test]
    fn a_database_at_another_version_is_refused() {
        let dir = TempDir::new();
        let path = dir.path().join("auth.db");
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 16)
            .unwrap();
        assert!(Db::open(&path).is_err());
        let fresh = dir.path().join("fresh.db");
        drop(open_conn(&fresh).unwrap());
        assert!(
            open_conn(&fresh).is_ok(),
            "reopening the current version works"
        );
    }

    #[test]
    fn connection_preserves_all_source_pragmas() {
        let dir = TempDir::new();
        let c = open_conn(&dir.path().join("system/auth.db")).unwrap();
        assert_eq!(
            c.pragma_query_value(None, "journal_mode", |r| r.get::<_, String>(0))
                .unwrap(),
            "wal"
        );
        for (pragma, expected) in [
            ("synchronous", 2),
            ("foreign_keys", 1),
            ("busy_timeout", 5000),
        ] {
            assert_eq!(
                c.pragma_query_value(None, pragma, |r| r.get::<_, i64>(0))
                    .unwrap(),
                expected
            );
        }
    }

    #[tokio::test]
    async fn worker_refuses_65th_queued_job_and_run_retains_waiting_backpressure() {
        let dir = TempDir::new();
        let db = Db::open(&dir.path().join("auth.db")).unwrap();
        let (release, blocked) = std::sync::mpsc::channel();
        let (started_tx, started) = tokio::sync::oneshot::channel();
        let first_db = db.clone();
        let first = tokio::spawn(async move {
            first_db
                .run(move |_| {
                    started_tx.send(()).unwrap();
                    blocked.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(())
                })
                .await
        });
        started.await.unwrap();
        let mut cx = Context::from_waker(Waker::noop());
        let mut admitted = Vec::new();
        for n in 0..64 {
            let mut future = Box::pin(db.try_run(move |_| Ok(n)));
            assert!(future.as_mut().poll(&mut cx).is_pending());
            admitted.push(future);
        }
        assert_eq!(db.worker.capacity(), 0);
        let mut refused =
            Box::pin(db.try_run(|_| -> Result<()> { panic!("refused job executed") }));
        assert!(matches!(
            refused.as_mut().poll(&mut cx),
            Poll::Ready(Err(Error::QueueFull))
        ));
        // Source run waits without growing the queue, while try_run refuses.
        let mut waiting = Box::pin(db.run(|_| Ok(65)));
        assert!(waiting.as_mut().poll(&mut cx).is_pending());
        assert_eq!(db.worker.capacity(), 0);
        release.send(()).unwrap();
        first.await.unwrap().unwrap();
        for (n, future) in admitted.into_iter().enumerate() {
            assert_eq!(future.await.unwrap(), n);
        }
        assert_eq!(waiting.await.unwrap(), 65);
        assert_eq!(db.worker.capacity(), 64);
    }

    // Ported from reliability_review.rs:805–846; probe the database directly
    // instead of the excluded HTTP /auth/status adapter.
    #[test]
    fn authentication_database_does_not_wait_for_general_blocking_workers() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(2)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let dir = TempDir::new();
            let db = Db::open(&dir.path().join("auth.db")).unwrap();
            db.run(|c| {
                c.query_row("SELECT count(*) FROM users", [], |r| r.get::<_, i64>(0))
                    .map_err(Error::internal)
            })
            .await
            .unwrap();
            let mut release = Vec::new();
            let mut workers = Vec::new();
            for _ in 0..2 {
                let (tx, rx) = std::sync::mpsc::channel();
                let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
                release.push(tx);
                workers.push(tokio::task::spawn_blocking(move || {
                    ready_tx.send(()).unwrap();
                    rx.recv_timeout(Duration::from_secs(5)).unwrap();
                }));
                ready_rx.await.unwrap();
            }
            let result = tokio::time::timeout(
                Duration::from_millis(100),
                db.run(|c| {
                    c.query_row("SELECT count(*) FROM users", [], |r| r.get::<_, i64>(0))
                        .map_err(Error::internal)
                }),
            )
            .await;
            for tx in release {
                tx.send(()).unwrap();
            }
            for worker in workers {
                worker.await.unwrap();
            }
            assert_eq!(
                result
                    .expect("auth waited for the general blocking pool")
                    .unwrap(),
                0
            );
        });
    }
}

pub(crate) use calternal_types::now_ms;

/// Converts a SQLite failure into a private host diagnostic.
pub(crate) fn sql(e: rusqlite::Error) -> crate::Error {
    crate::Error::internal(e)
}

#[cfg(test)]
mod role_batch_tests;
