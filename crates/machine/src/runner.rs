//! Applies server messages to sandboxes and streams exec output back.
//!
//! One slot per lease. A slot starts its sandbox once, on the first message that
//! needs it; execs arriving meanwhile wait for the same start. Each exec has an
//! output task that numbers events, uploads them in batches and keeps at most
//! [`UNACKED_BYTES`] unacknowledged; beyond that it stops reading, which blocks
//! the process (see [`crate::sandbox`]). Memory per running exec is therefore
//! bounded by about 256 KiB in the channel plus 256 KiB here.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use tokio::sync::OnceCell;
use uuid::Uuid;

use crate::Error;
use crate::link::Link;
use crate::protocol::{
    Capacity, Chunk, ChunkEvent, Down, ExitInfo, LeaseReport, OutputAck, OutputPost, SandboxSpec,
    SandboxState,
};
use crate::sandbox::{Backend, Exec, ExecRequest, OutputEvent, Sandbox, failed_exec};

/// Most output bytes an exec keeps waiting for the server's acknowledgement.
pub const UNACKED_BYTES: usize = 256 * 1024;
/// Largest upload body, in raw output bytes.
const POST_BYTES: usize = 1024 * 1024;
/// Short wait to batch small writes into one upload.
const COALESCE: Duration = Duration::from_millis(20);

struct Slot {
    lease_id: Uuid,
    fence: AtomicU64,
    spec: SandboxSpec,
    sandbox: OnceCell<Result<Arc<dyn Sandbox>, String>>,
    frozen: AtomicBool,
    /// Set when the slot left the map; whoever then sees a started sandbox stops it.
    removed: AtomicBool,
    stop: OnceCell<()>,
    keep_files: AtomicBool,
    execs: Mutex<HashSet<Uuid>>,
}

impl Slot {
    /// Stop the sandbox once, if the slot was removed and its start succeeded.
    async fn stop_if_removed(&self) {
        if self.removed.load(Ordering::Acquire)
            && let Some(Ok(sb)) = self.sandbox.get()
        {
            self.stop
                .get_or_init(|| async {
                    let result = if self.keep_files.load(Ordering::Acquire) {
                        sb.release().await
                    } else {
                        sb.stop().await
                    };
                    if let Err(e) = result {
                        tracing::warn!(lease=%self.lease_id,error=%e,"sandbox stop failed");
                    }
                })
                .await;
        }
    }
}

/// Machine-side state: the sandboxes of every lease this machine holds.
pub struct Runner {
    backend: Arc<dyn Backend>,
    link: Arc<Link>,
    max_sandboxes: u32,
    slots: Mutex<HashMap<Uuid, Arc<Slot>>>,
}

impl Runner {
    /// A runner starting sandboxes with `backend`, at most `max_sandboxes` at once.
    pub fn new(backend: Arc<dyn Backend>, link: Arc<Link>, max_sandboxes: u32) -> Self {
        Self {
            backend,
            link,
            max_sandboxes,
            slots: Mutex::new(HashMap::new()),
        }
    }

    /// Active backend name for server-side direct-execution policy admission.
    pub fn backend_name(&self) -> &'static str {
        self.backend.preference()
    }

    /// Install the server-authorized execution setting before assignments arrive.
    /// This changes only future sandbox starts; old fenced objects keep their backend.
    pub fn configure_backend(&self, mode: &str) -> Result<(), crate::Error> {
        self.backend
            .configure(mode)
            .map_err(|e| crate::Error::Protocol(e.0))
    }

    /// Current capacity. Reads `/proc/meminfo` on Linux (a few µs).
    pub fn capacity(&self) -> Capacity {
        Capacity {
            cpus: std::thread::available_parallelism().map_or(1, |n| n.get() as u32),
            memory_bytes: memory_bytes(),
            max_sandboxes: self.max_sandboxes,
            running_sandboxes: self.slots.lock().expect("slots").len() as u32,
        }
    }

    /// Every lease held, with its sandbox state.
    pub fn reports(&self) -> Vec<LeaseReport> {
        let slots = self.slots.lock().expect("slots");
        slots
            .values()
            .map(|s| LeaseReport {
                lease_id: s.lease_id,
                fence: s.fence.load(Ordering::Acquire),
                sandbox: match s.sandbox.get() {
                    None => SandboxState::Starting,
                    Some(Err(reason)) => SandboxState::Failed {
                        reason: reason.clone(),
                    },
                    Some(Ok(_)) if s.frozen.load(Ordering::Acquire) => SandboxState::Frozen,
                    Some(Ok(_)) => SandboxState::Running,
                },
            })
            .collect()
    }

    fn slot(&self, lease: Uuid) -> Option<Arc<Slot>> {
        self.slots.lock().expect("slots").get(&lease).cloned()
    }

    /// Apply one message. Starting sandboxes and execs run on their own tasks, so
    /// assignments and execs return at once. Stop awaits cleanup before a later
    /// assignment can adopt the same thread, preventing stop/start races.
    pub async fn handle(self: &Arc<Self>, message: Down) {
        match message {
            Down::Assign {
                lease_id,
                fence,
                spec,
                ..
            } => {
                let slot = {
                    let mut slots = self.slots.lock().expect("slots");
                    slots
                        .entry(lease_id)
                        .or_insert_with(|| {
                            Arc::new(Slot {
                                lease_id,
                                fence: AtomicU64::new(fence),
                                spec,
                                sandbox: OnceCell::new(),
                                frozen: AtomicBool::new(false),
                                removed: AtomicBool::new(false),
                                stop: OnceCell::new(),
                                keep_files: AtomicBool::new(false),
                                execs: Mutex::new(HashSet::new()),
                            })
                        })
                        .clone()
                };
                slot.fence.fetch_max(fence, Ordering::AcqRel);
                let me = self.clone();
                tokio::spawn(async move {
                    if let Ok(sb) = me.sandbox(&slot).await
                        && slot.frozen.swap(false, Ordering::AcqRel)
                    {
                        let _ = sb.resume().await;
                    }
                });
            }
            Down::Exec {
                lease_id,
                fence,
                exec_id,
                argv,
                stdin,
                mut env,
            } => {
                env.retain(|(key, _)| key != "CALTERNAL_URL");
                env.push(("CALTERNAL_URL".into(), self.link.base().to_owned()));
                let me = self.clone();
                tokio::spawn(async move {
                    let slot = me
                        .slot(lease_id)
                        .filter(|s| s.fence.load(Ordering::Acquire) == fence);
                    let exec = match &slot {
                        None => failed_exec("lease not held by this machine"),
                        Some(s) => {
                            if !s.execs.lock().expect("execs").insert(exec_id) {
                                return; // delivered twice; already running
                            }
                            match me.sandbox(s).await {
                                Err(reason) => failed_exec(reason),
                                Ok(sb) => sb
                                    .exec(ExecRequest { argv, stdin, env })
                                    .await
                                    .unwrap_or_else(|e| failed_exec(e.0)),
                            }
                        }
                    };
                    me.upload(lease_id, fence, exec_id, exec).await;
                });
            }
            Down::Stop {
                lease_id,
                reason,
                keep_files,
            } => {
                let slot = self.slots.lock().expect("slots").remove(&lease_id);
                if let Some(slot) = slot {
                    tracing::info!(lease = %lease_id, %reason, "stopping sandbox");
                    slot.keep_files.store(keep_files, Ordering::Release);
                    slot.removed.store(true, Ordering::Release);
                    // A start still in flight stops the sandbox when it finishes.
                    // Join any start already in flight, then join cleanup itself.
                    let _ = self.sandbox(&slot).await;
                    slot.stop_if_removed().await;
                }
            }
        }
    }

    async fn sandbox(&self, slot: &Slot) -> Result<Arc<dyn Sandbox>, String> {
        let result = slot
            .sandbox
            .get_or_init(|| async {
                if slot.removed.load(Ordering::Acquire) {
                    return Err("lease ended before sandbox start".into());
                }
                self.backend.start(&slot.spec).await.map_err(|e| {
                    tracing::warn!(lease = %slot.lease_id, error = %e, "sandbox start failed");
                    e.0
                })
            })
            .await
            .clone();
        slot.stop_if_removed().await;
        if slot.removed.load(Ordering::Acquire) {
            return Err("lease ended".into());
        }
        result
    }

    /// Freeze every started sandbox: the server stopped hearing from us for a
    /// lease TTL and may have given the leases to another machine.
    pub async fn freeze_all(&self) {
        for slot in self.all() {
            if let Some(Ok(sb)) = slot.sandbox.get()
                && !slot.frozen.swap(true, Ordering::AcqRel)
                && let Err(e) = sb.freeze().await
            {
                tracing::warn!(lease = %slot.lease_id, error = %e, "freeze failed");
            }
        }
    }

    /// Resume frozen sandboxes whose leases survived (called after a sync that
    /// carried the server's verdict on every reported lease).
    pub async fn resume_all(&self) {
        for slot in self.all() {
            if let Some(Ok(sb)) = slot.sandbox.get()
                && slot.frozen.swap(false, Ordering::AcqRel)
                && let Err(e) = sb.resume().await
            {
                tracing::warn!(lease = %slot.lease_id, error = %e, "resume failed");
            }
        }
    }

    /// Whether any sandbox is frozen.
    pub fn any_frozen(&self) -> bool {
        self.all().iter().any(|s| s.frozen.load(Ordering::Acquire))
    }

    /// Stop and delete every sandbox (revocation).
    pub async fn stop_all(&self) {
        let slots: Vec<_> = self
            .slots
            .lock()
            .expect("slots")
            .drain()
            .map(|(_, s)| s)
            .collect();
        for slot in slots {
            slot.removed.store(true, Ordering::Release);
            slot.stop_if_removed().await;
        }
    }

    /// End running execs but keep sandboxes for adoption (orderly exit).
    pub async fn release_all(&self) {
        for slot in self.all() {
            if let Some(Ok(sb)) = slot.sandbox.get() {
                let _ = sb.release().await;
            }
        }
    }

    fn all(&self) -> Vec<Arc<Slot>> {
        self.slots
            .lock()
            .expect("slots")
            .values()
            .cloned()
            .collect()
    }

    /// Number the exec's events and upload them until the exit is acknowledged,
    /// the lease is fenced or the machine is revoked.
    async fn upload(&self, lease_id: Uuid, fence: u64, exec_id: Uuid, mut exec: Exec) {
        let mut pending: VecDeque<(Chunk, usize)> = VecDeque::new();
        let (mut seq, mut bytes, mut ended, mut backoff) =
            (0u64, 0usize, false, Duration::from_millis(200));
        let push = |ev: OutputEvent,
                    seq: &mut u64,
                    pending: &mut VecDeque<(Chunk, usize)>,
                    bytes: &mut usize| {
            *seq += 1;
            let (event, size, last) = match ev {
                OutputEvent::Data(stream, b) => (
                    ChunkEvent::Data {
                        stream,
                        data: B64.encode(&b),
                    },
                    b.len(),
                    false,
                ),
                OutputEvent::Exit(info) => (ChunkEvent::Exit(info), 0, true),
            };
            pending.push_back((Chunk { seq: *seq, event }, size));
            *bytes += size;
            last
        };
        loop {
            if !ended && pending.is_empty() {
                match exec.events.recv().await {
                    Some(ev) => ended |= push(ev, &mut seq, &mut pending, &mut bytes),
                    None => {
                        let lost = OutputEvent::Exit(ExitInfo {
                            error: Some("output ended without exit".into()),
                            ..ExitInfo::default()
                        });
                        ended |= push(lost, &mut seq, &mut pending, &mut bytes);
                    }
                }
                if !ended {
                    tokio::time::sleep(COALESCE).await;
                }
            }
            while !ended && bytes < UNACKED_BYTES {
                match exec.events.try_recv() {
                    Ok(ev) => ended |= push(ev, &mut seq, &mut pending, &mut bytes),
                    Err(_) => break,
                }
            }
            let mut size = 0;
            let chunks: Vec<Chunk> = pending
                .iter()
                .take_while(|(_, n)| {
                    size += n;
                    size <= POST_BYTES || size == *n
                })
                .map(|(c, _)| c.clone())
                .collect();
            let post = OutputPost {
                lease_id,
                fence,
                exec_id,
                chunks,
            };
            match self
                .link
                .post::<_, OutputAck>(
                    "/api/v1/machine/output",
                    &post,
                    true,
                    Duration::from_secs(30),
                )
                .await
            {
                Ok(ack) => {
                    backoff = Duration::from_millis(200);
                    while pending.front().is_some_and(|(c, _)| c.seq <= ack.acked) {
                        bytes -= pending.pop_front().expect("front").1;
                    }
                    if ended && pending.is_empty() {
                        return;
                    }
                    if let Some(ms) = ack.retry_after_ms {
                        tokio::time::sleep(Duration::from_millis(ms.min(5_000))).await;
                    }
                }
                Err(Error::Revoked) => return,
                Err(e) if matches!(e.status(), Some(404 | 409)) => {
                    tracing::info!(lease = %lease_id, exec = %exec_id, "output refused: lease fenced");
                    return;
                }
                Err(e) => {
                    if e.status() == Some(401) {
                        self.link.wait_for_session().await;
                    }
                    tracing::debug!(error = %e, "output upload failed; retrying");
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(5));
                }
            }
        }
    }
}

fn memory_bytes() -> u64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("MemTotal:"))
                .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
        })
        .map_or(0, |kib| kib * 1024)
}
