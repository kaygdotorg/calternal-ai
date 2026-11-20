//! Select the machine owner's server-stored execution setting at each new session.
use super::{
    Backend, BoxFuture, Sandbox, SandboxError, SandboxResult, direct::DirectBackend,
    podman::PodmanBackend,
};
use crate::protocol::SandboxSpec;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

/// Default machine backend. Account-owned machines follow their owner's setting;
/// the server always selects Podman for pool machines. Existing sandbox objects
/// retain their original backend while a changed session fences and releases them.
pub struct ConfiguredBackend {
    direct: DirectBackend,
    podman: PodmanBackend,
    selected: AtomicU8,
    #[cfg(target_os = "linux")]
    small_vm: Option<Arc<super::microvm::SmallRuntime>>,
}
impl ConfiguredBackend {
    /// Build both cheap backend configurations; no process starts or file I/O.
    /// `configure` selects the authorized one before assignments are accepted.
    pub fn new(root: PathBuf, podman: PathBuf, image: String) -> Self {
        Self {
            direct: DirectBackend::new(root.clone()),
            podman: PodmanBackend::new(podman, root, image),
            selected: AtomicU8::new(0),
            #[cfg(target_os = "linux")]
            small_vm: None,
        }
    }

    /// Configure the separate offline plugin guest wall. Reads the operator
    /// file. Initialization verifies KVM and immutable images before admission.
    pub async fn with_small_vm(self, root: PathBuf, path: &std::path::Path) -> SandboxResult<Self> {
        #[cfg(target_os = "linux")]
        {
            let mut configured = self;
            configured.small_vm = Some(Arc::new(
                super::microvm::SmallRuntime::load(root, path).await?,
            ));
            Ok(configured)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (self, root, path);
            Err(SandboxError(
                "Small plugin VMs require Linux and KVM".into(),
            ))
        }
    }

    /// Supply the private operation CLI alias if the owner selects Direct.
    /// Cost: one path assignment. The machine role owns the directory lifetime.
    pub(crate) fn with_cli_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.direct = self.direct.with_cli_dir(dir);
        self
    }
}
impl Backend for ConfiguredBackend {
    fn initialize(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            #[cfg(target_os = "linux")]
            if let Some(vm) = &self.small_vm {
                vm.initialize().await?;
            }
            if self.podman.engine_available().await {
                self.podman.initialize().await
            } else {
                self.direct.initialize().await
            }
        })
    }
    fn name(&self) -> &'static str {
        if self.selected.load(Ordering::Acquire) == 1 {
            "direct"
        } else {
            "podman"
        }
    }
    fn preference(&self) -> &'static str {
        "auto"
    }
    fn configure(&self, mode: &str) -> SandboxResult<()> {
        let value = match mode {
            "podman" => 0,
            "direct" => 1,
            _ => return Err(SandboxError("server chose an unsupported backend".into())),
        };
        self.selected.store(value, Ordering::Release);
        Ok(())
    }
    fn discard(&self, account: uuid::Uuid, thread: uuid::Uuid) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            if self.podman.engine_available().await {
                self.podman.discard(account, thread).await
            } else if self.selected.load(Ordering::Acquire) == 1 {
                self.direct.discard(account, thread).await
            } else {
                Err(SandboxError(
                    "Podman is unavailable for workspace deletion".into(),
                ))
            }
        })
    }
    fn erase(
        &self,
        account: uuid::Uuid,
        thread: Option<uuid::Uuid>,
    ) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            if self.podman.engine_available().await {
                self.podman.erase(account, thread).await
            } else if self.name() == "direct" {
                self.direct.erase(account, thread).await
            } else {
                Err(SandboxError(
                    "Podman is unavailable for owner erasure".into(),
                ))
            }
        })
    }
    fn supports_small_plugin_vm(&self) -> bool {
        #[cfg(target_os = "linux")]
        return self.name() == "podman" && self.small_vm.is_some();
        #[cfg(not(target_os = "linux"))]
        false
    }

    fn erase_space_runtime(
        &self,
        space: calternal_types::SpaceId,
        through: Option<u64>,
    ) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            #[cfg(target_os = "linux")]
            if let Some(vm) = &self.small_vm {
                return vm.erase(space, through).await;
            }
            let _ = (space, through);
            Err(SandboxError(
                "Small plugin VM storage is unavailable for erasure".into(),
            ))
        })
    }

    fn start_space<'a>(
        &'a self,
        spec: &'a SandboxSpec,
        checkpoint: Option<u64>,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        Box::pin(async move {
            #[cfg(target_os = "linux")]
            if self.name() == "podman"
                && let Some(vm) = &self.small_vm
            {
                return Ok(vm.start(spec, checkpoint).await? as Arc<dyn Sandbox>);
            }
            let _ = (spec, checkpoint);
            Err(SandboxError(
                "Small plugin VM is unavailable on this machine".into(),
            ))
        })
    }
    fn start_harness<'a>(
        &'a self,
        spec: &'a SandboxSpec,
        harness: &'a crate::protocol::HarnessSandbox,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        if self.name() == "podman" {
            self.podman.start_harness(spec, harness)
        } else {
            self.direct.start_harness(spec, harness)
        }
    }

    fn start_entry<'a>(
        &'a self,
        ticket: uuid::Uuid,
        program: &'a calternal_plugin_host::entry::EntryProgram,
        live: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> BoxFuture<'a, SandboxResult<(Arc<dyn Sandbox>, super::Exec)>> {
        if self.name() == "podman" {
            self.podman.start_entry(ticket, program, live)
        } else {
            self.direct.start_entry(ticket, program, live)
        }
    }

    fn start<'a>(
        &'a self,
        spec: &'a SandboxSpec,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        if spec.worker.is_none() && self.selected.load(Ordering::Acquire) == 1 {
            self.direct.start(spec)
        } else {
            self.podman.start(spec)
        }
    }
}
