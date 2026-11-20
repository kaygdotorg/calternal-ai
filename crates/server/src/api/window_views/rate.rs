//! Admission counters retained by the actor book and each space’s live leases.
use super::*;

#[derive(Default)]
pub(super) struct Rate(Mutex<VecDeque<Instant>>);

impl Rate {
    pub(super) fn admit(&self, limit: usize) -> bool {
        let now = Instant::now();
        let mut starts = self.0.lock().unwrap_or_else(|e| e.into_inner());
        while starts
            .front()
            .is_some_and(|at| now.saturating_duration_since(*at) >= Duration::from_secs(1))
        {
            starts.pop_front();
        }
        if starts.len() >= limit {
            return false;
        }
        starts.push_back(now);
        true
    }
}

pub(super) struct ActorShard {
    pub(super) leases: Mutex<HashMap<(AccountId, SpaceId, Uuid), Arc<Lease>>>,
    pub(super) rate: Rate,
    pub(super) work: Arc<Semaphore>,
}

pub(super) struct SpaceShard {
    pub(super) subscriptions: Arc<Semaphore>,
    pub(super) work: Arc<Semaphore>,
    pub(super) rate: Rate,
    pub(super) count_rate: Rate,
}
