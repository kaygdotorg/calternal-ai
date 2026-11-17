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
//! - [`process::ProcessBackend`]: plain child processes in a per-thread directory.
//!   For tests and development only: it isolates environment, working directory and
//!   process groups, not the file system, network or resources.
//!
//! A future microVM backend (a VM per thread, containers inside) fits the same
//! trait: `start` boots or adopts the VM, `exec` runs through its agent, `freeze`
//! and `resume` map to the VM pause API, `release` leaves the VM running.
//!
//! Output back-pressure: each exec has a bounded channel of [`OUTPUT_CHANNEL`]
//! chunks of at most [`CHUNK_BYTES`]. When the consumer stops reading, the reader
//! task stops, the pipe fills and the process blocks on its next write.

pub mod podman;
pub mod process;

use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;

use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::protocol::{ExitInfo, SandboxSpec, Stream};

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
#[derive(Debug, Clone, Default)]
pub struct ExecRequest {
    /// Program and arguments; no shell is implied.
    pub argv: Vec<String>,
    /// Environment on top of the sandbox's minimal one.
    pub env: Vec<(String, String)>,
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
    /// Bounded event stream; dropping it lets the process block on output.
    pub events: mpsc::Receiver<OutputEvent>,
}

/// Starts per-thread sandboxes. Implementations must be cheap to share.
pub trait Backend: Send + Sync + 'static {
    /// Short name for logs and capacity reports.
    fn name(&self) -> &'static str;
    /// Start the thread's sandbox, or adopt the one left by an earlier run of the
    /// machine role (same thread id), resuming it if it was frozen.
    fn start<'a>(&'a self, spec: &'a SandboxSpec) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>>;
}

/// One thread's sandbox.
pub trait Sandbox: Send + Sync {
    /// The thread it belongs to.
    fn thread_id(&self) -> Uuid;
    /// Run a command in the work directory and stream its output.
    fn exec(&self, request: ExecRequest) -> BoxFuture<'_, SandboxResult<Exec>>;
    /// Pause every process; memory stays resident.
    fn freeze(&self) -> BoxFuture<'_, SandboxResult<()>>;
    /// Continue after [`Sandbox::freeze`].
    fn resume(&self) -> BoxFuture<'_, SandboxResult<()>>;
    /// Kill everything and delete the sandbox, its writable layer and work directory.
    fn stop(&self) -> BoxFuture<'_, SandboxResult<()>>;
    /// The machine role is exiting: end running execs (their output pipes die with
    /// it) but keep the sandbox and its files so a restarted machine role adopts it.
    fn release(&self) -> BoxFuture<'_, SandboxResult<()>>;
}

/// Spawn `cmd` with piped output and stream it as [`OutputEvent`]s.
///
/// Returns the exec and the child's process id. Costs two reader tasks and one
/// waiter task per exec; reads are at most [`CHUNK_BYTES`].
pub(crate) fn spawn_streaming(
    mut cmd: tokio::process::Command,
) -> SandboxResult<(Exec, Option<u32>)> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| SandboxError(format!("cannot start command: {}", e.kind())))?;
    let pid = child.id();
    let (tx, rx) = mpsc::channel(OUTPUT_CHANNEL);
    let out = child.stdout.take().map(|p| tokio::spawn(pump(p, Stream::Stdout, tx.clone())));
    let err = child.stderr.take().map(|p| tokio::spawn(pump(p, Stream::Stderr, tx.clone())));
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
        let _ = tx.send(OutputEvent::Exit(info)).await;
    });
    Ok((Exec { events: rx }, pid))
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
                if tx.send(OutputEvent::Data(stream, buf[..n].to_vec())).await.is_err() {
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
    Exec { events: rx }
}

/// Collect an exec's output into (stdout, stderr, exit). Test and CLI helper.
pub async fn collect(mut exec: Exec) -> (Vec<u8>, Vec<u8>, ExitInfo) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    while let Some(ev) = exec.events.recv().await {
        match ev {
            OutputEvent::Data(Stream::Stdout, b) => out.extend(b),
            OutputEvent::Data(Stream::Stderr, b) => err.extend(b),
            OutputEvent::Exit(info) => return (out, err, info),
        }
    }
    (out, err, ExitInfo { error: Some("output ended without exit".into()), ..ExitInfo::default() })
}
