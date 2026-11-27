//! Rootless Podman sandboxes: one container per thread.
//!
//! - Base image by reference (`docker.io/library/alpine:3` or a digest); the image
//!   must already be present or pullable by the machine user.
//! - The container's private overlay is disposable. The /work and home bind mounts persist.
//! - Harness containers use `--network=none` until mediated provider egress is proven.
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
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rustix::process::{Pid, Signal, kill_process_group, test_kill_process_group};
use uuid::Uuid;

use super::shared_broker::Broker;
use super::{Backend, BoxFuture, Exec, ExecRequest, Sandbox, SandboxError, SandboxResult};
use crate::protocol::SandboxSpec;

#[cfg(target_os = "linux")]
mod family;
mod ownership;
mod run_mounts;
mod run_sign_in;
mod start;

use run_sign_in::RunSignIn;

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
    run_sign_in: Option<RunSignIn>,
    run_egress_exclusions: Arc<[ipnet::IpNet]>,
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
            run_sign_in: None,
            run_egress_exclusions: Arc::default(),
        }
    }

    /// Authorize one account, space and qualified program to mount an
    /// operator-owned credential file. Configuration validates syntax only.
    /// The operator must control the private file and its ancestor directories.
    pub fn with_run_sign_in(
        mut self,
        account: calternal_types::AccountId,
        space: calternal_types::SpaceId,
        program: String,
        credential_file: PathBuf,
    ) -> SandboxResult<Self> {
        self.run_sign_in = Some(RunSignIn::new(account, space, program, credential_file)?);
        Ok(self)
    }

    /// Reject resolved run egress addresses in these comma-separated CIDRs
    /// in addition to the built-in special-use address exclusions.
    pub fn with_run_egress_exclusions(
        mut self,
        cidrs: &str,
    ) -> Result<Self, crate::egress::EgressError> {
        self.run_egress_exclusions = crate::egress::exclusions(cidrs)?;
        Ok(self)
    }

    pub(crate) fn run_egress_exclusions(&self) -> Arc<[ipnet::IpNet]> {
        self.run_egress_exclusions.clone()
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

    pub(crate) async fn run_work_root(&self) -> SandboxResult<PathBuf> {
        Ok(self.canonical_root().await?.join(run_mounts::WORK_ROOT))
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
        .kill_on_drop(true)
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
            for directory in ["workers", run_mounts::WORK_ROOT] {
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
        self.start_container(spec, None, None, None, None)
    }

    fn start_harness<'a>(
        &'a self,
        spec: &'a SandboxSpec,
        harness: &'a crate::protocol::HarnessSandbox,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        self.start_container(spec, Some(harness), None, None, None)
    }
}

/// Starts a harness with a read-only plugin bundle and one host-side egress socket.
impl PodmanBackend {
    /// Start a network-isolated harness with its installed plugin mounted
    /// read-only at /opt/calternal-plugin and its run proxy mounted read-only at
    /// /run/calternal-egress. Cost includes normal Podman container creation.
    pub fn start_harness_run<'a>(
        &'a self,
        spec: &'a SandboxSpec,
        harness: &'a crate::protocol::HarnessSandbox,
        plugin_root: &'a Path,
        egress_directory: &'a Path,
        program: &'a str,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        Box::pin(async move {
            if !cfg!(target_os = "linux") || self.guest || self.kvm_fallback {
                return Err(SandboxError(
                    "mediated harness egress requires rootless Podman".into(),
                ));
            }
            let credential = match &self.run_sign_in {
                Some(sign_in) => Some(
                    sign_in
                        .credential(
                            spec,
                            program,
                            egress_directory.parent().expect("private run root"),
                        )
                        .await?,
                ),
                None => None,
            };
            self.start_container(
                spec,
                Some(harness),
                Some(plugin_root),
                Some(egress_directory),
                credential.as_deref(),
            )
            .await
        })
    }
}

/// One thread's container.
pub struct PodmanSandbox {
    #[cfg(target_os = "linux")]
    family: Option<family::Family>,
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

    fn terminate(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            #[cfg(target_os = "linux")]
            if let Some(family) = &self.family {
                return family.terminate().await;
            }
            self.release().await
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
            #[cfg(target_os = "linux")]
            family: None,
            podman: engine,
            container: "fixture".into(),
            dir: dir.path().join("work"),
            clients: Mutex::new(HashSet::new()),
            worker: false,
            broker: None,
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
