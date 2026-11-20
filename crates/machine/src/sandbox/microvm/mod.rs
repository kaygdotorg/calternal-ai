//! Linux Firecracker guests, one persistent data disk per machine and account.
//! The dedicated control port is always attached. Online guests also lease an
//! operator-managed TAP. A rootless Podman container runs each thread.

mod config;
pub use config::Config;
mod disk;
mod guest;
mod image;
mod kvm;
#[cfg(test)]
mod measure;
mod network;
mod plain;
mod snapshot;
mod snapshot_api;
mod space;
pub(super) use space::SmallRuntime;
mod storage;
pub(crate) use storage::{erase_unconfigured, sweep_unconfigured};
mod vmm;
mod wire;

pub use guest::run_agent;

use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio::sync::{Mutex as AsyncMutex, Notify, OnceCell};
use uuid::Uuid;

use super::{Backend, BoxFuture, Sandbox, SandboxError, SandboxResult};
use crate::protocol::{GuestSize, SandboxSpec};
use vmm::Guest;
use wire::Request;

#[cfg(test)]
mod tests;
mod thread;
use thread::Thread;

/// Confirm the current user can open KVM and create a VM. Reports the
/// kernel error when nested virtualization or device access is unavailable.
pub async fn probe_kvm() -> SandboxResult<()> {
    tokio::task::spawn_blocking(|| kvm::check(Path::new("/dev/kvm")))
        .await
        .map_err(SandboxError::new)?
        .map_err(|error| SandboxError(format!("KVM is unavailable: {error}")))
}

const IDLE: Duration = Duration::from_secs(600);
const SWEEP: Duration = Duration::from_secs(15);

struct Account {
    vm: Option<Guest>,
    last_used: Instant,
}

/// Same-account gate for binding a disk to a VM, guest teardown, and erasure.
/// Held across vsock and disk flushes so a concurrent lease cannot reopen the
/// disk until teardown is durable. Other accounts never wait on this guard.
type AccountSlot = Arc<AsyncMutex<Account>>;

struct State {
    config: Config,
    root: PathBuf,
    owned: OnceCell<Arc<File>>,
    network: OnceCell<Option<Arc<network::Pool>>>,
    accounts: Mutex<HashMap<Uuid, AccountSlot>>,
    warm: AsyncMutex<Vec<Guest>>,
    capacity: Arc<tokio::sync::Semaphore>,
    stop: Notify,
    maintenance: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

/// Starts one Firecracker guest per account in the machine role's private root.
/// Account disks survive lease release, idle shutdown and machine restart.
pub struct MicrovmBackend(Arc<State>);

impl MicrovmBackend {
    /// Assemble a backend with no processes or disk I/O. The machine role
    /// calls initialization under its existing administrative ownership lock.
    pub fn new(root: PathBuf, config: Config) -> Self {
        let capacity = Arc::new(tokio::sync::Semaphore::new(config.max_guests));
        Self(Arc::new(State {
            config,
            root: root.join("microvm"),
            owned: OnceCell::new(),
            network: OnceCell::new(),
            accounts: Mutex::new(HashMap::new()),
            warm: AsyncMutex::new(Vec::new()),
            capacity,
            stop: Notify::new(),
            maintenance: Mutex::new(None),
        }))
    }
}

impl Backend for MicrovmBackend {
    fn name(&self) -> &'static str {
        "microvm"
    }

    fn guest_memory_mib(&self) -> Option<u32> {
        Some(self.0.config.memory_mib)
    }

    fn initialize(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            self.0.initialize().await?;
            let task = tokio::spawn(maintain(Arc::downgrade(&self.0)));
            *self.0.maintenance.lock().expect("maintenance") = Some(task);
            Ok(())
        })
    }

    fn shutdown(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            self.0.stop.notify_one();
            let task = self.0.maintenance.lock().expect("maintenance").take();
            if let Some(task) = task {
                task.await.map_err(SandboxError::new)?;
            }
            let mut failed = false;
            for slot in self.0.slots() {
                if let Some(vm) = slot.lock().await.vm.take() {
                    failed |= vm.stop().await.is_err();
                }
            }
            for vm in std::mem::take(&mut *self.0.warm.lock().await) {
                failed |= vm.stop().await.is_err();
            }
            if failed {
                return Err(SandboxError("microVM teardown incomplete".into()));
            }
            tokio::fs::remove_dir_all(self.0.root.join("run"))
                .await
                .map_err(SandboxError::new)
        })
    }

    fn start<'a>(
        &'a self,
        spec: &'a SandboxSpec,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        if spec.shared_provider_sponsor.is_some() {
            return Box::pin(async {
                Err(SandboxError(
                    "shared provider login is unavailable on this backend".into(),
                ))
            });
        }
        self.start_thread(spec, None)
    }

    fn start_harness<'a>(
        &'a self,
        spec: &'a SandboxSpec,
        harness: &'a crate::protocol::HarnessSandbox,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        self.start_thread(spec, Some(harness))
    }

    fn discard(&self, account: Uuid, thread: Uuid) -> BoxFuture<'_, SandboxResult<()>> {
        self.erase(account, Some(thread))
    }

    fn erase(&self, id: Uuid, thread: Option<Uuid>) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            let slot = self.0.account(id);
            let mut account = slot.lock().await;
            let disk = self.0.disk(id);
            if let Some(thread) = thread {
                if !tokio::fs::try_exists(&disk)
                    .await
                    .map_err(SandboxError::new)?
                {
                    return Ok(());
                }
                return self
                    .0
                    .ensure_guest(id, &mut account)
                    .await?
                    .control(&Request::Discard(id, thread))
                    .await;
            }
            if let Some(vm) = account.vm.take() {
                vm.stop().await?;
            }
            tokio::task::spawn_blocking(move || {
                match std::fs::remove_file(&disk) {
                    Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                    _ => {}
                }
                calternal_fsio::sync_dir(disk.parent().expect("data directory"))
            })
            .await
            .map_err(SandboxError::new)?
            .map_err(SandboxError::new)
        })
    }
}

fn account_disk(root: &Path, account: Uuid) -> PathBuf {
    root.join("accounts").join(format!("{account}.ext4"))
}

/// Remove an offline account disk while container fallback owns the role.
pub(super) async fn erase_without_kvm(
    sandbox_root: &Path,
    account: Uuid,
    thread: Option<Uuid>,
) -> SandboxResult<()> {
    let root = sandbox_root.join("microvm");
    tokio::task::spawn_blocking(move || {
        let disk = account_disk(&root, account);
        if !disk.try_exists()? {
            return Ok(());
        }
        if thread.is_some() {
            return Err(io::Error::other(
                "KVM is required to erase one workspace inside its account disk",
            ));
        }
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.join("vmm.lock"))?;
        lock.try_lock().map_err(io::Error::other)?;
        std::fs::remove_file(&disk)?;
        calternal_fsio::sync_dir(disk.parent().expect("data directory"))
    })
    .await
    .map_err(SandboxError::new)?
    .map_err(SandboxError::new)
}

impl State {
    fn disk(&self, account: Uuid) -> PathBuf {
        account_disk(&self.root, account)
    }

    fn account(&self, id: Uuid) -> AccountSlot {
        self.accounts
            .lock()
            .expect("microVM slots")
            .entry(id)
            .or_insert_with(|| {
                Arc::new(AsyncMutex::new(Account {
                    vm: None,
                    last_used: Instant::now(),
                }))
            })
            .clone()
    }

    fn slots(&self) -> Vec<AccountSlot> {
        self.accounts
            .lock()
            .expect("microVM slots")
            .values()
            .cloned()
            .collect()
    }

    async fn initialize(&self) -> SandboxResult<()> {
        probe_kvm().await?;
        self.network
            .set(
                network::Pool::load(self.config.network_policy.clone(), self.config.max_guests)
                    .await?,
            )
            .map_err(|_| SandboxError("microVM network already initialized".into()))?;
        image::check(&self.config).await?;
        let root = self.root.clone();
        let lock = tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&root)?;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))?;
            let lock = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .open(root.join("vmm.lock"))?;
            lock.try_lock().map_err(|_| {
                io::Error::other("previous Firecracker processes still hold the sandbox root")
            })?;
            // The VMM inherits this lock. Startup clears the abandoned runtime
            // files only after every process from the old role closed it.
            let run = root.join("run");
            match std::fs::remove_dir_all(&run) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
            std::fs::create_dir(&run)?;
            calternal_fsio::create_dir_all_durable(&root.join("accounts"))?;
            Ok::<_, io::Error>(lock)
        })
        .await
        .map_err(SandboxError::new)?
        .map_err(SandboxError::new)?;
        self.owned
            .set(Arc::new(lock))
            .map_err(|_| SandboxError("microVM backend already initialized".into()))
    }

    async fn ensure_guest<'a>(
        &self,
        id: Uuid,
        account: &'a mut Account,
    ) -> SandboxResult<&'a Guest> {
        account.last_used = Instant::now();
        if account.vm.as_mut().is_some_and(Guest::alive) {
            return Ok(account.vm.as_ref().expect("live guest"));
        }
        if let Some(vm) = account.vm.take() {
            vm.stop().await?;
        }
        let disk = self.disk(id);
        let fresh = !tokio::fs::try_exists(&disk)
            .await
            .map_err(SandboxError::new)?;
        if fresh {
            let warm_guest = self.warm.lock().await.pop();
            if let Some(mut vm) = warm_guest {
                if vm.alive() {
                    if let Err(error) = persist_disk(vm.disk.clone(), disk.clone()).await {
                        self.remove_warm(vm).await?;
                        return Err(error);
                    }
                    vm.disk = disk;
                    account.vm = Some(vm);
                    return Ok(account.vm.as_ref().expect("bound warm guest"));
                }
                self.remove_warm(vm).await?;
            }
            let staged = self.new_disk().await?;
            persist_disk(staged, disk.clone()).await?;
        } else {
            let existing = disk.clone();
            tokio::task::spawn_blocking(move || disk::check(&existing))
                .await
                .map_err(SandboxError::new)?
                .map_err(SandboxError::new)?;
        }
        if self.capacity.available_permits() == 0 {
            let unused = self.warm.lock().await.pop();
            if let Some(vm) = unused {
                self.remove_warm(vm).await?;
            }
        }
        account.vm = Some(self.boot(disk).await?);
        Ok(account.vm.as_ref().expect("booted guest"))
    }

    async fn remove_warm(&self, vm: Guest) -> SandboxResult<()> {
        let disk = vm.disk.clone();
        vm.stop().await?;
        match tokio::fs::remove_file(disk).await {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(SandboxError::new(e)),
            _ => Ok(()),
        }
    }

    async fn boot(&self, disk: PathBuf) -> SandboxResult<Guest> {
        let owned = self
            .owned
            .get()
            .ok_or_else(|| SandboxError("microVM backend not initialized".into()))?;
        let permit = self
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| SandboxError("microVM guest capacity is full".into()))?;
        Guest::boot(
            &self.config,
            &self.root.join("run"),
            disk,
            owned.clone(),
            permit,
            self.network
                .get()
                .and_then(Option::as_ref)
                .map(|pool| pool.lease())
                .transpose()?,
        )
        .await
    }

    async fn new_disk(&self) -> SandboxResult<PathBuf> {
        let path = self
            .root
            .join("run")
            .join(format!("{}.ext4", Uuid::new_v4()));
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .await
            .map_err(SandboxError::new)?;
        let created = async {
            file.set_len(u64::from(self.config.disk_mib) << 20)
                .await
                .map_err(SandboxError::new)?;
            let status = Command::new(&self.config.mkfs_ext4)
                .args(["-q", "-F", "-L", disk::LABEL])
                .args(["-E", "lazy_itable_init=0,lazy_journal_init=0"])
                .arg(&path)
                .kill_on_drop(true)
                .status()
                .await
                .map_err(SandboxError::new)?;
            if !status.success() {
                return Err(SandboxError("cannot format microVM data disk".into()));
            }
            file.sync_all().await.map_err(SandboxError::new)
        }
        .await;
        if created.is_err() {
            let _ = tokio::fs::remove_file(&path).await;
        }
        created.map(|()| path)
    }
}

async fn persist_disk(source: PathBuf, target: PathBuf) -> SandboxResult<()> {
    tokio::task::spawn_blocking(move || calternal_fsio::rename_safely(&source, &target))
        .await
        .map_err(SandboxError::new)?
        .map_err(SandboxError::new)
}

async fn maintain(backend: Weak<State>) {
    loop {
        let Some(state) = backend.upgrade() else {
            return;
        };
        let slots = state.slots();
        for slot in slots {
            // The registry and this iteration each own one reference. An active
            // sandbox or pending start pins the guest. No map lock spans I/O.
            if let Ok(mut account) = slot.try_lock() {
                if Arc::strong_count(&slot) == 2
                    && account.last_used.elapsed() >= IDLE
                    && let Some(vm) = account.vm.take()
                    && let Err(error) = vm.stop().await
                {
                    tracing::warn!(%error, "cannot stop idle microVM");
                }
            }
        }
        state
            .accounts
            .lock()
            .expect("microVM slots")
            .retain(|_, slot| {
                Arc::strong_count(slot) > 1
                    || slot.try_lock().map_or(true, |account| account.vm.is_some())
            });
        let (warm, dead) = {
            let mut warm = state.warm.lock().await;
            let mut dead = Vec::new();
            let mut index = 0;
            while index < warm.len() {
                if warm[index].alive() {
                    index += 1;
                } else {
                    dead.push(warm.swap_remove(index));
                }
            }
            (warm.len(), dead)
        };
        for vm in dead {
            if let Err(error) = state.remove_warm(vm).await {
                tracing::warn!(%error, "cannot remove exited warm microVM");
            }
        }
        if warm < state.config.warm_guests
            && state.capacity.available_permits() > 1
            && warm_memory_available(state.config.memory_mib).await
        {
            let started = async {
                let disk = state.new_disk().await?;
                match state.boot(disk.clone()).await {
                    Ok(vm) => {
                        state.warm.lock().await.push(vm);
                        Ok(())
                    }
                    Err(error) => {
                        let _ = tokio::fs::remove_file(disk).await;
                        Err(error)
                    }
                }
            }
            .await;
            if let Err(error) = started {
                tracing::warn!(%error, "cannot preboot microVM");
            }
        }
        tokio::select! {
            () = state.stop.notified() => return,
            () = tokio::time::sleep(SWEEP) => {}
        }
    }
}

impl MicrovmBackend {
    fn start_thread<'a>(
        &'a self,
        spec: &'a SandboxSpec,
        harness: Option<&'a crate::protocol::HarnessSandbox>,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        Box::pin(async move {
            let space = spec
                .space_id
                .ok_or_else(|| SandboxError("missing sandbox space".into()))?;
            if let Some(harness) = harness
                && harness.guest_image_digest.as_deref() != Some(&self.0.config.image_digest)
            {
                return Err(SandboxError(
                    "installed harness requires a different guest image".into(),
                ));
            }
            let slot = self.0.account(spec.account_id);
            let mut account = slot.lock().await;
            let vm = self.0.ensure_guest(spec.account_id, &mut account).await?;
            let request = match harness {
                Some(harness) => Request::StartHarness(space, spec.clone(), harness.clone()),
                None => Request::Start(space, spec.clone()),
            };
            vm.control(&request).await?;
            let thread = Thread::new(
                slot.clone(),
                vm.socket.clone(),
                vm.key.clone(),
                spec.thread_id,
                spec.worker.is_some(),
            );
            Ok(Arc::new(thread) as Arc<dyn Sandbox>)
        })
    }
}

/// One host sample on the warm-guest maintenance path. Leave one additional
/// guest reservation available for normal turn placement.
async fn warm_memory_available(memory_mib: u32) -> bool {
    let Some(size) = GuestSize::from_mib(memory_mib) else {
        return false;
    };
    tokio::task::spawn_blocking(move || {
        crate::memory::sample()
            .1
            .is_some_and(|available| available >= 2 * size.host_memory_bytes())
    })
    .await
    .unwrap_or(false)
}
