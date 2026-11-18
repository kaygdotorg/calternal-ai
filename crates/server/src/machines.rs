//! Execution machines, server side: enrolment, sessions, placement, fenced
//! leases and exec output.
//!
//! Machines connect out to this server (see [`calternal_machine::protocol`]); the
//! server never connects to them and never starts containers or VMs. Work for a
//! (space, account, thread) is a *lease*: the server picks a machine the account
//! may use with free capacity, records the lease with a *fence*, and sends the
//! assignment down that machine's long-poll. Every (re)assignment and every end
//! increases the fence; output carrying an older fence, or arriving from another
//! machine, is refused with 409, so a machine that lost its lease cannot deliver.
//!
//! Lease states: `waiting` (no machine with capacity: a visible state with a
//! queue position, not an error), `assigned` (sent), `running` (the machine
//! reported its sandbox up), `ended` (with a reason). A machine that has not
//! synced for `lease_ttl` loses its leases: they are fenced and marked interrupted.
//! Only an explicit resume allows placement again.
//!
//! Machine states: `online` (a sync is held or ended within `poll_hold + 5 s`),
//! `draining` (online, takes no new work), `offline`, `revoked` (sessions dropped
//! at once, a held poll is answered 403, leases ended).
//!
//! Who may use a machine: its owning account; a pool machine only accounts the
//! operator granted (`calternal machines pool-grant`). Space authority for a run
//! is checked by [`http`] on every call. Machine authority (codes, enrolment,
//! revocation, pool grants) commits in `auth.db` through the recovery barrier
//! ([`calternal_auth::machines`]); `machines.db` holds placement and cleanup state.
//!
//! Locking and cost: machines are installation-wide (not per-space), so one
//! registry mutex guards them. Every method except [`MachineService::sync`] and
//! [`MachineService::read_output`] takes it and may commit to SQLite while
//! holding it (one fsync, 1–5 ms, on placement, lease change and draining; an
//! authority change waits on the auth worker and its recovery publication, two
//! more fsyncs, held so the registry and authority never disagree); those
//! methods block and must run on a blocking task, which
//! [`http`] does. The two async methods only wait on a `Notify` and take the lock
//! on blocking tasks. Hot paths: a `sync` with nothing new reads and writes no
//! file; an `output` upload never touches SQLite. Placement scans waiting
//! leases × machines with indexed assignment counts. Exec output stays in memory, at most
//! [`Settings::output_buffer_bytes`] per exec until read, and finished execs are
//! dropped ten minutes after their exit; a server restart loses unread output.
//!
//! The seam for the jobs lane: [`MachineService::request_run`],
//! [`MachineService::exec`], [`MachineService::read_output`],
//! [`MachineService::run`] and [`MachineService::stop_run`] are the whole
//! caller-facing API; the HTTP endpoints are thin wrappers over them.

pub mod admin;
mod admission;
mod capacity;
mod compute;
mod group_storage;
pub(crate) mod resource_hooks;
pub use resource_hooks::ResourceHookBinding;
mod runtime;
pub use compute::ComputeLimits;
pub use runtime::RuntimeUsage;
/// Local deployment enrollment and the first-owner onboarding operation.
pub mod colocated;
pub mod db;
mod discards;
mod erasure;
mod erasure_queue;
mod execs;
pub mod http;
pub(crate) mod install;
mod microvm;
mod owners;
pub mod provider;
pub(crate) mod readiness;
mod sessions;
mod state;
mod workers;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use calternal_machine::protocol::{Capacity, Chunk, ChunkEvent, ExitInfo, Limits, Owner};
use calternal_types::{AccountId, SpaceId};
use serde::Serialize;
use tokio::sync::Notify;
use uuid::Uuid;

use calternal_types::Error;
use state::{Exec, ExecState, Lease, Machine, State};

/// Tunables. Defaults suit production; tests shorten the times.
#[derive(Debug, Clone)]
pub struct Settings {
    /// How long a lease survives without hearing from its machine.
    pub lease_ttl: Duration,
    /// Longest hold of a machine's `sync` request.
    pub poll_hold: Duration,
    /// Lifetime of an enrolment code.
    pub code_ttl: Duration,
    /// Output bytes held per exec before the machine is told to wait.
    pub output_buffer_bytes: usize,
    /// Limits given to every sandbox.
    pub limits: Limits,
    /// Sweep interval: offline detection, lease expiry, operator CLI changes.
    pub sweep_every: Duration,
    /// Ended leases are deleted from the database after this long.
    pub ended_lease_retention: Duration,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            lease_ttl: Duration::from_secs(90),
            poll_hold: Duration::from_secs(20),
            code_ttl: Duration::from_secs(3600),
            output_buffer_bytes: 1024 * 1024,
            limits: Limits {
                memory_bytes: Some(1 << 30),
                cpu_millis: None,
                pids: Some(256),
            },
            sweep_every: Duration::from_secs(1),
            ended_lease_retention: Duration::from_secs(7 * 86_400),
        }
    }
}

/// Where a lease stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseState {
    /// No machine with capacity yet.
    Waiting,
    /// Sent to a machine.
    Assigned,
    /// The machine reported the sandbox up.
    Running,
    /// Fenced after machine silence; only explicit resume returns it to placement.
    Interrupted,
    /// Internal worker teardown awaiting authenticated machine confirmation.
    Stopping,
    /// Over; see the reason.
    Ended,
}

impl LeaseState {
    /// Stored form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Assigned => "assigned",
            Self::Running => "running",
            Self::Ended => "ended",
            Self::Stopping => "stopping",
            Self::Interrupted => "interrupted",
        }
    }
    /// Parse the stored form; anything unknown reads as ended (fail closed).
    pub fn parse(s: &str) -> Self {
        match s {
            "waiting" => Self::Waiting,
            "assigned" => Self::Assigned,
            "running" => Self::Running,
            "interrupted" => Self::Interrupted,
            "stopping" => Self::Stopping,
            _ => Self::Ended,
        }
    }
}

/// What a caller sees of a run (a lease).
#[derive(Debug, Clone, Serialize)]
pub struct RunView {
    /// The lease id.
    pub lease_id: Uuid,
    /// Space it runs in.
    pub space_id: SpaceId,
    /// Its thread.
    pub thread_id: Uuid,
    /// State.
    pub state: LeaseState,
    /// Machine holding it, while assigned or running.
    pub machine_id: Option<Uuid>,
    /// Whether a saved turn can record a thread-scoped reference for this placement.
    #[serde(skip)]
    pub(crate) personal_machine: bool,
    /// Why it ended, or why it is waiting again.
    pub reason: Option<String>,
    /// While waiting: 1-based place among all waiting leases.
    pub queue_position: Option<usize>,
}

/// A machine as its owner sees it.
#[derive(Debug, Clone, Serialize)]
pub struct MachineView {
    /// Machine id.
    pub id: Uuid,
    /// Label.
    pub name: String,
    /// Operating system.
    pub os: String,
    /// CPU architecture.
    pub arch: String,
    /// Owner-selected default execution policy; pool machines always report false.
    pub direct: bool,
    /// Owner.
    pub owner: Owner,
    /// `online`, `draining`, `offline` or `revoked`.
    pub state: &'static str,
    /// Last reported capacity.
    pub capacity: Capacity,
    /// Leases it holds now.
    pub leases: usize,
}

/// Output of an exec after a cursor.
#[derive(Debug, Clone, Serialize)]
pub struct OutputView {
    /// Chunks with `seq > after`, oldest first.
    pub chunks: Vec<Chunk>,
    /// Pass as `after` next time.
    pub next: u64,
    /// Set once the exec has ended and its exit chunk is in `chunks`.
    pub exit: Option<ExitInfo>,
    /// Chunks before this sequence number were lost (server restart).
    pub lost_before: Option<u64>,
}

/// The server's machine registry. Share it as `Arc<MachineService>`.
pub struct MachineService {
    state: Mutex<State>,
    settings: Settings,
    worker_changes: tokio::sync::Notify,
    /// Bounded opportunistic title tasks, independent of foreground execution.
    pub(crate) title_slots: Arc<tokio::sync::Semaphore>,
    pub(super) lifecycle: Arc<std::sync::OnceLock<calternal_types::OwnerLifecycle>>,
}

const EXEC_RETENTION: Duration = Duration::from_secs(600);

fn sql(e: rusqlite::Error) -> Error {
    tracing::warn!(error = %e, "machines database");
    Error::internal("machine database failure")
}

/// Maps a machine-authority failure to a caller-safe error; the private detail is
/// logged here. An unknown owner account and a refused recovery publication both
/// leave authority unchanged.
pub(crate) fn authority_error(e: calternal_auth::Error) -> Error {
    tracing::warn!(error = %e, "machine authority");
    match e.code() {
        calternal_auth::ErrorCode::InvalidInput | calternal_auth::ErrorCode::NotFound => {
            Error::invalid_input("no such account or machine")
        }
        _ => Error::unavailable(
            "machine authority was not changed: unknown account, or the recovery log refused it",
        ),
    }
}

/// Run `f` on a blocking task; the registry lock and SQLite never block an async worker.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    crate::blocking::spawn(f)
        .await
        .map_err(|_| Error::internal("machine task failed"))?
}

impl MachineService {
    /// Attach the shared owner fence before accepting placements. No I/O.
    pub fn bind_lifecycle(&self, lifecycle: calternal_types::OwnerLifecycle) {
        let _ = self.lifecycle.set(lifecycle);
    }

    /// Open `<data>/system/machines.db`, then load machines and grants from
    /// `authority` and active leases. The host opens `data_dir` under the root
    /// guard and passes the authority handle of its recovery-protected `auth.db`.
    /// Blocks: schema checks, authority and placement reads, and recovery writes.
    pub fn open(
        data_dir: &Path,
        authority: calternal_auth::machines::Machines,
        settings: Settings,
    ) -> Result<Arc<Self>, Error> {
        let mut db = db::open(&data_dir.join("system/machines.db")).map_err(sql)?;
        let now = Instant::now();
        let draining = db::draining(&db).map_err(sql)?;
        let machines: HashMap<_, _> = authority
            .list()
            .map_err(authority_error)?
            .into_iter()
            .map(|r| {
                let drains = draining.contains(&r.id);
                (r.id, Machine::new(db::MachineRow::new(r, drains), now))
            })
            .collect();
        erasure::clear_orphan_drains(&mut db, &machines, draining)?;
        let lifecycle = Arc::new(std::sync::OnceLock::new());
        let mut st = State {
            lifecycle: lifecycle.clone(),
            pending_erasure_machines: erasure_queue::pending_machines(&db)?,
            readiness: readiness::Readiness::default(),
            pool_grants: authority
                .pool_grants()
                .map_err(authority_error)?
                .into_iter()
                .collect(),
            machines,
            sessions: HashMap::new(),
            challenges: HashMap::new(),
            leases: HashMap::new(),
            threads: HashMap::new(),
            execs: HashMap::new(),
            workers: HashMap::new(),
            worker_stops: HashMap::new(),
            next_order: 0,
            pool_load: HashMap::new(),
            compute_limits: compute::load(&db)?,
            runtime_windows: HashMap::new(),
            resource_hooks: resource_hooks::load(&db)?,
            resource_permits: HashMap::new(),
            next_runtime_retirement_ms: db::now_ms(),
            db,
            authority,
        };
        runtime::mark_missing(&st.db, None)?;
        for owner in st.compute_limits.keys().copied().collect::<Vec<_>>() {
            st.refresh_runtime(owner, db::now_ms())?;
        }
        for machine in st.machines.values() {
            st.readiness.insert(machine);
        }
        st.restore_erased_worker_stops()?;
        db::prune_ended(&st.db, settings.ended_lease_retention).map_err(sql)?;
        for mut row in db::active_leases(&st.db).map_err(sql)? {
            if row.background_title {
                row.fence += 1;
                row.state = "ended".into();
                db::put_lease(&st.db, &row).map_err(sql)?;
                st.leases.insert(
                    row.id,
                    Lease {
                        row,
                        state: LeaseState::Ended,
                        order: 0,
                        pool_diagnostic: false,
                        provider_login: false,
                    },
                );
                continue;
            }
            if row.worker {
                row.fence += 1;
                row.reason = Some("plugin workers ended by server restart".into());
                if let Some(machine) = row
                    .machine
                    .filter(|id| st.machines.get(id).is_some_and(|m| !m.row.revoked))
                {
                    row.state = "stopping".into();
                    st.worker_stops.insert(
                        row.id,
                        workers::WorkerStop::recovered(machine, Some(row.account)),
                    );
                    db::put_lease(&st.db, &row).map_err(sql)?;
                    st.take_slot(Some(machine), Some(row.account));
                    st.leases.insert(
                        row.id,
                        Lease {
                            row,
                            state: LeaseState::Stopping,
                            order: 0,
                            pool_diagnostic: false,
                            provider_login: false,
                        },
                    );
                } else {
                    row.state = "ended".into();
                    db::put_lease(&st.db, &row).map_err(sql)?;
                }
                continue;
            }
            st.next_order += 1;
            st.threads.insert(row.thread, row.id);
            let state = LeaseState::parse(&row.state);
            let order = st.next_order;
            let lease = Lease {
                row,
                state,
                order,
                pool_diagnostic: false,
                provider_login: false,
            };
            st.take_slot(lease.assigned_machine(), Some(lease.row.account));
            st.leases.insert(lease.row.id, lease);
        }
        Ok(Arc::new(Self {
            state: Mutex::new(st),
            settings,
            worker_changes: tokio::sync::Notify::new(),
            title_slots: Arc::new(tokio::sync::Semaphore::new(8)),
            lifecycle,
        }))
    }

    fn lock(&self) -> Result<MutexGuard<'_, State>, Error> {
        self.state
            .lock()
            .map_err(|_| Error::internal("machine registry poisoned"))
    }

    // ------------------------------------------------------------ callers

    /// Current view of a run owned by `account`; not found otherwise. Blocks
    /// (reads SQLite only for runs no longer in memory).
    pub fn run(&self, account: AccountId, lease: Uuid) -> Result<RunView, Error> {
        let st = self.lock()?;
        match st.leases.get(&lease) {
            Some(l) if l.row.account == account => Ok(st.view(lease)),
            Some(_) => Err(Error::not_found("run not found")),
            None => match db::lease(&st.db, lease).map_err(sql)? {
                Some(row) if row.account == account => Ok(RunView {
                    lease_id: row.id,
                    space_id: row.space,
                    thread_id: row.thread,
                    state: LeaseState::parse(&row.state),
                    machine_id: row.machine,
                    personal_machine: false,
                    reason: row.reason,
                    queue_position: None,
                }),
                _ => Err(Error::not_found("run not found")),
            },
        }
    }

    /// End a run; blocks (one commit). The machine is told to stop the sandbox
    /// and any later output is refused.
    pub fn stop_run(&self, account: AccountId, lease: Uuid) -> Result<RunView, Error> {
        let mut st = self.lock()?;
        let st = &mut *st;
        match st.leases.get(&lease) {
            Some(l) if l.row.account == account => {}
            _ => return Err(Error::not_found("run not found")),
        }
        st.end_lease(lease, "stopped by the caller", &self.settings);
        Ok(st.view(lease))
    }

    /// Output of an exec after `after`, waiting up to `wait` (at most 30 s) for
    /// something new.
    ///
    /// Reading with `after = n` releases chunks up to `n` from the buffer, which
    /// is what lets a machine that was told to wait continue: one reader per exec.
    pub async fn read_output(
        self: &Arc<Self>,
        account: AccountId,
        exec_id: Uuid,
        after: u64,
        wait: Duration,
    ) -> Result<OutputView, Error> {
        let deadline = tokio::time::Instant::now() + wait.min(Duration::from_secs(30));
        loop {
            let svc = self.clone();
            let step = blocking(move || svc.take_output(account, exec_id, after)).await?;
            let notify = match step {
                Ok(view) => return Ok(view),
                Err(notify) => notify,
            };
            if tokio::time::timeout_at(deadline, notify.notified())
                .await
                .is_err()
            {
                return Ok(OutputView {
                    chunks: Vec::new(),
                    next: after,
                    exit: None,
                    lost_before: None,
                });
            }
        }
    }

    /// Release chunks up to `after` and return the rest, or the exec's `Notify`
    /// when there is nothing yet.
    fn take_output(
        &self,
        account: AccountId,
        exec_id: Uuid,
        after: u64,
    ) -> Result<Result<OutputView, Arc<Notify>>, Error> {
        let mut st = self.lock()?;
        let exec = match st.execs.get_mut(&exec_id) {
            Some(e) if e.account == account => e,
            _ => return Err(Error::not_found("exec not found")),
        };
        while exec.chunks.front().is_some_and(|(c, _)| c.seq <= after) {
            exec.held -= exec.chunks.pop_front().expect("front").1;
        }
        if exec.chunks.is_empty() && exec.state != ExecState::Done {
            return Ok(Err(exec.notify.clone()));
        }
        let chunks: Vec<Chunk> = exec.chunks.iter().map(|(c, _)| c.clone()).collect();
        let exit = chunks.iter().find_map(|c| match &c.event {
            ChunkEvent::Exit(e) => Some(e.clone()),
            ChunkEvent::Data { .. } => None,
        });
        Ok(Ok(OutputView {
            next: chunks.last().map_or(after.max(exec.last_seq), |c| c.seq),
            exit,
            lost_before: exec.lost_before,
            chunks,
        }))
    }

    /// The space of a publicly addressable agent run; internal worker leases
    /// return None. Cost: registry lookup or one SQLite read after retention.
    pub fn run_space(&self, lease: Uuid) -> Result<Option<SpaceId>, Error> {
        let st = self.lock()?;
        Ok(match st.leases.get(&lease) {
            Some(l) => (!l.row.worker && !l.provider_login && !l.row.background_title)
                .then_some(l.row.space),
            None => db::lease(&st.db, lease)
                .map_err(sql)?
                .filter(|r| !r.worker && !r.background_title)
                .map(|r| r.space),
        })
    }

    /// The space of a public agent exec; internal worker streams stay hidden.
    /// Cost: one registry lock and two memory lookups, no I/O.
    pub fn exec_space(&self, exec: Uuid) -> Result<Option<SpaceId>, Error> {
        let st = self.lock()?;
        Ok(st
            .execs
            .get(&exec)
            .and_then(|e| st.leases.get(&e.lease))
            .filter(|l| !l.row.worker && !l.provider_login && !l.row.background_title)
            .map(|l| l.row.space))
    }

    fn check_owner(st: &State, account: AccountId, machine: Uuid) -> Result<(), Error> {
        match st.machines.get(&machine) {
            Some(m) if m.row.owner == (Owner::Account { id: account }) => Ok(()),
            _ => Err(Error::not_found("machine not found")),
        }
    }

    /// Revoke one of `account`'s machines now; blocks (one commit). Sessions are
    /// dropped, a held poll is answered 403 and its leases end.
    pub fn revoke(&self, account: AccountId, machine: Uuid) -> Result<(), Error> {
        let mut st = self.lock()?;
        let st = &mut *st;
        Self::check_owner(st, account, machine)?;
        st.authority.revoke(machine).map_err(authority_error)?;
        let result = st.apply_revocation(machine, &self.settings);
        self.worker_changes.notify_waiters();
        result
    }

    /// Set or clear draining on one of `account`'s machines; blocks (one commit).
    pub fn set_draining(
        &self,
        account: AccountId,
        machine: Uuid,
        draining: bool,
    ) -> Result<(), Error> {
        let mut st = self.lock()?;
        let st = &mut *st;
        Self::check_owner(st, account, machine)?;
        db::set_draining(&st.db, machine, draining).map_err(sql)?;
        if let Some(m) = st.machines.get_mut(&machine) {
            m.row.draining = draining;
            let owner = m.row.owner;
            st.readiness_changed(owner);
        }
        st.place_waiting(&self.settings);
        Ok(())
    }

    // ---------------------------------------------------------------- sweep

    /// Run [`MachineService::sweep`] on a blocking task every `sweep_every`
    /// until the service is dropped.
    pub fn spawn_sweeper(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let weak = Arc::downgrade(self);
        let every = self.settings.sweep_every;
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(every);
            loop {
                tick.tick().await;
                let Some(svc) = weak.upgrade() else { return };
                if let Err(e) = blocking(move || svc.sweep()).await {
                    tracing::warn!(error = %e, "machine sweep failed");
                }
            }
        })
    }

    /// Mark silent machines offline, fence and interrupt their leases after
    /// `lease_ttl`, drop finished exec buffers after ten minutes and ended lease
    /// rows after `ended_lease_retention`. Operator changes arrive through the
    /// service itself (the admin socket), never behind it. Blocks: one indexed
    /// delete per tick, which writes only when a lease row has expired.
    pub fn sweep(&self) -> Result<(), Error> {
        let mut st = self.lock()?;
        let st = &mut *st;
        db::prune_ended(&st.db, self.settings.ended_lease_retention).map_err(sql)?;
        let now_ms = db::now_ms();
        if now_ms >= st.next_runtime_retirement_ms {
            runtime::retire(&st.db, now_ms)?;
            st.next_runtime_retirement_ms = now_ms.saturating_add(3_600_000);
        }
        let rollover: Vec<_> = st
            .runtime_windows
            .iter()
            .filter_map(|(owner, window)| (now_ms >= window.ends_ms).then_some(*owner))
            .collect();
        for owner in rollover {
            st.refresh_runtime(owner, now_ms)?;
        }
        let now = Instant::now();
        let offline_after = self.settings.poll_hold + Duration::from_secs(5);
        let mut expired = Vec::new();
        let mut disconnected = Vec::new();
        let mut expired_checks = Vec::new();
        for m in st.machines.values_mut() {
            let owner = m.row.owner;
            expired_checks.extend(readiness::expire(m).map(|account| (owner, account)));
            m.checks.retain(|_, checks| !checks.is_empty());
            if m.live.as_ref().is_some_and(|l| l.polls > 0) {
                continue;
            }
            let silent = now.duration_since(m.last_seen);
            // A machine whose leases expire is offline too, so placement cannot
            // hand the fenced lease straight back to it.
            if silent >= offline_after.min(self.settings.lease_ttl)
                && let Some(live) = m.live.take()
            {
                st.sessions.remove(&live.session);
                disconnected.push((m.row.id, m.row.owner));
            }
            if silent >= self.settings.lease_ttl {
                expired.push(m.row.id);
            }
        }
        for (machine, owner) in disconnected {
            st.readiness_changed(owner);
            for scope in runtime::mark_missing(&st.db, Some(machine))? {
                st.refresh_runtime(scope, now_ms)?;
            }
        }
        for (owner, account) in expired_checks {
            if owner != Owner::Pool || st.pool_grants.contains(&account) {
                st.readiness.change(account);
            }
        }
        st.readiness.prune();
        for machine in expired {
            let leases: Vec<Uuid> = st
                .leases
                .values()
                .filter(|l| {
                    l.row.machine == Some(machine)
                        && !matches!(l.state, LeaseState::Ended | LeaseState::Stopping)
                })
                .map(|l| l.row.id)
                .collect();
            for lease in leases {
                if st.leases[&lease].row.worker {
                    st.end_lease(lease, "the machine stopped responding", &self.settings);
                } else {
                    tracing::info!(%lease, %machine, "lease expired; explicit resume required");
                    st.interrupt(lease, "the machine stopped responding");
                }
            }
        }
        st.execs.retain(|_, e| {
            e.finished
                .is_none_or(|t| now.duration_since(t) < EXEC_RETENTION)
        });
        st.place_waiting(&self.settings);
        Ok(())
    }
}
