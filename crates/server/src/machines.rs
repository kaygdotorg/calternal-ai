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
//! synced for `lease_ttl` loses its leases: they are fenced and placed again,
//! preferring the same machine when it returns (its frozen sandbox is resumed).
//!
//! Machine states: `online` (a sync is held or ended within `poll_hold + 5 s`),
//! `draining` (online, takes no new work), `offline`, `revoked` (sessions dropped
//! at once, a held poll is answered 403, leases ended).
//!
//! Who may use a machine: its owning account; a pool machine only accounts the
//! operator granted (`calternal machines pool-grant`). Space authority for a run
//! is checked by [`http`] on every call.
//!
//! Locking and cost: machines are installation-wide (not per-space), so one
//! registry mutex guards them. Every method except [`MachineService::sync`] and
//! [`MachineService::read_output`] takes it and may commit to SQLite while
//! holding it (one fsync, 1–5 ms, only on enrolment, placement, lease change and
//! revocation); those methods block and must run on a blocking task, which
//! [`http`] does. The two async methods only wait on a `Notify` and take the lock
//! on blocking tasks. Hot paths: a `sync` with nothing new reads and writes no
//! file; an `output` upload never touches SQLite. Placement scans waiting
//! leases × machines. Exec output stays in memory, at most
//! [`Settings::output_buffer_bytes`] per exec until read, and finished execs are
//! dropped ten minutes after their exit; a server restart loses unread output.
//!
//! The seam for the jobs lane: [`MachineService::request_run`],
//! [`MachineService::exec`], [`MachineService::read_output`],
//! [`MachineService::run`] and [`MachineService::stop_run`] are the whole
//! caller-facing API; the HTTP endpoints are thin wrappers over them.


pub mod admin;
pub mod db;
pub mod http;
mod sessions;
mod state;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use calternal_types::{AccountId, SpaceId};
use calternal_machine::protocol::{Capacity, Chunk, ChunkEvent, ExitInfo, Limits, Owner};
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
        }
    }
    /// Parse the stored form; anything unknown reads as ended (fail closed).
    pub fn parse(s: &str) -> Self {
        match s {
            "waiting" => Self::Waiting,
            "assigned" => Self::Assigned,
            "running" => Self::Running,
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
}

const EXEC_RETENTION: Duration = Duration::from_secs(600);

fn sql(e: rusqlite::Error) -> Error {
    tracing::warn!(error = %e, "machines database");
    Error::internal("machine database failure")
}

/// Run `f` on a blocking task; the registry lock and SQLite never block an async worker.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| Error::internal("machine task failed"))?
}

impl MachineService {
    /// Open `<data>/system/machines.db` and load machines, grants and active leases.
    /// Blocks: one schema check and three reads.
    pub fn open(data_dir: &Path, settings: Settings) -> Result<Arc<Self>, Error> {
        let db = db::open(&data_dir.join("system/machines.db")).map_err(sql)?;
        let now = Instant::now();
        let machines = db::machines(&db)
            .map_err(sql)?
            .into_iter()
            .map(|row| (row.id, Machine::new(row, now)))
            .collect();
        let mut st = State {
            data_version: db::data_version(&db).map_err(sql)?,
            pool_grants: db::pool_grants(&db).map_err(sql)?.into_iter().collect(),
            machines,
            sessions: HashMap::new(),
            challenges: HashMap::new(),
            leases: HashMap::new(),
            threads: HashMap::new(),
            execs: HashMap::new(),
            next_order: 0,
            db,
        };
        for row in db::active_leases(&st.db).map_err(sql)? {
            st.next_order += 1;
            st.threads.insert(row.thread, row.id);
            let state = LeaseState::parse(&row.state);
            let order = st.next_order;
            st.leases.insert(row.id, Lease { row, state, order });
        }
        Ok(Arc::new(Self {
            state: Mutex::new(st),
            settings,
        }))
    }

    fn lock(&self) -> Result<MutexGuard<'_, State>, Error> {
        self.state
            .lock()
            .map_err(|_| Error::internal("machine registry poisoned"))
    }

    // ------------------------------------------------------------ callers

    /// Ask to run work for (space, account, thread); blocks (one commit). Returns
    /// the existing run when the thread already has an active lease for this
    /// account and space. The caller must have checked the account's authority.
    pub fn request_run(
        &self,
        space: SpaceId,
        account: AccountId,
        thread: Uuid,
        image: Option<String>,
    ) -> Result<RunView, Error> {
        let mut st = self.lock()?;
        let st = &mut *st;
        if let Some(id) = st.threads.get(&thread).copied() {
            let l = &st.leases[&id];
            if l.row.account != account || l.row.space != space {
                return Err(Error::conflict("thread is running for another context"));
            }
            return Ok(st.view(id));
        }
        let row = db::LeaseRow {
            id: Uuid::new_v4(),
            space,
            account,
            thread,
            image,
            machine: None,
            last_machine: None,
            fence: 0,
            state: LeaseState::Waiting.as_str().into(),
            reason: None,
            created_ms: db::now_ms(),
        };
        db::put_lease(&st.db, &row).map_err(sql)?;
        st.next_order += 1;
        let id = row.id;
        st.threads.insert(thread, id);
        let order = st.next_order;
        st.leases.insert(
            id,
            Lease {
                row,
                state: LeaseState::Waiting,
                order,
            },
        );
        st.place_waiting(&self.settings);
        Ok(st.view(id))
    }

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

    /// Queue a command for a run; it is sent as soon as the run is assigned.
    /// Blocks briefly (no file I/O).
    ///
    /// `env` is plain configuration, never a credential. Secrets seam (not wired
    /// yet): the exec input gains `secrets: [{variable, handle}]`; the handle is
    /// resolved for the run's owner when the exec is queued, and the value is
    /// released by `calternal_secrets::SecretStore::use_value` with
    /// `Consumer::SandboxEnv { thread, variable }` on a blocking task right
    /// before the `Exec` message is pushed, so no plaintext waits in the queue
    /// or the database and every release is audited per thread.
    pub fn exec(
        &self,
        account: AccountId,
        lease: Uuid,
        argv: Vec<String>,
        env: Vec<(String, String)>,
    ) -> Result<Uuid, Error> {
        if argv.is_empty() || argv.len() > 4096 || argv.iter().any(|a| a.len() > 65_536) {
            return Err(Error::invalid_input(
                "argv must have 1 to 4096 arguments of at most 64 KiB",
            ));
        }
        let mut st = self.lock()?;
        let st = &mut *st;
        match st.leases.get(&lease) {
            Some(l) if l.row.account == account && l.state != LeaseState::Ended => {}
            Some(l) if l.row.account == account => {
                return Err(Error::conflict("run has ended"));
            }
            _ => return Err(Error::not_found("run not found")),
        }
        let id = Uuid::new_v4();
        st.execs
            .insert(id, Exec::new(lease, account, argv, env, ExecState::Queued));
        st.send_execs(lease);
        Ok(id)
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

    /// The space of a run, for re-checking the caller's authority. Blocks.
    pub fn run_space(&self, lease: Uuid) -> Result<Option<SpaceId>, Error> {
        let st = self.lock()?;
        Ok(match st.leases.get(&lease) {
            Some(l) => Some(l.row.space),
            None => db::lease(&st.db, lease).map_err(sql)?.map(|r| r.space),
        })
    }

    /// The space of an exec's run, for re-checking the caller's authority.
    pub fn exec_space(&self, exec: Uuid) -> Result<Option<SpaceId>, Error> {
        let st = self.lock()?;
        Ok(st
            .execs
            .get(&exec)
            .and_then(|e| st.leases.get(&e.lease))
            .map(|l| l.row.space))
    }

    // --------------------------------------------------------------- owners

    /// Issue a one-time enrolment code for `owner`; blocks (one commit).
    /// Returns the code and its expiry in Unix milliseconds.
    pub fn issue_code(&self, owner: Owner) -> Result<(String, i64), Error> {
        let mut st = self.lock()?;
        let issued = db::new_code(&st.db, owner, self.settings.code_ttl)?;
        st.data_version = db::data_version(&st.db).map_err(sql)?;
        Ok(issued)
    }

    /// Machines owned by `account`. Blocks briefly (no file I/O).
    pub fn machines_of(&self, account: AccountId) -> Result<Vec<MachineView>, Error> {
        let st = self.lock()?;
        let owner = Owner::Account { id: account };
        Ok(st
            .machines
            .values()
            .filter(|m| m.row.owner == owner)
            .map(|m| st.machine_view(m, &self.settings))
            .collect())
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
        db::revoke(&st.db, machine).map_err(sql)?;
        st.data_version = db::data_version(&st.db).map_err(sql)?;
        st.apply_revocation(machine, &self.settings);
        Ok(())
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
        st.data_version = db::data_version(&st.db).map_err(sql)?;
        if let Some(m) = st.machines.get_mut(&machine) {
            m.row.draining = draining;
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

    /// Pick up operator CLI changes, mark silent machines offline, fence and
    /// re-place their leases after `lease_ttl`, drop finished exec buffers after
    /// ten minutes and ended lease rows after `ended_lease_retention`. Blocks: one
    /// `PRAGMA data_version` read per tick; writes only when something changed.
    pub fn sweep(&self) -> Result<(), Error> {
        let mut st = self.lock()?;
        let st = &mut *st;
        let version = db::data_version(&st.db).map_err(sql)?;
        if version != st.data_version {
            st.data_version = version;
            st.reload(&self.settings)?;
            db::prune_ended(&st.db, self.settings.ended_lease_retention).map_err(sql)?;
        }
        let now = Instant::now();
        let offline_after = self.settings.poll_hold + Duration::from_secs(5);
        let mut expired = Vec::new();
        for m in st.machines.values_mut() {
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
            }
            if silent >= self.settings.lease_ttl {
                expired.push(m.row.id);
            }
        }
        for machine in expired {
            let leases: Vec<Uuid> = st
                .leases
                .values()
                .filter(|l| l.row.machine == Some(machine) && l.state != LeaseState::Ended)
                .map(|l| l.row.id)
                .collect();
            for lease in leases {
                tracing::info!(%lease, %machine, "lease expired; placing it again");
                st.requeue(lease, "the machine stopped responding");
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
