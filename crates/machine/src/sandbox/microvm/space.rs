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
    ) -> SandboxResult<Arc<dyn Sandbox>> {
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
        let disk = Scratch {
            path: self.state.new_disk().await?,
            runtime: tokio::runtime::Handle::current(),
        };
        let vm = Guest::boot_small(
            &self.state.config,
            &self.state.root.join("run"),
            disk.path.clone(),
            owned,
            permit,
        )
        .await?;
        vm.control(&Request::Start(space, spec.clone())).await?;
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
            _disk: disk,
            _gate: gate,
            saved,
            image,
        }))
    }
}

// The scratch path is single-use, so cancellation may unlink it after the
// VMM has inherited its descriptor. No other placement opens this path.
struct Scratch {
    path: PathBuf,
    runtime: tokio::runtime::Handle,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let path = self.path.clone();
        self.runtime.spawn(async move {
            let _ = tokio::fs::remove_file(path).await;
        });
    }
}

struct Placement {
    slot: AccountSlot,
    thread: Thread,
    _disk: Scratch,
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
    let generation = {
        let root = root.clone();
        let disk = disk.clone();
        let control = control.clone();
        tokio::task::spawn_blocking(move || Generation::create(&root, &disk, &control))
            .await
            .map_err(SandboxError::new)?
            .map_err(SandboxError::new)?
    };
    snapshot_api::pause(api).await?;
    snapshot_api::create(
        api,
        &generation.path.join("state"),
        &generation.path.join("memory"),
        false,
    )
    .await?;
    tokio::task::spawn_blocking(move || {
        clone_or_copy(&disk, &generation.path.join("disk"))?;
        clone_or_copy(&control, &generation.path.join("control"))?;
        generation.publish(&root, &image)?;
        // Only the newly published generation is needed after commit. An
        // interrupted write leaves its predecessor's pointer usable.
        for candidate in std::fs::read_dir(&root)? {
            let candidate = candidate?;
            if candidate.path() != generation.path
                && Uuid::parse_str(&candidate.file_name().to_string_lossy()).is_ok()
                && candidate.file_type()?.is_dir()
            {
                std::fs::remove_dir_all(candidate.path())?;
            }
        }
        Ok::<(), io::Error>(())
    })
    .await
    .map_err(SandboxError::new)?
    .map_err(SandboxError::new)
}
