//! Viewer-scoped, bounded memory cache for fetched media and pending results.

use calternal_types::{AccountId, Owner, SpaceId};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};
use tokio::sync::Notify;

const SHARDS: usize = 16;
const SHARD_BYTES: usize = 32 * 1024 * 1024;
const SHARD_ENTRIES: usize = 256;
const READY_TTL: Duration = Duration::from_secs(24 * 3600);
const FAILED_TTL: Duration = Duration::from_secs(60);
const PENDING_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(super) struct Key {
    pub(super) account: AccountId,
    pub(super) space: SpaceId,
    pub(super) target: [u8; 32],
}

#[derive(Clone)]
pub(super) struct Asset {
    pub(super) kind: &'static str,
    pub(super) bytes: axum::body::Bytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Failure {
    Unavailable,
    Capacity,
}

pub(super) struct Entry {
    started: Instant,
    ready: OnceLock<(Instant, Result<Asset, Failure>)>,
    changed: Notify,
}

impl Entry {
    fn expired(&self, now: Instant) -> bool {
        let (at, ttl) = match self.ready.get() {
            Some((at, Ok(_))) => (*at, READY_TTL),
            Some((at, Err(_))) => (*at, FAILED_TTL),
            None => (self.started, PENDING_TTL),
        };
        now.saturating_duration_since(at) >= ttl
    }

    fn bytes(&self) -> usize {
        self.ready
            .get()
            .and_then(|(_, asset)| asset.as_ref().ok())
            .map_or(0, |asset| asset.bytes.len())
    }

    pub(super) async fn read(&self) -> Result<Asset, Failure> {
        let changed = self.changed.notified();
        if self.ready.get().is_none() {
            tokio::time::timeout_at((self.started + PENDING_TTL).into(), changed)
                .await
                .map_err(|_| Failure::Capacity)?;
        }
        self.ready.get().ok_or(Failure::Unavailable)?.1.clone()
    }
}

/// Cache locks are account-sharded and never cover sockets or disk I/O.
#[derive(Default)]
pub(super) struct Cache([Mutex<VecDeque<(Key, Arc<Entry>)>>; SHARDS]);

impl Cache {
    fn bucket(&self, key: Key) -> std::sync::MutexGuard<'_, VecDeque<(Key, Arc<Entry>)>> {
        self.0[usize::from(key.account.as_uuid().as_bytes()[15]) % SHARDS]
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    pub(super) fn get(&self, key: Key) -> Option<Arc<Entry>> {
        let mut entries = self.bucket(key);
        entries.retain(|(_, entry)| !entry.expired(Instant::now()));
        let index = entries.iter().position(|(current, _)| *current == key)?;
        let entry = entries.remove(index)?;
        let hit = entry.1.clone();
        entries.push_back(entry);
        Some(hit)
    }

    pub(super) fn start(&self, key: Key) -> Option<Arc<Entry>> {
        let mut entries = self.bucket(key);
        let now = Instant::now();
        entries.retain(|(_, entry)| !entry.expired(now));
        if entries.iter().any(|(existing, _)| *existing == key) {
            return None;
        }
        if entries.len() == SHARD_ENTRIES {
            let index = entries
                .iter()
                .position(|(_, entry)| entry.ready.get().is_some())?;
            entries.remove(index);
        }
        let entry = Arc::new(Entry {
            started: now,
            ready: OnceLock::new(),
            changed: Notify::new(),
        });
        entries.push_back((key, entry.clone()));
        Some(entry)
    }

    pub(super) fn finish(&self, key: Key, entry: &Arc<Entry>, mut asset: Result<Asset, Failure>) {
        let mut entries = self.bucket(key);
        if !entries.iter().any(|(_, live)| Arc::ptr_eq(live, entry)) {
            asset = Err(Failure::Unavailable);
        }
        let _ = entry.ready.set((Instant::now(), asset));
        let mut bytes: usize = entries.iter().map(|(_, entry)| entry.bytes()).sum();
        while bytes > SHARD_BYTES {
            let Some(index) = entries.iter().position(|(_, entry)| entry.bytes() != 0) else {
                break;
            };
            if let Some((_, removed)) = entries.remove(index) {
                bytes -= removed.bytes();
            }
        }
        drop(entries);
        entry.changed.notify_waiters();
    }

    pub(super) fn erase(&self, owner: Owner) {
        for shard in &self.0 {
            shard
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .retain(|(key, _)| match owner {
                    Owner::Account(account) => key.account != account,
                    Owner::Space(space) => key.space != space,
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn rejected_admission_remains_visible_to_the_reader() {
        let cache = Cache::default();
        let key = Key { account: AccountId::new(), space: SpaceId::new(), target: [0; 32] };
        let entry = cache.start(key).unwrap();
        cache.finish(key, &entry, Err(Failure::Capacity));
        assert!(matches!(cache.get(key).unwrap().read().await, Err(Failure::Capacity)));
    }
}
