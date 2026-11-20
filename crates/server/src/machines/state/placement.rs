//! Machine selection, assignment and enrolled-session reconciliation.
use super::*;
use crate::machines as registry;
use calternal_machine::protocol::{LeaseReport, SandboxState};

impl State {
    pub(in super::super) fn may_use(&self, m: &Machine, account: AccountId) -> bool {
        match m.row.owner {
            Owner::Account { id } => id == account,
            Owner::Pool => self.pool_grants.contains(&account),
        }
    }

    /// Assign waiting leases in arrival order. Preference: the machine that held
    /// the thread before, then the account's own machines, then the pool; ties go
    /// to the machine with the most free slots. Shared-space plugin workers are
    /// restricted to operator pool machines pending their placement contract. Cost: waiting × machines, with an
    /// indexed load count; one commit per assignment.
    pub fn place_waiting(&mut self, settings: &Settings) {
        let mut waiting: Vec<(bool, u64, Uuid)> = self
            .leases
            .values()
            .filter(|l| l.state == LeaseState::Waiting)
            .map(|l| (l.row.background_title, l.order, l.row.id))
            .collect();
        waiting.sort_unstable();
        for (_, _, lease) in waiting {
            let l = &self.leases[&lease];
            let sponsor = l.row.shared_sponsor.unwrap_or(l.row.account);
            let shared =
                l.row
                    .shared_sponsor
                    .zip(l.row.pinned_machine)
                    .map(
                        |(sponsor, machine)| registry::space_execution::SpaceExecution {
                            sponsor,
                            machine,
                            tools_enabled: l.row.shared_tools.is_some(),
                        },
                    );
            if let Some(target) = shared
                && self.require_execution(l.row.space, target).is_err()
            {
                self.end_for_placement(lease, "shared space machine changed", false);
                continue;
            }
            let small = l.is_small_vm();
            let (checkpoint, erasures) = if small {
                match registry::checkpoint::placement(&self.db, l.row.space) {
                    Ok(placement) => placement,
                    Err(error) => {
                        tracing::warn!(%error, "cannot read small VM checkpoint placement");
                        continue;
                    }
                }
            } else {
                (None, HashSet::new())
            };
            let mut memory_blocked = false;
            let best = self
                .machines
                .values()
                .filter(|m| !l.row.worker || Self::worker_machine(m, settings, small))
                .filter(|m| !erasures.contains(&m.row.id))
                .filter(|m| l.row.harness.is_none() || Self::harness_machine(m))
                .filter(|m| l.row.pinned_machine.is_none_or(|id| id == m.row.id))
                .filter(|m| {
                    self.may_use(m, sponsor) || (l.pool_diagnostic && m.row.owner == Owner::Pool)
                })
                .filter(|m| registry::readiness::connected(m, settings))
                .map(|m| {
                    let free = if small {
                        self.free_small_vm(m)
                    } else {
                        self.free_for(m, sponsor, settings)
                    };
                    memory_blocked |= free == 0 && self.free_slots(m) > 0;
                    (m, free)
                })
                .filter(|(_, free)| *free > 0)
                .max_by_key(|(m, free)| {
                    (
                        checkpoint == Some(m.row.id),
                        l.row.last_machine == Some(m.row.id),
                        m.row.owner != Owner::Pool,
                        *free,
                    )
                })
                .map(|(m, _)| m.row.id);
            if let Some(machine) = best {
                self.assign(lease, machine, settings);
            } else if let Some(l) = self.leases.get_mut(&lease) {
                // A wait explanation is derived from the latest live report.
                l.row.reason = memory_blocked.then(|| registry::MEMORY_WAIT.into());
            }
        }
    }

    /// Give a lease to a machine with a new fence. One commit.
    fn assign(&mut self, lease: Uuid, machine: Uuid, settings: &Settings) {
        let Some(l) = self.leases.get(&lease) else {
            return;
        };
        let sponsor = l.row.shared_sponsor.unwrap_or(l.row.account);
        if self.machines.get(&machine).is_none_or(|m| {
            !registry::readiness::connected(m, settings)
                || if l.is_small_vm() {
                    self.free_small_vm(m) == 0
                } else {
                    self.free_for(m, sponsor, settings) == 0
                }
        }) {
            return;
        }
        let Ok(_steps) = self.lease_steps(&l.row) else {
            return;
        };
        let mut row = l.row.clone();
        row.fence += 1;
        row.machine = Some(machine);
        row.last_machine = Some(machine);
        row.reason = None;
        row.state = LeaseState::Assigned.as_str().into();
        if row.worker
            && row.image.as_deref() == Some(calternal_machine::sandbox::worker::SPACE_VM_IMAGE)
            && let Err(error) = self.prepare_checkpoint_placement(row.space, machine)
        {
            tracing::warn!(%error, "cannot retire the previous small VM checkpoint");
            return;
        }
        if let Err(error) = db::put_assignment(&mut self.db, &row) {
            tracing::error!(%lease, %error, "cannot persist machine assignment");
            return;
        }
        let l = self
            .leases
            .get_mut(&lease)
            .expect("existing lease under registry lock");
        l.row = row;
        l.state = LeaseState::Assigned;
        let account = l.row.account;
        self.take_slot(Some(machine), Some(account));
        self.send_assign(lease, settings);
        self.send_execs(lease);
    }

    /// Compare a machine's report with the registry: confirm running sandboxes,
    /// end failed ones, hand a waiting lease back to the machine that still has
    /// its sandbox, and stop everything else.
    pub fn reconcile(
        &mut self,
        machine: Uuid,
        reports: &[LeaseReport],
        settings: &Settings,
    ) -> Result<(), Error> {
        for r in reports {
            let Some(l) = self.leases.get(&r.lease_id) else {
                self.push(
                    machine,
                    Down::Stop {
                        lease_id: r.lease_id,
                        reason: "unknown lease".into(),
                        keep_files: true,
                    },
                );
                continue;
            };
            let Ok(_steps) = self.lease_steps(&l.row) else {
                self.push(
                    machine,
                    Down::Stop {
                        lease_id: r.lease_id,
                        reason: "owner erased".into(),
                        keep_files: !l.row.worker && !l.row.background_title,
                    },
                );
                continue;
            };
            let small = l.is_small_vm();
            let l = self
                .leases
                .get_mut(&r.lease_id)
                .expect("existing lease under registry lock");
            let keep_files = !l.row.worker && !l.row.background_title;
            let sponsor = l.row.shared_sponsor.unwrap_or(l.row.account);
            let current = l.row.machine == Some(machine)
                && l.row.fence == r.fence
                && !matches!(l.state, LeaseState::Ended | LeaseState::Stopping);
            if current {
                match &r.sandbox {
                    SandboxState::Running | SandboxState::Frozen
                        if l.state == LeaseState::Assigned
                            || r.sandbox == SandboxState::Running =>
                    {
                        let space = l.row.space;
                        if r.sandbox == SandboxState::Running {
                            self.consume_restore(space, machine, r.lease_id)?;
                        }
                        let lease = self.leases.get_mut(&r.lease_id).expect("current lease");
                        if lease.state == LeaseState::Assigned {
                            lease.state = LeaseState::Running;
                            self.persist(r.lease_id);
                        }
                    }
                    SandboxState::Lost => {
                        registry::microvm::lost(self, machine, r.lease_id, settings, keep_files)
                    }
                    SandboxState::Failed { reason } => {
                        let why = format!("sandbox failed: {reason}");
                        self.end_with_files(r.lease_id, &why, settings, keep_files);
                    }
                    _ => {}
                }
            } else if self.worker_stops.get(&r.lease_id).is_some_and(|stop| {
                stop.checkpoint
                    && stop.machine == machine
                    && self
                        .machines
                        .get(&machine)
                        .and_then(|m| m.live.as_ref())
                        .is_some_and(|live| live.session == stop.session)
            }) {
                // This session already has the numbered sleep command. A
                // second Stop could retire the slot before its proof is sent.
                continue;
            } else if l.state == LeaseState::Waiting
                && l.row.last_machine == Some(machine)
                && self.machines.get(&machine).is_some_and(|m| {
                    if small {
                        self.free_small_vm(m) > 0
                    } else {
                        self.free_for(m, sponsor, settings) > 0
                    }
                })
            {
                self.assign(r.lease_id, machine, settings);
            } else {
                self.push(
                    machine,
                    Down::Stop {
                        lease_id: r.lease_id,
                        reason: "lease no longer held by this machine".into(),
                        keep_files,
                    },
                );
            }
        }
        Ok(())
    }
}
