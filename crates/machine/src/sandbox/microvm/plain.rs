//! Package processes in the offline, single-space guest.

use crate::protocol::SandboxSpec;
use crate::sandbox::{
    BoxFuture, Exec, ExecRequest, Sandbox, SandboxError, SandboxResult, spawn_streaming, worker,
};
use std::{io, path::PathBuf, sync::Arc, time::Duration};
use tokio::{fs, sync::Mutex};
use uuid::Uuid;

const LAUNCHER: &str = "/opt/calternal-worker-launch";
const PACKAGE_ROOT: &str = "/run/agent/plain";
const CGROUP_ROOT: &str = "/sys/fs/cgroup/agent/ctr";

struct PlainWorker {
    id: Uuid,
    source: PathBuf,
    cgroup: PathBuf,
    launched_or_stopped: Mutex<bool>,
}

/// Stage verified code and configure the cgroup before admitting any exec.
/// The guest serializes Start and teardown. Admission never starts Node, so
/// snapshots can continue to require an empty guest before reseeding.
pub(super) async fn start(spec: &SandboxSpec) -> SandboxResult<Arc<dyn Sandbox>> {
    if spec.image.as_deref() != Some(worker::SPACE_VM_IMAGE)
        || spec.provider_login
        || spec.shared_tools.is_some()
        || spec.shared_provider_sponsor.is_some()
    {
        return Err(SandboxError(
            "small guest requires the offline worker profile".into(),
        ));
    }
    let package = spec
        .worker
        .as_ref()
        .filter(|package| spec.space_id == Some(package.space_id))
        .ok_or_else(|| SandboxError("small guest has no matching worker".into()))?;
    let id = spec.thread_id;
    let instance = PlainWorker {
        id,
        source: PathBuf::from(PACKAGE_ROOT).join(id.to_string()),
        cgroup: PathBuf::from(CGROUP_ROOT).join(id.to_string()),
        launched_or_stopped: Mutex::new(false),
    };
    fs::create_dir(&instance.cgroup)
        .await
        .map_err(SandboxError::new)?;
    if let Err(error) = instance.prepare(package).await {
        if let Err(cleanup) = instance.stop().await {
            tracing::warn!(%cleanup, "cannot clean failed small guest worker setup");
        }
        return Err(error);
    }
    Ok(Arc::new(instance))
}

impl PlainWorker {
    async fn prepare(&self, package: &crate::protocol::PluginWorker) -> SandboxResult<()> {
        for (file, limit) in [
            (
                "memory.max",
                worker::LIMITS.memory_bytes.unwrap().to_string(),
            ),
            ("memory.swap.max", "0".into()),
            ("pids.max", worker::LIMITS.pids.unwrap().to_string()),
            (
                "cpu.max",
                format!("{} 100000", worker::LIMITS.cpu_millis.unwrap() * 100),
            ),
        ] {
            fs::write(self.cgroup.join(file), limit)
                .await
                .map_err(SandboxError::new)?;
        }
        worker::prepare(&self.source, package).await
    }
}

impl Sandbox for PlainWorker {
    fn exec(&self, request: ExecRequest) -> BoxFuture<'_, SandboxResult<Exec>> {
        Box::pin(async move {
            if !request.argv.iter().map(String::as_str).eq(worker::ARGV)
                || !request.env.is_empty()
                || !request.interactive
            {
                return Err(SandboxError(
                    "small guest only starts the verified package with pipes".into(),
                ));
            }
            let mut launched = self.launched_or_stopped.lock().await;
            if *launched {
                return Err(SandboxError("package exec already used or closed".into()));
            }
            let mut command = tokio::process::Command::new(LAUNCHER);
            command.arg(self.id.to_string()).env_clear();
            let (exec, _) = spawn_streaming(command, request.stdin, true, true)?;
            *launched = true;
            Ok(exec)
        })
    }

    fn freeze(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            fs::write(self.cgroup.join("cgroup.freeze"), "1")
                .await
                .map_err(SandboxError::new)
        })
    }

    fn resume(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            fs::write(self.cgroup.join("cgroup.freeze"), "0")
                .await
                .map_err(SandboxError::new)
        })
    }

    fn stop(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            // Serialize teardown with exec and another release. A launcher
            // that has not joined by removal cannot open cgroup.procs.
            let mut launched = self.launched_or_stopped.lock().await;
            *launched = true;
            let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
            loop {
                match fs::write(self.cgroup.join("cgroup.kill"), "1").await {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => break,
                    Err(error) => return Err(SandboxError::new(error)),
                }
                match fs::remove_dir(&self.cgroup).await {
                    Ok(()) => break,
                    Err(error)
                        if error.kind() == io::ErrorKind::ResourceBusy
                            && tokio::time::Instant::now() < deadline =>
                    {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    Err(error) => return Err(SandboxError::new(error)),
                }
            }
            match fs::remove_dir_all(&self.source).await {
                Err(error) if error.kind() != io::ErrorKind::NotFound => {
                    Err(SandboxError::new(error))
                }
                _ => Ok(()),
            }
        })
    }

    fn release(&self) -> BoxFuture<'_, SandboxResult<()>> {
        self.stop()
    }
}
