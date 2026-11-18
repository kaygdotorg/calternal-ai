//! Bounded, independent rate-limit buckets. Forwarded headers never identify clients.
use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Source-keyed per-minute admission/failure buckets, bounded to 10,000 keys.
/// Live least-recently-used entries may be evicted to admit new sources.
pub struct Limiter {
    per_minute: u32,
    state: Mutex<Buckets>,
    /// When expired buckets were last dropped. Sweeping every call would
    /// scan the whole map under the lock on the hottest path.
    swept: Mutex<Instant>,
}

#[derive(Default)]
struct Buckets {
    entries: HashMap<String, (Instant, u32, Instant)>,
    used: BTreeSet<(Instant, String)>,
}

impl Limiter {
    /// Creates independent buckets allowing `per_minute` attempts per 60 s window.
    pub fn new(per_minute: u32) -> Self {
        Self {
            per_minute,
            state: Mutex::new(Buckets::default()),
            swept: Mutex::new(Instant::now()),
        }
    }

    /// True at most once every few seconds.
    fn sweep_due(&self) -> bool {
        let mut at = self.swept.lock().unwrap_or_else(|p| p.into_inner());
        if at.elapsed() < Duration::from_secs(5) {
            return false;
        }
        *at = Instant::now();
        true
    }

    /// Whether `key` still has room, without recording anything. Lets a
    /// caller count only failures: check first, then `attempt` on a failure.
    pub fn has_room(&self, key: &str) -> bool {
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        match state.entries.get(key) {
            Some((at, n, _)) if at.elapsed() < Duration::from_secs(60) => *n < self.per_minute,
            _ => true,
        }
    }

    /// Records an attempt atomically. Evict the least recently used bucket
    /// at capacity so new sources can still sign in.
    pub fn attempt(&self, key: &str) -> bool {
        self.record(key).0
    }

    /// Atomically records an attempt and reports whether it was the first
    /// in this window. Concurrent retries must not charge a source twice.
    pub fn first_attempt(&self, key: &str) -> bool {
        self.record(key).1
    }

    fn record(&self, key: &str) -> (bool, bool) {
        let sweep = self.sweep_due();
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if sweep {
            state
                .entries
                .retain(|_, (at, _, _)| at.elapsed() < Duration::from_secs(60));
            let Buckets { entries, used } = &mut *state;
            used.retain(|(_, key)| entries.contains_key(key));
        }
        if !state.entries.contains_key(key)
            && state.entries.len() >= 10_000
            && let Some((_, oldest)) = state.used.pop_first()
        {
            state.entries.remove(&oldest);
        }
        if let Some((_, _, used)) = state.entries.get(key) {
            let old = (*used, key.to_owned());
            state.used.remove(&old);
        }
        let now = Instant::now();
        state.used.insert((now, key.to_owned()));
        let entry =
            state
                .entries
                .entry(key.to_owned())
                .or_insert((Instant::now(), 0, Instant::now()));
        if entry.0.elapsed() >= Duration::from_secs(60) {
            // Expired but not swept yet: a new minute starts.
            *entry = (Instant::now(), 0, Instant::now());
        }
        entry.2 = now;
        let first = entry.1 == 0;
        if entry.1 >= self.per_minute {
            return (false, false);
        }
        entry.1 += 1;
        (true, first)
    }
}

/// Verified setup and recovery tokens keep their budget for the full window.
/// At capacity, only the least recently used expired entry may be evicted.
pub struct TargetLimiter {
    per_minute: u32,
    state: Mutex<HashMap<String, (Instant, u32, Instant)>>,
}

impl TargetLimiter {
    const CAPACITY: usize = 256;

    /// Creates independent buckets allowing `per_minute` attempts per 60 s window.
    pub fn new(per_minute: u32) -> Self {
        Self {
            per_minute,
            state: Mutex::new(HashMap::new()),
        }
    }

    /// Records an attempt; refuses new targets at capacity unless an expired
    /// entry can be evicted. A live verified target keeps its full-window budget.
    pub fn attempt(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if !state.contains_key(key) && state.len() >= Self::CAPACITY {
            let expired = state
                .iter()
                .filter(|(_, (at, _, _))| now.duration_since(*at) >= Duration::from_secs(60))
                .min_by_key(|(_, (_, _, used))| *used)
                .map(|(key, _)| key.clone());
            let Some(expired) = expired else {
                return false;
            };
            state.remove(&expired);
        }
        let entry = state.entry(key.to_owned()).or_insert((now, 0, now));
        if now.duration_since(entry.0) >= Duration::from_secs(60) {
            *entry = (now, 0, now);
        }
        entry.2 = now;
        if entry.1 >= self.per_minute {
            return false;
        }
        entry.1 += 1;
        true
    }
}

/// Account failures share a short back-off across all network sources.
/// Refused attempts do not extend the deadline; a successful sign-in clears it.
#[derive(Default)]
pub struct Failures {
    state: Mutex<HashMap<uuid::Uuid, Failure>>,
}
struct Failure {
    count: u32,
    next: Instant,
    used: Instant,
}
impl Failures {
    /// Whether the account may try again without extending its back-off.
    pub fn ready(&self, user: uuid::Uuid) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&user)
            .is_none_or(|f| Instant::now() >= f.next)
    }

    /// Records a failure and returns whether back-off is active. The third
    /// failure starts at one second, doubles thereafter, and caps at 60 seconds.
    /// Refusals leave the deadline unchanged; entries expire after 600 s idle.
    pub fn failed(&self, user: uuid::Uuid) -> bool {
        let now = Instant::now();
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.retain(|_, f| now.duration_since(f.used) < Duration::from_secs(600));
        if !state.contains_key(&user) && state.len() >= 10_000 {
            let oldest = state.iter().min_by_key(|(_, f)| f.used).map(|(u, _)| *u);
            if let Some(oldest) = oldest {
                state.remove(&oldest);
            }
        }
        let f = state.entry(user).or_insert(Failure {
            count: 0,
            next: now,
            used: now,
        });
        f.used = now;
        if now < f.next {
            return true;
        }
        f.count = f.count.saturating_add(1);
        if f.count < 3 {
            return false;
        }
        f.next = now + Duration::from_secs((1_u64 << (f.count - 3).min(6)).min(60));
        true
    }
    /// Clears an account's failures after a successful sign-in.
    pub fn clear(&self, user: uuid::Uuid) {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&user);
    }
}

#[cfg(test)]
mod reliability_tests {
    use super::*;
    #[test]
    fn concurrent_retries_have_only_one_first_attempt() {
        let limiter = Limiter::new(20);
        std::thread::scope(|scope| {
            let tasks: Vec<_> = (0..32)
                .map(|_| {
                    let limiter = &limiter;
                    scope.spawn(move || limiter.first_attempt("same malformed password"))
                })
                .collect();
            let first = tasks
                .into_iter()
                .map(|task| usize::from(task.join().unwrap()))
                .sum::<usize>();
            assert_eq!(first, 1);
        });
    }

    #[test]
    fn buckets_are_independent_and_bounded() {
        let l = Limiter::new(2);
        assert!(l.attempt("a"));
        assert!(l.attempt("a"));
        assert!(!l.attempt("a"));
        assert!(l.attempt("b"));
    }

    #[test]
    fn has_room_does_not_spend_the_budget() {
        let l = Limiter::new(1);
        for _ in 0..5 {
            assert!(l.has_room("a"));
        }
        assert!(l.attempt("a"));
        assert!(!l.has_room("a"));
        assert!(l.has_room("b"));
    }

    #[test]
    fn full_map_evicts_least_recent_without_refusing_new_sources() {
        let limiter = Limiter::new(2);
        let now = Instant::now();
        {
            let mut state = limiter.state.lock().unwrap();
            for n in 0..10_000 {
                let used = if n == 0 {
                    now - Duration::from_secs(1)
                } else {
                    now
                };
                state.entries.insert(n.to_string(), (now, 1, used));
                state.used.insert((used, n.to_string()));
            }
        }
        assert!(limiter.attempt("new"));
        let state = limiter.state.lock().unwrap();
        assert_eq!(state.entries.len(), 10_000);
        assert!(!state.entries.contains_key("0"));
        assert!(state.entries.contains_key("1"));
    }
    #[test]
    fn target_capacity_preserves_live_budgets_and_evicts_only_oldest_expired() {
        let limiter = TargetLimiter::new(1);
        for n in 0..TargetLimiter::CAPACITY {
            assert!(limiter.attempt(&n.to_string()));
        }
        assert!(!limiter.attempt("new"));
        assert!(!limiter.attempt("0"));
        let now = Instant::now();
        {
            let mut state = limiter.state.lock().unwrap();
            state.get_mut("1").unwrap().0 = now - Duration::from_secs(61);
            let oldest = state.get_mut("2").unwrap();
            oldest.0 = now - Duration::from_secs(61);
            oldest.2 = now - Duration::from_secs(62);
        }
        assert!(limiter.attempt("new"));
        assert!(!limiter.attempt("0"));
        {
            let state = limiter.state.lock().unwrap();
            assert_eq!(state.len(), TargetLimiter::CAPACITY);
            assert!(state.contains_key("1"));
            assert!(!state.contains_key("2"));
        }
        assert!(limiter.attempt("1"));
        assert!(!limiter.attempt("1"));
    }

    #[test]
    fn source_churn_cannot_reset_target_budget() {
        let source = Limiter::new(2);
        let target = TargetLimiter::new(2);
        assert!(target.attempt("verified-canonical-token"));
        for n in 0..10_001 {
            assert!(source.attempt(&n.to_string()));
        }
        assert!(target.attempt("verified-canonical-token"));
        assert!(!target.attempt("verified-canonical-token"));
    }

    #[test]
    fn backoff_expires_is_capped_and_refusals_do_not_extend_it() {
        let failures = Failures::default();
        let user = uuid::Uuid::new_v4();
        assert!(!failures.failed(user));
        assert!(!failures.failed(user));
        assert!(failures.failed(user));
        assert!(!failures.ready(user));
        let deadline = failures.state.lock().unwrap().get(&user).unwrap().next;
        assert!(failures.failed(user));
        assert_eq!(
            failures.state.lock().unwrap().get(&user).unwrap().next,
            deadline
        );
        for _ in 0..10 {
            failures.state.lock().unwrap().get_mut(&user).unwrap().next = Instant::now();
            let before = Instant::now();
            assert!(failures.ready(user));
            assert!(failures.failed(user));
            assert!(
                failures.state.lock().unwrap().get(&user).unwrap().next
                    <= Instant::now() + Duration::from_secs(60)
            );
            assert!(failures.state.lock().unwrap().get(&user).unwrap().next >= before);
        }
        failures.clear(user);
        assert!(failures.ready(user));
    }
}
