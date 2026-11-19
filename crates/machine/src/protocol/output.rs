//! Numbered exec output and backpressure messages for the enrolled connection.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Which pipe a chunk came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// Machine-generated exit status for a disappearing guest. The registry must
/// fence the lease and preserve its checkpoint before returning the exit.
pub const GUEST_LOST: &str = "the sandbox guest stopped";

/// How an exec ended.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExitInfo {
    /// Exit code, when the process exited normally.
    pub code: Option<i32>,
    /// Terminating signal, when it was killed.
    pub signal: Option<i32>,
    /// Why it could not run or was lost; path-free.
    pub error: Option<String>,
}

/// One numbered output event. Sequence numbers start at 1 per exec and the
/// exit event is always last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    /// Position in this exec's stream.
    pub seq: u64,
    /// The event.
    #[serde(flatten)]
    pub event: ChunkEvent,
}

/// Content of a [`Chunk`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChunkEvent {
    /// Bytes from one pipe, base64 (a third larger on the wire than raw).
    Data {
        /// Source pipe.
        stream: Stream,
        /// Base64 bytes.
        data: String,
    },
    /// The process ended.
    Exit(ExitInfo),
}

/// Upload of exec output. The server accepts a prefix that fits its buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputPost {
    /// The lease.
    pub lease_id: Uuid,
    /// The fence the machine holds; a stale fence gets 409.
    pub fence: u64,
    /// The exec.
    pub exec_id: Uuid,
    /// Consecutive chunks, oldest first.
    pub chunks: Vec<Chunk>,
}

/// Back-pressure answer to an [`OutputPost`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OutputAck {
    /// Highest sequence number the server holds; resend everything after it.
    pub acked: u64,
    /// Set when the server's buffer is full: wait this long before resending.
    pub retry_after_ms: Option<u64>,
}
