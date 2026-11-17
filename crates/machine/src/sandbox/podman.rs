//! Rootless Podman sandboxes: one container per thread.
//!
//! - Base image by reference (`docker.io/library/alpine:3` or a digest); the image
//!   must already be present or pullable by the machine user.
//! - The container's own overlay upper layer is the thread's private writable layer.
//! - `<root>/<thread-id>/work` is bind-mounted at `/work` and is the working directory.
//! - Limits map to `--memory`, `--cpus` and `--pids-limit`; defaults are 1 GiB and
//!   256 processes, the configuration measured on perf-01.
//! - Freeze and resume are `podman pause` / `unpause` (about 0.1 s each, memory
//!   stays resident). Stop is `podman rm -f -t 0` plus removing the work directory.
//! - Containers are named `calternal-thread-<thread-id>` and labelled
//!   `org.calternal.thread`, so a restarted machine role adopts them.
//!
//! The server never runs any of this: only the machine role calls Podman.

use std::collections::HashSet;
use std::os::unix::process::CommandExt as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rustix::process::{Pid, Signal, kill_process_group, test_kill_process_group};
use uuid::Uuid;

use super::{Backend, BoxFuture, Exec, ExecRequest, Sandbox, SandboxError, SandboxResult};
use crate::protocol::{Limits, SandboxSpec};

/// Default memory ceiling when the spec gives none.
pub const DEFAULT_MEMORY_BYTES: u64 = 1 << 30;
/// Default process ceiling when the spec gives none.
pub const DEFAULT_PIDS: u32 = 256;

/// Starts rootless Podman sandboxes.
pub struct PodmanBackend {
    podman: PathBuf,
    root: PathBuf,
    default_image: String,
}

impl PodmanBackend {
    /// `podman` is the executable; work directories live under `root`;
    /// `default_image` is used when a spec names none.
    pub fn new(
        podman: impl Into<PathBuf>,
        root: impl Into<PathBuf>,
        default_image: impl Into<String>,
    ) -> Self {
        Self {
            podman: podman.into(),
            root: root.into(),
            default_image: default_image.into(),
        }
    }

    /// Whether `podman version` runs. One process spawn, about 50 ms.
    pub async fn available(podman: &std::path::Path) -> bool {
        tokio::process::Command::new(podman)
            .args(["version", "--format", "{{.Client.Version}}"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .is_ok_and(|s| s.success())
    }

    /// Container name of a thread.
    pub fn container_name(thread_id: Uuid) -> String {
        format!("calternal-thread-{thread_id}")
    }
}

/// Run one Podman command; on failure return the last line of its error output.
async fn podman(bin: &std::path::Path, args: &[&str]) -> SandboxResult<String> {
    let out = tokio::process::Command::new(bin)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| SandboxError(format!("cannot run podman: {}", e.kind())))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("failed");
        Err(SandboxError(format!(
            "podman {}: {}",
            args.first().copied().unwrap_or(""),
            line.chars().take(200).collect::<String>()
        )))
    }
}

impl Backend for PodmanBackend {
    fn name(&self) -> &'static str {
        "podman"
    }

    fn start<'a>(
        &'a self,
        spec: &'a SandboxSpec,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        Box::pin(async move {
            let name = Self::container_name(spec.thread_id);
            let dir = self.root.join(spec.thread_id.to_string());
            let work = dir.join("work");
            tokio::fs::create_dir_all(&work)
                .await
                .map_err(|e| SandboxError(format!("cannot create work directory: {}", e.kind())))?;
            let state = podman(
                &self.podman,
                &[
                    "container",
                    "inspect",
                    "--format",
                    "{{.State.Status}}",
                    &name,
                ],
            )
            .await;
            match state.as_deref() {
                Ok("running") => {}
                Ok("paused") => {
                    podman(&self.podman, &["unpause", &name]).await?;
                }
                Ok(_) => {
                    podman(&self.podman, &["start", &name]).await?;
                }
                Err(_) => {
                    let Limits {
                        memory_bytes,
                        cpu_millis,
                        pids,
                    } = spec.limits;
                    let image = spec.image.as_deref().unwrap_or(&self.default_image);
                    let label = format!("org.calternal.thread={}", spec.thread_id);
                    let mount = format!("{}:/work", work.display());
                    let memory = memory_bytes.unwrap_or(DEFAULT_MEMORY_BYTES).to_string();
                    let pids = pids.unwrap_or(DEFAULT_PIDS).to_string();
                    let mut args: Vec<String> = [
                        "run",
                        "-d",
                        "--name",
                        &name,
                        "--label",
                        &label,
                        "--init",
                        "--memory",
                        &memory,
                        "--pids-limit",
                        &pids,
                        "--volume",
                        &mount,
                        "--workdir",
                        "/work",
                        "--env",
                        "HOME=/work",
                    ]
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                    if let Some(m) = cpu_millis {
                        args.push("--cpus".into());
                        args.push(format!("{}.{:03}", m / 1000, m % 1000));
                    }
                    args.extend([image.to_owned(), "sleep".into(), "2147483647".into()]);
                    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
                    podman(&self.podman, &refs).await?;
                }
            }
            Ok(Arc::new(PodmanSandbox {
                podman: self.podman.clone(),
                name,
                dir,
                clients: Mutex::new(HashSet::new()),
            }) as Arc<dyn Sandbox>)
        })
    }
}

/// One thread's container.
pub struct PodmanSandbox {
    podman: PathBuf,
    name: String,
    dir: PathBuf,
    /// Process groups of local `podman exec` clients, killed on release.
    clients: Mutex<HashSet<i32>>,
}

impl Sandbox for PodmanSandbox {
    fn exec(&self, request: ExecRequest) -> BoxFuture<'_, SandboxResult<Exec>> {
        Box::pin(async move {
            if request.argv.is_empty() {
                return Err(SandboxError("empty command".into()));
            }
            let mut cmd = tokio::process::Command::new(&self.podman);
            cmd.args(["exec", "--workdir", "/work"]);
            for (k, v) in &request.env {
                cmd.arg("--env").arg(format!("{k}={v}"));
            }
            cmd.arg(&self.name).args(&request.argv);
            cmd.as_std_mut().process_group(0);
            let (exec, pid) = super::spawn_streaming(cmd)?;
            let mut clients = self.clients.lock().expect("clients");
            clients
                .retain(|&g| Pid::from_raw(g).is_some_and(|p| test_kill_process_group(p).is_ok()));
            if let Some(pid) = pid {
                clients.insert(pid as i32);
            }
            Ok(exec)
        })
    }

    fn freeze(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move { podman(&self.podman, &["pause", &self.name]).await.map(drop) })
    }

    fn resume(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            podman(&self.podman, &["unpause", &self.name])
                .await
                .map(drop)
        })
    }

    fn stop(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            podman(&self.podman, &["rm", "-f", "-t", "0", &self.name]).await?;
            match tokio::fs::remove_dir_all(&self.dir).await {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                // Files written by a mapped non-root container user may need
                // `podman unshare` to remove; leave them and say so.
                Err(e) => Err(SandboxError(format!(
                    "container removed, work directory kept: {}",
                    e.kind()
                ))),
            }
        })
    }

    fn release(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            for &g in self.clients.lock().expect("clients").iter() {
                if let Some(pid) = Pid::from_raw(g) {
                    let _ = kill_process_group(pid, Signal::KILL);
                }
            }
            Ok(())
        })
    }
}
