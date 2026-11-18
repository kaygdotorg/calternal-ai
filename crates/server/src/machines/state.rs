//! Registry state behind the [`super::MachineService`] mutex: machines, live
//! sessions and their outboxes, leases, execs, and the placement and fencing
//! rules. Every function here runs with the lock held; the ones that persist a
//! lease commit to SQLite (one fsync) and say so.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use calternal_machine::protocol::{
    Capacity, Chunk, ChunkEvent, Down, Envelope, ExitInfo, LeaseReport, Owner, SandboxSpec,
    SandboxState,
};
use calternal_types::{AccountId, Error};
use tokio::sync::Notify;
use uuid::Uuid;

use super::{LeaseState, MachineView, RunView, Settings, db};

/// An open machine session: its outbox and last report.
pub(super) struct Live {
    pub session: [u8; 32],
    pub direct: bool,
    pub kvm_fallback: bool,
    pub outbox: VecDeque<Envelope>,
    pub next_seq: u64,
    pub polls: u32,
    pub capacity: Capacity,
    pub draining: bool,
    pub notify: Arc<Notify>,
}

/// An enrolled machine.
pub(super) struct Machine {
    pub row: db::MachineRow,
    pub assigned: usize,
    pub boot_id: Option<Uuid>,
    pub checks: HashMap<AccountId, HashMap<String, super::readiness::Observation>>,
    pub last_seen: Instant,
    pub last_seen_ms: Option<i64>,
    pub live: Option<Live>,
}

impl Machine {
    /// A machine known from the database, not yet connected. `last_seen` starts
    /// at `now`, so after a server restart leases get a full TTL of grace.
    pub fn new(row: db::MachineRow, now: Instant) -> Self {
        Self {
            row,
            assigned: 0,
            boot_id: None,
            checks: HashMap::new(),
            last_seen: now,
            last_seen_ms: None,
            live: None,
        }
    }
}

/// A lease and its place in the waiting queue.
pub(super) struct Lease {
    pub row: db::LeaseRow,
    pub state: LeaseState,
    pub order: u64,
    pub pool_diagnostic: bool,
    pub provider_login: bool,
}

impl Lease {
    /// A lease holds a placement slot from assignment through running.
    pub(super) fn assigned_machine(&self) -> Option<Uuid> {
        self.row
            .machine
            .filter(|_| matches!(self.state, LeaseState::Assigned | LeaseState::Running))
    }
}

#[derive(PartialEq, Eq)]
pub(super) enum ExecState {
    Queued,
    Sent,
    Done,
}

/// An exec and its unread output, bounded by `Settings::output_buffer_bytes`.
pub(super) struct Exec {
    pub lease: Uuid,
    pub account: AccountId,
    pub argv: Vec<String>,
    pub stdin: String,
    pub interactive: bool,
    pub env: Vec<(String, String)>,
    pub state: ExecState,
    pub chunks: VecDeque<(Chunk, usize)>,
    pub held: usize,
    pub last_seq: u64,
    pub lost_before: Option<u64>,
    pub finished: Option<Instant>,
    pub notify: Arc<Notify>,
}

impl Exec {
    pub fn new(
        lease: Uuid,
        account: AccountId,
        argv: Vec<String>,
        env: Vec<(String, String)>,
        state: ExecState,
    ) -> Self {
        Self {
            lease,
            account,
            argv,
            stdin: String::new(),
            interactive: false,
            env,
            state,
            chunks: VecDeque::new(),
            held: 0,
            last_seq: 0,
            lost_before: None,
            finished: None,
            notify: Arc::new(Notify::new()),
        }
    }

    /// Append the final chunk reporting why the exec is gone.
    fn lose(&mut self, why: &str) {
        self.state = ExecState::Done;
        self.finished = Some(Instant::now());
        self.last_seq += 1;
        let exit = ExitInfo {
            error: Some(why.into()),
            ..ExitInfo::default()
        };
        self.chunks.push_back((
            Chunk {
                seq: self.last_seq,
                event: ChunkEvent::Exit(exit),
            },
            0,
        ));
        self.notify.notify_one();
    }
}

/// Everything the registry knows. Bounded: challenges by `MAX_CHALLENGES`,
/// finished execs by ten minutes, sessions by one per machine.
pub(super) struct State {
    pub lifecycle: Arc<std::sync::OnceLock<calternal_types::OwnerLifecycle>>,
    pub db: rusqlite::Connection,
    /// Machines with at least one durable erasure. Idle syncs need no SQL read.
    pub pending_erasure_machines: HashSet<Uuid>,
    /// Recovery-protected machine authority in `auth.db`.
    pub authority: calternal_auth::machines::Machines,
    pub machines: HashMap<Uuid, Machine>,
    pub readiness: super::readiness::Readiness,
    pub sessions: HashMap<[u8; 32], Uuid>,
    pub challenges: HashMap<String, (Uuid, Instant)>,
    pub pool_grants: HashSet<AccountId>,
    pub leases: HashMap<Uuid, Lease>,
    pub threads: HashMap<Uuid, Uuid>,
    pub execs: HashMap<Uuid, Exec>,
    pub workers: HashMap<Uuid, calternal_machine::protocol::PluginWorker>,
    pub worker_stops: HashMap<Uuid, (Uuid, [u8; 32], u64)>,
    pub recovery_stops: HashSet<Uuid>,
    pub next_order: u64,
}

impl State {
    /// Pin durable work or message delivery to both principals of the lease.
    pub(super) fn lease_steps(
        &self,
        row: &db::LeaseRow,
    ) -> Result<Option<(calternal_types::OwnerStep, calternal_types::OwnerStep)>, Error> {
        self.lifecycle
            .get()
            .map(|owners| {
                Ok((
                    owners.step(row.account.as_uuid())?,
                    owners.step(row.space.as_uuid())?,
                ))
            })
            .transpose()
    }

    pub fn push(&mut self, machine: Uuid, message: Down) {
        if let Some(live) = self
            .machines
            .get_mut(&machine)
            .and_then(|m| m.live.as_mut())
        {
            let seq = live.next_seq;
            live.next_seq += 1;
            live.outbox.push_back(Envelope { seq, message });
            live.notify.notify_one();
        }
    }

    /// Write a lease's current state: one SQLite commit.
    fn persist(&self, lease: Uuid) {
        if let Some(l) = self.leases.get(&lease) {
            let mut row = l.row.clone();
            row.state = l.state.as_str().into();
            if let Err(e) = db::put_lease(&self.db, &row) {
                tracing::error!(error = %e, %lease, "cannot persist lease");
            }
        }
    }

    pub fn send_assign(&mut self, lease: Uuid, settings: &Settings) {
        let Some(l) = self.leases.get(&lease) else {
            return;
        };
        let Ok(_steps) = self.lease_steps(&l.row) else {
            return;
        };
        let Some(machine) = l.row.machine else { return };
        let msg = Down::Assign {
            lease_id: lease,
            fence: l.row.fence,
            space_id: l.row.space,
            account_id: l.row.account,
            spec: SandboxSpec {
                space_id: Some(l.row.space),
                provider_login: l.provider_login,
                code_directory: None,
                worker: self.workers.get(&lease).cloned(),
                account_id: l.row.account.as_uuid(),
                thread_id: l.row.thread,
                image: l.row.image.clone(),
                limits: settings.limits,
            },
        };
        self.push(machine, msg);
    }

    /// Send every unfinished exec of an assigned lease; the machine ignores repeats.
    pub fn send_execs(&mut self, lease: Uuid) {
        let Some(l) = self.leases.get(&lease) else {
            return;
        };
        let Ok(_steps) = self.lease_steps(&l.row) else {
            return;
        };
        let (Some(machine), fence) = (l.row.machine, l.row.fence) else {
            return;
        };
        if !matches!(l.state, LeaseState::Assigned | LeaseState::Running) {
            return;
        }
        let mut msgs = Vec::new();
        for (id, e) in self.execs.iter_mut() {
            if e.lease == lease && e.state != ExecState::Done && !e.argv.is_empty() {
                e.state = ExecState::Sent;
                msgs.push(Down::Exec {
                    interactive: e.interactive,
                    lease_id: lease,
                    fence,
                    exec_id: *id,
                    argv: e.argv.clone(),
                    stdin: e.stdin.clone(),
                    env: e.env.clone(),
                });
            }
        }
        for m in msgs {
            self.push(machine, m);
        }
    }

    /// Report every sent exec of a lease as lost.
    pub fn lose_execs(&mut self, lease: Uuid, why: &str) {
        for e in self.execs.values_mut() {
            if e.lease == lease && e.state == ExecState::Sent {
                e.lose(why);
            }
        }
    }

    /// Fence a lease and put it back in the queue (its machine went silent).
    /// One commit.
    pub fn interrupt(&mut self, lease: Uuid, why: &str) {
        self.lose_execs(lease, why);
        let Some(l) = self.leases.get_mut(&lease) else {
            return;
        };
        let assigned = l.assigned_machine();
        l.row.fence += 1;
        l.row.machine = None;
        l.row.reason = Some(why.into());
        l.state = LeaseState::Interrupted;
        self.release_slot(assigned);
        self.persist(lease);
    }

    /// End a lease, fence it, fail its execs and tell its machine to stop the
    /// sandbox. One commit, plus any placements it frees capacity for.
    pub fn end_lease(&mut self, lease: Uuid, why: &str, settings: &Settings) {
        self.end_with_files(lease, why, settings, false)
    }

    pub(super) fn end_with_files(
        &mut self,
        lease: Uuid,
        why: &str,
        settings: &Settings,
        keep_files: bool,
    ) {
        let Some(l) = self.leases.get_mut(&lease) else {
            return;
        };
        if matches!(l.state, LeaseState::Ended | LeaseState::Stopping) {
            return;
        }
        let machine = l.row.machine;
        let assigned = l.assigned_machine();
        let worker = l.row.worker;
        l.row.fence += 1;
        l.state = if worker && machine.is_some() {
            LeaseState::Stopping
        } else {
            LeaseState::Ended
        };
        l.row.reason = Some(why.into());
        let thread = l.row.thread;
        self.threads.remove(&thread);
        self.workers.remove(&lease);
        self.release_slot(assigned);
        self.persist(lease);
        for e in self.execs.values_mut() {
            if e.lease == lease && e.state != ExecState::Done {
                e.lose(why);
            }
        }
        if let Some(m) = machine {
            if worker {
                let (session, seq) = self
                    .machines
                    .get(&m)
                    .and_then(|m| m.live.as_ref())
                    .map_or(([0; 32], u64::MAX), |live| (live.session, live.next_seq));
                self.worker_stops.insert(lease, (m, session, seq));
            }
            self.push(
                m,
                Down::Stop {
                    lease_id: lease,
                    reason: why.into(),
                    keep_files,
                },
            );
        }
        self.place_waiting(settings);
    }

    /// Drop the machine's session at once and end every lease it holds.
    pub fn apply_revocation(&mut self, machine: Uuid, settings: &Settings) {
        let Some(m) = self.machines.get_mut(&machine) else {
            return;
        };
        m.row.revoked = true;
        if let Some(live) = m.live.take() {
            self.sessions.remove(&live.session);
            live.notify.notify_one();
        }
        let owner = m.row.owner;
        m.checks.clear();
        self.readiness_changed(owner);
        let held: Vec<Uuid> = self
            .leases
            .values()
            .filter(|l| l.row.machine == Some(machine) && l.state != LeaseState::Ended)
            .map(|l| l.row.id)
            .collect();
        for lease in held {
            self.end_lease(lease, "the machine was revoked", settings);
        }
    }

    pub(super) fn may_use(&self, m: &Machine, account: AccountId) -> bool {
        match m.row.owner {
            Owner::Account { id } => id == account,
            Owner::Pool => self.pool_grants.contains(&account),
        }
    }

    pub fn load(&self, machine: Uuid) -> usize {
        self.machines.get(&machine).map_or(0, |m| m.assigned)
    }

    /// Restore or reserve capacity for a live assigned/running lease.
    pub(super) fn take_slot(&mut self, machine: Option<Uuid>) {
        if let Some(m) = machine.and_then(|id| self.machines.get_mut(&id)) {
            m.assigned += 1;
            let owner = m.row.owner;
            self.readiness_changed(owner);
        }
    }

    fn release_slot(&mut self, machine: Option<Uuid>) {
        if let Some(m) = machine.and_then(|id| self.machines.get_mut(&id)) {
            m.assigned = m.assigned.saturating_sub(1);
            let owner = m.row.owner;
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

    /// Assign waiting leases in arrival order. Preference: the machine that held
    /// the thread before, then the account's own machines, then the pool; ties go
    /// to the machine with the most free slots. Shared-space plugin workers are
    /// restricted to operator pool machines pending their placement contract. Cost: waiting × machines, with an
    /// indexed load count; one commit per assignment.
    pub fn place_waiting(&mut self, settings: &Settings) {
        let mut waiting: Vec<(u64, Uuid)> = self
            .leases
            .values()
            .filter(|l| l.state == LeaseState::Waiting)
            .map(|l| (l.order, l.row.id))
            .collect();
        waiting.sort_unstable();
        for (_, lease) in waiting {
            let l = &self.leases[&lease];
            let best = self
                .machines
                .values()
                // Shared-space workers must not send another member's data to
                // a participant-owned host after that host's owner is revoked.
                // Until placement has a space-owned trust contract, use only
                // operator-enrolled pool machines and the existing pool grant.
                .filter(|m| !l.row.worker || m.row.owner == Owner::Pool)
                .filter(|m| l.row.pinned_machine.is_none_or(|id| id == m.row.id))
                .filter(|m| {
                    self.may_use(m, l.row.account)
                        || (l.pool_diagnostic && m.row.owner == Owner::Pool)
                })
                .map(|m| (m, self.free(m)))
                .filter(|(_, free)| *free > 0)
                .max_by_key(|(m, free)| {
                    (
                        l.row.last_machine == Some(m.row.id),
                        m.row.owner != Owner::Pool,
                        *free,
                    )
                })
                .map(|(m, _)| m.row.id);
            if let Some(machine) = best {
                self.assign(lease, machine, settings);
            }
        }
    }

    /// Give a lease to a machine with a new fence. One commit.
    fn assign(&mut self, lease: Uuid, machine: Uuid, settings: &Settings) {
        let Some(l) = self.leases.get(&lease) else {
            return;
        };
        let Ok(_steps) = self.lease_steps(&l.row) else {
            return;
        };
        let mut row = l.row.clone();
        row.fence += 1;
        row.machine = Some(machine);
        row.last_machine = Some(machine);
        row.reason = None;
        row.state = LeaseState::Assigned.as_str().into();
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
        self.take_slot(Some(machine));
        self.send_assign(lease, settings);
        self.send_execs(lease);
    }

    /// Compare a machine's report with the registry: confirm running sandboxes,
    /// end failed ones, hand a waiting lease back to the machine that still has
    /// its sandbox, and stop everything else.
    pub fn reconcile(&mut self, machine: Uuid, reports: &[LeaseReport], settings: &Settings) {
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
                        keep_files: !l.row.worker,
                    },
                );
                continue;
            };
            let l = self
                .leases
                .get_mut(&r.lease_id)
                .expect("existing lease under registry lock");
            let keep_files = !l.row.worker;
            let current = l.row.machine == Some(machine)
                && l.row.fence == r.fence
                && !matches!(l.state, LeaseState::Ended | LeaseState::Stopping);
            if current {
                match &r.sandbox {
                    SandboxState::Running | SandboxState::Frozen
                        if l.state == LeaseState::Assigned =>
                    {
                        l.state = LeaseState::Running;
                        self.persist(r.lease_id);
                    }
                    SandboxState::Lost => {
                        super::microvm::lost(self, machine, r.lease_id, settings, keep_files)
                    }
                    SandboxState::Failed { reason } => {
                        let why = format!("sandbox failed: {reason}");
                        self.end_with_files(r.lease_id, &why, settings, keep_files);
                    }
                    _ => {}
                }
            } else if l.state == LeaseState::Waiting
                && l.row.last_machine == Some(machine)
                && self
                    .machines
                    .get(&machine)
                    .is_some_and(|m| self.free(m) > 0)
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
    }

    pub fn view(&self, lease: Uuid) -> RunView {
        let l = &self.leases[&lease];
        let queue_position = (l.state == LeaseState::Waiting).then(|| {
            1 + self
                .leases
                .values()
                .filter(|o| o.state == LeaseState::Waiting && o.order < l.order)
                .count()
        });
        RunView {
            lease_id: lease,
            space_id: l.row.space,
            thread_id: l.row.thread,
            state: l.state,
            machine_id: l.row.machine.filter(|_| l.state != LeaseState::Waiting),
            personal_machine: l
                .row
                .machine
                .and_then(|id| self.machines.get(&id))
                .is_some_and(|machine| machine.row.owner == Owner::Account { id: l.row.account }),
            reason: l.row.reason.clone(),
            queue_position,
        }
    }

    pub fn machine_view(&self, m: &Machine, settings: &Settings) -> MachineView {
        let online = super::readiness::connected(m, settings);
        let state = if m.row.revoked {
            "revoked"
        } else if !online {
            "offline"
        } else if m.row.draining || m.live.as_ref().is_some_and(|l| l.draining) {
            "draining"
        } else {
            "online"
        };
        MachineView {
            id: m.row.id,
            name: m.row.name.clone(),
            os: m.row.os.clone(),
            arch: m.row.arch.clone(),
            direct: m.row.direct && m.row.owner != Owner::Pool,
            owner: m.row.owner,
            state,
            capacity: m.live.as_ref().map(|l| l.capacity).unwrap_or_default(),
            leases: self.load(m.row.id),
        }
    }
}
