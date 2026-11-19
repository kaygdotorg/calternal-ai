//! Delivery revisions for hosted space assignments, published by the authority worker.
use crate::{Error, Result, SpaceId};
use rusqlite::{Connection, functions::FunctionFlags};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

const MAX_ASSIGNMENTS: usize = 1024;
const CLOSED: u64 = u64::MAX;

/// Process-local revision of an enrolled assignment's space authority.
/// Keep this with the assignment. A clean revision is meaningful only for this
/// handle and runtime epoch.
#[derive(Clone)]
pub struct GrantRevision {
    space: SpaceId,
    revision: Arc<AtomicU64>,
    tracker: Weak<Mutex<State>>,
}

impl GrantRevision {
    /// Space whose authority may be admitted against this handle.
    /// Cost: one UUID copy.
    pub fn space(&self) -> SpaceId {
        self.space
    }

    /// Return the current clean revision or refuse delivery while a mutation
    /// is open, on assignment replacement or on worker shutdown.
    /// Cost: one atomic load and no locks, storage reads or flushes.
    pub fn current(&self) -> Option<u64> {
        let revision = self.revision.load(Ordering::Acquire);
        (revision & 1 == 0).then_some(revision)
    }
}

struct Assignment {
    machine: String,
    revision: Weak<AtomicU64>,
}

#[derive(Default)]
struct State {
    assignments: HashMap<String, Assignment>,
    pending: Vec<Arc<AtomicU64>>,
}

// The registry is used in control-plane jobs and SQLite trigger callbacks only.
// This mutex is never held across a storage call or worker round trip.
#[derive(Clone, Default)]
pub(super) struct Tracker(Arc<Mutex<State>>);

impl Tracker {
    pub(super) fn owns(&self, grants: &GrantRevision) -> bool {
        Weak::as_ptr(&grants.tracker) == Arc::as_ptr(&self.0)
    }

    pub(super) fn subscribe(&self, space: SpaceId, machine: String) -> Result<GrantRevision> {
        let key = space.to_string();
        let handle = |revision| GrantRevision {
            space,
            revision,
            tracker: Arc::downgrade(&self.0),
        };
        let mut state = self.0.lock().map_err(Error::internal)?;
        if let Some(existing) = state.assignments.get(&key)
            && let Some(revision) = existing.revision.upgrade()
        {
            if existing.machine == machine && revision.load(Ordering::Relaxed) != CLOSED {
                return Ok(handle(revision));
            }
            revision.store(CLOSED, Ordering::Release);
            state.assignments.remove(&key);
        }
        if state.assignments.len() == MAX_ASSIGNMENTS {
            state
                .assignments
                .retain(|_, entry| entry.revision.strong_count() != 0);
            if state.assignments.len() == MAX_ASSIGNMENTS {
                return Err(Error::QueueFull);
            }
        }
        let revision = Arc::new(AtomicU64::new(0));
        state.assignments.insert(
            key,
            Assignment {
                machine,
                revision: Arc::downgrade(&revision),
            },
        );
        Ok(handle(revision))
    }

    fn mark(&self, scope: &str, id: &str) -> rusqlite::Result<()> {
        let mut state = self.0.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        if scope == "machine" {
            for entry in state
                .assignments
                .values()
                .filter(|entry| entry.machine == id)
            {
                if let Some(revision) = entry.revision.upgrade() {
                    revision.store(CLOSED, Ordering::Release);
                }
            }
        } else if let Some(entry) = state.assignments.get(id)
            && let Some(revision) = entry.revision.upgrade()
        {
            let old = revision.load(Ordering::Relaxed);
            if old & 1 == 0 {
                revision.store(old + 1, Ordering::Release);
                state.pending.push(revision);
            }
        }
        Ok(())
    }

    pub(super) fn finish(&self) {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        for revision in state.pending.drain(..) {
            let dirty = revision.load(Ordering::Relaxed);
            if let Some(clean) = dirty.checked_add(1) {
                revision.store(clean, Ordering::Release);
            }
        }
    }

    pub(super) fn install(&self, connection: &Connection) -> rusqlite::Result<()> {
        let grants = self.clone();
        connection.create_scalar_function(
            "fence_space_grant",
            2,
            FunctionFlags::SQLITE_UTF8,
            move |context| {
                let scope = context.get::<String>(0)?;
                let id = context.get::<String>(1)?;
                grants.mark(&scope, &id)?;
                Ok(0_i64)
            },
        )?;
        super::grant_triggers::install(connection)
    }
}

// The SQLite worker owns this guard. Every retained handle fails closed if
// that worker exits without finishing a queued mutation.
pub(super) struct Worker(pub(super) Tracker);

impl Drop for Worker {
    fn drop(&mut self) {
        let state = self.0.0.lock().unwrap_or_else(|error| error.into_inner());
        for entry in state.assignments.values() {
            if let Some(revision) = entry.revision.upgrade() {
                revision.store(CLOSED, Ordering::Release);
            }
        }
    }
}
