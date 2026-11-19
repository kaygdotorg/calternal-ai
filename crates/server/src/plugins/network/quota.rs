//! Admission counters shared by the bounded Class B queue and checked IP dials.

use calternal_types::SpaceId;
use std::{
    collections::{HashMap, VecDeque},
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const SHARDS: usize = 16;
const TRACKED_PER_SHARD: usize = 32;
const QUEUED: usize = 128;
const QUEUED_PER_PLUGIN: usize = 16;
const QUEUED_PER_SPACE_PLUGIN: usize = 8;
const FETCHES_PER_MINUTE: usize = 60;
const PER_IP: usize = 2;
const MINUTE: Duration = Duration::from_secs(60);

#[derive(Default)]
struct SpaceUsage {
    starts: VecDeque<Instant>,
    queued: usize,
}

#[derive(Default)]
struct PluginUsage {
    spaces: HashMap<SpaceId, SpaceUsage>,
    queued: usize,
}

pub(super) struct Quota {
    queue: Arc<Semaphore>,
    plugins: [Mutex<HashMap<String, PluginUsage>>; SHARDS],
    addresses: Mutex<HashMap<IpAddr, usize>>,
}

impl Default for Quota {
    fn default() -> Self {
        Self {
            queue: Arc::new(Semaphore::new(QUEUED)),
            plugins: Default::default(),
            addresses: Default::default(),
        }
    }
}

impl Quota {
    fn shard(&self, id: &str) -> std::sync::MutexGuard<'_, HashMap<String, PluginUsage>> {
        let digest = ring::digest::digest(&ring::digest::SHA256, id.as_bytes());
        self.plugins[usize::from(digest.as_ref()[0]) % SHARDS]
            .lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Count source fetches before consuming a shared queue slot. Redirects
    /// hold this permit without charging a second source fetch.
    pub(super) fn enqueue(self: &Arc<Self>, plugin: &str, space: SpaceId) -> Option<Queued> {
        let mut plugins = self.shard(plugin);
        let now = Instant::now();
        plugins.retain(|_, usage| {
            usage.spaces.retain(|_, space| {
                while space.starts.front().is_some_and(|at| now.duration_since(*at) >= MINUTE) {
                    space.starts.pop_front();
                }
                space.queued > 0 || !space.starts.is_empty()
            });
            !usage.spaces.is_empty()
        });
        if plugins.len() >= TRACKED_PER_SHARD && !plugins.contains_key(plugin) {
            return None;
        }
        let usage = plugins.entry(plugin.into()).or_default();
        if usage.queued >= QUEUED_PER_PLUGIN
            || (usage.spaces.len() >= TRACKED_PER_SHARD && !usage.spaces.contains_key(&space))
        {
            return None;
        }
        let slot = usage.spaces.entry(space).or_default();
        if slot.queued >= QUEUED_PER_SPACE_PLUGIN || slot.starts.len() >= FETCHES_PER_MINUTE {
            return None;
        }
        let shared = self.queue.clone().try_acquire_owned().ok()?;
        usage.queued += 1;
        slot.queued += 1;
        slot.starts.push_back(now);
        Some(Queued { quota: self.clone(), plugin: plugin.into(), space, _shared: shared })
    }

    /// Limit simultaneous dials by the exact checked address, independent of hostname.
    pub(super) fn dial(&self, ip: IpAddr) -> Option<Dial<'_>> {
        let mut addresses = self.addresses.lock().unwrap_or_else(|error| error.into_inner());
        let active = addresses.entry(ip).or_default();
        if *active >= PER_IP { return None; }
        *active += 1;
        Some(Dial { quota: self, ip })
    }
}

pub(super) struct Queued {
    quota: Arc<Quota>,
    plugin: String,
    space: SpaceId,
    _shared: OwnedSemaphorePermit,
}

impl Drop for Queued {
    fn drop(&mut self) {
        let mut plugins = self.quota.shard(&self.plugin);
        if let Some(usage) = plugins.get_mut(&self.plugin) {
            usage.queued -= 1;
            if let Some(space) = usage.spaces.get_mut(&self.space) {
                space.queued -= 1;
            }
        }
    }
}

pub(super) struct Dial<'a> {
    quota: &'a Quota,
    ip: IpAddr,
}

impl Drop for Dial<'_> {
    fn drop(&mut self) {
        let mut addresses = self.quota.addresses.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(active) = addresses.get_mut(&self.ip) {
            *active -= 1;
            if *active == 0 { addresses.remove(&self.ip); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_rate_is_isolated_and_large_plugins_leave_queue_capacity() {
        let quota = Arc::new(Quota::default());
        let a = SpaceId::new();
        let b = SpaceId::new();
        for _ in 0..FETCHES_PER_MINUTE {
            assert!(quota.enqueue("same-package", a).is_some());
        }
        assert!(quota.enqueue("same-package", a).is_none());
        assert!(quota.enqueue("same-package", b).is_some());
        let jobs: Vec<_> = (0..QUEUED_PER_PLUGIN)
            .map(|_| quota.enqueue("flood", SpaceId::new()).unwrap()).collect();
        assert!(quota.enqueue("flood", SpaceId::new()).is_none());
        assert!(quota.enqueue("other", a).is_some());
        drop(jobs);
        assert!(quota.enqueue("flood", b).is_some());
    }

    #[test]
    fn aliases_share_the_checked_address_limit() {
        let quota = Quota::default();
        let ip = "93.184.215.18".parse().unwrap();
        let a = quota.dial(ip).unwrap();
        let _b = quota.dial(ip).unwrap();
        assert!(quota.dial(ip).is_none());
        assert!(quota.dial("93.184.215.19".parse().unwrap()).is_some());
        drop(a);
        assert!(quota.dial(ip).is_some());
    }
}
