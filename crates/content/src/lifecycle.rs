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
    pub(crate) space: Mutex<Option<Space>>,
    pub(crate) last_used: Mutex<Instant>,
}
impl Slot {
    /// Whether something outside the cache (a live subscriber) still holds this
    /// space's journal. Cost: a try-lock and two counter reads; a slot being worked
    /// on right now counts as in use.
    pub(crate) fn in_use(&self) -> bool {
        match self.space.try_lock() {
            Ok(space) => space.as_ref().is_some_and(|s| !s.journal.is_unused()),
            Err(_) => true,
        }
    }
}
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
/// Sweeps `spaces` every 30 seconds until the cache is dropped.
pub(crate) fn spawn_sweeper(spaces: &Cache, idle: Duration) -> Result<()> {
    let weak = Arc::downgrade(spaces);
    std::thread::Builder::new()
        .name("content-sweep".into())
        .spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(30));
                let Some(spaces) = weak.upgrade() else { return };
                sweep(&spaces, idle);
            }
        })
        .map(drop)
        .map_err(internal)
}
