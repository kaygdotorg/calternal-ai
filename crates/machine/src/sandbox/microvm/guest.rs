//! Rootless guest agent. The guest init binds vsock to this private Unix listener.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;

use super::wire::{self, Request, Response};
use crate::protocol::ChunkEvent;
use crate::sandbox::{
    Backend, OutputEvent, Sandbox, SandboxError, SandboxResult, podman::PodmanBackend,
};

struct Agent {
    backend: PodmanBackend,
    key: ring::hmac::Key,
    threads: Mutex<HashMap<Uuid, Arc<dyn Sandbox>>>,
}

/// Run the guest-only agent on a Unix socket behind the boot image's vsock listener.
/// The image names the packaged root filesystem and its writable-overlay flag.
/// Startup sweeps old Podman containers before accepting commands. The caller must
/// run as the dedicated unprivileged guest user, with a delegated cgroup.
/// The kernel-local token authenticates the host before any request is decoded.
pub async fn run_agent(
    socket: PathBuf,
    root: PathBuf,
    image: String,
    token: [u8; 32],
) -> SandboxResult<()> {
    let backend = PodmanBackend::in_microvm(root, image);
    backend.initialize().await?;
    let agent = Arc::new(Agent {
        backend,
        key: ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &token),
        threads: Mutex::new(HashMap::new()),
    });
    let listener = UnixListener::bind(socket).map_err(SandboxError::new)?;
    let connections = Arc::new(Semaphore::new(512));
    loop {
        let (stream, _) = listener.accept().await.map_err(SandboxError::new)?;
        if let Ok(permit) = connections.clone().try_acquire_owned() {
            let agent = agent.clone();
            tokio::spawn(async move {
                let _permit = permit;
                let _ = agent.serve(stream).await;
            });
        }
    }
}

impl Agent {
    async fn sandbox(&self, thread: Uuid) -> SandboxResult<Arc<dyn Sandbox>> {
        self.threads
            .lock()
            .await
            .get(&thread)
            .cloned()
            .ok_or_else(|| SandboxError("guest sandbox is absent".into()))
    }

    async fn serve(&self, mut io: UnixStream) -> SandboxResult<()> {
        let request = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            wire::verify(&mut io, &self.key).await?;
            wire::receive(&mut io).await
        })
        .await
        .map_err(SandboxError::new)?
        .map_err(SandboxError::new)?;
        if let Request::Exec(thread, request) = request {
            return match self.sandbox(thread).await {
                Ok(sandbox) => match sandbox.exec(request).await {
                    Ok(exec) => stream_exec(io, exec).await,
                    Err(_) => wire::send(&mut io, &Response::Error)
                        .await
                        .map_err(SandboxError::new),
                },
                Err(_) => wire::send(&mut io, &Response::Error)
                    .await
                    .map_err(SandboxError::new),
            };
        }
        let response = if matches!(request, Request::Observe) {
            match self.observe().await {
                Ok(sample) => sample,
                Err(_) => Response::Error,
            }
        } else if self.control(request).await.is_ok() {
            Response::Ready
        } else {
            Response::Error
        };
        wire::send(&mut io, &response)
            .await
            .map_err(SandboxError::new)
    }

    async fn observe(&self) -> SandboxResult<Response> {
        let meminfo = tokio::fs::read_to_string("/proc/meminfo")
            .await
            .map_err(SandboxError::new)?;
        let memory_available_bytes = available_memory(&meminfo)
            .ok_or_else(|| SandboxError("guest memory observation unavailable".into()))?;
        let active_threads =
            u32::try_from(self.threads.lock().await.len()).map_err(SandboxError::new)?;
        Ok(Response::Resources {
            memory_available_bytes,
            active_threads,
        })
    }

    async fn control(&self, request: Request) -> SandboxResult<()> {
        match request {
            Request::Ready => Ok(()),
            Request::Start(space, mut spec) => {
                spec.space_id = Some(space);
                let sandbox = self.backend.start(&spec).await?;
                self.threads.lock().await.insert(spec.thread_id, sandbox);
                Ok(())
            }
            Request::Freeze(thread) => self.sandbox(thread).await?.freeze().await,
            Request::Resume(thread) => self.sandbox(thread).await?.resume().await,
            Request::Release(thread, keep_files) => {
                let sandbox = self.sandbox(thread).await?;
                if keep_files {
                    sandbox.release().await?;
                } else {
                    sandbox.stop().await?;
                }
                sync_disk().await?;
                self.threads.lock().await.remove(&thread);
                Ok(())
            }
            Request::Discard(account, thread) => {
                self.backend.erase(account, Some(thread)).await?;
                sync_disk().await
            }
            Request::Exec(..) | Request::Observe => unreachable!(),
        }
    }
}

fn available_memory(meminfo: &str) -> Option<u64> {
    let line = meminfo.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key == "MemAvailable").then_some(value)
    })?;
    let mut parts = line.split_ascii_whitespace();
    let kib: u64 = parts.next()?.parse().ok()?;
    if parts.next()? != "kB" || parts.next().is_some() {
        return None;
    }
    kib.checked_mul(1024)
}

async fn sync_disk() -> SandboxResult<()> {
    let status = tokio::process::Command::new("sync")
        .args(["-f", "/data"])
        .status()
        .await
        .map_err(SandboxError::new)?;
    if status.success() {
        Ok(())
    } else {
        Err(SandboxError("guest disk sync failed".into()))
    }
}

async fn stream_exec(io: UnixStream, mut exec: crate::sandbox::Exec) -> SandboxResult<()> {
    let (mut reader, mut writer) = io.into_split();
    let input = tokio::spawn(async move {
        if let Some(input) = exec.input.take() {
            while let Ok(Some(data)) = wire::receive::<Option<Vec<u8>>>(&mut reader).await {
                if data.len() > calternal_types::MAX_FRAME_BYTES + 1
                    || input.send(data).await.is_err()
                {
                    break;
                }
            }
        }
    });
    let result = async {
        while let Some(output) = exec.events.recv().await {
            let event = match output {
                OutputEvent::Data(stream, data) => ChunkEvent::Data {
                    stream,
                    data: STANDARD.encode(data),
                },
                OutputEvent::Exit(exit) => ChunkEvent::Exit(exit),
            };
            wire::send(&mut writer, &Response::Output(event))
                .await
                .map_err(SandboxError::new)?;
        }
        Ok(())
    }
    .await;
    input.abort();
    result
}

#[cfg(test)]
mod tests {
    use super::available_memory;

    #[test]
    fn available_memory_uses_the_kernel_headroom_field_and_checks_units() {
        assert_eq!(
            available_memory("MemTotal: 512000 kB\nMemFree: 1234 kB\nMemAvailable: 8192 kB\n"),
            Some(8 * 1024 * 1024)
        );
        assert_eq!(available_memory("MemFree: 1234 kB\n"), None);
        assert_eq!(available_memory("MemAvailable: 8 MB\n"), None);
        assert_eq!(
            available_memory("MemAvailable: 18446744073709551615 kB\n"),
            None
        );
    }
}
