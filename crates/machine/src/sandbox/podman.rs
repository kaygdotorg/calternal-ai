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

use super::{Backend, BoxFuture, Exec, ExecRequest, Sandbox, SandboxError, SandboxResult};
use crate::protocol::{Limits, SandboxSpec};

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
            match tokio::fs::remove_dir_all(self.root.join("workers")).await {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(SandboxError::new(e)),
            }
        })
    }
    fn name(&self) -> &'static str {
        "podman"
    }

    fn discard(&self, account: uuid::Uuid, thread: uuid::Uuid) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
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
        Box::pin(async move {
            let root = self.canonical_root().await?;
            let name = Self::container_name(spec.account_id, spec.thread_id);
            let result = async {
                let account = root.join("accounts").join(spec.account_id.to_string());
                let dir = if spec.worker.is_some() {
                    root.join("workers").join(spec.thread_id.to_string())
                } else {
                    account.join("threads").join(spec.thread_id.to_string())
                };
                let (home, login) = super::provider_home::prepare(account, &dir, spec).await?;
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
                let image = spec.image.as_deref().unwrap_or(&self.default_image);
                let label = format!("org.calternal.thread={}", spec.thread_id);
                let home_mount = format!("{}:/home/agent", home.display());
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
                    "--volume",
                    &home_mount,
                    "--workdir",
                    "/work",
                    "--env",
                    "HOME=/home/agent",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect();
                if spec.worker.is_none() {
                    let mode = if spec.provider_login { "rw" } else { "ro" };
                    args.extend([
                        "--volume".into(),
                        format!("{}:/run/calternal-provider-login:{mode}", login.display()),
                        "--env".into(),
                        "CALTERNAL_CLAUDE_LOGIN_DIR=/run/calternal-provider-login".into(),
                    ]);
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
                // Linux hosts supply the matching operation CLI read-only. Other
                // host platforms must put a Linux CLI in the container image.
                // Nix builds retain immutable store references for their libraries.
                #[cfg(target_os = "linux")]
                if spec.worker.is_none()
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
                    image.to_owned(),
                    "sleep".into(),
                    "2147483647".into(),
                ]);
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
                }) as Arc<dyn Sandbox>)
            }
            .await;
            if result.is_err() && spec.worker.is_some() {
                let _ = tokio::fs::remove_dir_all(
                    root.join("workers").join(spec.thread_id.to_string()),
                )
                .await;
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
                if self.worker { "/plugin" } else { "/work" },
            ]);
            for (k, v) in &request.env {
                // Podman resolves a name-only --env from its own environment.
                // Values, especially thread credentials, never enter host argv.
                cmd.arg("--env").arg(k).env(k, v);
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

#[cfg(test)]
mod argv_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[tokio::test]
    async fn podman_exec_keeps_environment_values_out_of_host_argv() {
        let dir = tempfile::tempdir().unwrap();
        let engine = dir.path().join("podman");
        // A fake engine records argv and verifies the child environment, without
        // requiring Podman or emitting the credential in test output.
        tokio::fs::write(&engine,b"#!/bin/sh\nfor arg do printf '%s\\n' \"$arg\"; done\n[ \"$CALTERNAL_AGENT_TOKEN\" = 'synthetic-private-token' ] || exit 42\n[ \"$MULTILINE\" = 'first\nsecond=value' ] || exit 43\n").await.unwrap();
        tokio::fs::set_permissions(&engine, std::fs::Permissions::from_mode(0o700))
            .await
            .unwrap();
        let sandbox = PodmanSandbox {
            podman: engine,
            container: "fixture".into(),
            dir: dir.path().join("work"),
            clients: Mutex::new(HashSet::new()),
            worker: false,
        };
        let mut exec = sandbox
            .exec(ExecRequest {
                interactive: false,
                argv: vec!["true".into()],
                stdin: String::new(),
                env: vec![
                    (
                        "CALTERNAL_AGENT_TOKEN".into(),
                        "synthetic-private-token".into(),
                    ),
                    ("MULTILINE".into(), "first\nsecond=value".into()),
                ],
            })
            .await
            .unwrap();
        let mut output = Vec::new();
        let mut status = None;
        while let Some(event) = exec.events.recv().await {
            match event {
                super::super::OutputEvent::Data(_, bytes) => output.extend(bytes),
                super::super::OutputEvent::Exit(info) => status = info.code,
            }
        }
        assert_eq!(
            status,
            Some(0),
            "engine did not receive the environment intact"
        );
        let argv = String::from_utf8(output).unwrap();
        assert!(argv.lines().any(|s| s == "CALTERNAL_AGENT_TOKEN"));
        assert!(
            !argv.contains("synthetic-private-token"),
            "credential entered host argv"
        );
        assert!(
            !argv.contains("second=value"),
            "multiline value entered host argv"
        );
        let invalid = sandbox
            .exec(ExecRequest {
                interactive: false,
                argv: vec!["true".into()],
                env: vec![("NAME=inline-secret".into(), String::new())],
                ..ExecRequest::default()
            })
            .await;
        assert!(
            invalid.is_err(),
            "inline environment values must not become names"
        );
    }
}
