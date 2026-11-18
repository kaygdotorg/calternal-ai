//! Machine and account capacity accounting under the existing registry lock.

use calternal_machine::protocol::Owner;
use calternal_types::AccountId;
use uuid::Uuid;

use super::{Lease, LeaseState, Machine, Settings, State};

// Minimum headroom per tentative start, separate from the process memory cap.
const START_HEADROOM_BYTES: u64 = 64 * 1024 * 1024;

impl State {
    /// Read the number of leases reserved on this machine from the registry.
    pub fn load(&self, machine: Uuid) -> usize {
        self.machines.get(&machine).map_or(0, |m| m.assigned)
    }

    /// Reserve one machine slot. Only leases with owners count toward a pool account.
    pub(super) fn take_slot(&mut self, machine: Option<Uuid>, account: Option<AccountId>) {
        if let Some(m) = machine.and_then(|id| self.machines.get_mut(&id)) {
            m.assigned += 1;
            let owner = m.row.owner;
            if owner == Owner::Pool
                && let Some(account) = account
            {
                *self.pool_load.entry(account).or_default() += 1;
            }
            self.readiness_changed(owner);
        }
    }

    /// Release a machine slot and any associated pool account count. No I/O.
    pub(super) fn release_slot(&mut self, machine: Option<Uuid>, account: Option<AccountId>) {
        if let Some(m) = machine.and_then(|id| self.machines.get_mut(&id)) {
            m.assigned = m.assigned.saturating_sub(1);
            let owner = m.row.owner;
            if owner == Owner::Pool
                && let Some(account) = account
            {
                self.release_pool_slot(account);
            }
            self.readiness_changed(owner);
        }
    }

    /// Drop an account's pool count independently of the machine slot. No I/O.
    pub(super) fn release_pool_slot(&mut self, account: AccountId) {
        if let Some(count) = self.pool_load.get_mut(&account) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.pool_load.remove(&account);
            }
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

    /// Compute a lease's placement capacity after machine authorization and
    /// any explicit operator constraints. Personal machines retain their own
    /// placement limits. No SQL or cross-thread calls.
    pub(super) fn available_for_lease(
        &self,
        machine: &Machine,
        lease: &Lease,
        settings: &Settings,
    ) -> usize {
        if lease
            .row
            .pinned_machine
            .is_some_and(|id| id != machine.row.id)
            || (lease.row.worker && !Self::worker_machine(machine, settings))
            || (lease.row.harness.is_some() && !Self::harness_machine(machine))
        {
            return 0;
        }
        let mut free = self.free_for(machine, lease.row.account);
        if machine.row.owner != Owner::Pool {
            return if self.may_use(machine, lease.row.account) {
                free
            } else {
                0
            };
        }
        if machine
            .live
            .as_ref()
            .is_some_and(|live| live.guest_memory_mib.is_some())
            && (self.leases.values().any(|active| {
                active.row.account == lease.row.account
                    && active.assigned_machine() == Some(machine.row.id)
                    && active.row.resource_owner != lease.row.resource_owner
            }) || machine
                .live
                .as_ref()
                .and_then(|live| live.guest_resources.get(&lease.row.account))
                .is_some_and(|guest| {
                    guest.active_threads > 0 && guest.resource_owner != lease.row.resource_owner
                }))
        {
            return 0;
        }
        if !self.resource_hook_allows(lease, machine) {
            return 0;
        }
        let policy = lease
            .row
            .resource_owner
            .and_then(|owner| self.compute_limits.get(&owner));
        let Some(limits) = policy else {
            return if lease.pool_diagnostic || self.may_use(machine, lease.row.account) {
                free
            } else {
                0
            };
        };
        if limits.runtime_seconds.is_some()
            && (machine
                .live
                .as_ref()
                .is_none_or(|live| live.guest_memory_mib.is_none())
                || !lease
                    .row
                    .resource_owner
                    .and_then(|owner| self.runtime_windows.get(&owner))
                    .is_some_and(|window| window.allowed))
        {
            return 0;
        }
        if !limits.hosted
            || limits.memory_mib.is_some_and(|mib| {
                machine.live.as_ref().and_then(|live| live.guest_memory_mib) != Some(mib)
            })
        {
            return 0;
        }
        if let Some(cap) = limits.active_threads {
            let held = self
                .leases
                .values()
                .filter(|held| {
                    held.row.account == lease.row.account
                        && held.assigned_machine() == Some(machine.row.id)
                })
                .count();
            free = free.min((cap as usize).saturating_sub(held));
        }
        free
    }

    /// Limit guest placement to observed headroom after reserving in-flight
    /// starts. Without a sample, allow a single boot and wait for observations
    /// before using its capacity. Cost: lease scan bounded by machine slots.
    pub(super) fn free_for(&self, m: &Machine, account: AccountId) -> usize {
        let free = self.free(m);
        let Some(live) = m
            .live
            .as_ref()
            .filter(|live| live.guest_memory_mib.is_some())
        else {
            return free;
        };
        if free == 0 {
            return 0;
        }
        let (held, starting) = self
            .leases
            .values()
            .filter(|lease| {
                lease.row.account == account && lease.assigned_machine() == Some(m.row.id)
            })
            .fold((0, 0), |(held, starting), lease| {
                (
                    held + 1,
                    starting + usize::from(lease.state == LeaseState::Assigned),
                )
            });
        let memory = live
            .guest_resources
            .get(&account)
            .map(|report| report.memory_available_bytes);
        match memory {
            Some(bytes) => {
                let available = usize::try_from(bytes / START_HEADROOM_BYTES).unwrap_or(usize::MAX);
                free.min(available.saturating_sub(starting))
            }
            None => usize::from(held == 0),
        }
    }
}
