//! Lazy store lifecycle adapted from `engine/src/runtime.rs`, without actions.
use crate::{Options, Result, SpaceId, SpaceStore};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::OnceCell;

struct Entry {
    store: OnceCell<SpaceStore>,
    last_used: Mutex<Instant>,
}

/// Lazy per-space stores with bounded idle retention. The host owns this pool
/// and calls [`Self::sweep`] periodically (at most every idle deadline) even
/// when no requests arrive. Authorization remains the host's responsibility.
/// Opening the same space through separate pools is unsupported: use one pool
/// per locked installation, as the inherited engine did.
pub struct SpaceStores {
    state: PathBuf,
    secrets: PathBuf,
    options: Options,
    idle_after: Duration,
    map: Mutex<HashMap<SpaceId, Arc<Entry>>>,
}
impl SpaceStores {
    /// Creates an empty pool; allocates paths/map, with no SQLite or disk I/O.
    /// Use 600 seconds for the accepted idle-release target. A zero duration
    /// permits immediate release once all users/listeners/save guards leave.
    pub fn new(state: PathBuf, secrets: PathBuf, options: Options, idle_after: Duration) -> Self {
        Self {
            state,
            secrets,
            options,
            idle_after,
            map: Mutex::new(HashMap::new()),
        }
    }

    /// Gets a space's shared writer, opening it once on concurrent first use.
    /// Holds the map mutex only for lookup/allocation; SQLite/key I/O happens
    /// outside it. Allocates one handle clone; first use has [`SpaceStore::open`]
    /// costs. A returned handle pins the store against idle eviction.
    pub async fn get(&self, space: SpaceId) -> Result<SpaceStore> {
        self.sweep();
        let entry = {
            let mut map = self.map.lock().unwrap_or_else(|p| p.into_inner());
            map.entry(space)
                .or_insert_with(|| {
                    Arc::new(Entry {
                        store: OnceCell::new(),
                        last_used: Mutex::new(Instant::now()),
                    })
                })
                .clone()
        };
        let store = entry
            .store
            .get_or_try_init(|| {
                SpaceStore::open_with(&self.state, space, &self.secrets, self.options)
            })
            .await?
            .clone();
        *entry.last_used.lock().unwrap_or_else(|p| p.into_inner()) = Instant::now();
        Ok(store)
    }

    /// Releases unused stores after their idle bound, returning the count.
    /// Takes map/last-use mutexes, allocates a vector of removed entries, then
    /// drains writer threads and flushes witnesses outside the map lock.
    /// Never evicts an opening store, held handle, listener or active save.
    /// The host must schedule this method to bound release during total idleness.
    pub fn sweep(&self) -> usize {
        let mut dropped = Vec::new();
        {
            let mut map = self.map.lock().unwrap_or_else(|p| p.into_inner());
            map.retain(|_, entry| {
                let quiet = entry
                    .last_used
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .elapsed()
                    >= self.idle_after;
                let alone = Arc::strong_count(entry) == 1
                    && entry.store.get().is_none_or(SpaceStore::is_unused);
                if quiet && alone {
                    dropped.push(entry.clone());
                    false
                } else {
                    true
                }
            });
        }
        dropped.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Commit, StampedEvent, test_support::test_account};
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Adapted from engine/tests.rs:626; all 640 writes survive concurrent eviction.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn no_write_is_lost_while_spaces_are_released_and_reopened() {
        let temp = tempfile::tempdir().unwrap();
        let pool = Arc::new(SpaceStores::new(
            temp.path().join("state"),
            temp.path().join("secrets"),
            Options::default(),
            Duration::ZERO,
        ));
        let spaces: Vec<_> = (0..4).map(|_| SpaceId::new()).collect();
        // Guarantee that eviction is exercised, independently of task scheduling.
        let mut held = Vec::new();
        for &space in &spaces {
            held.push(pool.get(space).await.unwrap());
        }
        drop(held);
        assert_eq!(pool.sweep(), 4);
        let stop = Arc::new(AtomicBool::new(false));
        let sweeper = tokio::spawn({
            let pool = pool.clone();
            let stop = stop.clone();
            async move {
                let mut dropped = 0;
                while !stop.load(Ordering::SeqCst) {
                    dropped += pool.sweep();
                    tokio::task::yield_now().await;
                }
                dropped
            }
        });
        let mut tasks = Vec::new();
        for &space in &spaces {
            for t in 0..4 {
                let pool = pool.clone();
                tasks.push(tokio::spawn(async move {
                    for i in 0..40 {
                        pool.get(space)
                            .await
                            .unwrap()
                            .commit(Commit {
                                events: vec![StampedEvent {
                                    author: test_account(),
                                    kind: "t.say".into(),
                                    item: format!("{t}-{i}"),
                                    data: serde_json::json!({}),
                                    installation: None,
                                    request_id: None,
                                }],
                                idempotency: None,
                            })
                            .await
                            .unwrap();
                    }
                }));
            }
        }
        for task in tasks {
            task.await.unwrap();
        }
        stop.store(true, Ordering::SeqCst);
        let dropped = sweeper.await.unwrap();
        assert!(dropped > 0, "the concurrent sweeper never released a space");
        for space in spaces {
            let store = pool.get(space).await.unwrap();
            assert_eq!(
                store
                    .since(0, 1000)
                    .await
                    .unwrap()
                    .iter()
                    .map(|e| e.seq)
                    .collect::<Vec<_>>(),
                (1..=160).collect::<Vec<_>>()
            );
        }
    }

    #[tokio::test]
    async fn listeners_handles_and_owned_saves_prevent_release() {
        let temp = tempfile::tempdir().unwrap();
        let pool = SpaceStores::new(
            temp.path().join("state"),
            temp.path().join("secrets"),
            Options::default(),
            Duration::ZERO,
        );
        let store = pool.get(SpaceId::new()).await.unwrap();
        assert_eq!(pool.sweep(), 0);
        let listener = store.subscribe();
        let save = store.begin_owned_save_activity().unwrap();
        drop(store);
        assert_eq!(pool.sweep(), 0);
        drop(listener);
        assert_eq!(pool.sweep(), 0);
        drop(save);
        assert_eq!(pool.sweep(), 1);
    }
}
