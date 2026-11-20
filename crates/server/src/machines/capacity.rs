//! Machine and account capacity accounting under the existing registry lock.

use calternal_machine::protocol::{MIN_SANDBOX_MEMORY_BYTES, Owner, SMALL_VM_MEMORY_MIB};
use calternal_types::AccountId;
use uuid::Uuid;

use super::{Machine, State};

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
        let required = m
            .live
            .as_ref()
            .and_then(|live| live.guest_size)
            .map_or(MIN_SANDBOX_MEMORY_BYTES, |size| size.host_memory_bytes());
        self.memory_slots_for(m, required)
    }

    fn memory_slots_for(&self, m: &Machine, required: u64) -> usize {
        m.live.as_ref().map_or(0, |live| {
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

    /// Count server reservations and machine-held guests against the same RAM
    /// ceiling. A placement or teardown may be newer than the heartbeat.
    pub(super) fn small_vm_slots(&self, m: &Machine) -> usize {
        let Some(capacity) = m.live.as_ref().and_then(|live| live.capacity.small_vm) else {
            return 0;
        };
        let reserved = self
            .workers
            .keys()
            .chain(self.worker_stops.keys())
            .filter(|&lease_id| {
                self.leases.get(lease_id).is_some_and(|lease| {
                    lease.is_small_vm()
                        && (lease.assigned_machine() == Some(m.row.id)
                            || self
                                .worker_stops
                                .get(lease_id)
                                .is_some_and(|stop| stop.machine == m.row.id))
                })
            })
            .count();
        (capacity.max_residents as usize)
            .saturating_sub(reserved.max(capacity.resident_count as usize))
    }

    /// Apply the small-guest limit before creating a lease, accounting for
    /// host headroom and general sandbox slots in the existing registry.
    pub(super) fn free_small_vm(&self, m: &Machine) -> usize {
        self.small_vm_slots(m)
            .min(self.free_slots(m))
            .min(self.memory_slots_for(m, u64::from(SMALL_VM_MEMORY_MIB) << 20))
    }

    /// Available placements after the operator ceiling and host reservations.
    pub(super) fn free(&self, m: &Machine) -> usize {
        self.free_slots(m).min(self.memory_slots(m))
    }

    /// Combine host headroom with the RAM budget of an account guest.
    pub(super) fn free_for(
        &self,
        m: &Machine,
        account: AccountId,
        settings: &super::Settings,
    ) -> usize {
        let free = self.free(m);
        m.live
            .as_ref()
            .and_then(|live| live.guest_size)
            .map_or(free, |size| {
                let per_turn =
                    size.turn_memory_bytes(settings.limits.memory_bytes.unwrap_or(1 << 30));
                let held = self.pool_load.get(&account).copied().unwrap_or(0);
                free.min(
                    (size.turn_budget_bytes() / per_turn.max(1)).saturating_sub(held as u64)
                        as usize,
                )
            })
    }
}
