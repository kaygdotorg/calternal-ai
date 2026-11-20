//! Updater fence for the authenticated local operator socket
//! (`execution_recovery.update_fence`). An automatic update may restart the
//! server only after acquiring this fence at idle; it lasts at most 30 s.
//! While the fence is held, or while startup recovery runs, valid Sends are
//! committed to the durable held queue instead of being admitted, and are
//! released once, in order, on recovery or expiry. A forced update skips the
//! fence and relies on exact reattachment.
//!
//! The gate is in memory: its truth is the per-space `content.db`, reported
//! after every execution commit and rebuilt by the startup scan. The one
//! lock guards a few integers and is never held across storage work.
use super::*;
use calternal_content::execution;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// Longest fence the operator can hold (`update_fence.max_seconds`).
pub const MAX_FENCE_MS: i64 = 30_000;

/// Updater-visible state (`ExecutionUpdateFenceResult`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FenceState {
    /// No admitted, running, reconnecting, stopping or unsettled work.
    Idle,
    /// Execution work is in progress; do not update.
    Busy,
    /// Startup is reconstructing admission and live execution state.
    Recovering,
    /// This request's fence was already released; acquire again with a new ID.
    Released,
    /// The fence is held until this instant (Unix milliseconds).
    Held {
        /// Absolute expiry of the original grant.
        expires_at_ms: i64,
    },
}

impl FenceState {
    /// Render the wire result. One small allocation, no I/O.
    pub fn to_json(&self) -> Value {
        match self {
            Self::Idle => json!({"state": "idle"}),
            Self::Busy => json!({"state": "busy"}),
            Self::Recovering => json!({"state": "recovering"}),
            Self::Released => json!({"state": "released"}),
            Self::Held { expires_at_ms } => {
                let instant = time::OffsetDateTime::from_unix_timestamp_nanos(
                    i128::from(*expires_at_ms) * 1_000_000,
                )
                .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
                json!({"state": "held", "expires_at": calternal_content::format_rfc3339(instant)})
            }
        }
    }
}

enum Phase {
    Recovering,
    Open,
    Held { request: String, expires_at_ms: i64 },
    Releasing,
}

struct Inner {
    phase: Phase,
    unsettled: HashMap<SpaceId, usize>,
    held: HashSet<SpaceId>,
    permits: usize,
    released: Option<String>,
}

/// Installation-wide admission gate shared by every execution entry point.
pub struct UpdateGate {
    inner: Mutex<Inner>,
}

/// Whether a new execution may be admitted now.
pub enum Admission<'a> {
    /// Admission is open. Keep the permit until the commit has been observed.
    Open(Permit<'a>),
    /// Fence held or recovery in progress: hold the Send durably instead.
    Closed,
}

/// Lightweight admission permit. While any permit is outstanding the fence
/// cannot be granted, so a Send racing the grant is either admitted before
/// it or held after it. Dropping the permit releases it.
pub struct Permit<'a> {
    gate: &'a UpdateGate,
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.gate.inner.lock() {
            inner.permits -= 1;
        }
    }
}

impl UpdateGate {
    /// Start in `recovering`. `previous_grant` is the request ID of the fence
    /// held when the previous process stopped, if the operator socket
    /// persisted it; a retry of that request then reports `released`.
    /// No I/O.
    pub fn recovering(previous_grant: Option<String>) -> Self {
        Self {
            inner: Mutex::new(Inner {
                phase: Phase::Recovering,
                unsettled: HashMap::new(),
                held: HashSet::new(),
                permits: 0,
                released: previous_grant,
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Ask to admit one execution. Cost: one short lock.
    pub fn admission(&self) -> Admission<'_> {
        let mut inner = self.lock();
        if matches!(inner.phase, Phase::Open) {
            inner.permits += 1;
            Admission::Open(Permit { gate: self })
        } else {
            Admission::Closed
        }
    }

    /// Record a space's unsettled count after a commit (the `observe`
    /// callback of the execution writer). Cost: one short lock.
    pub fn observe(&self, space: SpaceId, unsettled: usize) {
        let mut inner = self.lock();
        if unsettled == 0 {
            inner.unsettled.remove(&space);
        } else {
            inner.unsettled.insert(space, unsettled);
        }
    }

    fn note_held(&self, space: SpaceId) {
        self.lock().held.insert(space);
    }

    /// Scan one space during startup recovery: report its unsettled count and
    /// remember whether it has held Sends to release. Call for every space
    /// before [`UpdateGate::finish_recovery`], from a blocking worker.
    /// Cost: one execution transaction with three bounded counts.
    pub fn recover_space(&self, content: &Content, space: SpaceId) -> Result<()> {
        let held = content.execution_transaction(space, &|n| self.observe(space, n), |tx| {
            execution::held_count(tx)
        })?;
        if held > 0 {
            self.note_held(space);
        }
        Ok(())
    }

    /// End the startup scan. Admission stays closed until the held Sends
    /// have been released; returns the spaces to pass to
    /// [`UpdateGate::release_space`] before [`UpdateGate::reopen`].
    /// Cost: one short lock.
    pub fn finish_recovery(&self) -> Vec<SpaceId> {
        let mut inner = self.lock();
        inner.phase = Phase::Releasing;
        inner.held.iter().copied().collect()
    }

    /// Release an expired fence exactly once. Returns the spaces whose held
    /// Sends must now be released, or `None` if nothing expired.
    /// Cost: one short lock.
    pub fn expire(&self, now_ms: i64) -> Option<Vec<SpaceId>> {
        let mut inner = self.lock();
        let Phase::Held {
            request,
            expires_at_ms,
        } = &inner.phase
        else {
            return None;
        };
        if now_ms < *expires_at_ms {
            return None;
        }
        inner.released = Some(request.clone());
        inner.phase = Phase::Releasing;
        Some(inner.held.iter().copied().collect())
    }

    /// Release one space's held Sends in original order under live checks.
    /// Each Send leaves the queue once whatever its outcome. Call from a
    /// blocking worker while the gate is releasing. Cost: one execution
    /// transaction bounded by the held rows.
    pub fn release_space(
        &self,
        content: &Content,
        space: SpaceId,
        live: impl FnMut(&execution::PinnedExecution) -> Result<bool>,
    ) -> Result<Vec<execution::Released>> {
        self.lock().held.remove(&space);
        let released = content.execution_transaction(space, &|n| self.observe(space, n), |tx| {
            execution::release_held(tx, super::super::jobs::MAX_ACTIVE_JOBS_PER_SPACE, live)
        });
        if released.is_err() {
            self.note_held(space);
        }
        released
    }

    /// Open admission after release. Returns spaces that received held Sends
    /// while releasing; release those and call again until it is empty.
    /// Cost: one short lock.
    pub fn reopen(&self) -> Vec<SpaceId> {
        let mut inner = self.lock();
        if inner.held.is_empty() {
            inner.phase = Phase::Open;
        }
        inner.held.iter().copied().collect()
    }

    /// Answer one operator request (`ExecutionUpdateFenceRequest`): `observe`
    /// is advisory; `acquire` atomically checks idle, with no admission
    /// permit outstanding, and closes admission for at most 30 s. A retry
    /// returns the same grant without extending it, and `released` after
    /// release. The operator socket must persist a granted request ID before
    /// replying, for [`UpdateGate::recovering`]. Cost: one short lock.
    pub fn request(&self, input: &Value, now_ms: i64) -> Result<FenceState> {
        let object = input.as_object().ok_or_else(invalid)?;
        let mode = object.get("mode").and_then(Value::as_str);
        let request = object.get("request_id");
        let request = match (mode, request) {
            (Some("observe"), None) if object.len() == 1 => None,
            (Some("acquire"), Some(Value::String(id)))
                if object.len() == 2 && !id.is_empty() && id.len() <= 128 =>
            {
                Some(id.as_str())
            }
            _ => return Err(invalid()),
        };
        let mut inner = self.lock();
        let busy = inner.permits > 0 || !inner.unsettled.is_empty() || !inner.held.is_empty();
        Ok(match (&inner.phase, request) {
            (Phase::Recovering, _) => FenceState::Recovering,
            (Phase::Held { expires_at_ms, .. }, _) => FenceState::Held {
                expires_at_ms: *expires_at_ms,
            },
            (_, Some(id)) if inner.released.as_deref() == Some(id) => FenceState::Released,
            (Phase::Releasing, _) => FenceState::Busy,
            (Phase::Open, _) if busy => FenceState::Busy,
            (Phase::Open, None) => FenceState::Idle,
            (Phase::Open, Some(id)) => {
                let expires_at_ms = now_ms + MAX_FENCE_MS;
                inner.phase = Phase::Held {
                    request: id.to_owned(),
                    expires_at_ms,
                };
                FenceState::Held { expires_at_ms }
            }
        })
    }

    /// Hold one Send durably while admission is closed. Cost: the receipt
    /// path and one insert; no capacity reservation and no launch.
    pub(super) fn hold(
        &self,
        content: &Content,
        space: SpaceId,
        request: Request<'_>,
        pin: &execution::PinnedExecution,
        live: impl FnOnce() -> Result<()>,
    ) -> Result<Value> {
        let result =
            content.execution_command(space, &|n| self.observe(space, n), request, live, |tx| {
                let held = execution::hold(tx, pin, super::super::jobs::MAX_ACTIVE_JOBS_PER_SPACE)?;
                Ok(json!({"job_id": held.job}))
            })?;
        self.note_held(space);
        Ok(result)
    }
}
