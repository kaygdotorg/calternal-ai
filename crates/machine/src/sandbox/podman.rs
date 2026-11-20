//! Rootless Podman sandboxes: one container per thread.
//!
//! - Base image by reference (`docker.io/library/alpine:3` or a digest); the image
//!   must already be present or pullable by the machine user.
//! - The container's private overlay is disposable. The /work and home bind mounts persist.
//! - `<root>/accounts/<account-id>/threads/<thread-id>/work` is bind-mounted at `/work` and is the working directory.
//! - Limits map to `--memory`, `--cpus` and `--pids-limit`; defaults are 1 GiB and
//!   256 processes, the configuration measured on perf-01.
//! - Freeze and resume are `podman pause` / `unpause` (about 0.1 s each, memory
//!   stays resident). Stop is `podman rm -f -t 0` plus removing the work directory.
//! - Containers are named `calternal-account-<account-id>-thread-<thread-id>` and labelled
//!   with their machine's sandbox root. Start-up removes orphans for that root.
//!
//! The server never runs any of this: only the machine role calls Podman.

use std::collections::HashSet;
use std::os::unix::process::CommandExt as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rustix::process::{Pid, Signal, kill_process_group, test_kill_process_group};
use uuid::Uuid;

use super::shared_broker::{Broker, ToolSandbox};
use super::{Backend, BoxFuture, Exec, ExecRequest, Sandbox, SandboxError, SandboxResult};
use crate::protocol::{Limits, SandboxSpec};

#[cfg(test)]
mod argv_tests;
mod entry;
#[cfg(all(test, target_os = "linux"))]
mod entry_tests;
mod ownership;

/// Default memory ceiling when the spec gives none.
pub const DEFAULT_MEMORY_BYTES: u64 = 1 << 30;
/// Default process ceiling when the spec gives none.
pub const DEFAULT_PIDS: u32 = 256;

/// Starts rootless Podman sandboxes.
pub struct PodmanBackend {
    podman: PathBuf,
    root: PathBuf,
    default_image: String,
    guest: bool,
    kvm_fallback: bool,
    guest_network: bool,
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
            guest: false,
            kvm_fallback: false,
            guest_network: false,
        }
    }

    pub(crate) fn kvm_fallback(mut self) -> Self {
        self.kvm_fallback = true;
        self
    }

    /// Reuse rootless Podman and the packaged image. Guest init enables
    /// networking for thread containers when the host attaches a TAP.
    #[cfg(target_os = "linux")]
    pub(super) fn in_microvm(root: PathBuf, image: String) -> Self {
        let mut backend = Self::new("podman", root, image);
        backend.guest = true;
        backend.guest_network = std::env::var_os("CALTERNAL_GUEST_NETWORK").is_some();
        backend
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

    /// Whether this configured engine can answer commands; no container is started.
    pub(super) async fn engine_available(&self) -> bool {
        Self::available(&self.podman).await
    }

    async fn canonical_root(&self) -> SandboxResult<PathBuf> {
        tokio::fs::create_dir_all(&self.root)
            .await
            .map_err(SandboxError::new)?;
        tokio::fs::canonicalize(&self.root)
            .await
            .map_err(SandboxError::new)
    }

    /// Account-qualified container name, so a caller reusing another account's
    /// thread UUID can never address its mounts or login home.
    pub fn container_name(account_id: Uuid, thread_id: Uuid) -> String {
        format!("calternal-account-{account_id}-thread-{thread_id}")
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
    fn initialize(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            let root = self.canonical_root().await?;
            ownership::sweep(&self.podman, &root).await?;
            for directory in ["workers", "entries"] {
                match tokio::fs::remove_dir_all(root.join(directory)).await {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(SandboxError::new(e)),
                }
            }
            Ok(())
        })
    }
    fn preference(&self) -> &'static str {
        if self.kvm_fallback {
            crate::protocol::PODMAN_KVM_FALLBACK
        } else {
            self.name()
        }
    }

    fn name(&self) -> &'static str {
        "podman"
    }

    fn discard(&self, account: uuid::Uuid, thread: uuid::Uuid) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            if self.kvm_fallback {
                return self.erase(account, Some(thread)).await;
            }
            let root = self.canonical_root().await?;
            let name = Self::container_name(account, thread);
            ownership::remove_named(&self.podman, &root, &name).await?;
            super::direct::DirectBackend::new(self.root.clone())
                .discard(account, thread)
                .await
        })
    }
    fn erase(&self, account: Uuid, thread: Option<Uuid>) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            let root = self.canonical_root().await?;
            #[cfg(target_os = "linux")]
            if self.kvm_fallback {
                super::microvm::erase_without_kvm(&root, account, thread).await?;
            }
            match thread {
                Some(thread) => {
                    let name = Self::container_name(account, thread);
                    ownership::remove_named(&self.podman, &root, &name).await?;
                }
                None => ownership::remove_account(&self.podman, &root, account).await?,
            }
            super::direct::DirectBackend::new(root)
                .erase(account, thread)
                .await
        })
    }
    fn start<'a>(
        &'a self,
        spec: &'a SandboxSpec,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        self.start_container(spec, None)
    }

    fn start_harness<'a>(
        &'a self,
        spec: &'a SandboxSpec,
        harness: &'a crate::protocol::HarnessSandbox,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        self.start_container(spec, Some(harness))
    }

    fn start_entry<'a>(
        &'a self,
        ticket: Uuid,
        program: &'a calternal_plugin_host::entry::EntryProgram,
        live: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> BoxFuture<'a, SandboxResult<(Arc<dyn Sandbox>, Exec)>> {
        Box::pin(async move {
            let (sandbox, exec) = self.launch_entry(ticket, program, live).await?;
            Ok((Arc::new(sandbox) as Arc<dyn Sandbox>, exec))
        })
    }
}

impl PodmanBackend {
    fn start_container<'a>(
        &'a self,
        spec: &'a SandboxSpec,
        harness: Option<&'a crate::protocol::HarnessSandbox>,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        Box::pin(async move {
            let root = self.canonical_root().await?;
            let name = Self::container_name(spec.account_id, spec.thread_id);
            let worker_dir = if self.guest {
                PathBuf::from("/run/agent/workers")
            } else {
                root.join("workers")
            }
            .join(spec.thread_id.to_string());
            let result = async {
                let account = root.join("accounts").join(spec.account_id.to_string());
                let dir = if spec.worker.is_some() {
                    worker_dir.clone()
                } else {
                    account.join("threads").join(spec.thread_id.to_string())
                };
                if spec.shared_tools.is_some()
                    && (self.guest || self.kvm_fallback || spec.provider_login
                        || spec.shared_provider_sponsor.is_none() || spec.worker.is_some()
                        || harness.is_some())
                {
                    return Err(SandboxError("shared tools require a dedicated Podman harness".into()));
                }
                let (home, login) = if spec.shared_tools.is_some() {
                    (PathBuf::new(), PathBuf::new())
                } else { match harness {
                    Some(harness) => super::provider_home::prepare_harness(
                        account, &dir, spec, harness,
                    ).await?,
                    None => super::provider_home::prepare(account, &dir, spec).await?,
                }};
                let work = dir.join("work");
                tokio::fs::create_dir_all(&work).await.map_err(|e| {
                    SandboxError(format!("cannot create work directory: {}", e.kind()))
                })?;
                if let Some(worker) = &spec.worker {
                    super::worker::prepare(&work, worker).await?;
                }
                let Limits {
                    memory_bytes,
                    cpu_millis,
                    pids,
                } = if spec.worker.is_some() {
                    super::worker::LIMITS
                } else {
                    spec.limits
                };
                let image = if self.guest {
                    &self.default_image
                } else {
                    spec.image.as_deref().unwrap_or(&self.default_image)
                };
                let label = format!("org.calternal.thread={}", spec.thread_id);
                let home_mount = format!("{}:/home/agent", home.display());
                let mount = if harness.is_some_and(|harness| harness.workspace_read_only) {
                    format!("{}:/work:ro", work.display())
                } else {
                    format!("{}:/work", work.display())
                };
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
                ]
                .iter()
                .map(|s| s.to_string())
                .collect();
                let broker = if let Some(tools) = spec.shared_tools {
                    let space = spec.space_id.ok_or_else(|| SandboxError("missing sandbox space".into()))?;
                    let broker = Broker::start(&root, &dir, space, tools.provider, ToolSandbox {
                        podman: self.podman.clone(),
                        image: image.to_owned(),
                        root_label: ownership::root_label(&root),
                        work: work.clone(),
                        writable: !tools.workspace_read_only,
                    }).await?;
                    args.extend([
                        "--pull=never".into(), "--network=none".into(), "--read-only".into(),
                        "--pid=private".into(), "--ipc=private".into(), "--cap-drop=all".into(),
                        "--security-opt=no-new-privileges".into(), "--http-proxy=false".into(),
                        "--log-driver=none".into(), "--image-volume=ignore".into(), "--tmpfs".into(), "/home/agent:mode=0700".into(),
                        "--volume".into(), format!("{}:/run/calternal-broker:ro", broker.directory.display()),
                        "--env".into(), "HOME=/home/agent".into(),
                    ]);
                    Some(broker)
                } else {
                    args.extend([
                        "--volume".into(), mount, "--volume".into(), home_mount,
                        "--workdir".into(), "/work".into(), "--env".into(), "HOME=/home/agent".into(),
                    ]);
                    None
                };
                if spec.worker.is_none() && broker.is_none() {
                    let login_env = if harness.is_some() {
                        "CALTERNAL_PROVIDER_LOGIN_DIR=/run/calternal-provider-login"
                    } else {
                        "CALTERNAL_CLAUDE_LOGIN_DIR=/run/calternal-provider-login"
                    };
                    let mode = if harness.is_some_and(|harness| !harness.legacy_profile) || spec.provider_login { "rw" } else { "ro" };
                    args.extend([
                        "--volume".into(),
                        format!("{}:/run/calternal-provider-login:{mode}", login.display()),
                        "--env".into(),
                        login_env.into(),
                    ]);
                    if harness.is_some() && !self.guest {
                        args.push("--pull=never".into());
                    }
                    if spec.provider_login {
                        args.extend(["--env".into(), "CALTERNAL_PROVIDER_LOGIN=1".into()]);
                    }
                }
                if spec.worker.is_some() {
                    // A worker sees only code. In particular it never inherits the
                    // account home, writable work tree, CLI or Nix store mounts.
                    let seccomp = super::worker::seccomp(&dir).await?;
                    args = [
                        "run",
                        "-d",
                        "--pull=never",
                        "--name",
                        &name,
                        "--label",
                        &label,
                        "--init",
                        "--network=none",
                        "--read-only",
                        "--read-only-tmpfs=false",
                        "--cap-drop=all",
                        "--security-opt=no-new-privileges",
                        &format!("--security-opt=seccomp={}", seccomp.display()),
                        "--log-driver=none",
                        "--memory",
                        &memory,
                        "--memory-swap",
                        &memory,
                        "--pids-limit",
                        &pids,
                        "--volume",
                        &format!(
                            "{}:/plugin:ro",
                            spec.code_directory.as_ref().unwrap_or(&work).display()
                        ),
                        "--workdir",
                        "/plugin",
                    ]
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                }
                if self.guest {
                    args.extend(["--cgroup-parent".into(), "/agent/ctr".into()]);
                    args.retain(|arg| arg != "--pull=never");
                    if spec.worker.is_none() {
                        args.extend([
                            if self.guest_network {
                                "--network=slirp4netns:allow_host_loopback=false"
                            } else {
                                "--network=none"
                            }.into(),
                            "--log-driver=none".into(),
                            "--volume".into(),
                            "/opt/calternal:/usr/local/bin/calternal:ro".into(),
                        ]);
                    }
                }
                // Linux hosts supply the matching operation CLI read-only. Other
                // host platforms must put a Linux CLI in the container image.
                // Nix builds retain immutable store references for their libraries.
                #[cfg(target_os = "linux")]
                if !self.guest
                    && spec.worker.is_none()
                    && broker.is_none()
                    && let Some(exe) = super::cli_program().await
                {
                    args.extend([
                        "--volume".into(),
                        format!("{}:/usr/local/bin/calternal:ro", exe.display()),
                    ]);
                    if tokio::fs::try_exists("/nix/store").await.unwrap_or(false) {
                        args.extend(["--volume".into(), "/nix/store:/nix/store:ro".into()]);
                    }
                }
                if self.kvm_fallback && spec.worker.is_none() {
                    args.push("--network=none".into());
                }
                if let Some(m) = cpu_millis {
                    args.push("--cpus".into());
                    args.push(format!("{}.{:03}", m / 1000, m % 1000));
                }
                // A failed create may leave a stopped container. A private cidfile
                // identifies only this attempt, even if the requested name is busy.
                let cidfile = dir.join(format!("create-{}.cid", Uuid::new_v4()));
                args.extend([
                    "--cidfile".into(),
                    cidfile.display().to_string(),
                    "--label".into(),
                    ownership::root_label(&root),
                ]);
                if self.guest {
                    args.extend([
                        "--rootfs".into(),
                        "--env".into(),
                        "PATH=/home/agent/.local/bin:/opt/calternal-provider/bin:/usr/local/bin:/usr/bin:/bin".into(),
                    ]);
                }
                args.extend([image.to_owned(), "sleep".into(), "2147483647".into()]);
                let refs: Vec<&str> = args.iter().map(String::as_str).collect();
                let created = podman(&self.podman, &refs).await;
                if created.is_err()
                    && let Ok(id) = tokio::fs::read_to_string(&cidfile).await
                {
                    ownership::remove(&self.podman, &[id.trim()]).await?;
                }
                let _ = tokio::fs::remove_file(&cidfile).await;
                let container = created?;
                Ok(Arc::new(PodmanSandbox {
                    podman: self.podman.clone(),
                    container,
                    dir,
                    clients: Mutex::new(HashSet::new()),
                    worker: spec.worker.is_some(),
                    broker,
                }) as Arc<dyn Sandbox>)
            }
            .await;
            if result.is_err() && spec.worker.is_some() {
                let _ = tokio::fs::remove_dir_all(&worker_dir).await;
            }
            result
        })
    }
}

/// One thread's container.
pub struct PodmanSandbox {
    podman: PathBuf,
    container: String,
    dir: PathBuf,
    /// Process groups of local `podman exec` clients, killed on release.
    clients: Mutex<HashSet<i32>>,
    worker: bool,
    broker: Option<Broker>,
}

impl Sandbox for PodmanSandbox {
    fn exec(&self, request: ExecRequest) -> BoxFuture<'_, SandboxResult<Exec>> {
        Box::pin(async move {
            if request.argv.is_empty() {
                return Err(SandboxError("empty command".into()));
            }
            request.validate_env_names()?;
            let mut cmd = tokio::process::Command::new(&self.podman);
            cmd.args([
                "exec",
                "-i",
                "--workdir",
                if self.worker {
                    "/plugin"
                } else if self.broker.is_some() {
                    "/home/agent"
                } else {
                    "/work"
                },
            ]);
            for (k, v) in &request.env {
                // Podman resolves a name-only --env from its own environment.
                // Values, especially thread credentials, never enter host argv.
                cmd.arg("--env").arg(k).env(k, v);
            }
            if let Some(broker) = &self.broker {
                cmd.args(["--env", "CALTERNAL_GATEWAY_TOKEN"])
                    .env("CALTERNAL_GATEWAY_TOKEN", &broker.access);
            }
            cmd.arg(&self.container);
            if self.worker {
                // Only this fixed, non-secret locale enters argv. Dynamic values
                // remain in the exec client's environment via name-only --env.
                cmd.args(["env", "-i", "LANG=C.UTF-8"]);
            }
            cmd.args(&request.argv);
            cmd.as_std_mut().process_group(0);
            let (exec, pid) =
                super::spawn_streaming(cmd, request.stdin, request.interactive, self.worker)?;
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
        Box::pin(async move {
            podman(&self.podman, &["pause", &self.container])
                .await
                .map(drop)
        })
    }

    fn resume(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            podman(&self.podman, &["unpause", &self.container])
                .await
                .map(drop)
        })
    }

    fn stop(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            self.release().await?;
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
            if let Some(broker) = &self.broker {
                broker.close();
                let _ = tokio::fs::remove_dir_all(&broker.directory).await;
            }
            ownership::remove(&self.podman, &[&self.container]).await?;
            for &g in self.clients.lock().expect("clients").iter() {
                if let Some(pid) = Pid::from_raw(g) {
                    let _ = kill_process_group(pid, Signal::KILL);
                }
            }
            Ok(())
        })
    }
}
