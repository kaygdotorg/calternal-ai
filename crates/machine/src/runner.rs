//! Applies server messages to sandboxes and streams exec output back.
//!
//! One slot per lease. A slot starts its sandbox once, on the first message that
//! needs it; execs arriving meanwhile wait for the same start. Each exec has an
//! output task that numbers events, uploads them in batches and keeps at most
//! [`UNACKED_BYTES`] unacknowledged; beyond that it stops reading, which blocks
//! the process (see [`crate::sandbox`]). Memory per running exec is therefore
//! bounded by about 256 KiB in the channel plus 256 KiB here.

mod erasure;
mod lifecycle;
/// Supervision of the isolated v2 package pipe.
mod space_channel;
mod upload;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::{OnceCell, RwLock};
use uuid::Uuid;

use crate::Error;
use crate::link::Link;
use crate::protocol::{Capacity, Down, LeaseReport, SandboxSpec, SandboxState};
use crate::sandbox::{Backend, ExecRequest, Sandbox, failed_exec};

/// Most output bytes an exec keeps waiting for the server's acknowledgement.
pub const UNACKED_BYTES: usize = 256 * 1024;

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
    /// The first terminal action chooses whether teardown preserves files.
    closed: OnceLock<bool>,
    stop: OnceCell<Option<u64>>,
    sleep_counter: OnceLock<u64>,
    restore_checkpoint: OnceLock<Option<u64>>,
    /// A v2 placement is supervised once and the supervisor owns its teardown.
    channel_started: AtomicBool,
    execs: Mutex<HashSet<Uuid>>,
    inputs: Mutex<
        HashMap<Uuid, tokio::sync::watch::Receiver<Option<tokio::sync::mpsc::Sender<Vec<u8>>>>>,
    >,
}

impl Slot {
    fn is_small_vm(&self) -> bool {
        self.spec.image.as_deref() == Some(crate::sandbox::worker::SPACE_VM_IMAGE)
    }

    /// Stop the sandbox once, if the slot was removed and its start succeeded.
    async fn stop_if_removed(&self) -> bool {
        if let Some(&keep_files) = self.closed.get()
            && let Some(Ok(sb)) = self.sandbox.read().await.get()
        {
            return self
                .stop
                .get_or_try_init(|| async {
                    let result = match self.sleep_counter.get().copied().filter(|_| keep_files) {
                        Some(counter) => sb
                            .checkpoint(counter)
                            .await
                            .map(|saved| saved.then_some(counter)),
                        None if keep_files => sb.release().await.map(|()| None),
                        None => sb.stop().await.map(|()| None),
                    };
                    result.map_err(|e| {
                        tracing::warn!(lease=%self.lease_id,error=%e,"sandbox stop failed");
                    })
                })
                .await
                .is_ok();
        }
        let _ = self.stop.set(None);
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
    #[cfg(target_os = "linux")]
    space_runtime_root: Option<std::path::PathBuf>,
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
            #[cfg(target_os = "linux")]
            space_runtime_root: None,
        }
    }

    /// Bind this role's sandbox root for erasures after a backend change.
    #[cfg(target_os = "linux")]
    pub(crate) fn with_space_runtime_root(mut self, root: std::path::PathBuf) -> Self {
        self.space_runtime_root = Some(root);
        self
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
            small_plugin_vm: self.backend.supports_small_plugin_vm(),
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
                sandbox: match s.closed.get() {
                    Some(true) if s.is_small_vm() => {
                        if let Some(&Some(counter)) = s.stop.get() {
                            SandboxState::CheckpointSaved { counter }
                        } else {
                            SandboxState::SleepPending
                        }
                    }
                    Some(_) => SandboxState::Failed {
                        reason: "the local lease expired or is closing".into(),
                    },
                    None => match s.sandbox.try_read().ok().as_deref().and_then(OnceCell::get) {
                        None => SandboxState::Starting,
                        Some(Err(reason)) => SandboxState::Failed {
                            reason: reason.clone(),
                        },
                        Some(Ok(sandbox)) if !sandbox.available() => SandboxState::Lost,
                        Some(Ok(_)) => SandboxState::Running,
                    },
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
                        closed: OnceLock::new(),
                        stop: OnceCell::new(),
                        sleep_counter: OnceLock::new(),
                        restore_checkpoint: OnceLock::new(),
                        channel_started: AtomicBool::new(false),
                        execs: Mutex::new(HashSet::new()),
                        inputs: Mutex::new(HashMap::new()),
                    })
                })
                .clone()
        };
        slot.fence.fetch_max(fence, Ordering::AcqRel);
        if slot.is_small_vm() {
            // An assignment alone is insufficient to start the plugin wall.
            // The enrolled carrier validates the binding before wake.
            return;
        }
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
            Down::EraseSpaceRuntime {
                request_id,
                space_id,
                through,
                background,
            } => {
                self.erase_space_runtime(request_id, space_id, through, background)
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
            Down::SleepSpace { lease_id, counter } => {
                if let Some(slot) = self.slot(lease_id)
                    && slot.spec.worker.is_some()
                    && slot.is_small_vm()
                {
                    slot.sleep_counter.get_or_init(|| counter);
                    if Self::close(&slot, true).await && !matches!(slot.stop.get(), Some(Some(_))) {
                        self.slots.lock().expect("slots").remove(&lease_id);
                    }
                }
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
                if slot.closed.get().is_some() {
                    return Err("lease ended before sandbox start".into());
                }
                if slot.spec.worker.is_some() && self.backend.preference() == "direct" {
                    return Err("plugin workers require the container backend; run this machine with --backend auto or podman".into());
                }
                let started = match &slot.harness {
                    Some(harness) => self.backend.start_harness(&slot.spec, harness).await,
                    None if slot.is_small_vm() => {
                        if !slot.channel_started.load(Ordering::Acquire) {
                            return Err("small plugin VM requires a space carrier".into());
                        }
                        self.backend.start_space(&slot.spec, slot.restore_checkpoint.get().copied().flatten()).await
                    }
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
        if slot.closed.get().is_some() {
            return Err("lease ended".into());
        }
        result
    }

    async fn close(slot: &Slot, keep_files: bool) -> bool {
        slot.closed.get_or_init(|| keep_files);
        // Join any start already in progress. Closing remains visible in the
        // slot map even if the connection future is cancelled during teardown.
        slot.sandbox
            .read()
            .await
            .get_or_init(|| async { Err("lease ended before sandbox start".into()) })
            .await;
        slot.stop_if_removed().await
    }

    fn all(&self) -> Vec<Arc<Slot>> {
        self.slots
            .lock()
            .expect("slots")
            .values()
            .cloned()
            .collect()
    }
}
