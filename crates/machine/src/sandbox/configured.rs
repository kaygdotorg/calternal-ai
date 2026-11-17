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
}
impl ConfiguredBackend {
    /// Build both cheap backend configurations; no process starts or file I/O.
    /// `configure` selects the authorized one before assignments are accepted.
    pub fn new(root: PathBuf, podman: PathBuf, image: String) -> Self {
        Self {
            direct: DirectBackend::new(root.clone()),
            podman: PodmanBackend::new(podman, root, image),
            selected: AtomicU8::new(0),
        }
    }
}
impl Backend for ConfiguredBackend {
    fn initialize(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
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
