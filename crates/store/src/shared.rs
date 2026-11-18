//! State the writer, the readers and the timer all look at.

use crate::highwater;
use crate::proof::{IssuedTokenCache, ProofCache};
use crate::token::{Key, entry_check};
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::Notify;
use tokio::sync::watch;

/// What turns on restore detection: the per-space key and open high-water file.
pub(crate) struct Guard {
    /// The private key that signs sync tokens.
    pub key: Key,
    /// The path-based witness writer used only by the flush task.
    pub head: Mutex<highwater::Writer>,
}

/// What was last written to the high-water file.
#[derive(Default)]
pub(crate) struct Written {
    /// Epoch whose mark was last made durable.
    pub epoch: String,
    /// Sequence covered by the durable mark.
    pub seq: i64,
    /// Entry check covered by the durable mark.
    pub check: u32,
}

/// State shared by the per-space writer, read tasks and witness timer.
pub(crate) struct Shared {
    epoch: RwLock<Arc<str>>,
    pub(crate) lifecycle:
        std::sync::OnceLock<(calternal_types::OwnerLifecycle, calternal_types::SpaceId)>,
    pub head: Arc<AtomicI64>,
    /// The head when the current epoch began; 0 for the first epoch.
    pub epoch_head: AtomicI64,
    /// `None`: no secrets root was given, so tokens stay plain and nothing
    /// is checked (how tests and one-off tools run).
    pub guard: Option<Guard>,
    /// Tracks save scopes and the oldest unwitnessed journal update.
    pub flush_activity: FlushActivity,
    /// Newest entry and its `c`, so a device at the head costs no read.
    last: Mutex<(i64, u32)>,
    /// Serializes witness updates with epoch rotations. Save commits never
    /// acquire this lock.
    pub file: Mutex<Written>,
    /// The one exact signed cursor whose entry proof is already known.
    proof: Mutex<ProofCache>,
    /// The 128 newest signed cursors, reused by catch-up and token replies.
    issued: Mutex<IssuedTokenCache>,
    /// Counts token computations so tests can prove journal reads reuse them.
    #[cfg(test)]
    mint_calls: AtomicUsize,
    /// Counts times the journal went back in time since the server
    /// started. Above 0 at startup means "restored".
    pub restored: watch::Sender<u64>,
}

/// One observation of when a witness write may start.
#[derive(Clone, Copy)]
pub(crate) struct FlushSnapshot {
    /// The oldest monotonic timestamp not yet covered by a witness.
    pub dirty_since_ns: u64,
    /// The end of the most recent save request.
    pub last_save_ns: u64,
    /// Save actions and commits still in flight.
    pub active_saves: usize,
}

/// The reason the current witness is ready to be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FlushReason {
    /// The save path has been idle for its quiet window.
    Idle,
    /// The oldest unwitnessed journal update reached its maximum age.
    Stale,
}

impl FlushSnapshot {
    /// Finds whether an idle or maximum-age condition is due at `now_ns`.
    ///
    /// The witness `fdatasync` shares the disk with save syncs, so a flush
    /// never starts while a save is in flight: the maximum-age flush waits
    /// for the save to end (its end wakes the flusher) and then runs in the
    /// gap before the next one. Only a save stream that never pauses for
    /// twice the maximum age forces the flush, which bounds staleness at
    /// `2 * maximum_age` in that worst case.
    pub(crate) fn reason(
        self,
        now_ns: u64,
        quiet: Duration,
        maximum_age: Duration,
    ) -> Option<FlushReason> {
        if self.dirty_since_ns == 0 {
            return None;
        }
        let age = now_ns.saturating_sub(self.dirty_since_ns);
        if age >= duration_ns(maximum_age).saturating_mul(2) {
            return Some(FlushReason::Stale);
        }
        if self.active_saves > 0 {
            return None;
        }
        if age >= duration_ns(maximum_age) {
            return Some(FlushReason::Stale);
        }
        if now_ns.saturating_sub(self.last_save_ns) >= duration_ns(quiet) {
            return Some(FlushReason::Idle);
        }
        None
    }

    /// Returns the next event-based deadline without polling on a fixed timer.
    ///
    /// With a save in flight only the forced bound is a deadline; the save's
    /// end notifies the flusher, which then re-checks at once.
    pub(crate) fn deadline(self, quiet: Duration, maximum_age: Duration) -> u64 {
        if self.active_saves > 0 {
            return self
                .dirty_since_ns
                .saturating_add(duration_ns(maximum_age).saturating_mul(2));
        }
        let stale = self.dirty_since_ns.saturating_add(duration_ns(maximum_age));
        stale.min(self.last_save_ns.saturating_add(duration_ns(quiet)))
    }
}

/// Tracks save activity without holding a save-path lock during witness I/O.
pub(crate) struct FlushActivity {
    started: Instant,
    last_save_ns: AtomicU64,
    dirty_since_ns: AtomicU64,
    active_saves: AtomicUsize,
    changed: Notify,
}

/// Marks one save action as in flight until it returns.
pub struct SaveActivity<'a> {
    activity: &'a FlushActivity,
}

impl FlushActivity {
    /// Starts a monotonic clock for activity and creates its wake signal.
    fn new() -> Self {
        Self {
            started: Instant::now(),
            last_save_ns: AtomicU64::new(1),
            dirty_since_ns: AtomicU64::new(0),
            active_saves: AtomicUsize::new(0),
            changed: Notify::new(),
        }
    }

    /// Marks a save in flight and makes its witness age visible to the waiter.
    pub(crate) fn begin_save(&self) -> SaveActivity<'_> {
        self.start_save();
        SaveActivity { activity: self }
    }

    /// Counts one save in flight; [`Self::end_save`] releases it.
    fn start_save(&self) {
        self.active_saves.fetch_add(1, Ordering::AcqRel);
        let now = self.now_ns();
        let _ = self
            .dirty_since_ns
            .compare_exchange(0, now, Ordering::AcqRel, Ordering::Acquire);
        self.changed.notify_one();
    }

    /// Returns the current activity snapshot for the single flusher task.
    pub(crate) fn snapshot(&self) -> FlushSnapshot {
        FlushSnapshot {
            dirty_since_ns: self.dirty_since_ns.load(Ordering::Acquire),
            last_save_ns: self.last_save_ns.load(Ordering::Acquire),
            active_saves: self.active_saves.load(Ordering::Acquire),
        }
    }

    /// Starts an idle window after a head change or a validated older startup mark.
    ///
    /// The writer only publishes its existing head atomic; this method runs
    /// on the flusher so observing a direct record adds no commit-path work.
    pub(crate) fn observe_head_change(&self) {
        let now = self.now_ns();
        self.last_save_ns.store(now, Ordering::Release);
        let _ = self
            .dirty_since_ns
            .compare_exchange(0, now, Ordering::AcqRel, Ordering::Acquire);
    }

    /// Returns nanoseconds since this store's monotonic clock began.
    pub(crate) fn now_ns(&self) -> u64 {
        self.started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    }

    /// Atomically reserves the oldest dirty age before a blocking flush starts.
    pub(crate) fn begin_flush(&self) -> Option<u64> {
        let dirty_since = self.dirty_since_ns.swap(0, Ordering::AcqRel);
        (dirty_since != 0).then_some(dirty_since)
    }

    /// Restores the old dirty age after failure, or wakes the next deadline.
    pub(crate) fn finish_flush(&self, dirty_since: u64, succeeded: bool) {
        if !succeeded {
            let _ =
                self.dirty_since_ns
                    .try_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                        Some(if current == 0 || dirty_since < current {
                            dirty_since
                        } else {
                            current
                        })
                    });
        }
        self.changed.notify_one();
    }

    /// Clears the dirty marker after startup or a required epoch write.
    pub(crate) fn mark_written(&self) {
        self.dirty_since_ns.store(0, Ordering::Release);
        self.changed.notify_one();
    }

    /// Returns the scheduler's single task wake signal.
    pub(crate) fn notifier(&self) -> &Notify {
        &self.changed
    }

    /// Wakes the scheduler when the store is shutting down.
    pub(crate) fn wake(&self) {
        self.changed.notify_one();
    }
}

impl FlushActivity {
    /// Ends one save: keeps the witness dirty, releases the in-flight count,
    /// starts a quiet window, and wakes the flusher so a due flush runs now,
    /// in the gap before the next save.
    fn end_save(&self) {
        let now = self.now_ns();
        self.last_save_ns.store(now, Ordering::Release);
        let _ = self
            .dirty_since_ns
            .compare_exchange(0, now, Ordering::AcqRel, Ordering::Acquire);
        self.active_saves.fetch_sub(1, Ordering::AcqRel);
        self.changed.notify_one();
    }
}

impl Drop for SaveActivity<'_> {
    /// Makes cancellation release the save-in-flight marker as well.
    fn drop(&mut self) {
        self.activity.end_save();
    }
}

/// A save in flight that owns its store state, for holders that cannot
/// borrow the store: a plugin's file save that runs outside any action.
pub struct OwnedSaveActivity {
    shared: Arc<Shared>,
}

impl OwnedSaveActivity {
    /// Marks a save in flight until the returned value drops.
    pub(crate) fn begin(shared: Arc<Shared>) -> Self {
        shared.flush_activity.start_save();
        Self { shared }
    }
}

impl Drop for OwnedSaveActivity {
    /// Releases the save-in-flight marker, on success, error or cancellation.
    fn drop(&mut self) {
        self.shared.flush_activity.end_save();
    }
}

/// Converts a duration to the monotonic clock's nanosecond unit.
fn duration_ns(duration: Duration) -> u64 {
    duration.as_nanos().min(u128::from(u64::MAX)) as u64
}

impl Shared {
    /// Builds the shared state for one store before its writer thread starts.
    pub fn new(epoch: String, head: i64, epoch_head: i64, guard: Option<Guard>) -> Self {
        Self {
            epoch: RwLock::new(epoch.into()),
            lifecycle: std::sync::OnceLock::new(),
            head: Arc::new(AtomicI64::new(head)),
            epoch_head: AtomicI64::new(epoch_head),
            guard,
            flush_activity: FlushActivity::new(),
            last: Mutex::new((0, 0)),
            file: Mutex::new(Written::default()),
            proof: Mutex::new(ProofCache::default()),
            issued: Mutex::new(IssuedTokenCache::default()),
            #[cfg(test)]
            mint_calls: AtomicUsize::new(0),
            restored: watch::channel(0).0,
        }
    }

    pub(crate) fn step(&self) -> crate::Result<Option<calternal_types::OwnerStep>> {
        self.lifecycle
            .get()
            .map(|(lifecycle, space)| lifecycle.step(space.as_uuid()))
            .transpose()
            .map_err(crate::Error::internal)
    }

    /// Returns the number of token computations made by this store in tests.
    #[cfg(test)]
    pub(crate) fn mint_calls(&self) -> usize {
        self.mint_calls.load(Ordering::Relaxed)
    }

    pub fn epoch(&self) -> Arc<str> {
        self.epoch.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Publishes an epoch and discards all cursors cached for the prior one.
    pub fn set_epoch(&self, epoch: &str) {
        let mut proof = self.proof.lock().unwrap_or_else(|p| p.into_inner());
        let mut issued = self.issued.lock().unwrap_or_else(|p| p.into_inner());
        *self.epoch.write().unwrap_or_else(|p| p.into_inner()) = epoch.into();
        proof.clear();
        issued.clear();
    }

    pub fn set_last(&self, seq: i64, check: u32) {
        *self.last.lock().unwrap_or_else(|p| p.into_inner()) = (seq, check);
    }

    /// `c` of the newest entry, if `seq` is it.
    pub fn last_check(&self, seq: i64) -> Option<u32> {
        let l = self.last.lock().unwrap_or_else(|p| p.into_inner());
        (l.0 == seq).then_some(l.1)
    }

    /// Returns a previously verified exact cursor without opening SQLite.
    pub fn cached_proof(&self, token: &str) -> Option<i64> {
        self.proof
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .lookup(token)
    }

    /// Returns a signed token already issued for this current epoch and sequence.
    pub fn issued_token(&self, epoch: &str, seq: i64) -> Option<String> {
        self.guard.as_ref()?;
        let issued = self.issued.lock().unwrap_or_else(|p| p.into_inner());
        if self.epoch().as_ref() != epoch {
            return None;
        }
        issued.lookup(epoch, seq).map(str::to_owned)
    }

    /// `c` of the entry at `seq`, read from a token this store minted itself.
    ///
    /// The writer caches the signed token of every new entry, and the token
    /// carries the entry check (`epoch-seq-c-m`). Reading it here lets the
    /// flush worker skip opening SQLite, without adding work to commits.
    pub fn issued_check(&self, epoch: &str, seq: i64) -> Option<u32> {
        let token = self.issued_token(epoch, seq)?;
        let (parsed, proof) = crate::SyncToken::parse_signed(&token).ok()?;
        (parsed.seq == seq).then_some(proof?.check)
    }

    /// Reads cached tokens for a page and reports when its misses need storing.
    pub fn issued_page(
        &self,
        epoch: &str,
        sequences: impl Iterator<Item = i64>,
    ) -> (Vec<Option<String>>, bool) {
        if self.guard.is_none() {
            return (sequences.map(|_| None).collect(), true);
        }
        let issued = self.issued.lock().unwrap_or_else(|p| p.into_inner());
        if self.epoch().as_ref() != epoch {
            return (sequences.map(|_| None).collect(), true);
        }
        issued.lookup_page(epoch, sequences)
    }

    /// Remembers a page's missing tokens without replacing tokens it already hit.
    pub fn remember_issued_page(
        &self,
        epoch: &str,
        tokens: &[(i64, String)],
        protected: impl Iterator<Item = i64>,
    ) {
        if self.guard.is_none() || tokens.is_empty() {
            return;
        }
        let mut issued = self.issued.lock().unwrap_or_else(|p| p.into_inner());
        if self.epoch().as_ref() == epoch {
            issued.remember_page(epoch, tokens, protected);
        }
    }

    /// Remembers a signed token only while its epoch remains current.
    pub fn remember_issued(&self, epoch: &str, seq: i64, token: &str) {
        if self.guard.is_none() {
            return;
        }
        let mut issued = self.issued.lock().unwrap_or_else(|p| p.into_inner());
        if self.epoch().as_ref() == epoch {
            issued.remember(epoch, seq, token);
        }
    }

    /// Remembers a token only while its issuing epoch is still current.
    pub fn remember_proof(&self, epoch: &str, token: &str, seq: i64) {
        let mut proof = self.proof.lock().unwrap_or_else(|p| p.into_inner());
        if self.epoch().as_ref() == epoch {
            proof.remember(token, seq);
        }
    }

    /// The token for an entry: signed when there is a key.
    pub fn mint(
        &self,
        epoch: &str,
        seq: i64,
        at_ms: i64,
        kind: &str,
        item: &str,
        request_id: Option<&str>,
    ) -> String {
        self.mint_with_check(epoch, seq, at_ms, kind, item, request_id)
            .0
    }

    /// Mints an entry cursor and returns the already computed entry check so
    /// the writer can reuse it for its head cache after the transaction commits.
    pub fn mint_with_check(
        &self,
        epoch: &str,
        seq: i64,
        at_ms: i64,
        kind: &str,
        item: &str,
        request_id: Option<&str>,
    ) -> (String, Option<u32>) {
        #[cfg(test)]
        self.mint_calls.fetch_add(1, Ordering::Relaxed);
        match &self.guard {
            None => (format!("{epoch}-{seq}"), None),
            Some(g) => {
                let c = entry_check(seq, at_ms, kind, item, request_id);
                (
                    crate::token::signed_token(epoch, seq, c, g.key.tag(epoch, seq, c)),
                    Some(c),
                )
            }
        }
    }

    /// The token for a position whose `c` is already known. At position zero,
    /// the known check is zero because no journal entry exists yet.
    pub fn sign(&self, epoch: &str, seq: i64, check: u32) -> String {
        match &self.guard {
            Some(g) if seq >= 0 => {
                format!(
                    "{epoch}-{seq}-{check:08x}-{:08x}",
                    g.key.tag(epoch, seq, check)
                )
            }
            _ => format!("{epoch}-{seq}"),
        }
    }

    pub fn head_now(&self) -> i64 {
        self.head.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod flush_tests {
    //! Proves idle and maximum-age deadlines are independent scheduler triggers.

    use super::*;

    /// Idle flush waits for a full quiet window and an empty in-flight count.
    #[test]
    fn idle_flush_requires_one_quiet_second_and_no_save_in_flight() {
        let snapshot = FlushSnapshot {
            dirty_since_ns: 1,
            last_save_ns: 500,
            active_saves: 0,
        };
        let quiet = Duration::from_secs(1);
        let maximum_age = Duration::from_secs(30);

        assert_eq!(snapshot.reason(1_000_000_499, quiet, maximum_age), None);
        assert_eq!(
            snapshot.reason(1_000_000_500, quiet, maximum_age),
            Some(FlushReason::Idle)
        );
        assert_eq!(
            FlushSnapshot {
                active_saves: 1,
                ..snapshot
            }
            .reason(2_000_000_000, quiet, maximum_age),
            None
        );
    }

    /// The maximum-age flush waits for an in-flight save to end, then runs.
    #[test]
    fn maximum_age_flush_waits_for_the_save_in_flight() {
        let quiet = Duration::from_secs(1);
        let maximum_age = Duration::from_secs(30);
        let busy = FlushSnapshot {
            dirty_since_ns: 1,
            last_save_ns: 30_000_000_000,
            active_saves: 1,
        };
        assert_eq!(busy.reason(30_000_000_001, quiet, maximum_age), None);
        assert_eq!(busy.deadline(quiet, maximum_age), 60_000_000_001);
        let between = FlushSnapshot {
            active_saves: 0,
            ..busy
        };
        assert_eq!(
            between.reason(30_000_000_001, quiet, maximum_age),
            Some(FlushReason::Stale)
        );
    }

    /// A save stream that never pauses still flushes at twice the maximum age.
    #[test]
    fn a_save_stream_that_never_pauses_forces_the_flush_at_twice_the_maximum_age() {
        let snapshot = FlushSnapshot {
            dirty_since_ns: 1,
            last_save_ns: 1,
            active_saves: 2,
        };
        let quiet = Duration::from_secs(1);
        let maximum_age = Duration::from_secs(30);
        assert_eq!(snapshot.reason(60_000_000_000, quiet, maximum_age), None);
        assert_eq!(
            snapshot.reason(60_000_000_001, quiet, maximum_age),
            Some(FlushReason::Stale)
        );
    }
}
