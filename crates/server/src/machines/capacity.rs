//! Machine and account capacity accounting under the existing registry lock.

use calternal_machine::protocol::Owner;
use calternal_types::AccountId;
use uuid::Uuid;

use super::{Machine, State};

impl State {
    /// Read the number of leases reserved on this machine from the registry.
    pub fn load(&self, machine: Uuid) -> usize {
        self.machines.get(&machine).map_or(0, |m| m.assigned)
    }

    /// Restore or reserve capacity for a live assigned/running lease.
    pub(super) fn take_slot(&mut self, machine: Option<Uuid>, account: AccountId) {
        if let Some(m) = machine.and_then(|id| self.machines.get_mut(&id)) {
            m.assigned += 1;
            let owner = m.row.owner;
            if owner == Owner::Pool {
                *self.pool_load.entry(account).or_default() += 1;
            }
            self.readiness_changed(owner);
        }
    }

    /// Drop a placement reservation and its account count under the registry lock.
    pub(super) fn release_slot(&mut self, machine: Option<Uuid>, account: AccountId) {
        if let Some(m) = machine.and_then(|id| self.machines.get_mut(&id)) {
            m.assigned = m.assigned.saturating_sub(1);
            let owner = m.row.owner;
            if owner == Owner::Pool
                && let Some(count) = self.pool_load.get_mut(&account)
            {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.pool_load.remove(&account);
                }
            }
            self.readiness_changed(owner);
        }
    }

    pub(super) fn free(&self, m: &Machine) -> usize {
        match &m.live {
            Some(live) if !live.draining && !m.row.draining && !m.row.revoked => {
                (live.capacity.max_sandboxes as usize)
                    .saturating_sub(m.assigned.max(live.capacity.running_sandboxes as usize))
            }
            _ => 0,
        }
    }

    /// Combine machine resources and the selected size's account limit.
    pub(super) fn free_for(&self, m: &Machine, account: AccountId) -> usize {
        let free = self.free(m);
        m.live
            .as_ref()
            .and_then(|live| live.thread_limit)
            .map_or(free, |limit| {
                free.min(limit.saturating_sub(self.pool_load.get(&account).copied().unwrap_or(0)))
            })
    }
}
