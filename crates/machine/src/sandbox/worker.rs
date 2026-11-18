//! Worker-only sandbox profile shared by the direct and Podman backends.
use super::{SandboxError, SandboxResult};
use crate::protocol::{Limits, PluginWorker};
use std::path::Path;

/// Maximum size of a verified, self-contained JavaScript worker bundle.
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
/// Maximum stderr bytes forwarded when an isolated plugin worker crashes.
pub const STDERR_TAIL_BYTES: usize = 4096;
/// Worker container image; operators must provision it on enrolled machines.
pub const IMAGE: &str = "docker.io/library/node:24-alpine";
/// Fixed worker resource ceiling: 256 MiB, one CPU and 64 processes.
pub const LIMITS: Limits = Limits {
    memory_bytes: Some(256 * 1024 * 1024),
    cpu_millis: Some(1000),
    pids: Some(64),
};

/// Stage only the verified bundle in a private code directory. Rejects oversized
/// source before I/O; costs one exclusive write and permission change on a cold start, or one
/// code read on adoption; changed source is refused.
/// The sandbox is not running yet, so no worker can race this preparation.
pub(crate) async fn prepare(dir: &Path, worker: &PluginWorker) -> SandboxResult<()> {
    if worker.source.len() > MAX_SOURCE_BYTES {
        return Err(SandboxError("plugin bundle exceeds 1 MiB".into()));
    }
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(SandboxError::new)?;
    let path = dir.join("worker.js");
    use tokio::io::AsyncWriteExt;
    match tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .await
    {
        Ok(mut file) => file
            .write_all(worker.source.as_bytes())
            .await
            .map_err(SandboxError::new)?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if tokio::fs::read(&path).await.map_err(SandboxError::new)? != worker.source.as_bytes()
            {
                return Err(SandboxError("worker source changed during adoption".into()));
            }
        }
        Err(e) => return Err(SandboxError::new(e)),
    }
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o444))
        .await
        .map_err(SandboxError::new)
}

/// Stage the bundled Moby default seccomp allowlist with socket creation and
/// io_uring removed. The filter stacks with the kernel's container restrictions.
/// Costs one small file write per cold start; never mounted into the worker.
pub(crate) async fn seccomp(dir: &Path) -> SandboxResult<std::path::PathBuf> {
    let path = dir.join("worker-seccomp.json");
    tokio::fs::write(&path, include_bytes!("worker-seccomp.json"))
        .await
        .map_err(SandboxError::new)?;
    Ok(path)
}
