//! Construct mounts, process-family tracking and engine arguments for a sandbox.

use super::*;
use crate::protocol::Limits;
use crate::sandbox::shared_broker::ToolSandbox;

impl PodmanBackend {
    pub(super) fn start_container<'a>(
        &'a self,
        spec: &'a SandboxSpec,
        harness: Option<&'a crate::protocol::HarnessSandbox>,
        plugin_root: Option<&'a Path>,
        egress_directory: Option<&'a Path>,
        credential: Option<&'a Path>,
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
                let dir = if let Some(sockets) = egress_directory {
                    sockets.parent().expect("private run root").join("engine")
                } else if spec.worker.is_some() {
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
                let (home, login) = if spec.shared_tools.is_some() || egress_directory.is_some() {
                    (PathBuf::new(), PathBuf::new())
                } else { match harness {
                    Some(harness) => crate::sandbox::provider_home::prepare_harness(
                        account, &dir, spec, harness,
                    ).await?,
                    None => crate::sandbox::provider_home::prepare(account, &dir, spec).await?,
                }};
                let work = dir.join("work");
                let provision = if egress_directory.is_some() { &dir } else { &work };
                tokio::fs::create_dir_all(provision).await.map_err(|e| {
                    SandboxError(format!("cannot create work directory: {}", e.kind()))
                })?;
                if let Some(worker) = &spec.worker {
                    crate::sandbox::worker::prepare(&work, worker).await?;
                }
                let Limits {
                    memory_bytes,
                    cpu_millis,
                    pids,
                } = if spec.worker.is_some() {
                    crate::sandbox::worker::LIMITS
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
                } else if let Some(sockets) = egress_directory {
                    run_mounts::apply(
                        &mut args,
                        plugin_root.expect("run plugin root"),
                        sockets,
                        credential,
                        &memory,
                    );
                    None
                } else {
                    args.extend([
                        "--volume".into(), mount, "--volume".into(), home_mount,
                        "--workdir".into(), "/work".into(), "--env".into(), "HOME=/home/agent".into(),
                    ]);
                    None
                };
                if spec.worker.is_none() && broker.is_none() && egress_directory.is_none() {
                    let mode = if harness.is_some_and(|harness| !harness.legacy_profile) || spec.provider_login { "rw" } else { "ro" };
                    args.extend([
                        "--volume".into(),
                        format!("{}:/run/calternal-provider-login:{mode}", login.display()),
                        "--env".into(),
                        "CALTERNAL_PROVIDER_LOGIN_DIR=/run/calternal-provider-login".into(),
                    ]);
                    if harness.is_some() && !self.guest {
                        args.extend([
                            "--network=none".into(),
                            "--pull=never".into(),
                            "--http-proxy=false".into(),
                        ]);
                    }
                    if spec.provider_login {
                        args.extend(["--env".into(), "CALTERNAL_PROVIDER_LOGIN=1".into()]);
                    }
                }
                if spec.worker.is_some() {
                    // A worker sees only code. In particular it never inherits the
                    // account home, writable work tree, CLI or Nix store mounts.
                    let seccomp = crate::sandbox::worker::seccomp(&dir).await?;
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
                            if harness.is_none() && self.guest_network {
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
                    && egress_directory.is_none()
                    && let Some(exe) = crate::sandbox::cli_program().await
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
                #[cfg(target_os = "linux")]
                let pid_file = egress_directory.map(|_| dir.join("container.pid"));
                #[cfg(target_os = "linux")]
                if let Some(pid_file) = &pid_file {
                    args.extend(["--pidfile".into(), pid_file.display().to_string()]);
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
                #[cfg(target_os = "linux")]
                let family = if let Some(pid_file) = &pid_file {
                    match family::Family::open(pid_file).await {
                        Ok(family) => Some(family),
                        Err(error) => {
                            ownership::remove(&self.podman, &[&container]).await?;
                            return Err(error);
                        }
                    }
                } else {
                    None
                };
                Ok(Arc::new(PodmanSandbox {
                    #[cfg(target_os = "linux")]
                    family,
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
