//! Bounded lazy space lifecycle shared by admission and idle retirement.
use crate::{Result, db::Space, journal::lock, types::internal};
use calternal_types::SpaceId;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
/// One cached space: opened on first use under its own lock, so a cold open in
/// one space never blocks another.
pub(crate) struct Slot {
    /// Lazily opened stores, serialized only within this space.
    pub(crate) space: Mutex<Option<Space>>,
    /// Idle-retirement clock, updated after each completed operation.
    pub(crate) last_used: Mutex<Instant>,
}
impl Slot {
    /// Whether something outside the cache (a live subscriber) still holds this
    /// space's journal. Cost: a try-lock and two counter reads; a slot being worked
    /// on right now counts as in use.
    pub(crate) fn in_use(&self) -> bool {
        match self.space.try_lock() {
            Ok(space) => space
                .as_ref()
                .is_some_and(|s| s.pending_since.is_some() || !s.journal.is_unused()),
            Err(_) => true,
        }
    }
}
/// The host root locks; a worker clones their Arc only for active placement.
pub(crate) type RootGuard = Arc<Mutex<Option<Arc<calternal_roots::RootLocks>>>>;
/// The existing cache holds at most 64 slots; its map lock never spans I/O.
pub(crate) type Cache = Arc<Mutex<HashMap<SpaceId, Arc<Slot>>>>;
/// Closes idle spaces nobody holds. Closing drops the space's journal store, which
/// drains its writer, outside the map lock.
pub(crate) fn sweep(spaces: &Cache, idle: Duration) -> usize {
    let mut closed = Vec::new();
    {
        let mut map = lock(spaces);
        map.retain(|_, slot| {
            let keep = Arc::strong_count(slot) > 1
                || lock(&slot.last_used).elapsed() < idle
                || slot.in_use();
            if !keep {
                closed.push(slot.clone());
            }
            keep
        });
    }
    closed.len()
}
/// Materializes pending heads and retires idle spaces on the existing worker.
/// Wakes after writes; failed work retries in 50 ms and idle sweeping remains
/// every 30 seconds. Visits at most 64 slots, with write
/// backpressure at one second. No queue, per-write task or second cache is added.
pub(crate) fn spawn_sweeper(
    spaces: &Cache,
    root_guard: &RootGuard,
    idle: Duration,
    fault: fn(crate::FaultPoint),
) -> Result<std::thread::Thread> {
    let weak = Arc::downgrade(spaces);
    let root_guard = Arc::downgrade(root_guard);
    let worker = std::thread::Builder::new()
        .name("content-sweep".into())
        .spawn(move || {
            let mut wait = Duration::from_secs(30);
            loop {
                std::thread::park_timeout(wait);
                wait = Duration::from_secs(30);
                let Some(guard) = root_guard.upgrade() else {
                    return;
                };
                let _pin = lock(&guard).clone();
                let Some(spaces) = weak.upgrade() else { return };
                // Clone at most 64 slots; never hold the map lock across disk I/O.
                let slots: Vec<_> = lock(&spaces).values().cloned().collect();
                for slot in slots {
                    if let Ok(mut space) = slot.space.try_lock()
                        && let Some(space) = space.as_mut()
                        && space.pending_since.is_some()
                    {
                        if space.materialize_pending(fault).is_err() {
                            // Keep the original age: writes backpressure after 1s.
                            wait = Duration::from_millis(50);
                        }
                    }
                }
                sweep(&spaces, idle);
            }
        })
        .map_err(internal)?;
    Ok(worker.thread().clone())
}
