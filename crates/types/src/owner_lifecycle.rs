//! Admission and drain for the durable steps of owner-local stores.
use crate::{Error, Result};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::{Arc, Condvar, Mutex, MutexGuard},
};
use uuid::Uuid;

const SHARDS: usize = 64;

#[derive(Default)]
struct State {
    running: usize,
    erasing: bool,
}

#[derive(Default)]
struct Bucket {
    owners: Mutex<HashMap<Uuid, State>>,
    drained: Condvar,
}
impl Bucket {
    fn lock(&self) -> MutexGuard<'_, HashMap<Uuid, State>> {
        self.owners
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}

/// The owner-local lifecycle shared by stores in one locked installation.
/// Account and space UUIDs are disjoint. Admissions take a short sharded
/// registry lock, with no SQLite or file I/O. Idle live entries are released.
/// Closed entries stay until process exit and are restored from protected
/// erasure records before serving again.
#[derive(Clone)]
pub struct OwnerLifecycle(Arc<[Bucket; SHARDS]>);

impl Default for OwnerLifecycle {
    fn default() -> Self {
        Self(Arc::new(std::array::from_fn(|_| Bucket::default())))
    }
}

impl OwnerLifecycle {
    fn bucket(&self, id: Uuid) -> &Bucket {
        &self.0[id.as_bytes()[15] as usize % SHARDS]
    }

    /// Refuse a closed owner before waiting to schedule I/O. This is an
    /// in-memory preflight. The actual I/O must still acquire a step.
    pub fn check(&self, id: Uuid) -> Result<()> {
        if self.is_closed(id) {
            return Err(closed_owner());
        }
        Ok(())
    }

    /// Admit one blocking durable step. Dropping the permit after the step
    /// lets erasure proceed. A closed owner is reported as absent.
    pub fn step(&self, id: Uuid) -> Result<OwnerStep> {
        let mut owners = self.bucket(id).lock();
        let state = owners.entry(id).or_default();
        if state.erasing {
            return Err(closed_owner());
        }
        state.running += 1;
        Ok(OwnerStep {
            lifecycle: self.clone(),
            id,
        })
    }

    /// Refuse new admission for the owner. This does not wait for admitted I/O,
    /// so it can run in the authority commit's in-memory revocation callback.
    pub fn close(&self, id: Uuid) {
        self.bucket(id).lock().entry(id).or_default().erasing = true;
    }

    /// Check a retired UUID for historical display. One short memory lookup.
    /// Use `step` instead when admitting new durable work.
    pub fn is_closed(&self, id: Uuid) -> bool {
        self.bucket(id)
            .lock()
            .get(&id)
            .is_some_and(|owner| owner.erasing)
    }

    /// Wait for already admitted durable steps to return, after closing
    /// admission. Call on a blocking worker outside any authority transaction.
    pub fn drain(&self, id: Uuid) {
        self.close(id);
        let bucket = self.bucket(id);
        drop(
            bucket
                .drained
                .wait_while(bucket.lock(), |m| m[&id].running != 0)
                .unwrap_or_else(|error| error.into_inner()),
        );
    }
}

fn closed_owner() -> Error {
    Error::not_found("owner storage has been erased")
}

/// A permit for a single durable store step. Never hold it while waiting for
/// client input or a live subscription.
pub struct OwnerStep {
    lifecycle: OwnerLifecycle,
    id: Uuid,
}

impl OwnerStep {
    /// Bind a connection or transaction guard to the admitted step. The value
    /// drops before the permit. No allocation or I/O.
    pub fn hold<T>(self, value: T) -> OwnerScoped<T> {
        OwnerScoped { value, _step: self }
    }
}

impl Drop for OwnerStep {
    fn drop(&mut self) {
        let bucket = self.lifecycle.bucket(self.id);
        let mut owners = bucket.lock();
        let state = owners.get_mut(&self.id).expect("active owner step");
        state.running -= 1;
        if state.running == 0 {
            if state.erasing {
                bucket.drained.notify_all();
            } else {
                owners.remove(&self.id);
            }
        }
    }
}

/// A store value pinned until its current durable step finishes.
pub struct OwnerScoped<T> {
    value: T,
    _step: OwnerStep,
}

impl<T> Deref for OwnerScoped<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for OwnerScoped<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
