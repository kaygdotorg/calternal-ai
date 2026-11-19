//! Dedicated offline placements for the small plugin guest.
use super::*;

pub(in crate::sandbox) struct SmallRuntime {
    state: Arc<State>,
    slots: Mutex<HashMap<calternal_types::SpaceId, Weak<AsyncMutex<()>>>>,
}

impl SmallRuntime {
    pub(in crate::sandbox) async fn load(root: PathBuf, path: &Path) -> SandboxResult<Self> {
        let config = Config::load_profile(path, true).await?;
        Ok(Self {
            state: MicrovmBackend::new(root.join("space-runtime"), config).0,
            slots: Mutex::default(),
        })
    }

    pub(in crate::sandbox) async fn initialize(&self) -> SandboxResult<()> {
        self.state.initialize().await
    }

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
        let gate = {
            let mut slots = self.slots.lock().expect("small runtime registry");
            slots.retain(|_, gate| gate.strong_count() != 0);
            let weak = slots.entry(space).or_default();
            weak.upgrade().unwrap_or_else(|| {
                let gate = Arc::new(AsyncMutex::new(()));
                *weak = Arc::downgrade(&gate);
                gate
            })
        };
        // Hold across VMM work and fsync so an admission cannot race snapshot
        // publication or attach another VMM to this space's saved device paths.
        let gate = gate.lock_owned().await;
        let saved = self.state.root.join("spaces").join(space.to_string());
        let image = self.state.config.image_digest.clone();
        // Reserve VMM capacity before allocating scratch disk resources.
        let permit = self
            .state
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| SandboxError("small plugin VM capacity is full".into()))?;
        let owned = self
            .state
            .owned
            .get()
            .cloned()
            .ok_or_else(|| SandboxError("small plugin VM is not initialized".into()))?;
        let generation = {
            let saved = saved.clone();
            let image = image.clone();
            tokio::task::spawn_blocking(move || {
                super::snapshot::Generation::current(&saved, &image)
            })
            .await
            .map_err(SandboxError::new)?
        };
        let generation = match generation {
            Ok(generation) => generation,
            Err(error) => {
                tracing::warn!(%error, "refusing inconsistent small VM snapshot; starting cold");
                None
            }
        };
        let run = self.state.root.join("run");
        let runtime = tokio::runtime::Handle::current();
        let (vm, disk) = if let Some(generation) = generation {
            let disk = Scratch {
                path: run.join(&generation.disk_name),
                cleanup: None,
            };
            let control = run.join(&generation.control_name);
            let disk_path = disk.path.clone();
            let control_path = control.clone();
            let source = generation.path.clone();
            tokio::task::spawn_blocking(move || {
                for (from, to) in [
                    (source.join("disk"), disk_path),
                    (source.join("control"), control_path),
                ] {
                    match std::fs::remove_file(&to) {
                        Ok(()) => {}
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error),
                    }
                    super::snapshot::clone_or_copy(&from, &to)?;
                }
                Ok::<(), io::Error>(())
            })
            .await
            .map_err(SandboxError::new)?
            .map_err(SandboxError::new)?;
            let vm = Guest::restore_small(
                &self.state.config,
                &run,
                &generation,
                disk.path.clone(),
                control,
                owned,
                permit,
            )
            .await?;
            (vm, disk)
        } else {
            let disk = Scratch {
                path: self.state.new_disk().await?,
                cleanup: Some(runtime),
            };
            let vm = Guest::boot_small(&self.state.config, &run, disk.path.clone(), owned, permit)
                .await?;
            (vm, disk)
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
            _gate: gate,
            saved,
            image,
        }))
    }
}

// Boot scratch is a single-use path. Restore scratch uses the pathname recorded
// in Firecracker state and must only be removed under the placement gate.
struct Scratch {
    path: PathBuf,
    cleanup: Option<tokio::runtime::Handle>,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Some(runtime) = &self.cleanup {
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
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(SandboxError::new(error)),
        }
    }
}

pub(in crate::sandbox) struct Placement {
    slot: AccountSlot,
    thread: Thread,
    disk: Scratch,
    _gate: tokio::sync::OwnedMutexGuard<()>,
    saved: PathBuf,
    image: String,
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
            Ok(())
        })
    }

    fn release(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            let quiesced = self.thread.stop().await;
            let mut slot = self.slot.lock().await;
            if let Some(vm) = slot.vm.take() {
                let saved = match quiesced {
                    Ok(()) => save(&vm, &self.saved, &self.image).await,
                    Err(error) => Err(error),
                };
                vm.stop().await?;
                self.disk.clear().await?;
                if let Err(error) = saved {
                    tracing::warn!(%error, "small plugin VM snapshot failed; preserving previous snapshot");
                }
            }
            Ok(())
        })
    }
}

async fn save(vm: &Guest, root: &Path, image: &str) -> SandboxResult<()> {
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
    let image = image.to_owned();
    let disk = vm.disk.clone();
    let control = control.clone();
    let mut generation = {
        let root = root.clone();
        let disk = disk.clone();
        let control = control.clone();
        tokio::task::spawn_blocking(move || Generation::create(&root, &disk, &control))
            .await
            .map_err(SandboxError::new)?
            .map_err(SandboxError::new)?
    };
    snapshot_api::pause(api).await?;
    if let Some(restored) = vm.restored {
        let previous = {
            let root = root.clone();
            let image = image.clone();
            tokio::task::spawn_blocking(move || Generation::current(&root, &image))
                .await
                .map_err(SandboxError::new)?
                .map_err(SandboxError::new)?
        }
        .filter(|generation| generation.id == restored);
        if let Some(previous) = previous {
            let dirty = generation.path.join("dirty");
            snapshot_api::create(api, &generation.path.join("state"), &dirty, true).await?;
            let target = generation.path.join("memory");
            if tokio::task::spawn_blocking(move || {
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
        clone_or_copy(&disk, &generation.path.join("disk"))?;
        clone_or_copy(&control, &generation.path.join("control"))?;
        generation.publish(&root, &image)?;
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
