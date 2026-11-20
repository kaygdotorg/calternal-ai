//! Registry state behind the [`super::MachineService`] mutex: machines, live
//! sessions and their outboxes, leases, execs, and the placement and fencing
//! rules. Every function here runs with the lock held; the ones that persist a
//! lease commit to SQLite (one fsync) and say so.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use calternal_machine::protocol::{
    Capacity, Chunk, ChunkEvent, Down, Envelope, ExitInfo, Owner, SandboxSpec,
};
use calternal_types::{AccountId, Error};
use tokio::sync::Notify;
use uuid::Uuid;

mod placement;

use super::{LeaseState, MachineView, RunView, Settings, db, workers::WorkerStop};

/// An open machine session: its outbox and last report.
pub(super) struct Live {
    pub session: [u8; 32],
    pub direct: bool,
    pub harness_protocol: bool,
    pub shared_provider: bool,
    pub shared_tools: bool,
    pub kvm_fallback: bool,
    pub guest_size: Option<calternal_machine::protocol::GuestSize>,
    /// Reservations against the current memory sample, including releases
    /// whose memory has not yet been observed as free in another heartbeat.
    pub memory_reservations: usize,
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
    pub title_blocked: Option<(String, String)>,
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
            title_blocked: None,
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
    /// Identify the small plugin VM profile on the existing worker lease.
    pub(super) fn is_small_vm(&self) -> bool {
        self.row.worker
            && self.row.image.as_deref() == Some(calternal_machine::sandbox::worker::SPACE_VM_IMAGE)
    }

    /// A live assignment's machine. Stopping worker reservations are in
    /// `worker_stops` until teardown proof or revocation.
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
    pub space_channels: HashMap<calternal_types::SpaceId, super::space_channel::Reservation>,
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
    /// In-flight pool leases per account, including placement reservations.
    pub pool_load: HashMap<AccountId, usize>,
    pub leases: HashMap<Uuid, Lease>,
    pub threads: HashMap<Uuid, Uuid>,
    pub execs: HashMap<Uuid, Exec>,
    pub workers: HashMap<Uuid, calternal_machine::protocol::PluginWorker>,
    pub worker_stops: HashMap<Uuid, WorkerStop>,
    pub next_order: u64,
}

impl State {
    /// Pin durable work or message delivery to both principals of the lease.
    pub(super) fn lease_steps(
        &self,
        row: &db::LeaseRow,
    ) -> Result<Option<[Option<calternal_types::OwnerStep>; 3]>, Error> {
        self.lifecycle
            .get()
            .map(|owners| {
                Ok([
                    Some(owners.step(row.account.as_uuid())?),
                    Some(owners.step(row.space.as_uuid())?),
                    row.shared_sponsor
                        .filter(|sponsor| *sponsor != row.account)
                        .map(|sponsor| owners.step(sponsor.as_uuid()))
                        .transpose()?,
                ])
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
        if l.row.shared_sponsor.is_some()
            && !self
                .machines
                .get(&machine)
                .and_then(|m| m.live.as_ref())
                .is_some_and(|live| {
                    live.shared_provider && (l.row.shared_tools.is_none() || live.shared_tools)
                })
        {
            return;
        }
        let lease_id = l.row.id;
        let fence = l.row.fence;
        let space_id = l.row.space;
        let account_id = l.row.account;
        let mut limits = settings.limits;
        if let Some(size) = self
            .machines
            .get(&machine)
            .and_then(|m| m.live.as_ref())
            .and_then(|l| l.guest_size)
        {
            limits.memory_bytes =
                Some(size.turn_memory_bytes(limits.memory_bytes.unwrap_or(1 << 30)));
        }
        let spec = SandboxSpec {
            space_id: Some(space_id),
            provider_login: l.provider_login,
            shared_provider_sponsor: l.row.shared_sponsor,
            shared_tools: l.row.shared_tools,
            code_directory: None,
            worker: self.workers.get(&lease).cloned(),
            account_id: account_id.as_uuid(),
            thread_id: l.row.thread,
            image: l.row.image.clone(),
            limits,
        };
        let msg = match &l.row.harness {
            Some(harness) => {
                if !self
                    .machines
                    .get(&machine)
                    .is_some_and(Self::harness_machine)
                {
                    return;
                }
                Down::AssignHarness {
                    lease_id,
                    fence,
                    space_id,
                    account_id,
                    spec,
                    harness: harness.clone(),
                }
            }
            None => Down::Assign {
                lease_id,
                fence,
                space_id,
                account_id,
                spec,
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
        self.close_space_lease(lease);
        self.lose_execs(lease, why);
        let Some(l) = self.leases.get_mut(&lease) else {
            return;
        };
        let assigned = l.assigned_machine();
        l.row.fence += 1;
        l.row.machine = None;
        l.row.reason = Some(why.into());
        l.state = LeaseState::Interrupted;
        let account = l.row.account;
        self.release_slot(assigned, Some(account));
        self.persist(lease);
    }

    /// End a lease, fence its execs and tell its machine to stop the sandbox.
    /// One commit, plus placements when a slot was freed. Stopping workers
    /// hold their reservation until the scheduler receives proof.
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
        self.end_for_placement(lease, why, keep_files);
        self.place_waiting(settings);
    }

    fn end_for_placement(&mut self, lease: Uuid, why: &str, keep_files: bool) {
        self.close_space_lease(lease);
        let Some(l) = self.leases.get_mut(&lease) else {
            return;
        };
        if matches!(l.state, LeaseState::Ended | LeaseState::Stopping) {
            return;
        }
        let machine = l.row.machine;
        let assigned = l.assigned_machine();
        let worker = l.row.worker;
        let checkpoint = worker
            && keep_files
            && l.row.image.as_deref() == Some(calternal_machine::sandbox::worker::SPACE_VM_IMAGE);
        let stopping = worker
            && machine.is_some_and(|id| self.machines.get(&id).is_some_and(|m| !m.row.revoked));
        let counter = checkpoint
            .then(|| super::checkpoint::sleep_counter(&self.db, lease, l.row.space))
            .and_then(|result| match result {
                Ok(counter) => counter,
                Err(error) => {
                    tracing::warn!(%error, "cannot read authorized checkpoint counter");
                    None
                }
            });
        l.row.fence += 1;
        l.state = if stopping {
            LeaseState::Stopping
        } else {
            LeaseState::Ended
        };
        l.row.reason = Some(why.into());
        let thread = l.row.thread;
        self.threads.remove(&thread);
        self.workers.remove(&lease);
        let account = l.row.account;
        if !stopping {
            self.release_slot(assigned, Some(account));
        }
        self.persist(lease);
        for e in self.execs.values_mut() {
            if e.lease == lease && e.state != ExecState::Done {
                e.lose(why);
            }
        }
        if let Some(m) = machine {
            if stopping {
                let (session, seq) = self
                    .machines
                    .get(&m)
                    .and_then(|m| m.live.as_ref())
                    .map_or(([0; 32], u64::MAX), |live| (live.session, live.next_seq));
                self.worker_stops.insert(
                    lease,
                    WorkerStop {
                        machine: m,
                        account: Some(account),
                        session,
                        seq,
                        checkpoint: counter.is_some(),
                    },
                );
            }
            let command = if let Some(counter) = counter {
                Down::SleepSpace {
                    lease_id: lease,
                    counter,
                }
            } else {
                Down::Stop {
                    lease_id: lease,
                    reason: why.into(),
                    keep_files,
                }
            };
            self.push(m, command);
        }
    }

    /// Fence the revoked session and release its reservations, including
    /// stopping workers. The caller has durably revoked the machine first.
    pub fn apply_revocation(&mut self, machine: Uuid, settings: &Settings) -> Result<(), Error> {
        self.close_machine_channels(machine);
        let Some(m) = self.machines.get_mut(&machine) else {
            return Ok(());
        };
        m.row.revoked = true;
        if let Some(live) = m.live.take() {
            self.sessions.remove(&live.session);
            live.notify.notify_one();
        }
        let owner = m.row.owner;
        m.checks.clear();
        self.readiness_changed(owner);
        let stopping: Vec<_> = self
            .worker_stops
            .iter()
            .filter_map(|(lease, stop)| (stop.machine == machine).then_some(*lease))
            .collect();
        for lease in stopping {
            self.finish_worker_stop(lease)?;
        }
        let held: Vec<Uuid> = self
            .leases
            .values()
            .filter(|l| l.row.machine == Some(machine) && l.state != LeaseState::Ended)
            .map(|l| l.row.id)
            .collect();
        for lease in held {
            self.end_lease(lease, "the machine was revoked", settings);
        }
        Ok(())
    }

    fn harness_machine(machine: &Machine) -> bool {
        machine
            .live
            .as_ref()
            .is_some_and(|live| live.harness_protocol && !live.direct)
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
