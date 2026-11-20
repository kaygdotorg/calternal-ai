//! Firecracker process, persistent block device and optional TAP device.

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use std::{fs::File, io, sync::Arc};
use tokio::sync::OwnedSemaphorePermit;

use ring::{
    hmac,
    rand::{SecureRandom as _, SystemRandom},
};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use uuid::Uuid;

use super::snapshot_api::AttemptError;
use super::wire::{self, Request, Response};
use super::{Config, SandboxError, SandboxResult};

pub(super) struct Guest {
    pub(super) disk: PathBuf,
    pub(super) socket: PathBuf,
    child: Child,
    pub(super) api_socket: Option<PathBuf>,
    pub(super) control_disk: Option<PathBuf>,
    pub(super) restored: Option<Uuid>,
    pub(super) key: Arc<hmac::Key>,
    _permit: Arc<OwnedSemaphorePermit>,
    _network: Option<super::network::Lease>,
}

impl Guest {
    pub(super) async fn boot(
        config: &Config,
        run: &Path,
        disk: PathBuf,
        owned: Arc<File>,
        permit: OwnedSemaphorePermit,
        network: Option<super::network::Lease>,
    ) -> SandboxResult<Self> {
        Self::boot_profile(config, run, disk, owned, Arc::new(permit), network, false).await
    }

    /// Boot the offline guest and wait for its cold reseed acknowledgement.
    pub(super) async fn boot_small(
        config: &Config,
        run: &Path,
        disk: PathBuf,
        owned: Arc<File>,
        permit: Arc<OwnedSemaphorePermit>,
    ) -> SandboxResult<Self> {
        Self::boot_profile(config, run, disk, owned, permit, None, true).await
    }

    async fn boot_profile(
        config: &Config,
        run: &Path,
        disk: PathBuf,
        owned: Arc<File>,
        permit: Arc<OwnedSemaphorePermit>,
        network: Option<super::network::Lease>,
        small: bool,
    ) -> SandboxResult<Self> {
        let socket = run.join(format!(
            "{}.sock",
            &Uuid::new_v4().simple().to_string()[..16]
        ));
        if socket.as_os_str().len() >= 100 {
            return Err(SandboxError(
                "microVM runtime directory path is too long".into(),
            ));
        }
        let definition = socket.with_extension("json");
        let token_drive = socket.with_extension("key");
        let api_socket = small.then(|| socket.with_extension("api"));
        let mut token = [0; 512];
        let entropy_end = if small { 64 } else { 32 };
        SystemRandom::new()
            .fill(&mut token[..entropy_end])
            .map_err(|_| SandboxError("cannot generate guest control capability".into()))?;
        let key = Arc::new(hmac::Key::new(hmac::HMAC_SHA256, &token[..32]));
        let mut drive = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&token_drive)
            .await
            .map_err(SandboxError::new)?;
        drive.write_all(&token).await.map_err(SandboxError::new)?;
        drop(drive);
        let mut launch = json!({
            "boot-source": {
                "kernel_image_path": config.image_directory.join("vmlinux"),
                "boot_args": "root=/dev/vda ro rootfstype=ext4 init=/init reboot=k panic=1 pci=off console=ttyS0 quiet"
            },
            "drives": [
                {"drive_id":"system","path_on_host":config.image_directory.join("root.ext4"),"is_root_device":true,"is_read_only":true},
                {"drive_id":"data","path_on_host":disk,"is_root_device":false,"is_read_only":false},
                {"drive_id":"control","path_on_host":token_drive,"is_root_device":false,"is_read_only":true}
            ],
            "machine-config":{"vcpu_count":config.vcpus,"mem_size_mib":config.memory_mib,"smt":false},
            "vsock":{"guest_cid":3,"uds_path":socket}
        });
        if small {
            let boot = launch["boot-source"]["boot_args"]
                .as_str()
                .expect("boot arguments");
            launch["boot-source"]["boot_args"] = json!(format!("{boot} calternal_small_guest=1"));
            launch["machine-config"]["track_dirty_pages"] = json!(true);
        }
        if let Some(lease) = &network {
            let tap = lease.tap();
            let boot = launch["boot-source"]["boot_args"]
                .as_str()
                .expect("boot arguments");
            launch["boot-source"]["boot_args"] = json!(format!(
                "{boot} net.ifnames=0 calternal_net={}/{},{},{}",
                tap.address, tap.prefix, tap.gateway, tap.dns
            ));
            launch["network-interfaces"] = json!([{"iface_id":"eth0","host_dev_name":tap.name}]);
        }
        tokio::fs::write(&definition, launch.to_string())
            .await
            .map_err(SandboxError::new)?;
        let mut command = Command::new(&config.firecracker);
        if let Some(api) = &api_socket {
            command.arg("--api-sock").arg(api);
        } else {
            command.arg("--no-api");
        }
        command.arg("--config-file").arg(&definition);
        let child = match spawn_vm(command, owned) {
            Ok(child) => child,
            Err(error) => {
                let _ = tokio::fs::remove_file(definition).await;
                let _ = tokio::fs::remove_file(token_drive).await;
                return Err(SandboxError::new(error));
            }
        };
        let mut guest = Self {
            disk,
            socket,
            child,
            api_socket,
            control_disk: small.then(|| token_drive.clone()),
            restored: None,
            key,
            _permit: permit,
            _network: network,
        };
        let probe = if small {
            Request::SmallReady
        } else {
            Request::Ready
        };
        let ready = tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                if !guest.alive() {
                    break Err(SandboxError(
                        "Firecracker exited before the guest was ready".into(),
                    ));
                }
                if matches!(guest.control(&probe).await, Ok(())) {
                    break Ok(());
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
        let _ = tokio::fs::remove_file(definition).await;
        if !small {
            let _ = tokio::fs::remove_file(token_drive).await;
        }
        match ready {
            Ok(Ok(())) => Ok(guest),
            result => {
                guest.stop().await?;
                Err(match result {
                    Ok(Err(error)) => error,
                    _ => SandboxError("microVM did not boot within 60 seconds".into()),
                })
            }
        }
    }

    /// Resume and wait for the guest to acknowledge a fresh kernel reseed and
    /// publish readiness. Stop the VMM on failure.
    pub(super) async fn restore_small(
        config: &Config,
        run: &Path,
        generation: &super::snapshot::Generation,
        disk: PathBuf,
        control_disk: PathBuf,
        owned: Arc<File>,
        permit: Arc<OwnedSemaphorePermit>,
    ) -> Result<Self, AttemptError> {
        let guest =
            Self::load_small(config, run, generation, disk, control_disk, owned, permit).await?;
        let ready = tokio::time::timeout(Duration::from_secs(2), async {
            let mut seed = [0; 32];
            SystemRandom::new()
                .fill(&mut seed)
                .map_err(|_| SandboxError("Cannot get fresh small VM entropy".into()))?;
            // Ready acknowledges both the kernel update and the guest's
            // admission marker. Start still checks the marker independently.
            guest.control_checked(&Request::ReseedSmall(seed)).await
        })
        .await;
        match ready {
            Ok(Ok(())) => Ok(guest),
            result => {
                guest.stop().await?;
                match result {
                    Ok(Err(error)) => Err(error),
                    _ => Err(SandboxError("Small plugin VM reseed timed out".into()).into()),
                }
            }
        }
    }

    /// Resume the VMM with package admission still closed in the guest.
    pub(super) async fn load_small(
        config: &Config,
        run: &Path,
        generation: &super::snapshot::Generation,
        disk: PathBuf,
        control_disk: PathBuf,
        owned: Arc<File>,
        permit: Arc<OwnedSemaphorePermit>,
    ) -> Result<Self, AttemptError> {
        use super::snapshot_api;
        let socket = run.join(format!(
            "{}.sock",
            &Uuid::new_v4().simple().to_string()[..16]
        ));
        if socket.as_os_str().len() >= 100 {
            return Err(SandboxError("microVM runtime directory path is too long".into()).into());
        }
        let api_socket = socket.with_extension("api");
        let token = tokio::fs::read(&control_disk)
            .await
            .map_err(SandboxError::new)?;
        let token: &[u8] = token.get(..32).ok_or_else(|| {
            AttemptError::Refused(SandboxError("Invalid saved control drive".into()))
        })?;
        let key = Arc::new(hmac::Key::new(hmac::HMAC_SHA256, token));
        let mut command = Command::new(&config.firecracker);
        command.arg("--api-sock").arg(&api_socket);
        let child = spawn_vm(command, owned).map_err(SandboxError::new)?;
        let mut guest = Self {
            disk,
            socket,
            child,
            api_socket: Some(api_socket.clone()),
            control_disk: Some(control_disk),
            restored: Some(generation.id),
            key,
            _permit: permit,
            _network: None,
        };
        let ready = tokio::time::timeout(Duration::from_secs(3), async {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
            while !tokio::fs::try_exists(&api_socket)
                .await
                .map_err(SandboxError::new)?
            {
                if !guest.alive() || tokio::time::Instant::now() >= deadline {
                    return Err(
                        SandboxError("Firecracker did not open the snapshot API".into()).into(),
                    );
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            snapshot_api::load(
                &api_socket,
                &generation.path.join("state"),
                &generation.path.join("memory"),
                &guest.socket,
            )
            .await?;
            snapshot_api::resume(&api_socket).await.map_err(Into::into)
        })
        .await
        .unwrap_or_else(|_| Err(SandboxError("Small plugin VM restore timed out".into()).into()));
        if let Err(error) = ready {
            guest.stop().await?;
            return Err(error);
        }
        Ok(guest)
    }

    #[cfg(test)]
    pub(super) fn pid(&self) -> u32 {
        self.child.id().expect("running guest")
    }

    pub(super) fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub(super) async fn control(&self, request: &Request) -> SandboxResult<()> {
        self.control_checked(request).await.map_err(Into::into)
    }

    /// Distinguish an authenticated guest refusal from a broken or late pipe.
    pub(super) async fn control_checked(&self, request: &Request) -> Result<(), AttemptError> {
        tokio::time::timeout(Duration::from_secs(15), async {
            let mut channel = connect(&self.socket).await?;
            wire::prove(channel.get_mut(), &self.key)
                .await
                .map_err(SandboxError::new)?;
            wire::send(channel.get_mut(), request)
                .await
                .map_err(SandboxError::new)?;
            match wire::receive(&mut channel).await {
                Ok(Response::Ready) => Ok(()),
                Ok(Response::Error) => Err(AttemptError::Refused(SandboxError(
                    "microVM guest rejected sandbox operation".into(),
                ))),
                Ok(Response::Unavailable) => {
                    Err(SandboxError("microVM guest operation unavailable".into()).into())
                }
                _ => Err(SandboxError("microVM guest channel closed".into()).into()),
            }
        })
        .await
        .map_err(|_| {
            AttemptError::Unavailable(SandboxError("microVM guest operation timed out".into()))
        })?
    }

    pub(super) async fn stop(mut self) -> SandboxResult<()> {
        if self.alive() {
            self.child.kill().await.map_err(SandboxError::new)?;
        }
        let paths = [
            Some(self.socket.as_path()),
            self.api_socket.as_deref(),
            self.control_disk.as_deref(),
        ];
        for path in paths.into_iter().flatten() {
            match tokio::fs::remove_file(path).await {
                Err(error) if error.kind() != io::ErrorKind::NotFound => {
                    return Err(SandboxError::new(error));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn spawn_vm(mut command: Command, owned: Arc<File>) -> io::Result<Child> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let parent = rustix::process::getpid();
    // SAFETY: the child invokes only async-signal-safe system calls and
    // compares the parent to close the race with a concurrent machine exit.
    unsafe {
        command.as_std_mut().pre_exec(move || {
            rustix::io::fcntl_setfd(owned.as_ref(), rustix::io::FdFlags::empty())?;
            rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::KILL))?;
            if rustix::process::getppid() != Some(parent) {
                return Err(io::Error::from(io::ErrorKind::Interrupted));
            }
            Ok(())
        });
    }
    command.spawn()
}

pub(super) async fn connect(socket: &Path) -> SandboxResult<BufReader<UnixStream>> {
    let mut attempt = 0;
    loop {
        match handshake(socket).await {
            Ok(channel) => return Ok(channel),
            Err(error) if attempt == 5 => return Err(error),
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
        attempt += 1;
    }
}

async fn handshake(socket: &Path) -> SandboxResult<BufReader<UnixStream>> {
    let io = UnixStream::connect(socket)
        .await
        .map_err(SandboxError::new)?;
    let mut io = BufReader::new(io);
    io.get_mut()
        .write_all(format!("CONNECT {}\n", wire::PORT).as_bytes())
        .await
        .map_err(SandboxError::new)?;
    let mut header = [0; 32];
    let mut len = 0;
    loop {
        let bytes = io.fill_buf().await.map_err(SandboxError::new)?;
        let take = bytes
            .iter()
            .position(|&b| b == b'\n')
            .map_or(bytes.len(), |i| i + 1);
        if take == 0 || len + take > header.len() {
            return Err(SandboxError("invalid microVM vsock handshake".into()));
        }
        header[len..len + take].copy_from_slice(&bytes[..take]);
        len += take;
        io.consume(take);
        if header[len - 1] == b'\n' {
            break;
        }
    }
    if !header[..len].starts_with(b"OK ") {
        return Err(SandboxError("microVM vsock refused the connection".into()));
    }
    Ok(io)
}
