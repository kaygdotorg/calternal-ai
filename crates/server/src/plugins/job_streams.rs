//! Bounded stream ownership and subscriber admission, separate from durable jobs.
use super::jobs::{Jobs, MAX_SUBSCRIBERS, MAX_SUBSCRIBERS_PER_JOB};
use super::live::{Live, Subscription};
use calternal_types::Error;
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
impl Jobs {
    /// The live stream of a job, created on demand. Cost: one map lock.
    pub fn live(&self, id: &str) -> Arc<Live> {
        let mut map = self.live.lock().unwrap_or_else(|p| p.into_inner());
        let now = std::time::Instant::now();
        map.retain(|_, l| {
            let b = l.lock();
            b.running || b.subscribers > 0 || b.retain_until.is_none_or(|t| t > now)
        });
        let mut stopped: Vec<_> = map
            .iter()
            .filter_map(|(id, l)| {
                let b = l.lock();
                (!b.running && b.subscribers == 0).then_some((id.clone(), b.retain_until))
            })
            .collect();
        stopped.sort_by_key(|(_, t)| *t);
        let excess = stopped.len().saturating_sub(63);
        for (old, _) in stopped.into_iter().take(excess) {
            map.remove(&old);
        }
        map.entry(id.to_owned())
            .or_insert_with(|| Arc::new(Live::new()))
            .clone()
    }

    /// Stop a job's stream after its run ended: wake subscribers and drop the buffer
    /// once nobody is subscribed. Cost: two map/stream locks.
    pub(super) fn settle(&self, id: &str) {
        self.settle_inner(id, None)
    }

    /// Settle only a matching live run; late completion cannot stop its successor.
    pub(super) fn settle_run(&self, id: &str, run: u32) {
        self.settle_inner(id, Some(run))
    }

    fn settle_inner(&self, id: &str, expected: Option<u32>) {
        let mut live = self.live.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(l) = live.get(id).cloned() {
            let idle = {
                let mut b = l.lock();
                if expected.is_some_and(|run| b.run != run) {
                    return;
                }
                b.running = false;
                b.cancel = None;
                if b.retain_until.is_some() {
                    b.retain_until = Some(std::time::Instant::now() + Duration::from_secs(30));
                }
                b.subscribers == 0 && b.retain_until.is_none()
            };
            l.bump();
            if idle {
                live.remove(id);
            }
        }
    }

    /// Take a subscriber slot on a job's stream; refused beyond the per-job and
    /// server-wide bounds. Cost: one map lock and one stream lock.
    pub fn subscribe(&self, id: &str) -> Result<Subscription, Error> {
        let busy = || {
            Error::unavailable("too many job subscribers; retry shortly")
                .with_details(serde_json::json!({"reason": "subscriber_capacity"}))
        };
        if self.subscribers.fetch_add(1, Ordering::AcqRel) >= MAX_SUBSCRIBERS {
            self.subscribers.fetch_sub(1, Ordering::AcqRel);
            return Err(busy());
        }
        let live = self.live(id);
        let mut b = live.lock();
        if b.subscribers >= MAX_SUBSCRIBERS_PER_JOB {
            drop(b);
            self.subscribers.fetch_sub(1, Ordering::AcqRel);
            return Err(busy());
        }
        b.subscribers += 1;
        drop(b);
        Ok(Subscription {
            live,
            id: id.to_owned(),
            map: self.live.clone(),
            total: self.subscribers.clone(),
        })
    }
}
