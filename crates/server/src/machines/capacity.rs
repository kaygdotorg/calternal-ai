//! Machine and account capacity accounting under the existing registry lock.

use calternal_machine::protocol::{MIN_SANDBOX_MEMORY_BYTES, Owner};
use calternal_types::AccountId;
use uuid::Uuid;

use super::{Lease, LeaseState, Machine, Settings, State};

// Minimum headroom per tentative start, separate from the process memory cap.
const START_HEADROOM_BYTES: u64 = 64 * 1024 * 1024;

/// User-facing reason shared by the run view and the thread projection.
pub(crate) const MEMORY_WAIT: &str = "Waiting for enough available memory on a machine.";

impl State {
    /// Read the number of leases reserved on this machine from the registry.
    pub fn load(&self, machine: Uuid) -> usize {
        self.machines.get(&machine).map_or(0, |m| m.assigned)
    }

    /// Reserve one machine slot. Only leases with owners count toward a pool account.
    pub(super) fn take_slot(&mut self, machine: Option<Uuid>, account: Option<AccountId>) {
        if let Some(m) = machine.and_then(|id| self.machines.get_mut(&id)) {
            m.assigned += 1;
            if let Some(live) = &mut m.live {
                live.memory_reservations += 1;
            }
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

    /// Number of placements possible under the explicit operator ceiling.
    pub(super) fn free_slots(&self, m: &Machine) -> usize {
        match &m.live {
            Some(live) if !live.draining && !m.row.draining && !m.row.revoked => {
                (live.capacity.max_sandboxes as usize)
                    .saturating_sub(m.assigned.max(live.capacity.running_sandboxes as usize))
            }
            _ => 0,
        }
    }

    /// Available reservations in the latest report. Charge already admitted
    /// work conservatively even when some of its RSS is reflected in the sample.
    /// No reads outside the in-memory machine record.
    pub(super) fn memory_slots(&self, m: &Machine) -> usize {
        m.live.as_ref().map_or(0, |live| {
            let required = live
                .guest_size
                .map_or(MIN_SANDBOX_MEMORY_BYTES, |size| size.host_memory_bytes());
            let bytes = live.capacity.available_memory_bytes.unwrap_or(0);
            let bytes = if live.capacity.memory_bytes == 0 {
                bytes
            } else {
                bytes.min(live.capacity.memory_bytes)
            };
            usize::try_from(bytes / required)
                .unwrap_or(usize::MAX)
                .saturating_sub(live.memory_reservations)
        })
    }

    /// Available placements after the operator ceiling and host reservations.
    pub(super) fn free(&self, m: &Machine) -> usize {
        self.free_slots(m).min(self.memory_slots(m))
    }

    /// Compute a lease's placement capacity after machine authorization and
    /// any explicit operator constraints. Personal machines retain their own
    /// placement limits. No SQL or cross-thread calls.
    pub(super) fn available_for_lease(
        &self,
        machine: &Machine,
        lease: &Lease,
        settings: &Settings,
    ) -> Option<usize> {
        if !super::readiness::connected(machine, settings)
            || lease
                .row
                .pinned_machine
                .is_some_and(|id| id != machine.row.id)
            || (lease.row.worker && !Self::worker_machine(machine, settings))
            || (lease.row.harness.is_some() && !Self::harness_machine(machine))
        {
            return None;
        }
        let sponsor = lease.row.shared_sponsor.unwrap_or(lease.row.account);
        if lease.row.shared_sponsor.is_some()
            && (!self.may_use(machine, sponsor)
                || !machine.live.as_ref().is_some_and(|live| {
                    live.shared_provider && (lease.row.shared_tools.is_none() || live.shared_tools)
                }))
        {
            return None;
        }
        let mut free = self.free_for(machine, lease.row.account);
        if machine.row.owner != Owner::Pool {
            return self.may_use(machine, sponsor).then_some(free);
        }
        if machine
            .live
            .as_ref()
            .is_some_and(|live| live.guest_size.is_some())
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
            return None;
        }
        if !self.resource_hook_allows(lease, machine) {
            return None;
        }
        let policy = lease
            .row
            .resource_owner
            .and_then(|owner| self.compute_limits.get(&owner));
        let Some(limits) = policy else {
            return (lease.pool_diagnostic || self.may_use(machine, sponsor)).then_some(free);
        };
        if limits.runtime_seconds.is_some()
            && (machine
                .live
                .as_ref()
                .is_none_or(|live| live.guest_size.is_none())
                || !lease
                    .row
                    .resource_owner
                    .and_then(|owner| self.runtime_windows.get(&owner))
                    .is_some_and(|window| window.allowed))
        {
            return None;
        }
        if !limits.hosted
            || limits.memory_mib.is_some_and(|mib| {
                machine
                    .live
                    .as_ref()
                    .and_then(|live| live.guest_size)
                    .map(|size| size.memory_mib)
                    != Some(mib)
            })
        {
            return None;
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
        Some(free)
    }

    /// Limit guest placement to observed headroom after reserving in-flight
    /// starts. Without a sample, allow a single boot and wait for observations
    /// before using its capacity. Cost: lease scan bounded by machine slots.
    pub(super) fn free_for(&self, m: &Machine, account: AccountId) -> usize {
        let free = self.free(m);
        let Some(live) = m.live.as_ref().filter(|live| live.guest_size.is_some()) else {
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
