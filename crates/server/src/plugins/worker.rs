//! Long-lived worker processes speaking protocol v2, one per (plugin, space).
//!
//! Workers use the machine sandbox and exec interfaces; the server never starts
//! a container. The default is an enrolled machine, with a read-only code mount,
//! no network, 256 MiB, one CPU and 64 PIDs. DirectBackend is an explicit
//! development/test adapter with no OS isolation. Protocol v2 is unchanged.
//!
//! Bounds: eight workers, 32 concurrent invocations and 32 outgoing frames per
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

/// Deadline after resource admission, including cold launch and queued execution.
pub const WORKER_TIMEOUT: Duration = Duration::from_secs(3);
/// Controller admission deadline covering prior teardown and capacity. Podman
/// teardown alone takes about two seconds on the development VM; it is distinct
/// from the invocation deadline, which also bounds launch after capacity is held.
/// An expired startup is detached and cleaned up.
pub const WORKER_START_TIMEOUT: Duration = Duration::from_secs(10);
/// A worker with no in-flight invocation for this long exits.
pub const WORKER_IDLE: Duration = Duration::from_secs(60);
/// Maximum worker slots, including cold starts and unconfirmed teardown.
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
    let abs = tokio::task::spawn_blocking(move || {
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
    /// Deadline from admission to delivery: [`WORKER_TIMEOUT`] for operations, longer
    /// for job runs. A missed deadline stops the whole worker.
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

/// Bounded pool of long-lived workers keyed by (plugin ID, space key).
pub struct WorkerPool {
    node: Option<NodeRuntime>,
    backend: Option<Arc<dyn calternal_machine::sandbox::Backend>>,
    workers: Mutex<HashMap<(String, SpaceId), Worker>>,
    in_flight: Arc<AtomicUsize>,
    starts: Mutex<HashMap<(String, SpaceId), std::sync::Weak<StartKey>>>,
    max_workers: usize,
    permits: Arc<tokio::sync::Semaphore>,
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
        Self {
            backend: node.as_ref().map(|_| {
                let root =
                    std::env::temp_dir().join(format!("calternal-worker-{}", uuid::Uuid::new_v4()));
                if std::env::var("CALTERNAL_WORKER_TEST_BACKEND").as_deref() == Ok("podman") {
                    Arc::new(calternal_machine::sandbox::podman::PodmanBackend::new(
                        "podman",
                        root,
                        calternal_machine::sandbox::worker::IMAGE,
                    )) as Arc<dyn calternal_machine::sandbox::Backend>
                } else {
                    Arc::new(calternal_machine::sandbox::direct::DirectBackend::new(root))
                        as Arc<dyn calternal_machine::sandbox::Backend>
                }
            }),
            node,
            workers: Mutex::new(HashMap::new()),
            in_flight: Arc::new(AtomicUsize::new(0)),
            starts: Mutex::new(HashMap::new()),
            max_workers: max_workers.max(1),
            permits: Arc::new(tokio::sync::Semaphore::new(max_workers.max(1))),
        }
    }

    /// Route workers through enrolled machines. No local runtime or engine is
    /// inspected or launched. Cost: one shared backend allocation, no I/O.
    pub fn machines(service: Arc<crate::machines::MachineService>) -> Self {
        let mut pool = Self::new(None);
        pool.backend = Some(Arc::new(super::machine_worker::MachineBackend(service)));
        pool
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

    /// Run one operation on the (plugin, space) worker, starting it if needed.
    /// `space` selects the process and is the space its session is pinned to.
    /// Cost: the pool lock for a lookup (never held across I/O); a cold start hashes
    /// the bundle on a blocking task and awaits sandbox startup with a reserved pool
    /// slot; then one channel round trip plus the worker's own frames and effects.
    pub async fn invoke(&self, plugin: &Arc<Plugin>, space: SpaceId, call: Call) -> Outcome {
        let Some(backend) = &self.backend else {
            return Outcome::Failed(Error::unavailable("worker execution unavailable"));
        };
        if let Some(why) = &plugin.unavailable {
            return Outcome::Failed(Error::unavailable(format!("plugin unavailable: {why}")));
        }
        let Some(bundle) = &plugin.worker else {
            return Outcome::Failed(Error::unavailable("plugin has no worker"));
        };
        let (reply, answer) = oneshot::channel();
        let deadline = Instant::now() + call.timeout;
        let mut pending = Box::new(Pending {
            call,
            reply,
            deadline,
        });
        // A worker that exited idle between lookup and send drops its receiver;
        // the send then fails and one fresh worker is started.
        let key = (plugin.manifest.id.clone(), space);
        // A per-key admission lock prevents duplicate cold starts. The small map
        // lock is released before waiting or I/O; weak entries are pruned on use.
        let gate = {
            let mut starts = self.starts.lock().unwrap_or_else(|p| p.into_inner());
            starts.retain(|_, weak| weak.strong_count() > 0);
            if let Some(gate) = starts.get(&key).and_then(std::sync::Weak::upgrade) {
                gate
            } else {
                if starts.len() >= self.max_workers.saturating_mul(2) {
                    return Outcome::Failed(
                        Error::unavailable("worker admission capacity exhausted")
                            .with_details(serde_json::json!({"reason":"worker_capacity"})),
                    );
                }
                let gate = Arc::new(StartKey {
                    admission: Arc::new(tokio::sync::Mutex::new(())),
                    worker: Arc::new(tokio::sync::Semaphore::new(1)),
                });
                starts.insert(key.clone(), Arc::downgrade(&gate));
                gate
            }
        };
        let startup_deadline = tokio::time::Instant::now() + WORKER_START_TIMEOUT;
        let mut admission =
            match tokio::time::timeout_at(startup_deadline, gate.admission.clone().lock_owned())
                .await
            {
                Ok(guard) => guard,
                Err(_) => {
                    return Outcome::Failed(Error::unavailable(
                        "worker admission timed out; retry shortly",
                    ));
                }
            };
        pending.deadline = Instant::now() + pending.call.timeout;
        for _ in 0..2 {
            let start = {
                let mut workers = match self.workers.lock() {
                    Ok(workers) => workers,
                    Err(_) => return Outcome::Failed(Error::internal("worker pool poisoned")),
                };
                if workers.get(&key).is_some_and(Worker::usable) {
                    false
                } else {
                    workers.remove(&key);
                    if let Err(error) = self.make_room(&mut workers) {
                        return Outcome::Failed(error);
                    }
                    true
                }
            };
            if start {
                let key_permit = match tokio::time::timeout_at(
                    startup_deadline,
                    gate.worker.clone().acquire_owned(),
                )
                .await
                {
                    Ok(Ok(permit)) => permit,
                    _ => {
                        return Outcome::Failed(Error::unavailable(
                            "the previous worker is still draining; retry shortly",
                        ));
                    }
                };
                // A permit survives retirement until actual sandbox teardown,
                // bounding physical workers even while an old session drains.
                let permit = match tokio::time::timeout_at(
                    startup_deadline,
                    self.permits.clone().acquire_owned(),
                )
                .await
                {
                    Ok(Ok(permit)) => permit,
                    _ => {
                        return Outcome::Failed(Error::unavailable(
                            "worker capacity is still draining; retry shortly",
                        ));
                    }
                };
                pending.deadline = Instant::now() + pending.call.timeout;
                let execution_deadline =
                    startup_deadline.min(tokio::time::Instant::from_std(pending.deadline));
                let (node, plugin2, bundle2, in_flight) = (
                    self.node.clone(),
                    plugin.clone(),
                    bundle.clone(),
                    self.in_flight.clone(),
                );
                // Admission and lifetime permits travel with the start. A cancelled
                // caller cannot release either while a detached startup runs.
                let backend = backend.clone();
                let account = pending.call.account;
                let runtime = tokio::runtime::Handle::current();
                let start_key = gate.clone();
                let start_task = tokio::spawn(async move {
                    (
                        admission,
                        spawn(
                            node.as_ref(),
                            permit,
                            key_permit,
                            start_key,
                            backend,
                            runtime,
                            account,
                            &plugin2,
                            &bundle2,
                            space,
                            in_flight,
                        )
                        .await,
                    )
                });
                let spawned = match tokio::time::timeout_at(execution_deadline, start_task).await {
                    Ok(result) => result,
                    Err(_) => {
                        return Outcome::Failed(Error::unavailable(
                            if Instant::now() >= pending.deadline {
                                "extension timed out"
                            } else {
                                "worker startup timed out; check the enrolled machine runtime"
                            },
                        ));
                    }
                };
                let (guard, worker) = match spawned {
                    Ok((guard, Ok(worker))) => (guard, worker),
                    Ok((_, Err(error))) => return Outcome::Failed(error),
                    Err(_) => return Outcome::Failed(Error::internal("worker start failed")),
                };
                admission = guard;
                let mut workers = match self.workers.lock() {
                    Ok(w) => w,
                    Err(_) => return Outcome::Failed(Error::internal("worker pool poisoned")),
                };
                // A concurrent cold start may have won; keep one worker per key.
                if workers.get(&key).is_some_and(Worker::usable) {
                    drop(worker); // stops the losing duplicate
                } else {
                    workers.insert(key.clone(), worker);
                }
            }
            let sent = {
                let workers = match self.workers.lock() {
                    Ok(w) => w,
                    Err(_) => return Outcome::Failed(Error::internal("worker pool poisoned")),
                };
                let Some(worker) = workers.get(&key) else {
                    continue;
                };
                if worker
                    .busy
                    .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                        (n < calternal_plugin_host::MAX_IN_FLIGHT).then_some(n + 1)
                    })
                    .is_err()
                {
                    return Outcome::Failed(Error::unavailable(
                        "extension worker is busy; retry shortly",
                    ));
                }
                self.in_flight.fetch_add(1, Ordering::AcqRel);
                if let Some(cancel) = &pending.call.cancel {
                    cancel.bind(&worker.events);
                }
                match worker.events.try_send(Event::Invoke(pending)) {
                    Ok(()) => None,
                    Err(mpsc::TrySendError::Full(_)) => {
                        worker.busy.fetch_sub(1, Ordering::AcqRel);
                        self.in_flight.fetch_sub(1, Ordering::AcqRel);
                        return Outcome::Failed(Error::unavailable(
                            "extension worker input queue is full",
                        ));
                    }
                    Err(mpsc::TrySendError::Disconnected(event)) => {
                        worker.busy.fetch_sub(1, Ordering::AcqRel);
                        self.in_flight.fetch_sub(1, Ordering::AcqRel);
                        drop(workers);
                        if let Ok(mut workers) = self.workers.lock() {
                            workers.remove(&key);
                        }
                        match event {
                            Event::Invoke(p) => Some(p),
                            _ => unreachable!("only invocations are sent here"),
                        }
                    }
                }
            };
            match sent {
                None => {
                    drop(admission);
                    return answer.await.unwrap_or_else(|_| {
                        Outcome::Failed(Error::unavailable("extension worker stopped"))
                    });
                }
                Some(p) => pending = p,
            }
        }
        Outcome::Failed(Error::unavailable("extension worker could not start"))
    }

    /// Evict one idle worker if the pool is full; refuse when every worker is busy.
    fn make_room(&self, workers: &mut HashMap<(String, SpaceId), Worker>) -> Result<(), Error> {
        workers.retain(|_, w| w.usable());
        if self.permits.available_permits() > 0 {
            return Ok(());
        }
        let idle = workers
            .iter()
            .find(|(_, w)| w.busy.load(Ordering::Acquire) == 0)
            .map(|(k, _)| k.clone());
        match idle {
            Some(key) => {
                tracing::debug!("evicting idle worker");
                workers.remove(&key); // dropping the handle stops it
                Ok(())
            }
            None if workers.len() < self.max_workers => Ok(()),
            None => Err(
                Error::unavailable("extension workers are busy; retry shortly")
                    .with_details(serde_json::json!({ "reason": "worker_capacity" })),
            ),
        }
    }
}

mod session;
use session::Thread;

mod transport;
use transport::{Process, spawn};
