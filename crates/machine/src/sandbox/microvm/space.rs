//! Dedicated offline placements for the small plugin guest.
use super::snapshot::{Binding, Generation};
use super::*;
use calternal_types::SpaceId;
use tokio::sync::{OwnedMutexGuard, OwnedSemaphorePermit};

type PlacementGate = Arc<OwnedMutexGuard<()>>;

const GATE_WAIT: Duration = Duration::from_secs(3);
const RETENTION: Duration = Duration::from_secs(24 * 60 * 60);
const RETENTION_SWEEP: Duration = Duration::from_secs(15 * 60);

/// Machine-local capacity and per-space exclusion for small offline guests.
pub(in crate::sandbox) struct SmallRuntime {
    state: Arc<State>,
    slots: Mutex<HashMap<calternal_types::SpaceId, Weak<AsyncMutex<()>>>>,
    version: OnceCell<String>,
}

impl SmallRuntime {
    /// Read the small VM configuration without starting a VMM.
    pub(in crate::sandbox) async fn load(root: PathBuf, path: &Path) -> SandboxResult<Self> {
        let config = Config::load_profile(path, true).await?;
        Ok(Self {
            state: MicrovmBackend::new(root.join("space-runtime"), config).0,
            slots: Mutex::default(),
            version: OnceCell::new(),
        })
    }

    /// Verify the executable and image, then sweep abandoned scratch and expired snapshots.
    pub(in crate::sandbox) async fn initialize(self: &Arc<Self>) -> SandboxResult<()> {
        let mut command = Command::new(&self.state.config.firecracker);
        command.arg("--version").kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(2), command.output())
            .await
            .map_err(SandboxError::new)?
            .map_err(SandboxError::new)?;
        let version = std::str::from_utf8(&output.stdout)
            .ok()
            .and_then(|text| text.lines().next())
            .and_then(|line| line.strip_prefix("Firecracker "))
            .filter(|version| !version.is_empty() && version.len() < 128)
            .filter(|_| output.status.success())
            .ok_or_else(|| {
                SandboxError("Cannot determine the small VM Firecracker version".into())
            })?;
        self.version
            .set(version.to_owned())
            .map_err(|_| SandboxError("Small plugin VM is already initialized".into()))?;
        self.state.initialize().await?;
        self.sweep_expired().await?;
        let runtime = Arc::downgrade(self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(RETENTION_SWEEP).await;
                let Some(runtime) = runtime.upgrade() else {
                    return;
                };
                if let Err(error) = runtime.sweep_expired().await {
                    tracing::warn!(%error, "cannot prune expired small plugin VM snapshots");
                }
            }
        });
        Ok(())
    }

    async fn sweep_expired(&self) -> SandboxResult<()> {
        let root = self.state.root.join("spaces");
        let mut entries = match tokio::fs::read_dir(root).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(SandboxError::new(error)),
        };
        while let Some(entry) = entries.next_entry().await.map_err(SandboxError::new)? {
            let name = entry.file_name();
            let Some(space) = name.to_str().and_then(|name| name.parse::<SpaceId>().ok()) else {
                continue;
            };
            if let Ok(gate) = self.gate(space).try_lock_owned() {
                let path = entry.path();
                if super::snapshot::expired(&path, RETENTION).await? {
                    erase_saved(path, Arc::new(gate)).await?;
                }
            }
        }
        Ok(())
    }

    /// Reserve a space and a capacity slot, then restore or boot and admit its worker.
    pub(in crate::sandbox) async fn start(
        &self,
        spec: &SandboxSpec,
    ) -> SandboxResult<Arc<Placement>> {
        let space = spec
            .space_id
            .filter(|space| {
                spec.worker
                    .as_ref()
                    .is_some_and(|worker| worker.space_id == *space)
            })
            .ok_or_else(|| SandboxError("small plugin VM has no matching worker".into()))?;
        if spec.provider_login
            || spec.shared_tools.is_some()
            || spec.shared_provider_sponsor.is_some()
        {
            return Err(SandboxError(
                "small plugin VM cannot mount provider credentials".into(),
            ));
        }
        // The space lock spans VM startup and durable publication. It excludes
        // simultaneous device reuse, snapshot replacement and physical erasure.
        let gate = Arc::new(
            tokio::time::timeout(GATE_WAIT, self.gate(space).lock_owned())
                .await
                .map_err(|_| {
                    SandboxError("Previous small plugin VM is still shutting down".into())
                })?,
        );
        let saved = self.saved(space);
        if super::snapshot::expired(&saved, RETENTION).await? {
            erase_saved(saved.clone(), gate.clone()).await?;
        }
        let binding = Binding {
            space,
            image: self.state.config.image_digest.clone(),
            firecracker: self
                .version
                .get()
                .ok_or_else(|| SandboxError("Small plugin VM is not initialized".into()))?
                .clone(),
        };
        let permit = Arc::new(
            self.state
                .capacity
                .clone()
                .try_acquire_owned()
                .map_err(|_| SandboxError("small plugin VM capacity is full".into()))?,
        );
        let owned = self
            .state
            .owned
            .get()
            .cloned()
            .ok_or_else(|| SandboxError("small plugin VM is not initialized".into()))?;
        let generation = {
            let saved = saved.clone();
            let binding = binding.clone();
            tokio::task::spawn_blocking(move || Generation::current(&saved, &binding))
                .await
                .map_err(SandboxError::new)?
        };
        let run = self.state.root.join("run");
        let restored = match generation {
            Ok(Some(generation)) => {
                match self
                    .restore(&run, &generation, owned.clone(), permit.clone(), &gate)
                    .await
                {
                    Ok(restored) => Some(restored),
                    Err(error) => {
                        tracing::warn!(%space, %error, "small plugin VM restore failed; starting cold");
                        clear_restore_scratch(&run, &generation, gate.clone()).await?;
                        erase_saved(saved.clone(), gate.clone()).await?;
                        None
                    }
                }
            }
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(%space, %error, "refusing inconsistent small VM snapshot; starting cold");
                erase_saved(saved.clone(), gate.clone()).await?;
                None
            }
        };
        let (vm, disk) = match restored {
            Some(restored) => restored,
            None => {
                let disk = Scratch {
                    path: self.state.new_disk().await?,
                    cleanup: Mutex::new(Some(tokio::runtime::Handle::current())),
                };
                let vm =
                    Guest::boot_small(&self.state.config, &run, disk.path.clone(), owned, permit)
                        .await?;
                (vm, disk)
            }
        };
        if let Err(error) = vm.control(&Request::Start(space, spec.clone())).await {
            vm.stop().await?;
            disk.clear().await?;
            return Err(error);
        }
        let socket = vm.socket.clone();
        let key = vm.key.clone();
        let slot = Arc::new(AsyncMutex::new(Account {
            vm: Some(vm),
            last_used: Instant::now(),
        }));
        let thread = Thread::new(slot.clone(), socket, key, spec.thread_id, true);
        Ok(Arc::new(Placement {
            slot,
            thread,
            disk,
            gate: Mutex::new(Some(gate)),
            saved,
            binding,
        }))
    }

    fn gate(&self, space: SpaceId) -> Arc<AsyncMutex<()>> {
        let mut slots = self.slots.lock().expect("small runtime registry");
        slots.retain(|_, gate| gate.strong_count() != 0);
        let weak = slots.entry(space).or_default();
        weak.upgrade().unwrap_or_else(|| {
            let gate = Arc::new(AsyncMutex::new(()));
            *weak = Arc::downgrade(&gate);
            gate
        })
    }

    fn saved(&self, space: SpaceId) -> PathBuf {
        self.state.root.join("spaces").join(space.to_string())
    }

    async fn restore(
        &self,
        run: &Path,
        generation: &Generation,
        owned: Arc<File>,
        permit: Arc<OwnedSemaphorePermit>,
        gate: &PlacementGate,
    ) -> SandboxResult<(Guest, Scratch)> {
        let disk = Scratch {
            path: run.join(&generation.disk_name),
            cleanup: Mutex::new(None),
        };
        let control = run.join(&generation.control_name);
        let disk_path = disk.path.clone();
        let control_path = control.clone();
        let source = generation.path.clone();
        // A cancelled start can drop its future while the blocking copy
        // is still writing the device names captured in this snapshot.
        let copy_gate = gate.clone();
        tokio::task::spawn_blocking(move || {
            let _gate = copy_gate;
            for (from, to) in [
                (source.join("disk"), disk_path),
                (source.join("control"), control_path),
            ] {
                remove_file(&to)?;
                super::snapshot::clone_or_copy(&from, &to)?;
            }
            Ok::<(), io::Error>(())
        })
        .await
        .map_err(SandboxError::new)?
        .map_err(SandboxError::new)?;
        let vm = Guest::restore_small(
            &self.state.config,
            run,
            generation,
            disk.path.clone(),
            control,
            owned,
            permit,
        )
        .await?;
        Ok((vm, disk))
    }

    /// Delete an offline space checkpoint after the runner has stopped its leases.
    /// Keep this space's gate through directory sync so a racing start cannot
    /// reopen or recreate the path mid-erasure.
    pub(in crate::sandbox) async fn erase(&self, space: SpaceId) -> SandboxResult<()> {
        let gate = tokio::time::timeout(GATE_WAIT, self.gate(space).lock_owned())
            .await
            .map_err(|_| SandboxError("Small plugin VM is still shutting down".into()))?;
        erase_saved(self.saved(space), Arc::new(gate)).await
    }
}

fn remove_file(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

async fn clear_restore_scratch(
    run: &Path,
    generation: &Generation,
    gate: PlacementGate,
) -> SandboxResult<()> {
    let disk = run.join(&generation.disk_name);
    let control = run.join(&generation.control_name);
    tokio::task::spawn_blocking(move || {
        let _gate = gate;
        let disk = remove_file(&disk);
        let control = remove_file(&control);
        disk.and(control)
    })
    .await
    .map_err(SandboxError::new)?
    .map_err(SandboxError::new)
}

async fn erase_saved(path: PathBuf, gate: PlacementGate) -> SandboxResult<()> {
    tokio::task::spawn_blocking(move || {
        let _gate = gate;
        match std::fs::remove_dir_all(&path) {
            Ok(()) => calternal_fsio::sync_dir(path.parent().expect("runtime space parent")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    })
    .await
    .map_err(SandboxError::new)?
    .map_err(SandboxError::new)
}

// Failed cold boots may remove unused scratch asynchronously. A published
// device name is reusable and must only be removed under the placement gate.
struct Scratch {
    path: PathBuf,
    cleanup: Mutex<Option<tokio::runtime::Handle>>,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Some(runtime) = self.cleanup.get_mut().expect("scratch cleanup").take() {
            let path = self.path.clone();
            runtime.spawn(async move {
                let _ = tokio::fs::remove_file(path).await;
            });
        }
    }
}

impl Scratch {
    async fn clear(&self) -> SandboxResult<()> {
        match tokio::fs::remove_file(&self.path).await {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                return Err(SandboxError::new(error));
            }
            _ => {}
        }
        // A published snapshot may reopen this same device name immediately.
        // Disarm deferred cleanup before the caller releases the space gate.
        self.disarm_cleanup();
        Ok(())
    }

    fn disarm_cleanup(&self) {
        self.cleanup.lock().expect("scratch cleanup").take();
    }
}

/// A live worker and the exclusive guard for its checkpoint and device files.
/// The space gate spans VMM lifetime and publication. The placement slot lock
/// serializes teardown through the snapshot commit and waitpid so stop and
/// release cannot act on the same VMM. Other spaces use independent locks.
pub(in crate::sandbox) struct Placement {
    slot: AccountSlot,
    thread: Thread,
    disk: Scratch,
    gate: Mutex<Option<PlacementGate>>,
    saved: PathBuf,
    binding: Binding,
}

impl Sandbox for Placement {
    fn available(&self) -> bool {
        self.thread.available()
    }

    fn exec(
        &self,
        request: crate::sandbox::ExecRequest,
    ) -> BoxFuture<'_, SandboxResult<crate::sandbox::Exec>> {
        self.thread.exec(request)
    }

    fn freeze(&self) -> BoxFuture<'_, SandboxResult<()>> {
        self.thread.freeze()
    }

    fn resume(&self) -> BoxFuture<'_, SandboxResult<()>> {
        self.thread.resume()
    }

    fn stop(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            // Same-placement shutdown excludes a second teardown while waitpid
            // confirms that the VMM and all guest processes are gone.
            let mut slot = self.slot.lock().await;
            if let Some(vm) = slot.vm.take() {
                vm.stop().await?;
                self.disk.clear().await?;
            }
            self.gate.lock().expect("placement gate").take();
            Ok(())
        })
    }

    fn release(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            let quiesced = self.thread.stop().await;
            let mut slot = self.slot.lock().await;
            if let Some(vm) = slot.vm.take() {
                let saved = match quiesced {
                    Ok(()) => {
                        self.disk.disarm_cleanup();
                        let gate = self
                            .gate
                            .lock()
                            .expect("placement gate")
                            .as_ref()
                            .expect("live placement holds the gate")
                            .clone();
                        save(&vm, &self.saved, &self.binding, gate).await
                    }
                    Err(error) => Err(error),
                };
                vm.stop().await?;
                self.disk.clear().await?;
                if let Err(error) = saved {
                    tracing::warn!(%error, "small plugin VM snapshot failed; preserving previous snapshot");
                }
            }
            self.gate.lock().expect("placement gate").take();
            Ok(())
        })
    }
}

async fn save(
    vm: &Guest,
    root: &Path,
    binding: &Binding,
    gate: PlacementGate,
) -> SandboxResult<()> {
    use super::{
        snapshot::{Generation, clone_or_copy},
        snapshot_api,
    };
    let api = vm
        .api_socket
        .as_ref()
        .ok_or_else(|| SandboxError("snapshot API unavailable".into()))?;
    let control = vm
        .control_disk
        .as_ref()
        .ok_or_else(|| SandboxError("snapshot control drive unavailable".into()))?;
    let root = root.to_owned();
    let binding = binding.clone();
    let disk = vm.disk.clone();
    let control = control.clone();
    let mut generation = {
        let root = root.clone();
        let disk = disk.clone();
        let control = control.clone();
        let draft_gate = gate.clone();
        tokio::task::spawn_blocking(move || {
            let _gate = draft_gate;
            Generation::create(&root, &disk, &control)
        })
        .await
        .map_err(SandboxError::new)?
        .map_err(SandboxError::new)?
    };
    vm.control(&Request::QuiesceSmall).await?;
    snapshot_api::pause(api).await?;
    if let Some(restored) = vm.restored {
        let previous = {
            let root = root.clone();
            let binding = binding.clone();
            tokio::task::spawn_blocking(move || Generation::current(&root, &binding))
                .await
                .map_err(SandboxError::new)?
                .map_err(SandboxError::new)?
        }
        .filter(|generation| generation.id == restored);
        if let Some(previous) = previous {
            let dirty = generation.path.join("dirty");
            snapshot_api::create(api, &generation.path.join("state"), &dirty, true).await?;
            let target = generation.path.join("memory");
            let merge_gate = gate.clone();
            if tokio::task::spawn_blocking(move || {
                let _gate = merge_gate;
                let merged =
                    super::snapshot::merge_dirty(&previous.path.join("memory"), &dirty, &target)?;
                std::fs::remove_file(dirty)?;
                Ok::<_, io::Error>(merged)
            })
            .await
            .map_err(SandboxError::new)?
            .map_err(SandboxError::new)?
            {
                generation.storage = super::snapshot::Storage::Reflink;
            } else {
                tokio::fs::remove_file(generation.path.join("state"))
                    .await
                    .map_err(SandboxError::new)?;
            }
        }
    }
    if matches!(generation.storage, super::snapshot::Storage::Full) {
        snapshot_api::create(
            api,
            &generation.path.join("state"),
            &generation.path.join("memory"),
            false,
        )
        .await?;
    }
    tokio::task::spawn_blocking(move || {
        let _gate = gate;
        clone_or_copy(&disk, &generation.path.join("disk"))?;
        clone_or_copy(&control, &generation.path.join("control"))?;
        generation.publish(&root, &binding)?;
        tracing::info!(
            snapshot_format = generation.storage.label(),
            generation = %generation.id,
            "small plugin VM asleep"
        );
        // Only the newly published generation is needed after commit. An
        // interrupted write leaves its predecessor's pointer usable.
        super::snapshot::prune_drafts(&root, Some(generation.id))?;
        Ok::<(), io::Error>(())
    })
    .await
    .map_err(SandboxError::new)?
    .map_err(SandboxError::new)
}

#[cfg(test)]
#[path = "space_tests.rs"]
mod tests;
