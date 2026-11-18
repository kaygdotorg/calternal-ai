//! Shared per-space ceilings for brokered outbound activity.

use calternal_types::{Error, SpaceId};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const SHARDS: usize = 32;
const SPACES_PER_SHARD: usize = 128;
const CONCURRENT: usize = 4;
const STARTS: usize = 120;
const TRAFFIC: u64 = 64 * 1024 * 1024;
const MINUTE: Duration = Duration::from_secs(60);
const HOUR: Duration = Duration::from_secs(60 * 60);

#[derive(Default)]
struct Usage {
    starts: VecDeque<Instant>,
    traffic: VecDeque<(Instant, u64)>,
    actual: u64,
    in_flight: usize,
    reserved: u64,
}

impl Usage {
    fn trim_starts(&mut self, now: Instant) {
        while self
            .starts
            .front()
            .is_some_and(|at| now.duration_since(*at) >= MINUTE)
        {
            self.starts.pop_front();
        }
    }

    fn trim_bytes(&mut self, now: Instant) {
        while self
            .traffic
            .front()
            .is_some_and(|(at, _)| now.duration_since(*at) >= HOUR + MINUTE)
        {
            self.actual -= self.traffic.pop_front().expect("checked byte bucket").1;
        }
    }

    fn idle(&self, now: Instant) -> bool {
        self.in_flight == 0
            && self
                .starts
                .back()
                .is_none_or(|at| now.duration_since(*at) >= MINUTE)
            && self
                .traffic
                .back()
                .is_none_or(|(at, _)| now.duration_since(*at) >= HOUR + MINUTE)
    }

    fn record(&mut self, now: Instant, bytes: u64) {
        if bytes == 0 {
            return;
        }
        self.actual += bytes;
        if let Some((at, count)) = self.traffic.back_mut()
            && now.duration_since(*at) < MINUTE
        {
            *count += bytes;
        } else {
            self.traffic.push_back((now, bytes));
        }
    }
}

/// Short sharded accounting locks with no I/O. Bounded by active or recently
/// charged spaces. Byte windows use one-minute buckets and retain the oldest
/// overlapping bucket, so the rolling-hour cap is conservative.
#[derive(Default)]
pub(super) struct Admission([Mutex<HashMap<SpaceId, Usage>>; SHARDS]);

impl Admission {
    fn shard(&self, space: SpaceId) -> &Mutex<HashMap<SpaceId, Usage>> {
        &self.0[usize::from(space.as_uuid().as_bytes()[15]) % SHARDS]
    }

    pub(super) fn try_start(
        self: &Arc<Self>,
        space: SpaceId,
        reserved: u64,
    ) -> Result<Reservation, Error> {
        self.start(space, reserved, Instant::now())
    }

    fn start(
        self: &Arc<Self>,
        space: SpaceId,
        reserved: u64,
        now: Instant,
    ) -> Result<Reservation, Error> {
        let busy = || Error::unavailable("Space network budget is exhausted.");
        let mut shard = self
            .shard(space)
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if shard.len() == SPACES_PER_SHARD && !shard.contains_key(&space) {
            shard.retain(|_, usage| !usage.idle(now));
            if shard.len() == SPACES_PER_SHARD {
                return Err(busy());
            }
        }
        let usage = shard.entry(space).or_default();
        usage.trim_starts(now);
        usage.trim_bytes(now);
        if usage.starts.len() == STARTS {
            return Err(busy());
        }
        usage.starts.push_back(now);
        if usage.in_flight == CONCURRENT || reserved > TRAFFIC - usage.actual - usage.reserved {
            return Err(busy());
        }
        usage.in_flight += 1;
        usage.reserved += reserved;
        Ok(Reservation {
            admission: self.clone(),
            space,
            reserved,
            reported: None,
        })
    }
}

/// Hold one concurrency slot and reserve bytes before a request or stream.
/// Cancelled work with an unknown byte count is conservatively charged in full.
pub(super) struct Reservation {
    admission: Arc<Admission>,
    space: SpaceId,
    reserved: u64,
    reported: Option<u64>,
}

impl Reservation {
    pub(super) fn finish(mut self, transferred: u64) {
        self.reported = Some(transferred.min(self.reserved));
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        let mut shard = self
            .admission
            .shard(self.space)
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let usage = shard
            .get_mut(&self.space)
            .expect("reservations hold their space");
        usage.in_flight -= 1;
        usage.reserved -= self.reserved;
        usage.record(Instant::now(), self.reported.unwrap_or(self.reserved));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space() -> SpaceId {
        SpaceId::parse("01932910-0000-7000-8000-000000000002").unwrap()
    }

    #[test]
    fn concurrent_and_reserved_byte_limits_are_charged_before_network() {
        let admission = Arc::new(Admission::default());
        let space = space();
        let pending: Vec<_> = (0..CONCURRENT)
            .map(|_| {
                admission
                    .try_start(space, TRAFFIC / CONCURRENT as u64)
                    .unwrap()
            })
            .collect();
        assert!(admission.try_start(space, 1).is_err());
        for reservation in pending {
            reservation.finish(1);
        }
        assert!(admission.try_start(space, TRAFFIC).is_err());
        assert!(
            admission
                .try_start(space, TRAFFIC - CONCURRENT as u64)
                .is_ok()
        );
    }

    #[test]
    fn refused_admissions_count_against_the_rolling_minute() {
        let admission = Arc::new(Admission::default());
        let space = space();
        let now = Instant::now();
        for _ in 0..STARTS {
            assert!(admission.start(space, TRAFFIC + 1, now).is_err());
        }
        assert!(admission.start(space, 0, now).is_err());
        assert!(admission.start(space, 0, now + MINUTE).is_ok());
    }
}
