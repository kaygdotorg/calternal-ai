//! Rootless guest agent. The guest init binds vsock to this private Unix listener.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;

use super::snapshot_api::AttemptError;
use super::wire::{self, Request, Response};
use crate::protocol::ChunkEvent;
use crate::sandbox::{
    Backend, OutputEvent, Sandbox, SandboxError, SandboxResult, podman::PodmanBackend,
};

const SMALL_READY: &str = "/run/agent/small-ready";

struct Agent {
    backend: Option<PodmanBackend>,
    key: ring::hmac::Key,
    threads: Mutex<HashMap<Uuid, Arc<dyn Sandbox>>>,
}

/// Run the guest-only agent on a Unix socket behind the boot image's vsock listener.
/// The image names the packaged root filesystem and its writable-overlay flag.
/// Agent guests sweep old Podman containers. The small guest stages package code
/// in private tmpfs and uses a dedicated launcher. The caller must
/// run as the dedicated unprivileged guest user, with a delegated cgroup.
/// The kernel-local token authenticates the host before any request is decoded.
pub async fn run_agent(
    socket: PathBuf,
    root: PathBuf,
    image: String,
    token: [u8; 32],
) -> SandboxResult<()> {
    let backend = if std::env::var_os("CALTERNAL_SMALL_GUEST").is_some() {
        // The package path stages only into new boot tmpfs. Restores use an
        // empty package directory after checkpoint drain, so there is no
        // Podman scratch inventory to scan in either case.
        None
    } else {
        let backend = PodmanBackend::in_microvm(root, image);
        backend.initialize().await?;
        Some(backend)
    };
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
        let result = match request {
            Request::ReseedSmall(seed) => self.reseed_small(seed).await,
            request => self.control(request).await.map_err(AttemptError::Refused),
        };
        let response = match result {
            Ok(()) => Response::Ready,
            Err(AttemptError::Refused(_)) => Response::Error,
            Err(AttemptError::Unavailable(_)) => Response::Unavailable,
        };
        wire::send(&mut io, &response)
            .await
            .map_err(SandboxError::new)
    }

    async fn reseed_small(&self, seed: [u8; 32]) -> Result<(), AttemptError> {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
        // The small guest snapshots only after the package and its
        // exec pipes have been stopped. An active guest cannot reseed.
        // Only this guest's startup can wait on the privileged
        // helper. Keep the lock until acknowledgement so Start cannot
        // observe an incomplete seed exchange.
        let threads = self.threads.lock().await;
        if !threads.is_empty() {
            return Err(AttemptError::Refused(SandboxError(
                "small plugin guest was not drained".into(),
            )));
        }
        let mut control = UnixStream::connect("/run/calternal-reseed.sock")
            .await
            .map_err(SandboxError::new)?;
        control.write_all(&seed).await.map_err(SandboxError::new)?;
        if control.read_u8().await.map_err(SandboxError::new)? != b'1' {
            return Err(AttemptError::Refused(SandboxError(
                "small plugin guest reseed was not acknowledged".into(),
            )));
        }
        tokio::fs::write(SMALL_READY, [])
            .await
            .map_err(|error| SandboxError::new(error).into())
    }

    async fn control(&self, request: Request) -> SandboxResult<()> {
        match request {
            Request::Ready => Ok(()),
            Request::SmallReady => {
                let _threads = self.threads.lock().await;
                small_ready().await
            }
            Request::QuiesceSmall => {
                let threads = self.threads.lock().await;
                if !threads.is_empty() {
                    return Err(SandboxError("small plugin guest was not drained".into()));
                }
                tokio::fs::remove_file(SMALL_READY)
                    .await
                    .map_err(SandboxError::new)
            }
            Request::ReseedSmall(_) => unreachable!(),
            Request::Start(space, mut spec) => {
                spec.space_id = Some(space);
                if let Some(backend) = &self.backend {
                    let sandbox = backend.start(&spec).await?;
                    self.threads.lock().await.insert(spec.thread_id, sandbox);
                } else {
                    // Serialize package admission with Quiesce and Reseed.
                    let mut threads = self.threads.lock().await;
                    small_ready().await?;
                    if !threads.is_empty() {
                        return Err(SandboxError("small guest already has a package".into()));
                    }
                    let sandbox = super::plain::start(&spec).await?;
                    threads.insert(spec.thread_id, sandbox);
                }
                Ok(())
            }
            Request::StartHarness(space, mut spec, harness) => {
                let backend = self.backend.as_ref().ok_or_else(|| {
                    SandboxError("small plugin guest cannot start a harness".into())
                })?;
                spec.space_id = Some(space);
                let sandbox = backend.start_harness(&spec, &harness).await?;
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
                if self.backend.is_some() {
                    sync_disk().await?;
                }
                self.threads.lock().await.remove(&thread);
                Ok(())
            }
            Request::Discard(account, thread) => {
                if let Some(backend) = &self.backend {
                    backend.erase(account, Some(thread)).await?;
                    sync_disk().await?;
                }
                Ok(())
            }
            Request::Exec(..) => unreachable!(),
        }
    }
}

async fn small_ready() -> SandboxResult<()> {
    if tokio::fs::try_exists(SMALL_READY)
        .await
        .map_err(SandboxError::new)?
    {
        Ok(())
    } else {
        Err(SandboxError("small plugin kernel was not reseeded".into()))
    }
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
