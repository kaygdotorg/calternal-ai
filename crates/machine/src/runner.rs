//! Applies server messages to sandboxes and streams exec output back.
//!
//! One slot per lease. A slot starts its sandbox once, on the first message that
//! needs it; execs arriving meanwhile wait for the same start. Each exec has an
//! output task that numbers events, uploads them in batches and keeps at most
//! [`UNACKED_BYTES`] unacknowledged; beyond that it stops reading, which blocks
//! the process (see [`crate::sandbox`]). Memory per running exec is therefore
//! bounded by about 256 KiB in the channel plus 256 KiB here.

mod erasure;
/// Supervision of the isolated v2 package pipe.
mod space_channel;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use tokio::sync::{OnceCell, RwLock};
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

type SandboxCell = OnceCell<Result<Arc<dyn Sandbox>, String>>;

struct Slot {
    lease_id: Uuid,
    fence: AtomicU64,
    spec: SandboxSpec,
    harness: Option<crate::protocol::HarnessSandbox>,
    // Execs hold a read guard until output upload ends. An idle pass takes an
    // uncontended write guard so it cannot remove a container during exec.
    sandbox: RwLock<SandboxCell>,
    last_use: Mutex<Instant>,
    /// Set when the slot left the map; whoever then sees a started sandbox stops it.
    removed: AtomicBool,
    stop: OnceCell<()>,
    /// A v2 placement is supervised once and the supervisor owns its teardown.
    channel_started: AtomicBool,
    keep_files: AtomicBool,
    execs: Mutex<HashSet<Uuid>>,
    inputs: Mutex<
        HashMap<Uuid, tokio::sync::watch::Receiver<Option<tokio::sync::mpsc::Sender<Vec<u8>>>>>,
    >,
}

impl Slot {
    /// Stop the sandbox once, if the slot was removed and its start succeeded.
    async fn stop_if_removed(&self) -> bool {
        if self.removed.load(Ordering::Acquire)
            && let Some(Ok(sb)) = self.sandbox.read().await.get()
        {
            return self
                .stop
                .get_or_try_init(|| async {
                    let result = if self.keep_files.load(Ordering::Acquire) {
                        sb.release().await
                    } else {
                        sb.stop().await
                    };
                    result.map_err(|e| {
                        tracing::warn!(lease=%self.lease_id,error=%e,"sandbox stop failed");
                    })
                })
                .await
                .is_ok();
        }
        let _ = self.stop.set(());
        true
    }
}

/// Machine-side state: the sandboxes of every lease this machine holds.
pub struct Runner {
    backend: Arc<dyn Backend>,
    link: Arc<Link>,
    max_sandboxes: u32,
    slots: Mutex<HashMap<Uuid, Arc<Slot>>>,
    discards: Mutex<HashMap<Uuid, bool>>,
    parking: tokio::sync::Mutex<()>,
    uploads: Arc<()>,
}

impl Runner {
    /// A runner starting sandboxes with `backend`, at most `max_sandboxes` at once.
    pub fn new(backend: Arc<dyn Backend>, link: Arc<Link>, max_sandboxes: u32) -> Self {
        Self {
            backend,
            link,
            max_sandboxes,
            slots: Mutex::new(HashMap::new()),
            discards: Mutex::new(HashMap::new()),
            parking: tokio::sync::Mutex::new(()),
            uploads: Arc::new(()),
        }
    }

    /// Bounded deletion results retained until a successful sync acknowledges them.
    pub fn discard_reports(&self) -> Vec<crate::protocol::DiscardReport> {
        self.discards
            .lock()
            .expect("discards")
            .iter()
            .map(|(id, success)| crate::protocol::DiscardReport {
                request_id: *id,
                success: *success,
            })
            .collect()
    }
    /// A successful sync durably processed this submitted batch; release its results.
    pub fn ack_discards(&self, reports: &[crate::protocol::DiscardReport]) {
        let mut held = self.discards.lock().expect("discards");
        for r in reports {
            held.remove(&r.request_id);
        }
    }

    /// Active backend name for server-side direct-execution policy admission.
    pub fn backend_name(&self) -> &'static str {
        self.backend.preference()
    }

    /// Send the configured account guest size with the first session request.
    pub fn guest_memory_mib(&self) -> Option<u32> {
        self.backend.guest_memory_mib()
    }

    /// Install the server-authorized execution setting before assignments arrive.
    /// This changes only future sandbox starts; old fenced objects keep their backend.
    pub fn configure_backend(&self, mode: &str) -> Result<(), crate::Error> {
        self.backend
            .configure(mode)
            .map_err(|e| crate::Error::Protocol(e.0))
    }

    /// Sample capacity for a heartbeat. Cost: one host read on Linux, or one
    /// platform memory query on macOS, plus reading the local slot count.
    /// Call from a blocking worker.
    pub fn capacity(&self) -> Capacity {
        let (memory_bytes, available_memory_bytes) = crate::memory::sample();
        Capacity {
            cpus: std::thread::available_parallelism().map_or(1, |n| n.get() as u32),
            memory_bytes,
            available_memory_bytes,
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
                sandbox: if s.removed.load(Ordering::Acquire) {
                    SandboxState::Failed {
                        reason: "the local lease expired or is closing".into(),
                    }
                } else {
                    match s.sandbox.try_read().ok().as_deref().and_then(OnceCell::get) {
                        None => SandboxState::Starting,
                        Some(Err(reason)) => SandboxState::Failed {
                            reason: reason.clone(),
                        },
                        Some(Ok(sandbox)) if !sandbox.available() => SandboxState::Lost,
                        Some(Ok(_)) => SandboxState::Running,
                    }
                },
            })
            .collect()
    }

    fn slot(&self, lease: Uuid) -> Option<Arc<Slot>> {
        self.slots.lock().expect("slots").get(&lease).cloned()
    }

    fn assign(
        self: &Arc<Self>,
        lease_id: Uuid,
        fence: u64,
        space_id: calternal_types::SpaceId,
        mut spec: SandboxSpec,
        harness: Option<crate::protocol::HarnessSandbox>,
    ) {
        spec.space_id = Some(space_id);
        let slot = {
            let mut slots = self.slots.lock().expect("slots");
            let closing_workspace = slots.iter().any(|(id, slot)| {
                *id != lease_id
                    && slot.spec.account_id == spec.account_id
                    && slot.spec.thread_id == spec.thread_id
            });
            let (held, account_held) = slots
                .values()
                .filter(|s| {
                    s.stop.get().is_none()
                        && !s
                            .sandbox
                            .try_read()
                            .ok()
                            .as_deref()
                            .and_then(OnceCell::get)
                            .is_some_and(Result::is_err)
                })
                .fold((0, 0), |(held, account_held), slot| {
                    (
                        held + 1,
                        account_held + usize::from(slot.spec.account_id == spec.account_id),
                    )
                });
            let account_full = self
                .guest_memory_mib()
                .and_then(crate::protocol::GuestSize::from_mib)
                .is_some_and(|size| {
                    spec.limits.memory_bytes.is_some_and(|bytes| {
                        bytes.saturating_mul(account_held as u64 + 1) > size.turn_budget_bytes()
                    })
                });
            let denied = if closing_workspace {
                Some("previous sandbox is still closing")
            } else if account_full {
                Some("account guest capacity exhausted")
            } else if held >= self.max_sandboxes as usize {
                Some("machine sandbox capacity exhausted")
            } else {
                None
            };
            slots
                .entry(lease_id)
                .or_insert_with(|| {
                    Arc::new(Slot {
                        lease_id,
                        fence: AtomicU64::new(fence),
                        spec,
                        harness,
                        sandbox: RwLock::new(OnceCell::new_with(
                            denied.map(|reason| Err(reason.into())),
                        )),
                        last_use: Mutex::new(Instant::now()),
                        removed: AtomicBool::new(false),
                        stop: OnceCell::new(),
                        channel_started: AtomicBool::new(false),
                        keep_files: AtomicBool::new(false),
                        execs: Mutex::new(HashSet::new()),
                        inputs: Mutex::new(HashMap::new()),
                    })
                })
                .clone()
        };
        slot.fence.fetch_max(fence, Ordering::AcqRel);
        let me = self.clone();
        tokio::spawn(async move {
            let sandbox = slot.sandbox.read().await;
            let _ = me.sandbox(&slot, &sandbox).await;
        });
    }

    /// Apply one message. Starting sandboxes and execs run on their own tasks, so
    /// assignments and execs return at once. Stop awaits cleanup before a later
    /// assignment can reuse the same workspace.
    pub async fn handle(self: &Arc<Self>, message: Down) {
        match message {
            Down::OpenSpaceChannel { channel } => {
                let runner = self.clone();
                tokio::spawn(async move { runner.open_space_channel(channel).await });
            }
            Down::Assign {
                lease_id,
                fence,
                space_id,
                spec,
                ..
            } => self.assign(lease_id, fence, space_id, spec, None),
            Down::AssignHarness {
                lease_id,
                fence,
                space_id,
                spec,
                harness,
                ..
            } => self.assign(lease_id, fence, space_id, spec, Some(harness)),
            Down::Exec {
                interactive,
                lease_id,
                fence,
                exec_id,
                argv,
                stdin,
                mut env,
            } => {
                env.retain(|(key, _)| key != "CALTERNAL_URL");
                env.push(("CALTERNAL_URL".into(), self.link.base().to_owned()));
                let (ready, input) = tokio::sync::watch::channel(None);
                if interactive && let Some(slot) = self.slot(lease_id) {
                    slot.inputs
                        .lock()
                        .expect("inputs")
                        .entry(exec_id)
                        .or_insert(input);
                }
                let me = self.clone();
                let upload = self.uploads.clone();
                tokio::spawn(async move {
                    let _upload = upload;
                    let slot = me
                        .slot(lease_id)
                        .filter(|s| s.fence.load(Ordering::Acquire) == fence);
                    let guard = match &slot {
                        Some(slot) => Some(slot.sandbox.read().await),
                        None => None,
                    };
                    let exec = match (&slot, &guard) {
                        (Some(s), Some(sandbox)) => {
                            if !s.execs.lock().expect("execs").insert(exec_id) {
                                return; // delivered twice; already running
                            }
                            match me.sandbox(s, sandbox).await {
                                Err(reason) => failed_exec(reason),
                                Ok(sb) => sb
                                    .exec(ExecRequest {
                                        argv,
                                        stdin,
                                        env,
                                        interactive,
                                    })
                                    .await
                                    .unwrap_or_else(|e| failed_exec(e.0)),
                            }
                        }
                        _ => failed_exec("lease not held by this machine"),
                    };
                    let _ = ready.send(exec.input.clone());
                    let batch = slot.as_ref().is_none_or(|slot| slot.spec.worker.is_none());
                    me.upload(lease_id, fence, exec_id, exec, batch).await;
                    if let Some(slot) = &slot {
                        *slot.last_use.lock().expect("sandbox last use") = Instant::now();
                        slot.inputs.lock().expect("inputs").remove(&exec_id);
                    }
                    drop(guard);
                });
            }
            Down::Input {
                lease_id,
                fence,
                exec_id,
                data,
            } => {
                if data.len() > calternal_types::MAX_FRAME_BYTES + 1 {
                    return;
                }
                let Some(slot) = self
                    .slot(lease_id)
                    .filter(|s| s.fence.load(Ordering::Acquire) == fence)
                else {
                    return;
                };
                let input = slot.inputs.lock().expect("inputs").get(&exec_id).cloned();
                if let Some(mut input) = input {
                    let result = tokio::time::timeout(Duration::from_secs(5), async {
                        loop {
                            let writer = input.borrow().clone();
                            if let Some(writer) = writer {
                                return writer.try_send(data).map_err(|_| ());
                            }
                            if input.changed().await.is_err() {
                                return Err(());
                            }
                        }
                    })
                    .await;
                    if !matches!(result, Ok(Ok(()))) {
                        Self::close(&slot, slot.spec.worker.is_none()).await;
                    }
                }
            }
            Down::Erase {
                request_id,
                account_id,
                thread_id,
            } => {
                self.erase_owner_data(request_id, account_id, thread_id)
                    .await;
            }
            Down::Discard {
                request_id,
                account_id,
                thread_id,
            } => {
                if self
                    .discards
                    .lock()
                    .expect("discards")
                    .contains_key(&request_id)
                {
                    return;
                }
                let active = self
                    .slots
                    .lock()
                    .expect("slots")
                    .values()
                    .any(|s| s.spec.account_id == account_id && s.spec.thread_id == thread_id);
                let success = !active && self.backend.discard(account_id, thread_id).await.is_ok();
                self.discards
                    .lock()
                    .expect("discards")
                    .insert(request_id, success);
            }
            Down::Stop {
                lease_id,
                reason,
                keep_files,
            } => {
                if let Some(slot) = self.slot(lease_id) {
                    tracing::info!(lease = %lease_id, %reason, "stopping sandbox");
                    let keep_files = keep_files && slot.spec.worker.is_none();
                    if Self::close(&slot, keep_files).await {
                        let lacks_sandbox =
                            slot.sandbox.read().await.get().is_some_and(Result::is_err);
                        if !keep_files
                            && slot.spec.worker.is_none()
                            && lacks_sandbox
                            && let Err(error) = self
                                .backend
                                .discard(slot.spec.account_id, slot.spec.thread_id)
                                .await
                        {
                            tracing::warn!(lease = %lease_id, %error, "workspace removal failed");
                            return;
                        }
                        self.slots.lock().expect("slots").remove(&lease_id);
                    }
                }
            }
        }
    }

    async fn sandbox(
        &self,
        slot: &Slot,
        sandbox: &SandboxCell,
    ) -> Result<Arc<dyn Sandbox>, String> {
        let result = sandbox
            .get_or_init(|| async {
                if slot.removed.load(Ordering::Acquire) {
                    return Err("lease ended before sandbox start".into());
                }
                if slot.spec.worker.is_some() && self.backend.preference() == "direct" {
                    return Err("plugin workers require the container backend; run this machine with --backend auto or podman".into());
                }
                let started = match &slot.harness {
                    Some(harness) => self.backend.start_harness(&slot.spec, harness).await,
                    None => self.backend.start(&slot.spec).await,
                };
                let result = started.map_err(|e| {
                    tracing::warn!(lease = %slot.lease_id, error = %e, "sandbox start failed");
                    e.0
                });
                *slot.last_use.lock().expect("sandbox last use") = Instant::now();
                result
            })
            .await
            .clone();
        if slot.removed.load(Ordering::Acquire) {
            return Err("lease ended".into());
        }
        result
    }

    async fn close(slot: &Slot, keep_files: bool) -> bool {
        slot.keep_files.store(keep_files, Ordering::Release);
        slot.removed.store(true, Ordering::Release);
        // Join any start already in progress. Closing remains visible in the
        // slot map even if the connection future is cancelled during teardown.
        slot.sandbox
            .read()
            .await
            .get_or_init(|| async { Err("lease ended before sandbox start".into()) })
            .await;
        slot.stop_if_removed().await
    }

    /// Release idle turn containers after a lease TTL, keeping each logical
    /// lease and its files. Its next exec starts a new container on this machine.
    /// Scans at most the assigned slots; only a parked container incurs engine I/O.
    pub async fn park_idle(&self, idle: Duration) {
        if self.backend.name() == "direct" {
            return;
        }
        // Slow engine cleanup can outlast a poll. Keep at most one idle sweep
        // in progress while the connection continues to renew its leases.
        let Ok(_parking) = self.parking.try_lock() else {
            return;
        };
        for slot in self.all() {
            if slot.spec.worker.is_some() || slot.removed.load(Ordering::Acquire) {
                continue;
            }
            let Ok(mut cell) = slot.sandbox.try_write() else {
                continue;
            };
            let idle_for = slot.last_use.lock().expect("sandbox last use").elapsed();
            if idle_for >= idle
                && let Some(Ok(sb)) = cell.get()
            {
                match sb.release().await {
                    Ok(()) => {
                        cell.take();
                    }
                    Err(e) => {
                        tracing::warn!(lease = %slot.lease_id, error = %e, "idle sandbox removal failed")
                    }
                }
            }
        }
    }

    /// Close machine-owned sandboxes on orderly exit, revocation or lease expiry.
    /// Provider work and homes remain; worker work is transient. On expiry,
    /// closed slots stay as tombstones until the server reconciles them.
    /// Joins in-flight starts. Returns false if any teardown needs a retry.
    pub async fn release_all(&self) -> bool {
        let mut closed = true;
        for slot in self.all() {
            closed &= Self::close(&slot, slot.spec.worker.is_none()).await;
        }
        closed
    }

    pub(crate) async fn drain_outputs(&self) {
        // After the session stops dispatching, the count can only fall. Each
        // exec holds its reference until upload is acknowledged or fenced.
        while Arc::strong_count(&self.uploads) > 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
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
    /// the lease is fenced or the machine is revoked. Worker replies can
    /// unblock a broker round trip, so only ordinary exec output waits to batch.
    async fn upload(&self, lease_id: Uuid, fence: u64, exec_id: Uuid, mut exec: Exec, batch: bool) {
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
                if !ended && batch {
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
