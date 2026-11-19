//! Machine sandbox startup and bounded protocol frame transport.
use super::*;
use calternal_machine::sandbox::worker::STDERR_TAIL_BYTES as STDERR_BYTES;

pub(super) type StderrTail = Arc<Mutex<Vec<u8>>>;

/// Start one worker process and its threads. The pinned digest is rechecked first.
pub(super) async fn spawn(
    node: Option<&NodeRuntime>,
    permit: tokio::sync::OwnedSemaphorePermit,
    key_permit: tokio::sync::OwnedSemaphorePermit,
    key: Arc<StartKey>,
    backend: WorkerBackend,
    runtime: tokio::runtime::Handle,
    account: AccountId,
    plugin: &Arc<Plugin>,
    bundle: &Path,
    space: SpaceId,
    in_flight: Arc<AtomicUsize>,
    requires_ready: bool,
) -> Result<Worker, Error> {
    use calternal_machine::{
        protocol::{PluginWorker, SandboxSpec},
        sandbox::{ExecRequest, worker},
    };
    let path = bundle.to_path_buf();
    let digest = plugin.entry.worker_sha256.clone();
    let source = crate::blocking::spawn(move || {
        use std::io::Read;
        let mut source = String::new();
        std::fs::File::open(path)
            .and_then(|file| {
                file.take(worker::MAX_SOURCE_BYTES as u64 + 1)
                    .read_to_string(&mut source)
            })
            .map_err(|_| Error::unavailable("plugin bundle unreadable"))?;
        if source.len() > worker::MAX_SOURCE_BYTES {
            return Err(Error::unavailable("plugin bundle exceeds 1 MiB"));
        }
        if !digest.as_deref().is_some_and(|digest| {
            digest.eq_ignore_ascii_case(&crate::store::sha256_hex(source.as_bytes()))
        }) {
            return Err(Error::unavailable("plugin bundle failed verification"));
        }
        Ok(source)
    })
    .await
    .map_err(|_| Error::internal("plugin verification task failed"))??;
    let session = WorkerSession::new(plugin.manifest.clone(), plugin.schemas.clone(), space)
        .map_err(|_| Error::unavailable("plugin manifest rejected by worker session"))?;
    let spec = SandboxSpec {
        resource_owner: None,
        guest_idle_seconds: None,
        resource_assignment: None,
        space_id: Some(space),
        provider_login: false,
        shared_provider_sponsor: None,
        shared_tools: None,
        // Operator-installed packages always start from the private copy of
        // verified bytes. Explicit in-memory fixtures can supply test assets.
        code_directory: (plugin.revision == 0 && backend.name() != "machine")
            .then(|| bundle.parent().unwrap_or(Path::new("/")).to_path_buf()),
        account_id: account.as_uuid(),
        thread_id: uuid::Uuid::new_v4(),
        image: Some(worker::IMAGE.into()),
        limits: worker::LIMITS,
        worker: Some(PluginWorker {
            space_id: space,
            source,
        }),
    };
    let sandbox = match &backend {
        WorkerBackend::Enrolled(service) => {
            super::super::machine_worker::start(service.clone(), spec).await?
        }
        WorkerBackend::Local(local) => local.start(&spec).await.map_err(|error| {
            tracing::warn!(%error, "worker sandbox transport unavailable");
            Error::unavailable("plugin worker sandbox unavailable; check the local runtime")
        })?,
    };
    let process = Process {
        permits: Some((permit, key_permit, key)),
        sandbox,
        runtime: runtime.clone(),
    };
    let mut exec = process
        .sandbox
        .exec(ExecRequest {
            argv: vec![
                if backend.name() == "podman" {
                    "node".into()
                } else {
                    node.map_or_else(|| "node".into(), |n| n.path.to_string_lossy().into_owned())
                },
                "--permission".into(),
                "--allow-fs-read=.".into(),
                "--disallow-code-generation-from-strings".into(),
                format!("--max-old-space-size={WORKER_HEAP_MB}"),
                "worker.js".into(),
            ],
            interactive: true,
            ..ExecRequest::default()
        })
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "worker sandbox transport unavailable");
            Error::unavailable(
                "plugin worker sandbox unavailable; check the enrolled machine runtime",
            )
        })?;
    let stderr = StderrTail::default();
    if requires_ready {
        let ready = tokio::time::timeout(WORKER_START_TIMEOUT, readiness(&mut exec, &stderr))
            .await
            .map_err(|_| Error::unavailable("plugin worker readiness timed out"))
            .and_then(|result| result);
        if let Err(error) = ready {
            let bytes = stderr.lock().unwrap_or_else(|e| e.into_inner());
            plugin
                .installed_diagnostics
                .fault(space, "worker_handshake", &bytes);
            return Err(error);
        }
    }
    let input = exec
        .input
        .take()
        .ok_or_else(|| Error::unavailable("worker stdin unavailable"))?;
    let plugin = plugin.clone();
    crate::blocking::spawn(move || {
        let (events, rx) = mpsc::sync_channel(64);
        let (frames, frames_rx) = mpsc::sync_channel::<Vec<u8>>(MAX_QUEUE);
        let reader_events = events.clone();
        let reader_runtime = runtime.clone();
        let reader_stderr = stderr.clone();
        std::thread::Builder::new()
            .name("plugin-worker-read".into())
            .spawn(move || read_frames(exec, reader_events, reader_runtime, reader_stderr))
            .map_err(|_| Error::unavailable("extension worker could not start"))?;
        std::thread::Builder::new()
            .name("plugin-worker-write".into())
            .spawn(move || {
                for frame in frames_rx {
                    if runtime.block_on(input.send(frame)).is_err() {
                        return;
                    }
                }
            })
            .map_err(|_| Error::unavailable("extension worker could not start"))?;
        let alive = Arc::new(AtomicBool::new(true));
        let busy = Arc::new(AtomicUsize::new(0));
        plugin.installed_diagnostics.ready(space);
        let thread = Thread {
            plugin,
            stderr,
            _process: process,
            session,
            space,
            frames,
            active: BTreeMap::new(),
            next: 0,
            alive: alive.clone(),
            busy: busy.clone(),
            in_flight,
        };
        std::thread::Builder::new()
            .name("plugin-worker".into())
            .spawn(move || thread.run(rx))
            .map_err(|_| Error::unavailable("extension worker could not start"))?;
        Ok(Worker {
            events,
            alive,
            busy,
            #[cfg(feature = "dev-plugin-drafts")]
            retire_after_result: None,
        })
    })
    .await
    .map_err(|_| Error::internal("worker transport setup failed"))?
}

// The worker must echo the exact challenge before it receives an invocation.
async fn readiness(
    exec: &mut calternal_machine::sandbox::Exec,
    stderr: &StderrTail,
) -> Result<(), Error> {
    use calternal_machine::{protocol::Stream, sandbox::OutputEvent};
    let nonce = uuid::Uuid::new_v4();
    let challenge =
        format!("{{\"type\":\"ready\",\"protocol_version\":2,\"nonce\":\"{nonce}\"}}\n")
            .into_bytes();
    let invalid = || Error::unavailable("plugin worker did not answer its readiness challenge");
    exec.input
        .as_ref()
        .ok_or_else(invalid)?
        .send(challenge.clone())
        .await
        .map_err(|_| invalid())?;
    let mut offset = 0;
    while let Some(event) = exec.events.recv().await {
        match event {
            OutputEvent::Data(Stream::Stdout, bytes) => {
                let end = offset + bytes.len();
                if challenge.get(offset..end) != Some(bytes.as_slice()) {
                    return Err(invalid());
                }
                offset = end;
                if offset == challenge.len() {
                    return Ok(());
                }
            }
            OutputEvent::Data(Stream::Stderr, bytes) => append_stderr(stderr, &bytes),
            OutputEvent::Exit(_) => break,
        }
    }
    Err(invalid())
}

/// Read newline-delimited frames, never buffering more than one frame budget.
fn read_frames(
    mut exec: calternal_machine::sandbox::Exec,
    events: SyncSender<Event>,
    runtime: tokio::runtime::Handle,
    stderr: StderrTail,
) {
    use calternal_machine::{protocol::Stream, sandbox::OutputEvent};
    runtime.block_on(async move {
        let mut line = Vec::new();
        while let Some(event) = exec.events.recv().await {
            if let OutputEvent::Data(Stream::Stdout, bytes) = event {
                for byte in bytes {
                    line.push(byte);
                    if line.len() > MAX_FRAME_BYTES + 1 {
                        let _ = events.send(Event::Overflow);
                        return;
                    }
                    if byte == b'\n' && events.send(Event::Line(std::mem::take(&mut line))).is_err()
                    {
                        return;
                    }
                }
            } else if let OutputEvent::Data(Stream::Stderr, bytes) = event {
                append_stderr(&stderr, &bytes);
            } else if matches!(event, OutputEvent::Exit(_)) {
                break;
            }
        }
        let _ = events.send(Event::Closed);
    });
}

fn append_stderr(tail: &StderrTail, bytes: &[u8]) {
    let bytes = &bytes[bytes.len().saturating_sub(STDERR_BYTES)..];
    let mut tail = tail.lock().unwrap_or_else(|e| e.into_inner());
    let discard = (tail.len() + bytes.len()).saturating_sub(STDERR_BYTES);
    tail.drain(..discard);
    tail.extend_from_slice(bytes);
}

/// Owns teardown even when a cold start fails or loses an admission race.
pub(super) struct Process {
    permits: Option<(
        tokio::sync::OwnedSemaphorePermit,
        tokio::sync::OwnedSemaphorePermit,
        Arc<StartKey>,
    )>,
    pub(super) sandbox: Arc<dyn calternal_machine::sandbox::Sandbox>,
    pub(super) runtime: tokio::runtime::Handle,
}
impl Drop for Process {
    fn drop(&mut self) {
        tracing::debug!("worker teardown queued");
        let sandbox = self.sandbox.clone();
        let permits = self.permits.take();
        self.runtime.spawn(async move {
            while let Err(error) = sandbox.stop().await {
                tracing::warn!(%error, "worker teardown failed; retrying with capacity reserved");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            tracing::debug!("worker teardown confirmed");
            drop(permits);
        });
    }
}
