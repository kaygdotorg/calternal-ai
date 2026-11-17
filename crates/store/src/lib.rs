//! # Per-space storage
//!
//! Each space has its own SQLite database, `index.db`, holding the
//! change journal and idempotency records. Independent writers avoid a global
//! application write queue; SQLite and host resource contention remain possible.
//! The journal and receipts are durable state, not rebuildable file indexes.
//!
//! All writes for one space go through a single writer thread, in arrival
//! order, as small transactions. Nothing holds a lock while waiting on the
//! disk: callers send a message and wait for the reply. Reads use their own
//! short-lived connections and never wait for the writer.

#![deny(missing_docs, rustdoc::broken_intra_doc_links)]

mod highwater;
mod proof;
mod read;
mod restore;
mod schema;
mod shared;
mod token;
mod writer;

mod contracts;
mod types;
pub use contracts::{JournalEntry, SyncToken, TokenProof};
use serde_json::Value;
use shared::{Guard, Shared};
pub use shared::{OwnedSaveActivity, SaveActivity};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use calternal_types::{AccountId, SpaceId};
pub use types::{Error, ErrorCode, Result};

/// One event, stamped with who made it, ready to store.
#[derive(Debug, Clone)]
pub struct StampedEvent {
    /// Acting account; independent from the owning space.
    pub author: AccountId,
    /// Namespaced kind, `notes.edited`.
    pub kind: String,
    /// The item that changed.
    pub item: String,
    /// Plugin-defined details.
    pub data: Value,
    /// The client that made the change.
    pub installation: Option<String>,
    /// The request that made the change.
    pub request_id: Option<String>,
}

/// An idempotency record to save with a commit.
#[derive(Debug, Clone)]
pub struct IdemWrite {
    /// Account whose request produced this receipt.
    pub account: AccountId,
    /// The key the executor uses to identify this action request.
    pub key: String,
    /// The action that ran.
    pub action: String,
    /// Hash of the action name and input, to spot a reused key.
    pub payload_hash: String,
    /// What the action returned, replayed on a retry.
    pub result: Value,
}

/// A saved idempotency record.
#[derive(Debug, Clone, PartialEq)]
pub struct IdemRecord {
    /// The action that ran.
    pub action: String,
    /// Hash of the action name and input.
    pub payload_hash: String,
    /// What the action returned.
    pub result: Value,
    /// Journal position right after the action.
    pub seq: i64,
}

/// One atomic write: events plus, optionally, an idempotency record.
#[derive(Debug, Clone, Default)]
pub struct Commit {
    /// Events to append, in order.
    pub events: Vec<StampedEvent>,
    /// Saved in the same transaction as the events.
    pub idempotency: Option<IdemWrite>,
}

/// Knobs for one store. The defaults suit a real server; tests shrink them.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// How long a finished action's idempotency record is kept. A retry
    /// that arrives later runs the action again, so this is far longer
    /// than any client keeps retrying.
    pub idempotency_retention: Duration,
    /// How often old records are swept. Also swept once when the store
    /// opens.
    pub prune_every: Duration,
    /// How long saves must be quiet before a dirty witness is written.
    pub highwater_quiet: Duration,
    /// Maximum age of the oldest journal update not yet in the witness,
    /// measured to the end of the save in flight: the flush never starts
    /// while a save runs. A save stream that never pauses forces it at twice
    /// this age.
    pub highwater_max_staleness: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            idempotency_retention: Duration::from_secs(7 * 24 * 3600),
            prune_every: Duration::from_secs(3600),
            highwater_quiet: Duration::from_secs(1),
            highwater_max_staleness: Duration::from_secs(30),
        }
    }
}

/// A handle to one space's `index.db`. Cheap to clone.
#[derive(Clone)]
pub struct SpaceStore {
    inner: Arc<Inner>,
}

/// Owns one store's writer channel, drain handle and state for all cheap clones.
struct Inner {
    space: SpaceId,
    strict: bool,
    path: PathBuf,
    shared: Arc<Shared>,
    tx: Option<mpsc::Sender<writer::Cmd>>,
    writer_thread: Option<std::thread::JoinHandle<rusqlite::Connection>>,
    live: broadcast::Sender<Arc<JournalEntry>>,
}

impl Drop for Inner {
    /// Drains accepted writes before flushing the final head on a worker thread.
    fn drop(&mut self) {
        drop(self.tx.take());
        if let Some(thread) = self.writer_thread.take() {
            match thread.join() {
                Ok(conn) if self.shared.guard.is_some() => {
                    let shared = self.shared.clone();
                    match std::thread::Builder::new()
                        .name("store-highwater-shutdown".into())
                        .spawn(move || restore::flush_after_writer(&conn, &shared))
                    {
                        Ok(worker) => match worker.join() {
                            Ok(Ok(())) => {}
                            Ok(Err(e)) => {
                                tracing::warn!(%e, "could not update the high-water file")
                            }
                            Err(_) => {
                                tracing::warn!("high-water shutdown worker stopped unexpectedly")
                            }
                        },
                        Err(e) => {
                            tracing::warn!(%e, "could not start the high-water shutdown worker")
                        }
                    }
                }
                Ok(conn) => drop(conn),
                Err(_) => tracing::warn!("store writer stopped unexpectedly during shutdown"),
            }
        }
        self.shared.flush_activity.wake();
    }
}

impl Inner {
    /// Returns the store sender while this shared handle is still open.
    fn sender(&self) -> &mpsc::Sender<writer::Cmd> {
        self.tx.as_ref().expect("store writer is open")
    }
}

/// How many live entries a slow listener may fall behind before it has to
/// catch up from the database instead.
pub const LIVE_BUFFER: usize = 1024;

impl SpaceStore {
    /// Opens the journal at `<state>/spaces/<space-id>/index.db` with signed
    /// cursors. `secrets` is a separate witness root outside state backups.
    /// The host validates root locks and membership before opening/using it.
    /// Allocates channels/caches and a writer thread; opens/migrates SQLite
    /// and reads/writes key/witness files. No application writer mutex is held.
    pub async fn open(state: impl AsRef<Path>, space: SpaceId, secrets: &Path) -> Result<Self> {
        Self::open_with(state, space, secrets, Options::default()).await
    }

    /// Opens a bound space with custom retention and witness deadlines.
    /// Same allocation, SQLite and filesystem costs as [`Self::open`].
    pub async fn open_with(
        state: impl AsRef<Path>,
        space: SpaceId,
        secrets: &Path,
        options: Options,
    ) -> Result<Self> {
        Self::open_bound(
            space,
            true,
            state.as_ref().join("spaces").join(space.to_string()),
            options,
            Some(secrets),
        )
        .await
    }

    /// Returns the validated journal binding. Constant time; no lock, allocation or I/O.
    pub fn space(&self) -> SpaceId {
        self.inner.space
    }

    #[cfg(test)]
    async fn open_dir_secured(
        dir: impl AsRef<Path>,
        options: Options,
        secrets: Option<&Path>,
    ) -> Result<Self> {
        Self::open_bound(test_support::test_space(), false, dir, options, secrets).await
    }

    /// Opens (or creates) the store in `dir`. Fast: it only checks the
    /// schema and reads the journal head.
    #[cfg(test)]
    async fn open_dir(dir: impl AsRef<Path>) -> Result<Self> {
        Self::open_dir_with(dir, Options::default()).await
    }

    /// Like [`Self::open`], with your own [`Options`]. No secrets root: tokens
    /// stay plain and nothing checks for a restore.
    #[cfg(test)]
    async fn open_dir_with(dir: impl AsRef<Path>, options: Options) -> Result<Self> {
        Self::open_dir_secured(dir, options, None).await
    }

    /// Like [`Self::open_with`], and with `secrets` (the `--secrets` root)
    /// tokens are signed and a journal that went back in time is found at
    /// open (#918). The secrets root must be outside every backup.
    async fn open_bound(
        space: SpaceId,
        strict: bool,
        dir: impl AsRef<Path>,
        options: Options,
        secrets: Option<&Path>,
    ) -> Result<Self> {
        let dir = dir.as_ref().to_owned();
        let secrets = secrets.map(Path::to_owned);
        let with_check = secrets.is_some();
        let path = dir.join("index.db");
        let secret_dir = secrets
            .as_ref()
            .map(|root| root.join("spaces").join(space.to_string()));
        let db_dir = dir.clone();
        let db_path = path.clone();
        // The database opens on a pool thread, started now. While it runs,
        // this task reads the two small secret files itself: they overlap a
        // cold database open (as two tasks would), but no second pool thread
        // is needed, and spawning one cost more than the reads on a warm
        // reopen. They are two tiny files (32 bytes and one 4 KiB page), so
        // the time this holds the async worker is bounded and short.
        // The secrets folder is not created here: a missing key reads as
        // `None`, and creating the key below creates the folder.
        let database = tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&db_dir).map_err(Error::internal)?;
            schema::open_bound_writer(&db_path, with_check, space)
        });
        let files = match secret_dir {
            Some(secret_dir) => read_secret_files(secret_dir),
            None => Ok(None),
        };
        let opened = database.await.map_err(Error::internal)??;
        let files = files?;
        let schema::OpenState {
            mut conn,
            epoch,
            head,
            head_check,
            epoch_head,
            highwater_initialized,
        } = opened;
        let (guard, mark) = match files {
            Some((_dir, Some(key), head, mark)) => (
                Some(Guard {
                    key,
                    head: std::sync::Mutex::new(head),
                }),
                mark,
            ),
            Some((dir, None, head, mark)) => {
                let key_dir = dir.clone();
                let key = blocking(move || {
                    std::fs::create_dir_all(&key_dir).map_err(Error::internal)?;
                    token::Key::load_or_create(&key_dir)
                })
                .await?;
                (
                    Some(Guard {
                        key,
                        head: std::sync::Mutex::new(head),
                    }),
                    mark,
                )
            }
            None => (None, None),
        };
        let guard = guard.map(|mut g| {
            g.key = g.key.for_space(space);
            g
        });
        let shared = Arc::new(Shared::new(epoch.clone(), head, epoch_head, guard));
        let known_head = shared.guard.as_ref().map(|_| {
            let check = head_check.unwrap_or(0);
            shared.set_last(head, check);
            (head, check)
        });
        if shared.guard.is_some()
            && restore::restored_at_open(&conn, &shared, head, mark, highwater_initialized)?
        {
            restore::rotate(&mut conn, &shared, &epoch, known_head)?;
        }
        let (live, _) = broadcast::channel(LIVE_BUFFER);
        let (tx, writer_thread) =
            writer::spawn(conn, shared.clone(), live.clone(), &path, options)?;
        let inner = Arc::new(Inner {
            space,
            strict,
            path,
            shared: shared.clone(),
            tx: Some(tx),
            writer_thread: Some(writer_thread),
            live,
        });
        if shared.guard.is_some() {
            let flusher_path = inner.path.clone();
            spawn_flusher(
                Arc::downgrade(&inner),
                shared,
                flusher_path,
                options.highwater_quiet,
                options.highwater_max_staleness,
            );
        }
        Ok(Self { inner })
    }

    /// Listens for new entries as they are committed, in journal order.
    ///
    /// Subscribe first, then read anything older with [`Self::since`]; skip
    /// entries at or below the last position you have, and nothing is
    /// missed or shown twice. If a listener falls more than
    /// [`LIVE_BUFFER`] entries behind, it gets a lag error and should
    /// catch up with [`Self::since`].
    ///
    /// Cost: Allocates receiver bookkeeping; no writer lock, SQLite or disk I/O.
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<JournalEntry>> {
        self.inner.live.subscribe()
    }

    /// How many live listeners (connected clients) there are right now.
    ///
    /// Cost: Atomic receiver count; no writer lock, allocation or I/O.
    pub fn listeners(&self) -> usize {
        self.inner.live.receiver_count()
    }

    /// True when nothing else uses this store: no other handle, no live
    /// listener. The owner may then drop it to stop the writer thread and
    /// free the memory; opening it again later loses nothing, because
    /// accepted writes are drained before release. NORMAL sync retains the
    /// source power-loss limits; permanent publications use FULL.
    ///
    /// Cost: Reads handle/listener/save counters; no writer lock, allocation or I/O.
    pub fn is_unused(&self) -> bool {
        Arc::strong_count(&self.inner) == 1
            && self.inner.live.receiver_count() == 0
            && self.inner.shared.flush_activity.snapshot().active_saves == 0
    }

    /// The journal's head sequence number, updated on every append. Read it
    /// instead of asking for [`Self::head`] on a hot path.
    ///
    /// Cost: Clones an Arc; no writer lock, allocation or I/O.
    pub fn head_cell(&self) -> Arc<AtomicI64> {
        self.inner.shared.head.clone()
    }

    /// The journal's head now, as a plain token (not signed; see
    /// [`Self::token_at`]).
    ///
    /// Cost: Allocates an epoch string under a short epoch read lock; no SQLite.
    pub fn head(&self) -> SyncToken {
        SyncToken {
            epoch: self.inner.shared.epoch().to_string(),
            seq: self.inner.shared.head_now(),
        }
    }

    /// The token to give a client for position `seq`: signed when the store
    /// has a secrets root, so the client's next request proves what it saw.
    /// Position zero uses a signed zero check because no entry exists there.
    ///
    /// Cost: Allocates a token; short cache locks, no writer queue. Cache misses
    /// may open a read-only SQLite connection for one indexed lookup.
    pub async fn token_at(&self, seq: i64) -> Result<String> {
        let sh = &self.inner.shared;
        let epoch = sh.epoch();
        if self.inner.strict && seq < 0 {
            return Err(Error::new(
                ErrorCode::SyncTokenExpired,
                "negative cursor position",
            ));
        }
        if sh.guard.is_none() || seq < 0 {
            return Ok(format!("{epoch}-{seq}"));
        }
        if let Some(token) = sh.issued_token(&epoch, seq) {
            return Ok(token);
        }
        let check = if seq == 0 {
            Some(0)
        } else {
            match sh.last_check(seq) {
                Some(c) => Some(c),
                None => self.check_at(seq).await?,
            }
        };
        let token = match check {
            Some(c) => {
                let token = sh.sign(&epoch, seq, c);
                sh.remember_issued(&epoch, seq, &token);
                sh.remember_proof(&epoch, &token, seq);
                token
            }
            None if self.inner.strict => {
                return Err(Error::new(
                    ErrorCode::SyncTokenExpired,
                    "cursor entry missing",
                ));
            }
            None => format!("{epoch}-{seq}"),
        };
        Ok(token)
    }

    async fn check_at(&self, seq: i64) -> Result<Option<u32>> {
        let path = self.inner.path.clone();
        blocking(move || token::check_at(&schema::open_reader(&path)?, seq)).await
    }

    /// Checks a client's token and returns its position.
    ///
    /// - A token that does not parse, or is from another epoch, is
    ///   `SyncTokenExpired`: read again.
    /// - A signed token whose keyed tag is wrong is `SyncTokenExpired` with
    ///   "invalid token". Whoever sent it, it never starts a rotation.
    /// - A signed token with a good tag that points past the head, or at an
    ///   entry that is missing or different, **proves** the journal went
    ///   back in time. The epoch rotates (durably) and the answer is
    ///   `SyncTokenExpired`.
    /// - Plain positions are rejected by public space stores. Legacy plain
    ///   parsing remains exercised only through private source-test fixtures.
    ///
    /// Cost for a signed token: an exact cached cursor is O(1), a cache miss
    /// uses no read at the head or one indexed primary-key read elsewhere.
    ///
    /// Cost: Short cache/epoch locks and parsing allocations; at most one indexed
    /// SQLite lookup normally. Proven rollback queues a writer transaction
    /// and durable witness flush; invalid foreign signatures never rotate.
    pub async fn position(&self, token: &str) -> Result<i64> {
        let sh = &self.inner.shared;
        if let Some(seq) = sh.cached_proof(token) {
            return Ok(seq);
        }
        let (t, proof) = SyncToken::parse_signed(token)?;
        if self.inner.strict && proof.is_none() {
            return Err(Error::new(
                ErrorCode::SyncTokenExpired,
                "signed space cursor required",
            ));
        }
        let epoch = sh.epoch();
        let head = sh.head_now();
        let expired = |why: &str| {
            Error::new(
                ErrorCode::SyncTokenExpired,
                format!("token {token} does not belong to journal {epoch}-{head}: {why}"),
            )
        };
        let (Some(g), Some(p)) = (&sh.guard, proof) else {
            if t.epoch != *epoch || t.seq > head {
                return Err(expired("other epoch or past the head"));
            }
            return Ok(t.seq);
        };
        if g.key.tag(&t.epoch, t.seq, p.check) != p.tag {
            return Err(Error::new(ErrorCode::SyncTokenExpired, "invalid token"));
        }
        if t.epoch != *epoch {
            return Err(expired("other epoch"));
        }
        let proven = t.seq > head
            || (t.seq > 0
                && match sh.last_check(t.seq) {
                    Some(c) => c,
                    None => match self.check_at(t.seq).await? {
                        Some(c) => c,
                        None => return self.rolled_back(&epoch, expired).await,
                    },
                } != p.check);
        if proven {
            return self.rolled_back(&epoch, expired).await;
        }
        sh.remember_proof(&t.epoch, token, t.seq);
        Ok(t.seq)
    }

    async fn rolled_back(&self, epoch: &str, expired: impl FnOnce(&str) -> Error) -> Result<i64> {
        let (reply, rx) = oneshot::channel();
        self.inner
            .sender()
            .send(writer::Cmd::Rotate(epoch.to_owned(), reply))
            .await
            .map_err(|_| Error::internal("store writer stopped"))?;
        rx.await
            .map_err(|_| Error::internal("store writer dropped a rotation"))??;
        Err(expired("the journal went back in time"))
    }

    /// Counts how often the journal went back in time since this store
    /// opened. Above 0 right after open means the data was **restored**:
    /// plugins drop work that would only exist in the lost part of the log
    /// (#918).
    ///
    /// Cost: Clones watch receiver bookkeeping; no writer lock or SQLite work.
    pub fn restored(&self) -> watch::Receiver<u64> {
        self.inner.shared.restored.subscribe()
    }

    /// Keeps the witness scheduler idle-gated across a complete save action.
    ///
    /// Callers that write files before committing journal entries hold this
    /// guard around that whole operation, outside the journal writer thread.
    ///
    /// Cost: Updates atomics and wakes the scheduler; no allocation, writer lock or SQLite.
    pub fn begin_save_activity(&self) -> Option<SaveActivity<'_>> {
        self.inner
            .shared
            .guard
            .as_ref()
            .map(|_| self.inner.shared.flush_activity.begin_save())
    }

    /// Like [`Self::begin_save_activity`], for holders that cannot borrow the
    /// store (a plugin's file save outside any action). `None` without a
    /// secrets root, where there is no witness to schedule.
    ///
    /// Cost: Clones an Arc and updates atomics; no allocation, writer lock or SQLite.
    pub fn begin_owned_save_activity(&self) -> Option<OwnedSaveActivity> {
        self.inner.shared.guard.as_ref()?;
        Some(OwnedSaveActivity::begin(self.inner.shared.clone()))
    }

    /// The head when the current epoch began (0 for the first epoch).
    /// Plugin marks stamped in an older epoch count as this.
    ///
    /// Cost: Reads one atomic; no lock, allocation or SQLite work.
    pub fn epoch_head(&self) -> i64 {
        self.inner.shared.epoch_head.load(Ordering::Acquire)
    }

    /// Brings the high-water file up to date now. Tests use it to model a
    /// durable witness; normal writes use the idle and maximum-age scheduler.
    ///
    /// Cost: Schedules a blocking worker and takes the witness mutex; may read
    /// SQLite and fsync the witness. Allocates task/path data; no writer queue.
    pub async fn flush_highwater(&self) -> Result<()> {
        let shared = self.inner.shared.clone();
        let path = self.inner.path.clone();
        blocking(move || restore::flush(&shared, &path)).await
    }

    /// Saves events (and an idempotency record) atomically and returns the
    /// stored entries.
    ///
    /// Cost: Allocates a reply channel, encoded account key and output entries.
    /// Waits on the bounded single-writer queue; the writer serializes one
    /// SQLite transaction with NORMAL sync, then broadcasts. No caller mutex
    /// is held over I/O. This is not a power-loss-durable edit ack contract.
    pub async fn commit(&self, mut commit: Commit) -> Result<Vec<JournalEntry>> {
        if let Some(idem) = &mut commit.idempotency {
            idem.key = receipt_key(idem.account, &idem.key);
        }
        let (reply, rx) = oneshot::channel();
        self.inner
            .sender()
            .send(writer::Cmd::Commit(commit, reply))
            .await
            .map_err(|_| Error::internal("store writer stopped"))?;
        rx.await
            .map_err(|_| Error::internal("store writer dropped a commit"))?
    }

    /// Appends a complete publication batch once. Receipts do not expire.
    ///
    /// Cost: Account-qualified publication keys prevent cross-member receipt reuse.
    /// Allocates a reply channel/key/events; queues one SQLite transaction
    /// with FULL sync. The single writer serializes it; no caller mutex is held.
    pub async fn commit_once(
        &self,
        account: AccountId,
        key: String,
        digest: String,
        events: Vec<StampedEvent>,
    ) -> Result<crate::SyncToken> {
        let (reply, rx) = oneshot::channel();
        self.inner
            .sender()
            .send(writer::Cmd::Once(
                receipt_key(account, &key),
                digest,
                events,
                reply,
            ))
            .await
            .map_err(|_| Error::internal("store writer stopped"))?;
        rx.await
            .map_err(|_| Error::internal("store writer dropped publication"))?
    }

    /// Finds an earlier result for an idempotency key.
    ///
    /// Cost: Requires the authenticated acting account, supplied by trusted host code.
    /// Allocates an encoded key and decoded result; opens one read-only SQLite
    /// connection for an indexed lookup, without entering the writer queue.
    pub async fn idempotency(&self, account: AccountId, key: &str) -> Result<Option<IdemRecord>> {
        let path = self.inner.path.clone();
        let key = receipt_key(account, key);
        blocking(move || read::idempotency(&path, &key)).await
    }

    /// Entries after position `after`, oldest first, at most `limit`.
    ///
    /// Cost: Allocates at most 10,000 entries plus JSON/token strings; opens a
    /// read-only SQLite connection and indexed page query. Uses short token
    /// cache locks, without entering the writer queue.
    pub async fn since(&self, after: i64, limit: usize) -> Result<Vec<JournalEntry>> {
        let path = self.inner.path.clone();
        let shared = self.inner.shared.clone();
        blocking(move || read::since(&path, &shared, after, limit)).await
    }
}

/// Polls the head atomic and flushes after a quiet save window or its maximum age.
///
/// Both wait until no save is in flight, so the witness sync never shares
/// the disk with a save's sync; the end of a save wakes this task at once.
///
/// The initial head snapshot is taken before scheduling the task, so a commit
/// that lands before the task's first poll is still observed.
fn spawn_flusher(
    inner: Weak<Inner>,
    shared: Arc<Shared>,
    store_path: PathBuf,
    quiet: Duration,
    maximum_age: Duration,
) {
    let mut observed_head = shared.head.load(Ordering::Acquire);
    tokio::spawn(async move {
        let poll_cap = maximum_age.min(Duration::from_millis(200));
        let poll = if maximum_age.is_zero() {
            Duration::from_millis(100)
        } else {
            Duration::from_nanos((poll_cap.as_nanos() / 2).max(1) as u64)
        };
        let stale_after = maximum_age.saturating_sub(poll);
        let mut retry_after_ns = 0;
        loop {
            if inner.strong_count() == 0 {
                break;
            }
            let head = shared.head.load(Ordering::Acquire);
            if head != observed_head {
                observed_head = head;
                shared.flush_activity.observe_head_change();
            }
            let notified = shared.flush_activity.notifier().notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let now_ns = shared.flush_activity.now_ns();
            let snapshot = shared.flush_activity.snapshot();
            if snapshot.dirty_since_ns == 0 {
                tokio::select! {
                    _ = notified.as_mut() => {},
                    _ = tokio::time::sleep(poll) => {},
                }
                continue;
            }

            if let Some(reason) = snapshot.reason(now_ns, quiet, stale_after) {
                if now_ns < retry_after_ns {
                    let wait = Duration::from_nanos(retry_after_ns - now_ns);
                    tokio::select! {
                        _ = notified.as_mut() => retry_after_ns = 0,
                        _ = tokio::time::sleep(wait) => {}
                    }
                    continue;
                }
                let Some(dirty_since) = shared.flush_activity.begin_flush() else {
                    continue;
                };
                let s = shared.clone();
                let path = store_path.clone();
                let result = tokio::task::spawn_blocking(move || restore::flush(&s, &path)).await;
                match result {
                    Ok(Ok(())) => {
                        shared.flush_activity.finish_flush(dirty_since, true);
                        retry_after_ns = 0;
                    }
                    Ok(Err(e)) => {
                        shared.flush_activity.finish_flush(dirty_since, false);
                        tracing::warn!(%e, ?reason, "could not update the high-water file");
                        retry_after_ns = shared
                            .flush_activity
                            .now_ns()
                            .saturating_add(Duration::from_secs(1).as_nanos() as u64);
                    }
                    Err(e) => {
                        shared.flush_activity.finish_flush(dirty_since, false);
                        tracing::warn!(%e, ?reason, "high-water worker stopped before finishing");
                        retry_after_ns = shared
                            .flush_activity
                            .now_ns()
                            .saturating_add(Duration::from_secs(1).as_nanos() as u64);
                    }
                }
                continue;
            }

            let deadline = snapshot.deadline(quiet, stale_after).max(retry_after_ns);
            let wait = Duration::from_nanos(deadline.saturating_sub(now_ns)).min(poll);
            tokio::select! {
                _ = notified.as_mut() => {},
                _ = tokio::time::sleep(wait) => {}
            }
        }
    });
}

/// A space's secrets folder with its key (if made yet), witness writer and
/// last durable mark (if any).
type SecretFiles = (
    PathBuf,
    Option<token::Key>,
    highwater::Writer,
    Option<highwater::Mark>,
);

/// The per-space key and the witness file, read without creating either.
fn read_secret_files(secret_dir: PathBuf) -> Result<Option<SecretFiles>> {
    let key = token::Key::load_existing(&secret_dir);
    let (head, mark) =
        highwater::Writer::open(&secret_dir.join("head")).map_err(Error::internal)?;
    Ok(Some((secret_dir, key, head, mark)))
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(Error::internal)?
}

use calternal_types::now_ms;

#[cfg(test)]
mod tests;

// Fixed-width UUID prefix plus the opaque request ID is injective even when
// request IDs contain separators; callers never supply the encoded key.
fn receipt_key(account: AccountId, request: &str) -> String {
    format!("{account}:{request}")
}

#[cfg(test)]
mod test_support;
