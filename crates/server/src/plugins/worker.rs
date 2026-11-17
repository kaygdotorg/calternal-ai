//! Long-lived worker processes speaking protocol v2, one per (plugin, space).
//!
//! Each worker is a Node child process driven by a [`WorkerSession`] on a dedicated
//! session thread, with one reader thread (bounded newline frames from stdout) and
//! one writer thread (bounded frame queue to stdin). Plugin code reaches host state
//! only through host-call frames, which the session refuses unless declared in the
//! manifest and which the per-invocation [`InvocationHost`] rechecks against
//! current authority before and after the effect and again before delivery.
//!
//! Bounds: at most [`MAX_WORKERS`] live processes (an idle one is evicted to make
//! room; if all are busy the call is refused with `worker_capacity`), at most 32
//! in-flight invocations and 32 queued frames per direction per worker (library
//! limits), frames of at most 256 KiB, a [`WORKER_TIMEOUT`] deadline per invocation,
//! a V8 heap cap of [`WORKER_HEAP_MB`] MiB, and retirement after
//! [`MAX_LIFETIME_INVOCATIONS`]. A worker idle for [`WORKER_IDLE`] exits.
//! Any protocol violation, overflow or missed deadline kills the whole process and
//! fails its in-flight invocations: one JavaScript event loop serves them all, so a
//! stuck invocation would stall the rest anyway.
//!
//! **Not an OS sandbox.** The child gets a cleared environment and Node's permission
//! model (filesystem reads limited to the bundle directory; no writes, child
//! processes, worker threads, native addons or WASI; no string code generation).
//! It can still open network connections (Node 22's permission model does not
//! restrict sockets, including to this server on loopback), use CPU until its
//! deadline, allocate up to its heap cap plus native memory, read its own bundle
//! directory, and keep state in memory across invocations in the same
//! (plugin, space). Cost per live worker: one process and three OS threads.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use calternal_core::ManifestCapability;
use calternal_plugin_host::{
    AccountId, Authority, Error, Frame, HostBroker, MAX_FRAME_BYTES, MAX_QUEUE, Message, SpaceId,
    WorkerSession, decode_frame, encode_frame,
};
use serde_json::Value;
use tokio::sync::oneshot;

use super::{InvocationHost, Plugin, verify_digest};
use crate::error::HostError;

/// Deadline for one invocation, from admission to delivery.
pub const WORKER_TIMEOUT: Duration = Duration::from_secs(3);
/// A worker with no in-flight invocation for this long exits.
pub const WORKER_IDLE: Duration = Duration::from_secs(60);
/// Maximum live worker processes across all plugins and spaces.
pub const MAX_WORKERS: usize = 8;
/// Invocations after which a worker is retired and replaced (library replay bound: 4096).
pub const MAX_LIFETIME_INVOCATIONS: u32 = 4096;
/// V8 old-generation heap cap per worker, in MiB.
pub const WORKER_HEAP_MB: u32 = 128;
const MIN_NODE: (u32, u32) = (22, 13);

/// A verified Node executable used to run worker bundles.
#[derive(Debug, Clone)]
pub struct NodeRuntime {
    /// Canonical absolute executable path.
    pub path: PathBuf,
    /// `node --version` output, such as `v22.13.0`.
    pub version: String,
}

/// Resolve and verify an absolute Node executable (>= 22.13 for `--permission`).
/// Cost: one `node --version` process.
pub async fn resolve_node(explicit: Option<&Path>) -> Result<NodeRuntime, String> {
    let candidate = match explicit {
        Some(p) => p.to_path_buf(),
        None => {
            let path_var = std::env::var_os("PATH").ok_or("PATH not set; pass --node")?;
            std::env::split_paths(&path_var)
                .map(|d| d.join("node"))
                .find(|p| p.is_file())
                .ok_or("node not found on PATH; pass --node")?
        }
    };
    let abs = std::fs::canonicalize(&candidate).map_err(|e| format!("node: {e}"))?;
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
    Failed(HostError),
}

enum Event {
    Invoke(Box<Pending>),
    Line(Vec<u8>),
    Overflow,
    Closed,
    Shutdown,
}

struct Pending {
    call: Call,
    reply: oneshot::Sender<Outcome>,
}

struct Active {
    host: Arc<dyn InvocationHost>,
    reply: oneshot::Sender<Outcome>,
    deadline: Instant,
}

/// Handle to one live worker's session thread.
struct Worker {
    events: Sender<Event>,
    alive: Arc<AtomicBool>,
    busy: Arc<AtomicUsize>,
}

impl Worker {
    fn usable(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
}

/// Bounded pool of long-lived workers keyed by (plugin ID, space key).
pub struct WorkerPool {
    node: Option<NodeRuntime>,
    workers: Mutex<HashMap<(String, String), Worker>>,
    in_flight: Arc<AtomicUsize>,
    max_workers: usize,
}

impl WorkerPool {
    /// A pool that runs workers with `node`; `None` refuses every call as unavailable.
    pub fn new(node: Option<NodeRuntime>) -> Self {
        Self::with_capacity(node, MAX_WORKERS)
    }

    /// A pool with a custom live-worker cap (tests use 1 to prove admission).
    pub fn with_capacity(node: Option<NodeRuntime>, max_workers: usize) -> Self {
        Self {
            node,
            workers: Mutex::new(HashMap::new()),
            in_flight: Arc::new(AtomicUsize::new(0)),
            max_workers: max_workers.max(1),
        }
    }

    /// The configured Node runtime, if any.
    pub fn node(&self) -> Option<&NodeRuntime> {
        self.node.as_ref()
    }

    /// Invocations admitted to a worker and not yet answered. Cost: one atomic load.
    pub fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::Acquire)
    }

    /// Live worker processes. Cost: pool lock and O(workers) scan.
    pub fn live(&self) -> usize {
        self.workers
            .lock()
            .map(|w| w.values().filter(|w| w.usable()).count())
            .unwrap_or(0)
    }

    /// Run one operation on the (plugin, space) worker, starting it if needed.
    /// `space_key` selects the process; `space` is the UUID the session is pinned to.
    /// Cost: pool lock; on a cold start one bundle hash and one process spawn; then
    /// one channel round trip plus the worker's own frames and broker effects.
    pub async fn invoke(
        &self,
        plugin: &Arc<Plugin>,
        space_key: &str,
        space: SpaceId,
        call: Call,
    ) -> Outcome {
        let Some(node) = &self.node else {
            return Outcome::Failed(HostError::unavailable("Node runtime unavailable"));
        };
        if let Some(why) = &plugin.unavailable {
            return Outcome::Failed(HostError::unavailable(format!(
                "plugin unavailable: {why}"
            )));
        }
        let Some(bundle) = &plugin.worker else {
            return Outcome::Failed(HostError::unavailable("plugin has no worker"));
        };
        let (reply, answer) = oneshot::channel();
        let mut pending = Box::new(Pending { call, reply });
        // A worker that exited idle between lookup and send drops its receiver;
        // the send then fails and one fresh worker is started.
        for _ in 0..2 {
            let sent = {
                let mut workers = match self.workers.lock() {
                    Ok(w) => w,
                    Err(_) => return Outcome::Failed(HostError::internal("worker pool poisoned")),
                };
                let key = (plugin.manifest.id.clone(), space_key.to_owned());
                if !workers.get(&key).is_some_and(Worker::usable) {
                    workers.remove(&key);
                    if let Err(e) = self.make_room(&mut workers) {
                        return Outcome::Failed(e);
                    }
                    match spawn(node, plugin, bundle, space, self.in_flight.clone()) {
                        Ok(w) => {
                            workers.insert(key.clone(), w);
                        }
                        Err(e) => return Outcome::Failed(e),
                    }
                }
                let worker = &workers[&key];
                worker.busy.fetch_add(1, Ordering::AcqRel);
                self.in_flight.fetch_add(1, Ordering::AcqRel);
                match worker.events.send(Event::Invoke(pending)) {
                    Ok(()) => None,
                    Err(mpsc::SendError(event)) => {
                        worker.busy.fetch_sub(1, Ordering::AcqRel);
                        self.in_flight.fetch_sub(1, Ordering::AcqRel);
                        workers.remove(&key);
                        match event {
                            Event::Invoke(p) => Some(p),
                            _ => unreachable!("only invocations are sent here"),
                        }
                    }
                }
            };
            match sent {
                None => {
                    return answer.await.unwrap_or_else(|_| {
                        Outcome::Failed(HostError::unavailable("extension worker stopped"))
                    });
                }
                Some(p) => pending = p,
            }
        }
        Outcome::Failed(HostError::unavailable("extension worker could not start"))
    }

    /// Evict one idle worker if the pool is full; refuse when every worker is busy.
    fn make_room(&self, workers: &mut HashMap<(String, String), Worker>) -> Result<(), HostError> {
        workers.retain(|_, w| w.usable());
        if workers.len() < self.max_workers {
            return Ok(());
        }
        let idle = workers
            .iter()
            .find(|(_, w)| w.busy.load(Ordering::Acquire) == 0)
            .map(|(k, _)| k.clone());
        match idle {
            Some(key) => {
                if let Some(w) = workers.remove(&key) {
                    let _ = w.events.send(Event::Shutdown);
                }
                Ok(())
            }
            None => Err(HostError::unavailable("extension workers are busy; retry shortly")
                .with_details(serde_json::json!({ "reason": "worker_capacity" }))),
        }
    }
}

/// Start one worker process and its threads. The pinned digest is rechecked first.
fn spawn(
    node: &NodeRuntime,
    plugin: &Arc<Plugin>,
    bundle: &Path,
    space: SpaceId,
    in_flight: Arc<AtomicUsize>,
) -> Result<Worker, HostError> {
    verify_digest(bundle, plugin.entry.worker_sha256.as_deref()).map_err(|why| {
        tracing::warn!(plugin = %plugin.manifest.id, reason = %why, "refusing to start worker");
        HostError::unavailable("plugin bundle failed verification")
    })?;
    let session = WorkerSession::new(plugin.manifest.clone(), space)
        .map_err(|_| HostError::unavailable("plugin manifest rejected by worker session"))?;
    let dir = bundle.parent().unwrap_or(Path::new("/"));
    let mut child = Command::new(&node.path)
        .arg("--permission")
        .arg(format!("--allow-fs-read={}", dir.display()))
        .arg("--disallow-code-generation-from-strings")
        .arg(format!("--max-old-space-size={WORKER_HEAP_MB}"))
        .arg(bundle)
        .env_clear()
        .env("LANG", "C.UTF-8")
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // Never read: diagnostics may echo private item text.
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            tracing::error!(error = %e, "failed to start worker");
            HostError::unavailable("extension worker could not start")
        })?;
    let stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let (events, rx) = mpsc::channel();
    let (frames, frames_rx) = mpsc::sync_channel::<Vec<u8>>(MAX_QUEUE);
    let reader_events = events.clone();
    std::thread::Builder::new()
        .name("plugin-worker-read".into())
        .spawn(move || read_frames(stdout, reader_events))
        .map_err(|_| HostError::unavailable("extension worker could not start"))?;
    std::thread::Builder::new()
        .name("plugin-worker-write".into())
        .spawn(move || {
            let mut stdin = stdin;
            for frame in frames_rx {
                if stdin.write_all(&frame).and_then(|()| stdin.flush()).is_err() {
                    return;
                }
            }
        })
        .map_err(|_| HostError::unavailable("extension worker could not start"))?;
    let alive = Arc::new(AtomicBool::new(true));
    let busy = Arc::new(AtomicUsize::new(0));
    let thread = Thread {
        child,
        session,
        space,
        frames,
        active: BTreeMap::new(),
        next: 0,
        alive: alive.clone(),
        busy: busy.clone(),
        in_flight,
    };
    std::thread::Builder::new()
        .name("plugin-worker".into())
        .spawn(move || thread.run(rx))
        .map_err(|_| HostError::unavailable("extension worker could not start"))?;
    tracing::debug!(plugin = %plugin.manifest.id, "worker started");
    Ok(Worker {
        events,
        alive,
        busy,
    })
}

/// Read newline-delimited frames, never buffering more than one frame budget.
fn read_frames(stdout: ChildStdout, events: Sender<Event>) {
    let mut reader = BufReader::new(stdout);
    loop {
        let mut line = Vec::new();
        let limit = MAX_FRAME_BYTES as u64 + 1;
        match (&mut reader).take(limit).read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => {
                let _ = events.send(Event::Closed);
                return;
            }
            Ok(_) if line.last() == Some(&b'\n') => {
                if events.send(Event::Line(line)).is_err() {
                    return;
                }
            }
            Ok(_) => {
                // Either the budget ran out before a newline, or output ended mid-frame.
                let event = if line.len() as u64 >= limit {
                    Event::Overflow
                } else {
                    Event::Closed
                };
                let _ = events.send(event);
                return;
            }
        }
    }
}

/// Adapts the per-invocation hosts to the library's broker interface.
/// The grant in each authority is the host-issued invocation ID.
struct Broker<'a>(&'a BTreeMap<String, Active>);

impl HostBroker for Broker<'_> {
    fn authorize(&mut self, authority: &Authority, required: ManifestCapability) -> bool {
        self.0
            .get(&authority.grant)
            .is_some_and(|a| a.host.authorize(required))
    }
    fn call(
        &mut self,
        authority: &Authority,
        method: calternal_core::HostCallKind,
        input: &Value,
    ) -> Result<Value, Error> {
        let active = self.0.get(&authority.grant).ok_or(Error::Refused)?;
        active.host.call(method, input)
    }
}

/// State owned by one worker's session thread.
struct Thread {
    child: Child,
    session: WorkerSession,
    space: SpaceId,
    frames: SyncSender<Vec<u8>>,
    active: BTreeMap<String, Active>,
    next: u32,
    alive: Arc<AtomicBool>,
    busy: Arc<AtomicUsize>,
    in_flight: Arc<AtomicUsize>,
}

/// Why a worker process is being stopped.
enum Stop {
    Idle,
    Fault(&'static str),
}

impl Thread {
    fn run(mut self, rx: Receiver<Event>) {
        let stop = loop {
            let wait = self
                .active
                .values()
                .map(|a| a.deadline.saturating_duration_since(Instant::now()))
                .min()
                .unwrap_or(WORKER_IDLE);
            match rx.recv_timeout(wait) {
                Ok(Event::Invoke(p)) => self.invoke(*p),
                Ok(Event::Line(line)) => {
                    if let Err(why) = self.line(&line) {
                        break Stop::Fault(why);
                    }
                }
                Ok(Event::Overflow) => break Stop::Fault("extension output exceeded limit"),
                Ok(Event::Closed) => break Stop::Fault("extension worker stopped"),
                Ok(Event::Shutdown) if self.active.is_empty() => break Stop::Idle,
                Ok(Event::Shutdown) => self.retire(),
                Err(RecvTimeoutError::Timeout) if self.active.is_empty() => {
                    // Mark unusable first; an invocation that raced in is still served.
                    self.alive.store(false, Ordering::Release);
                    match rx.try_recv() {
                        Ok(Event::Invoke(p)) => self.invoke(*p),
                        _ => break Stop::Idle,
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break Stop::Idle,
            }
            // Checked after every event, so a chatty worker cannot outrun its deadline.
            if self.active.values().any(|a| a.deadline <= Instant::now()) {
                break Stop::Fault("extension timed out");
            }
            if !self.alive.load(Ordering::Acquire) && self.active.is_empty() {
                break Stop::Idle;
            }
        };
        self.alive.store(false, Ordering::Release);
        let _ = self.child.kill();
        let _ = self.child.wait();
        let message = match stop {
            Stop::Idle => "extension worker stopped",
            Stop::Fault(why) => {
                tracing::warn!(reason = why, "worker stopped");
                why
            }
        };
        for id in self.active.keys().cloned().collect::<Vec<_>>() {
            self.finish(&id, Outcome::Failed(HostError::unavailable(message)));
        }
        // Reply to invocations queued behind the stop instead of dropping them silently.
        while let Ok(event) = rx.try_recv() {
            if let Event::Invoke(p) = event {
                self.busy.fetch_sub(1, Ordering::AcqRel);
                self.in_flight.fetch_sub(1, Ordering::AcqRel);
                let _ = p
                    .reply
                    .send(Outcome::Failed(HostError::unavailable(message)));
            }
        }
    }

    /// Stop accepting new work; the thread exits once in-flight invocations finish.
    fn retire(&mut self) {
        self.alive.store(false, Ordering::Release);
    }

    fn invoke(&mut self, p: Pending) {
        self.next += 1;
        if self.next >= MAX_LIFETIME_INVOCATIONS {
            // Serve this last one; the pool starts a replacement for the next call.
            self.retire();
        }
        let id = format!("i{}", self.next);
        let authority = Authority {
            account_id: p.call.account,
            space_id: self.space,
            grant: id.clone(),
        };
        self.active.insert(
            id.clone(),
            Active {
                host: p.call.host,
                reply: p.reply,
                deadline: Instant::now() + WORKER_TIMEOUT,
            },
        );
        let admitted = self.session.invoke(
            id.clone(),
            p.call.operation,
            p.call.input,
            authority,
            &mut Broker(&self.active),
        );
        match admitted {
            Ok(()) => self.flush(&id),
            Err(Error::Invalid) => self.finish(&id, Outcome::InvalidInput),
            Err(Error::Refused) => self.finish(&id, Outcome::Refused),
            Err(_) => self.finish(
                &id,
                Outcome::Failed(
                    HostError::unavailable("extension worker is busy; retry shortly")
                        .with_details(serde_json::json!({ "reason": "worker_capacity" })),
                ),
            ),
        }
    }

    /// Handle one worker frame. `Err` means the process must be stopped.
    fn line(&mut self, line: &[u8]) -> Result<(), &'static str> {
        let frame = decode_frame(line).map_err(|_| "extension returned invalid output")?;
        let id = frame.invocation_id.clone();
        if !self.active.contains_key(&id) {
            // Late output for an invocation already answered: discard it.
            return Ok(());
        }
        let ends = matches!(frame.message, Message::Complete { .. } | Message::Failed {});
        match self.session.receive(frame, &mut Broker(&self.active)) {
            Ok(()) => {}
            Err(Error::Refused) => {
                // Undeclared/forged host call, or authority no longer current.
                let _ = self.session.cancel(&id);
                self.finish(&id, Outcome::Refused);
            }
            Err(Error::BackPressure) => return Err("extension exceeded its queue"),
            Err(_) => {
                let _ = self.session.cancel(&id);
                self.finish(
                    &id,
                    Outcome::Failed(HostError::unavailable("extension returned invalid output")),
                );
            }
        }
        if ends {
            self.deliver(&id);
        }
        self.flush(&id);
        Ok(())
    }

    /// Drain the delivery for `id` that a successful completion just queued.
    fn deliver(&mut self, id: &str) {
        if !self.active.contains_key(id) {
            return;
        }
        let outcome = match self.session.next_delivery(&mut Broker(&self.active)) {
            Ok(Some(Frame {
                message: Message::Complete { data },
                ..
            })) => Outcome::Complete(data),
            Ok(Some(_)) => Outcome::Failed(HostError::unavailable("extension failed")),
            Ok(None) => return,
            Err(_) => Outcome::Refused,
        };
        self.finish(id, outcome);
    }

    /// Send every queued host frame. Every enqueue is followed by a flush on this
    /// thread, so queued frames belong to `id` (or are cancellations, which bypass
    /// the authority check). A frame refused because `id`'s authority no longer
    /// holds is discarded and `id` refused. A full transport queue stops the worker.
    fn flush(&mut self, id: &str) {
        loop {
            let frame = match self.session.next_outgoing(&mut Broker(&self.active)) {
                Ok(Some(frame)) => frame,
                Ok(None) => return,
                Err(Error::Refused) => {
                    let _ = self.session.cancel(id);
                    self.finish(id, Outcome::Refused);
                    continue;
                }
                Err(_) => continue,
            };
            let Ok(bytes) = encode_frame(&frame) else {
                continue;
            };
            if self.frames.try_send(bytes).is_err() {
                // The worker stopped reading: stop it rather than block this thread.
                self.alive.store(false, Ordering::Release);
                let _ = self.child.kill();
                return;
            }
        }
    }

    fn finish(&mut self, id: &str, outcome: Outcome) {
        if let Some(active) = self.active.remove(id) {
            self.busy.fetch_sub(1, Ordering::AcqRel);
            self.in_flight.fetch_sub(1, Ordering::AcqRel);
            let _ = active.reply.send(outcome);
        }
    }
}
