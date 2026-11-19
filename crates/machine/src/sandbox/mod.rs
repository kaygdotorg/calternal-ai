//! Per-thread sandboxes behind one interface.
//!
//! A [`Backend`] starts (or adopts after a restart) the sandbox of one thread; a
//! [`Sandbox`] runs commands in it and streams their output, can be frozen and
//! resumed, and is stopped when its thread's lease ends.
//!
//! Backends:
//! - [`podman::PodmanBackend`]: one rootless Podman container per thread, a base
//!   image by reference, the container's own writable layer and a work directory
//!   at `/work`. Measured fresh start about 0.63 s p50 alone and 3 s p50 with ten
//!   in parallel (`docs/measurements/sandbox-containers-vs-microvms.md`).
//! - [`direct::DirectBackend`]: plain child processes in a per-thread directory.
//!   Direct processes as the machine user; owner consent is required.
//!   It provides no filesystem, network or resource isolation.
//!
//! The Linux microVM backend runs thread containers inside an account guest.
//!
//! Output back-pressure: each exec has a bounded channel of [`OUTPUT_CHANNEL`]
//! chunks of at most [`CHUNK_BYTES`]. When the consumer stops reading, the reader
//! task stops, the pipe fills and the process blocks on its next write.

pub mod configured;
pub mod direct;
#[cfg(target_os = "linux")]
pub mod microvm;
pub mod podman;
mod provider_home;
mod shared_broker;
pub mod worker;

use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;

use crate::protocol::{ExitInfo, SandboxSpec, Stream};
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

/// A boxed, sendable future; keeps the traits object-safe without extra crates.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Chunks buffered per exec before the process is made to wait (16 × 16 KiB = 256 KiB).
pub const OUTPUT_CHANNEL: usize = 16;
/// Largest single read from a pipe.
pub const CHUNK_BYTES: usize = 16 * 1024;

/// Sandbox failure with a short, path-free reason suitable for the server.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct SandboxError(pub String);

impl SandboxError {
    /// Wrap any displayable failure.
    pub fn new(e: impl std::fmt::Display) -> Self {
        Self(e.to_string())
    }
}

/// Result of sandbox operations.
pub type SandboxResult<T> = std::result::Result<T, SandboxError>;

/// A command to run inside a sandbox.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ExecRequest {
    /// Program and arguments; no shell is implied.
    pub argv: Vec<String>,
    /// Keep stdin open and return its bounded writer for a worker session.
    pub interactive: bool,
    /// Input delivered once and closed; callers enforce the 128 KiB budget.
    pub stdin: String,
    /// Environment on top of the sandbox's minimal one. Names must be POSIX
    /// identifiers; values must travel through child environment, never host argv.
    pub env: Vec<(String, String)>,
}

impl ExecRequest {
    /// Refuse names that could be interpreted as an inline value or option by a
    /// container CLI. Linear in name bytes, no I/O; values are never inspected.
    pub(super) fn validate_env_names(&self) -> SandboxResult<()> {
        if self.env.iter().any(|(name, _)| {
            let mut bytes = name.bytes();
            !bytes
                .next()
                .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
                || !bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
        }) {
            return Err(SandboxError(
                "environment names must be POSIX identifiers".into(),
            ));
        }
        Ok(())
    }
}

/// One output event of a running exec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputEvent {
    /// Bytes from one pipe.
    Data(Stream, Vec<u8>),
    /// The process ended; always the last event.
    Exit(ExitInfo),
}

/// A running exec: read its events until [`OutputEvent::Exit`].
pub struct Exec {
    /// Bounded stdin writer for interactive execs; absent for one-shot commands.
    pub input: Option<mpsc::Sender<Vec<u8>>>,
    /// Bounded event stream; dropping it lets the process block on output.
    pub events: mpsc::Receiver<OutputEvent>,
}

/// Starts per-thread sandboxes. Implementations must be cheap to share.
pub trait Backend: Send + Sync + 'static {
    /// Reconcile machine-owned transient artifacts once after acquiring its
    /// process lock, before accepting assignments. Default backends need no work.
    fn initialize(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async { Ok(()) })
    }
    /// Reap backend-owned runtime processes before dropping machine ownership.
    /// Stateless backends finish when the runner releases their leases.
    fn shutdown(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async { Ok(()) })
    }
    /// Short name for logs and capacity reports.
    fn name(&self) -> &'static str;
    /// Configured account VM memory for admission. Container and direct backends
    /// have no account VM size.
    fn guest_memory_mib(&self) -> Option<u32> {
        None
    }
    /// Backend request in the machine handshake. `auto` follows the owner's stored setting.
    fn preference(&self) -> &'static str {
        self.name()
    }
    /// Apply the server-authorized selection before accepting assignments.
    /// Fixed backends refuse another selection; the default configured backend switches.
    fn configure(&self, mode: &str) -> SandboxResult<()> {
        if mode == self.name() {
            Ok(())
        } else {
            Err(SandboxError(
                "server backend does not match this machine configuration".into(),
            ))
        }
    }
    /// Discard a released account/thread workspace, preserving its account home.
    /// The caller must exclude active work. Missing artifacts succeed; I/O or
    /// engine failure returns an error. Cost: one recursive removal and optional container removal.
    fn discard(&self, account: uuid::Uuid, thread: uuid::Uuid) -> BoxFuture<'_, SandboxResult<()>>;

    /// Erase one workspace or the account's whole machine-role directory.
    /// Refuse on teardown or deletion failure so the server can retry.
    fn erase(
        &self,
        account: uuid::Uuid,
        thread: Option<uuid::Uuid>,
    ) -> BoxFuture<'_, SandboxResult<()>>;
    /// Start a sandbox using the existing mounted workspace for the thread.
    fn start<'a>(&'a self, spec: &'a SandboxSpec)
    -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>>;

    /// Whether the initialized backend accepts offline plugin VM placements.
    /// Cost: one field check, with no I/O or guest start.
    fn supports_small_plugin_vm(&self) -> bool {
        false
    }

    /// Erase a space's offline VM checkpoint after its placements are stopped.
    /// Refuse if the configured backend cannot establish that deletion succeeded.
    fn erase_space_runtime(
        &self,
        _space: calternal_types::SpaceId,
    ) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            Err(SandboxError(
                "Small plugin VM storage is unavailable for erasure".into(),
            ))
        })
    }

    /// Start an offline plugin-space placement. Called only after the enrolled
    /// carrier opens. Backends without the configured wall refuse before I/O.
    fn start_space<'a>(
        &'a self,
        _spec: &'a SandboxSpec,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        Box::pin(async {
            Err(SandboxError(
                "Small plugin VM is unavailable on this machine".into(),
            ))
        })
    }

    /// Start a harness under the requested provider isolation. Unsupported
    /// backends fail before mounting any profile or starting its adapter.
    fn start_harness<'a>(
        &'a self,
        _spec: &'a SandboxSpec,
        _harness: &'a crate::protocol::HarnessSandbox,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        Box::pin(async {
            Err(SandboxError(
                "backend does not support isolated harnesses".into(),
            ))
        })
    }
}

/// One thread's sandbox.
pub trait Sandbox: Send + Sync {
    /// Whether the backing guest is still alive. Cheap and synchronous for
    /// reporting to the machine registry. Native sandboxes rely on exec exits.
    fn available(&self) -> bool {
        true
    }

    /// Run a command in the work directory and stream its output.
    fn exec(&self, request: ExecRequest) -> BoxFuture<'_, SandboxResult<Exec>>;
    /// Pause every process; memory stays resident.
    fn freeze(&self) -> BoxFuture<'_, SandboxResult<()>>;
    /// Continue after [`Sandbox::freeze`].
    fn resume(&self) -> BoxFuture<'_, SandboxResult<()>>;
    /// Kill everything and delete the sandbox, its writable layer and work directory.
    fn stop(&self) -> BoxFuture<'_, SandboxResult<()>>;
    /// End running execs and remove the runtime sandbox, preserving mounted workspace
    /// files and the account home. Costs one engine removal or process-group signal.
    fn release(&self) -> BoxFuture<'_, SandboxResult<()>>;
}

/// Spawn `cmd` with piped output and stream it as [`OutputEvent`]s.
///
/// Returns the exec and the child's process id. Costs two reader tasks and one
/// waiter task per exec; reads are at most [`CHUNK_BYTES`].
pub(crate) fn spawn_streaming(
    mut cmd: tokio::process::Command,
    input: String,
    interactive: bool,
    worker_output: bool,
) -> SandboxResult<(Exec, Option<u32>)> {
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| SandboxError(format!("cannot start command: {}", e.kind())))?;
    let pid = child.id();
    let (input_tx, mut input_rx) = mpsc::channel::<Vec<u8>>(16);
    if let Some(mut stdin) = child.stdin.take() {
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            if stdin.write_all(input.as_bytes()).await.is_err() {
                return;
            }
            if interactive {
                while let Some(data) = input_rx.recv().await {
                    if data.len() > calternal_types::MAX_FRAME_BYTES + 1
                        || stdin.write_all(&data).await.is_err()
                    {
                        return;
                    }
                }
            }
            let _ = stdin.shutdown().await;
        });
    }
    let (tx, rx) = mpsc::channel(OUTPUT_CHANNEL);
    let out = child
        .stdout
        .take()
        .map(|p| tokio::spawn(pump(p, Stream::Stdout, tx.clone())));
    let stderr = child.stderr.take();
    let (err, worker_tail) = if worker_output {
        (
            None,
            stderr.map(|pipe| tokio::spawn(capture_worker_stderr(pipe))),
        )
    } else {
        (
            stderr.map(|pipe| tokio::spawn(pump(pipe, Stream::Stderr, tx.clone()))),
            None,
        )
    };
    tokio::spawn(async move {
        let status = child.wait().await;
        // Drain both pipes before announcing the exit so it is the last event.
        for task in [out, err].into_iter().flatten() {
            let _ = task.await;
        }
        let info = match status {
            Ok(s) => {
                use std::os::unix::process::ExitStatusExt;
                ExitInfo {
                    code: s.code(),
                    signal: s.signal(),
                    error: None,
                }
            }
            Err(e) => ExitInfo {
                error: Some(format!("wait failed: {}", e.kind())),
                ..ExitInfo::default()
            },
        };
        if let Some(tail) = worker_tail
            && let Ok(bytes) = tail.await
            && info.code != Some(0)
            && !bytes.is_empty()
        {
            let _ = tx.send(OutputEvent::Data(Stream::Stderr, bytes)).await;
        }
        let _ = tx.send(OutputEvent::Exit(info)).await;
    });
    Ok((
        Exec {
            events: rx,
            input: interactive.then_some(input_tx),
        },
        pid,
    ))
}

// Worker stderr has no live fanout. Drain the pipe continuously and disclose
// only its bounded tail on a failed exit, before the terminal event.
async fn capture_worker_stderr(mut pipe: impl tokio::io::AsyncRead + Unpin) -> Vec<u8> {
    let mut tail = Vec::new();
    let mut chunk = vec![0; CHUNK_BYTES.min(worker::STDERR_TAIL_BYTES)];
    while let Ok(n) = pipe.read(&mut chunk).await {
        if n == 0 {
            break;
        }
        let discard = (tail.len() + n).saturating_sub(worker::STDERR_TAIL_BYTES);
        tail.drain(..discard);
        tail.extend_from_slice(&chunk[..n]);
    }
    tail
}

async fn pump(
    mut pipe: impl tokio::io::AsyncRead + Unpin,
    stream: Stream,
    tx: mpsc::Sender<OutputEvent>,
) {
    let mut buf = vec![0u8; CHUNK_BYTES];
    loop {
        match pipe.read(&mut buf).await {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                if tx
                    .send(OutputEvent::Data(stream, buf[..n].to_vec()))
                    .await
                    .is_err()
                {
                    return;
                }
            }
        }
    }
}

/// An exec that could not start, reported as an immediate exit with an error.
pub fn failed_exec(reason: impl Into<String>) -> Exec {
    let (tx, rx) = mpsc::channel(1);
    let _ = tx.try_send(OutputEvent::Exit(ExitInfo {
        error: Some(reason.into()),
        ..ExitInfo::default()
    }));
    Exec {
        events: rx,
        input: None,
    }
}

/// Collect an exec's output into (stdout, stderr, exit), for unit tests.
#[cfg(test)]
pub(crate) async fn collect(mut exec: Exec) -> (Vec<u8>, Vec<u8>, ExitInfo) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    while let Some(ev) = exec.events.recv().await {
        match ev {
            OutputEvent::Data(Stream::Stdout, b) => out.extend(b),
            OutputEvent::Data(Stream::Stderr, b) => err.extend(b),
            OutputEvent::Exit(info) => return (out, err, info),
        }
    }
    (
        out,
        err,
        ExitInfo {
            error: Some("output ended without exit".into()),
            ..ExitInfo::default()
        },
    )
}

/// Immutable machine-role executable metadata. Resolved once on a blocking task;
/// never includes a tenant path or credential. Tests without the real binary omit it.
pub(crate) async fn cli_program() -> Option<std::path::PathBuf> {
    static PROGRAM: tokio::sync::OnceCell<Option<std::path::PathBuf>> =
        tokio::sync::OnceCell::const_new();
    PROGRAM
        .get_or_init(|| async {
            tokio::task::spawn_blocking(|| {
                std::env::current_exe()
                    .ok()
                    .filter(|p| p.file_name().is_some_and(|n| n == "calternal"))
            })
            .await
            .ok()
            .flatten()
        })
        .await
        .clone()
}
