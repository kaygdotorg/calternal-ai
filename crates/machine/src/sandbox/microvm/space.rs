//! Dedicated offline placements for the small plugin guest.
use super::*;

pub(in crate::sandbox) struct SmallRuntime(Arc<State>);

impl SmallRuntime {
    pub(in crate::sandbox) async fn load(root: PathBuf, path: &Path) -> SandboxResult<Self> {
        let config = Config::load_profile(path, true).await?;
        Ok(Self(
            MicrovmBackend::new(root.join("space-runtime"), config).0,
        ))
    }

    pub(in crate::sandbox) async fn initialize(&self) -> SandboxResult<()> {
        self.0.initialize().await
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
        // Reserve VMM capacity before allocating scratch disk resources.
        let permit = self
            .0
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| SandboxError("small plugin VM capacity is full".into()))?;
        let owned = self
            .0
            .owned
            .get()
            .cloned()
            .ok_or_else(|| SandboxError("small plugin VM is not initialized".into()))?;
        let disk = Scratch {
            path: self.0.new_disk().await?,
            runtime: tokio::runtime::Handle::current(),
        };
        let vm = Guest::boot_small(
            &self.0.config,
            &self.0.root.join("run"),
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
        self.stop()
    }
}
