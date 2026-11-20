//! Per-carrier activity shared by the broker and the placement scheduler.

use calternal_plugin_host::space_channel::MAX_ACTIVE_TICKETS;
use calternal_types::Error;
use std::sync::{Arc, Mutex};
use tokio::{
    sync::{Notify, watch},
    time::{Duration, Instant},
};

/// Track last call completion and fence admission when an idle carrier is
/// selected for sleep. All updates are memory-only and local to the carrier.
pub(crate) struct Residency {
    idle: Duration,
    activity: Mutex<Activity>,
    changed: Notify,
    drained: watch::Sender<bool>,
}

#[derive(Default)]
struct Activity {
    last_used: Option<Instant>,
    calls: usize,
    sleeping: bool,
}

impl Residency {
    /// Set the idle window for the carrier. Time starts at readiness.
    pub(crate) fn new(idle: Duration) -> Self {
        Self {
            idle,
            activity: Mutex::default(),
            changed: Notify::new(),
            drained: watch::channel(false).0,
        }
    }

    /// Start idle tracking after the worker and its restore proof are ready.
    pub(crate) fn start(&self) {
        self.activity.lock().expect("space residency").last_used = Some(Instant::now());
    }

    /// Reserve one bounded call slot, or let the caller join an ongoing sleep.
    pub(crate) fn admit(self: &Arc<Self>) -> Result<Option<Invocation>, Error> {
        let mut state = self.activity.lock().expect("space residency");
        if state.sleeping {
            return Ok(None);
        }
        if state.calls == MAX_ACTIVE_TICKETS {
            return Err(Error::unavailable("space package is busy; retry shortly")
                .with_details(serde_json::json!({"reason":"space_ticket_capacity"})));
        }
        state.calls += 1;
        Ok(Some(Invocation {
            residency: self.clone(),
            used: false,
        }))
    }

    /// Rank only ready, idle carriers eligible to sleep immediately.
    pub(super) fn last_idle(&self) -> Option<Instant> {
        let state = self.activity.lock().expect("space residency");
        (state.calls == 0 && !state.sleeping)
            .then_some(state.last_used)
            .flatten()
    }

    /// Fence a selected idle carrier only if it has seen no intervening call.
    pub(super) fn sleep(&self, last: Instant) -> bool {
        let mut state = self.activity.lock().expect("space residency");
        if state.calls != 0 || state.sleeping || state.last_used != Some(last) {
            return false;
        }
        state.sleeping = true;
        self.changed.notify_one();
        true
    }

    /// Wait for idle expiry or a capacity eviction. Taking the same lock as
    /// admission prevents new calls crossing the drain boundary.
    pub(crate) async fn wait_for_sleep(&self) {
        loop {
            let (sleeping, last) = {
                let state = self.activity.lock().expect("space residency");
                (
                    state.sleeping,
                    (state.calls == 0).then_some(state.last_used).flatten(),
                )
            };
            if sleeping {
                return;
            }
            if let Some(last) = last {
                tokio::select! {
                    _ = self.changed.notified() => {}
                    _ = tokio::time::sleep_until(last + self.idle) => {
                        if self.sleep(last) {
                            return;
                        }
                    }
                }
            } else {
                self.changed.notified().await;
            }
        }
    }

    /// Subscribe only once new calls are fenced.
    pub(crate) fn drain_waiter(&self) -> Option<watch::Receiver<bool>> {
        self.activity
            .lock()
            .expect("space residency")
            .sleeping
            .then(|| self.drained.subscribe())
    }

    /// Release waiting calls once teardown has acknowledged the checkpoint.
    pub(crate) fn finish(&self) {
        self.drained.send_replace(true);
    }
}

/// Hold one call slot until its result, cancellation or deadline.
pub(crate) struct Invocation {
    residency: Arc<Residency>,
    used: bool,
}

impl Invocation {
    /// Count an authorized call toward the idle window when its work ends.
    pub(crate) fn mark_used(&mut self) {
        self.used = true;
    }
}

impl Drop for Invocation {
    fn drop(&mut self) {
        let mut state = self.residency.activity.lock().expect("space residency");
        state.calls -= 1;
        if self.used {
            state.last_used = Some(Instant::now());
        }
        if state.calls == 0 {
            self.residency.changed.notify_one();
        }
    }
}
