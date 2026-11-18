//! Long-lived worker processes speaking protocol v2, one per (plugin, space).
//!
//! Workers use the machine sandbox and exec interfaces; the server never starts
//! a container. The default is an enrolled machine, with a read-only code mount,
//! no network, 256 MiB, one CPU and 64 PIDs. DirectBackend is an explicit
//! development/test adapter with no OS isolation. Protocol v2 is unchanged.
//!
//! Bounds: enrolled sandbox capacity (eight workers in local fixtures), 32
//! concurrent invocations and 32 outgoing frames per
//! worker, 256 KiB frames, 60 seconds idle, 3 seconds per interactive invocation,
//! 128 MiB V8 heap, retirement at 4096 calls. A fault kills the worker; current
//! callers receive a safe error and the next call starts a fresh worker without
//! replaying an uncertain operation.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use calternal_core::ManifestCapability;
use calternal_plugin_host::{
    Authority, Frame, HostBroker, MAX_FRAME_BYTES, MAX_QUEUE, Message, WorkerSession, decode_frame,
    encode_frame,
};
use calternal_types::{AccountId, SpaceId};
use serde_json::Value;
use tokio::sync::oneshot;

use super::{InvocationHost, Plugin};
use calternal_types::Error;

/// Interactive execution budget after admission. The first call to a fresh
/// enrolled worker also allows time to start Podman and Node on its machine.
pub const WORKER_TIMEOUT: Duration = Duration::from_secs(3);
// Round up the measured 2.184 s dev-01 p99 for a fresh enrolled worker's first
// call. Add that allowance to the interactive execution budget.
const REMOTE_WORKER_START_ALLOWANCE: Duration = Duration::from_secs(3);
/// Controller admission deadline covering prior teardown and capacity. Podman
/// teardown alone takes about two seconds on the development VM; it is distinct
/// from the invocation deadline, which also bounds launch after capacity is held.
/// An expired startup is detached and cleaned up.
pub const WORKER_START_TIMEOUT: Duration = Duration::from_secs(10);
/// A worker with no in-flight invocation for this long exits.
pub const WORKER_IDLE: Duration = Duration::from_secs(60);
/// Default local fixture slots, including cold starts and unconfirmed teardown.
pub const MAX_WORKERS: usize = 8;
/// Invocations after which a worker is retired and replaced (library replay bound: 4096).
pub const MAX_LIFETIME_INVOCATIONS: u32 = 4096;
/// V8 old-generation heap cap per worker, in MiB.
pub const WORKER_HEAP_MB: u32 = 128;
const MIN_NODE: (u32, u32) = (22, 13);

/// A verified Node executable for explicit development and fixture adapters.
#[derive(Debug, Clone)]
pub struct NodeRuntime {
    /// Canonical absolute executable path.
    pub path: PathBuf,
    /// `node --version` output, such as `v22.13.0`.
    pub version: String,
}

/// Resolve and verify an absolute Node executable (>= 22.13 for `--permission`).
/// Cost: one blocking path lookup and one `node --version` process.
pub async fn resolve_node(explicit: Option<&Path>) -> Result<NodeRuntime, String> {
    let explicit = explicit.map(Path::to_path_buf);
    let abs = crate::blocking::spawn(move || {
        let candidate = match explicit {
            Some(path) => path,
            None => {
                let path_var = std::env::var_os("PATH").ok_or("PATH not set; pass --node")?;
                std::env::split_paths(&path_var)
                    .map(|dir| dir.join("node"))
                    .find(|path| path.is_file())
                    .ok_or("node not found on PATH; pass --node")?
            }
        };
        std::fs::canonicalize(&candidate).map_err(|e| format!("node: {e}"))
    })
    .await
    .map_err(|_| "Node lookup task failed")??;
    let out = tokio::process::Command::new(&abs)
        .arg("--version")
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("node --version: {e}"))?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let nums: Vec<u32> = v
        .trim_start_matches('v')
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    if nums.len() < 2 || (nums[0], nums[1]) < MIN_NODE {
        return Err(format!(
            "node {v} is too old; need >= {}.{}",
            MIN_NODE.0, MIN_NODE.1
        ));
    }
    Ok(NodeRuntime {
        path: abs,
        version: v,
    })
}

/// One operation call routed to a worker.
pub struct Call {
    /// Declared operation ID.
    pub operation: String,
    /// Plugin input; the session validates it against the declared input schema.
    pub input: Value,
    /// Acting account UUIDv7 (correlation only; the host re-derives authority).
    pub account: AccountId,
    /// Trusted broker carrying this invocation's authority.
    pub host: Arc<dyn InvocationHost>,
    /// Execution budget: [`WORKER_TIMEOUT`] for operations, longer for job runs.
    /// A missed deadline stops the whole worker.
    pub timeout: Duration,
    /// Cancels this invocation from outside, as `jobs.cancel` does for a job's run.
    pub cancel: Option<Arc<CancelFlag>>,
    /// Job the host admitted for this run invocation; it may stream chunks for it.
    pub job: Option<String>,
}

/// Cancellation handle for one invocation. Cancelling invalidates the invocation in
/// the worker session (late output and effects are refused), tells the worker, and
/// answers the caller with [`Outcome::Cancelled`].
pub struct CancelFlag {
    cancelled: AtomicBool,
    wake: Mutex<Option<SyncSender<Event>>>,
}

impl CancelFlag {
    /// A fresh, uncancelled handle. Cost: one allocation.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            cancelled: AtomicBool::new(false),
            wake: Mutex::new(None),
        })
    }

    /// Whether [`Self::cancel`] was called. Cost: one atomic load.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Cancel; idempotent. Cost: one atomic store and at most one channel send.
    pub fn cancel(self: &Arc<Self>) {
        self.cancelled.store(true, Ordering::Release);
        let wake = self.wake.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(events) = wake {
            let _ = events.try_send(Event::Cancel(self.clone()));
        }
    }

    fn bind(&self, events: &SyncSender<Event>) {
        *self.wake.lock().unwrap_or_else(|p| p.into_inner()) = Some(events.clone());
    }
}

/// Outcome of a worker call before the dispatcher's own delivery checks.
#[derive(Debug)]
pub enum Outcome {
    /// The worker completed and every boundary check passed.
    Complete(Value),
    /// A boundary refused the invocation: current authority failed, or the worker
    /// requested an undeclared or forbidden host call. Nothing further is delivered.
    Refused,
    /// The input did not match the declared input schema.
    InvalidInput,
    /// The worker failed, broke protocol, timed out or could not run.
    Failed(Error),
    /// The invocation was cancelled through its [`CancelFlag`].
    Cancelled,
}

/// Input to a worker's session thread.
enum Event {
    /// Admit one invocation.
    Invoke(Box<Pending>),
    /// Cancel the invocation holding this flag.
    Cancel(Arc<CancelFlag>),
    Line(Vec<u8>),
    Overflow,
    Closed,
    Shutdown,
}

struct Pending {
    call: Call,
    reply: oneshot::Sender<Outcome>,
    deadline: Instant,
}

struct Active {
    host: Arc<dyn InvocationHost>,
    cancel: Option<Arc<CancelFlag>>,
    reply: oneshot::Sender<Outcome>,
    deadline: Instant,
}

/// Handle to one live worker's session thread.
struct Worker {
    events: SyncSender<Event>,
    alive: Arc<AtomicBool>,
    busy: Arc<AtomicUsize>,
    #[cfg(feature = "dev-plugin-drafts")]
    retire_after_result: Option<WorkerKey>,
}

impl Worker {
    fn usable(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
}

/// A worker handle that is dropped (evicted, replaced, or started for a caller
/// that was cancelled) stops its process; a second Shutdown is ignored.
impl Drop for Worker {
    fn drop(&mut self) {
        tracing::debug!("worker handle retired");
        self.alive.store(false, Ordering::Release);
        let _ = self.events.try_send(Event::Shutdown);
    }
}

/// Per-key admission serialization and physical worker lifetime. The latter
/// remains held through teardown, including a cancelled caller's cold start.
struct StartKey {
    admission: Arc<tokio::sync::Mutex<()>>,
    worker: Arc<tokio::sync::Semaphore>,
}

type WorkerKey = (String, SpaceId, u32);

/// Bounded pool of workers keyed by plugin ID, space and package revision.
pub struct WorkerPool {
    closing: AtomicBool,
    node: Option<NodeRuntime>,
    backend: Option<WorkerBackend>,
    workers: Mutex<HashMap<WorkerKey, Worker>>,
    in_flight: Arc<AtomicUsize>,
    starts: Mutex<HashMap<WorkerKey, std::sync::Weak<StartKey>>>,
    fixture_limit: Option<usize>,
    // The enrolled backend uses this only to join teardown on shutdown.
    permits: Arc<tokio::sync::Semaphore>,
}

#[derive(Clone)]
enum WorkerBackend {
    Local(Arc<dyn calternal_machine::sandbox::Backend>),
    Enrolled(Arc<crate::machines::MachineService>),
}

impl WorkerBackend {
    fn name(&self) -> &'static str {
        match self {
            Self::Local(backend) => backend.name(),
            Self::Enrolled(_) => "machine",
        }
    }
}

impl WorkerPool {
    /// Explicit development/fixture pool; `None` refuses calls as unavailable.
    /// The direct adapter has no OS isolation; tests may select Podman with
    /// `CALTERNAL_WORKER_TEST_BACKEND=podman`. Cost: configuration allocations only.
    pub fn new(node: Option<NodeRuntime>) -> Self {
        Self::with_capacity(node, MAX_WORKERS)
    }

    /// Development/fixture pool with a custom worker-slot cap (at least one).
    /// Same adapter selection as [`Self::new`]; costs configuration allocations only.
    pub fn with_capacity(node: Option<NodeRuntime>, max_workers: usize) -> Self {
        let root = std::env::temp_dir().join(format!("calternal-worker-{}", uuid::Uuid::new_v4()));
        Self::with_local_root(node, max_workers, root)
    }

    /// Configure the development adapter with a sandbox root owned by the caller.
    /// Construction performs no I/O. Use a separate root for each fixture.
    pub fn with_local_root(node: Option<NodeRuntime>, max_workers: usize, root: PathBuf) -> Self {
        Self {
            closing: AtomicBool::new(false),
            backend: node.as_ref().map(|_| {
                let backend =
                    if std::env::var("CALTERNAL_WORKER_TEST_BACKEND").as_deref() == Ok("podman") {
                        Arc::new(calternal_machine::sandbox::podman::PodmanBackend::new(
                            "podman",
                            root,
                            calternal_machine::sandbox::worker::IMAGE,
                        )) as Arc<dyn calternal_machine::sandbox::Backend>
                    } else {
                        Arc::new(calternal_machine::sandbox::direct::DirectBackend::new(root))
                            as Arc<dyn calternal_machine::sandbox::Backend>
                    };
                WorkerBackend::Local(backend)
            }),
            node,
            workers: Mutex::new(HashMap::new()),
            in_flight: Arc::new(AtomicUsize::new(0)),
            starts: Mutex::new(HashMap::new()),
            fixture_limit: Some(max_workers.max(1)),
            permits: Arc::new(tokio::sync::Semaphore::new(max_workers.max(1))),
        }
    }

    /// Route workers through enrolled machines and use their fenced leases for
    /// capacity. Does not inspect a local runtime or engine. No I/O.
    pub fn machines(service: Arc<crate::machines::MachineService>) -> Self {
        let mut pool = Self::new(None);
        pool.backend = Some(WorkerBackend::Enrolled(service));
        pool.fixture_limit = None;
        pool.permits = Arc::new(tokio::sync::Semaphore::new(
            tokio::sync::Semaphore::MAX_PERMITS,
        ));
        pool
    }

    /// Wake and retire every worker while the runtime and machine link still
    /// work. Stop accepting new workers and allow at most one startup budget
    /// for their outstanding teardown confirmations.
    pub async fn shutdown(&self) {
        self.closing.store(true, Ordering::Release);
        self.permits.close();
        self.workers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
        let drained = tokio::time::timeout(WORKER_START_TIMEOUT, async {
            let initial = self
                .fixture_limit
                .unwrap_or(tokio::sync::Semaphore::MAX_PERMITS);
            while self.permits.available_permits() != initial {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await;
        if drained.is_err() {
            tracing::warn!("worker teardown was not confirmed before shutdown");
        }
    }

    /// Invocations admitted to a worker and not yet answered. Cost: one atomic load.
    pub fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::Acquire)
    }

    /// Usable worker sessions; teardown retains a separate admission permit.
    /// Cost: pool lock and O(workers) scan.
    pub fn live(&self) -> usize {
        self.workers
            .lock()
            .map(|w| w.values().filter(|w| w.usable()).count())
            .unwrap_or(0)
    }
}

mod admission;

mod session;
use session::Thread;

mod transport;
use transport::{Process, spawn};
