//! Wire protocol between the server role and the machine role, version 4.
//!
//! The machine always connects out; the server never opens a connection to a
//! machine. Everything is plain JSON over HTTP(S) requests the machine makes:
//!
//! | Request (machine → server) | Purpose |
//! | --- | --- |
//! | `POST /api/v1/machine/enroll` | Exchange a one-time code for a machine identity ([`EnrolRequest`]). |
//! | `POST /api/v1/machine/session/start` | Ask for a challenge ([`SessionStart`] → [`SessionChallenge`]). |
//! | `POST /api/v1/machine/session/finish` | Sign it with the machine key ([`SessionFinish`] → [`SessionGranted`]). |
//! | `POST /api/v1/machine/sync` | Long-poll: report state, receive work ([`SyncRequest`] → [`SyncResponse`]). |
//! | `POST /api/v1/machine/output` | Upload exec output ([`OutputPost`] → [`OutputAck`]). |
//!
//! Why long-poll rather than a websocket or SSE: it needs no new dependency on
//! either side, passes any HTTP proxy that already fronts the server, reuses the
//! server's existing request guard, and a cancelled request loses nothing because
//! both directions carry sequence numbers and acknowledgements. The cost is one
//! held request per machine (one socket, about 10 KiB of server memory) and up to
//! one extra round trip of latency when output and work cross.
//!
//! Failures use the operation error envelope (`{"ok":false,"error":{code,message}}`).
//! The machine branches only on the HTTP status: 401 means the session is unknown
//! (start a new one), 403 means the machine is revoked (stop everything and exit),
//! 409 means a lease is fenced (stop that sandbox), 404 an unknown code or lease.

use calternal_types::{AccountId, SpaceId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Protocol version spoken by this build; the server refuses other versions.
pub const PROTOCOL_VERSION: u32 = 5;

/// Path prefix of every machine-facing endpoint. The server's account guard
/// skips it: these endpoints authenticate the machine themselves.
pub const PATH_PREFIX: &str = "/api/v1/machine/";

/// Domain separation prefix of the signed session challenge.
pub const SESSION_CONTEXT: &[u8] = b"calternal-machine-session-v3\n";

/// Bytes a machine signs to open a session: context, machine id, challenge.
/// Binding the machine id stops a signature for one machine opening another's session.
pub fn session_message(machine_id: Uuid, challenge: &str) -> Vec<u8> {
    let mut m = Vec::with_capacity(SESSION_CONTEXT.len() + 37 + challenge.len());
    m.extend_from_slice(SESSION_CONTEXT);
    m.extend_from_slice(machine_id.to_string().as_bytes());
    m.push(b'\n');
    m.extend_from_slice(challenge.as_bytes());
    m
}

/// Who a machine belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Owner {
    /// The user's own machine; only that account's work is placed on it.
    Account {
        /// The owning account.
        id: AccountId,
    },
    /// An operator-enrolled machine in the shared pool.
    Pool,
}

/// First request of a new machine: a one-time code plus its public key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrolRequest {
    /// The one-time enrolment code exactly as issued (dashes and case ignored).
    pub code: String,
    /// Raw 32-byte Ed25519 public key, base64 (standard alphabet, padded).
    pub public_key: String,
    /// Human label, for example the host name; at most 64 characters.
    pub name: String,
    /// `std::env::consts::OS` of the machine.
    pub os: String,
    /// `std::env::consts::ARCH` of the machine.
    pub arch: String,
    /// Owner chose direct execution at enrollment; refused for pool machines.
    pub direct: bool,
}

/// Answer to a successful enrolment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrolResponse {
    /// The new machine identity; the private key never leaves the machine.
    pub machine_id: Uuid,
    /// Whose machine this is.
    pub owner: Owner,
}

/// Ask for a session challenge.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionStart {
    /// The enrolled machine.
    pub machine_id: Uuid,
}

/// A single-use challenge that expires after 60 seconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionChallenge {
    /// Random challenge to sign with [`session_message`].
    pub challenge: String,
}

/// Session preference emitted when a Linux microVM role falls back to Podman
/// because the machine user cannot create KVM guests.
pub const PODMAN_KVM_FALLBACK: &str = "podman-no-kvm";

/// Approved account guest sizing and its measured admission ceiling.
#[derive(Clone, Copy, Debug)]
pub struct GuestSize {
    /// Configured RAM, in MiB.
    pub memory_mib: u32,
    /// Concurrent thread ceiling where a measurement has established one.
    pub active_threads: Option<usize>,
}

impl GuestSize {
    /// Resolve a configured RAM value against the supported size profiles.
    pub fn from_mib(memory_mib: u32) -> Option<Self> {
        const PROFILES: [GuestSize; 2] = [
            GuestSize {
                memory_mib: 512,
                active_threads: Some(2),
            },
            GuestSize {
                memory_mib: 1024,
                // Three threads were at the latency cutoff on the busy dev host.
                active_threads: Some(2),
            },
        ];
        PROFILES
            .into_iter()
            .find(|size| size.memory_mib == memory_mib)
    }
}

/// Proof of the machine key, opening a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionFinish {
    /// The enrolled machine.
    pub machine_id: Uuid,
    /// The challenge from [`SessionChallenge`].
    pub challenge: String,
    /// Ed25519 signature of [`session_message`], base64.
    pub signature: String,
    /// Random id chosen when the machine role process starts. A new value tells
    /// the server that every exec that was running before is gone.
    pub boot_id: Uuid,
    /// Must equal [`PROTOCOL_VERSION`].
    pub protocol: u32,
    /// Requested backend, or auto for the owner's setting; direct requires owner consent.
    pub backend: String,
    /// Configured size of an account guest on a microVM session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guest_memory_mib: Option<u32>,
}

/// An open session. Opening another session for the same machine ends this one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionGranted {
    /// Actual backend authorized for this session: podman, direct or microvm.
    pub backend: String,
    /// Bearer token for `sync` and `output`; held in memory only, on both sides.
    pub token: String,
    /// How long the server keeps leases without a sync. After local expiry the
    /// machine removes disposable containers and retains mounted files.
    pub lease_ttl_ms: u64,
    /// The longest the server holds a `sync` request open.
    pub poll_hold_ms: u64,
}

/// Machine capacity. Placement accounts for both held machine slots and the
/// server's active leases. CPU and memory figures are informational.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capacity {
    /// Logical CPUs.
    pub cpus: u32,
    /// Physical memory in bytes; 0 when unknown.
    pub memory_bytes: u64,
    /// Most provider and worker slots the machine can hold at once.
    pub max_sandboxes: u32,
    /// Slots currently held, including idle leases and pending removals.
    pub running_sandboxes: u32,
}

/// State of one lease's sandbox as the machine sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SandboxState {
    /// Being created, or pending a new exec after idle removal.
    Starting,
    /// Ready for execs.
    Running,
    /// A paused sandbox.
    Frozen,
    /// A running guest exited. Provider leases may resume from their last checkpoint.
    Lost,
    /// Could not start, or was closed locally; the server reconciles the lease.
    Failed {
        /// Short, path-free description.
        reason: String,
    },
}

/// One lease the machine holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseReport {
    /// The lease.
    pub lease_id: Uuid,
    /// The fence the machine was given with it.
    pub fence: u64,
    /// Its sandbox.
    pub sandbox: SandboxState,
}

/// Long-poll request: acknowledge received messages and report state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncRequest {
    /// Highest [`Envelope::seq`] received in this session (0 before any).
    pub ack: u64,
    /// Completed explicit workspace deletions, retransmitted until a successful sync.
    /// At most [`MAX_DISCARD_REPORTS`]. An absent batch means no completed deletions.
    #[serde(default)]
    pub discards: Vec<DiscardReport>,
    /// Current capacity.
    pub capacity: Capacity,
    /// True once the machine is shutting down: place nothing new on it.
    pub draining: bool,
    /// Every lease the machine holds.
    pub leases: Vec<LeaseReport>,
}

/// Maximum retained owner erasures delivered alongside regular discards per sync.
pub const OWNER_ERASURE_BATCH: usize = 16;
/// Maximum regular workspace discards and owner erasure results in one sync.
pub const MAX_DISCARD_REPORTS: usize = 128 + OWNER_ERASURE_BATCH;

/// A signed, read-only status query for the enrolled machine.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErasureStatusRequest {
    /// Enrolled machine UUID.
    pub machine_id: Uuid,
    /// Unused machine session challenge.
    pub challenge: String,
    /// Base64 Ed25519 signature for the status challenge domain.
    pub signature: String,
}

/// Number of durable owner erasures still awaiting this machine.
#[derive(Debug, Serialize, Deserialize)]
pub struct ErasureStatus {
    /// Pending erasure instructions, which survive disconnection.
    pub pending_erasures: u64,
}

/// Sign a status challenge in a domain distinct from session establishment.
pub fn erasure_status_message(machine_id: Uuid, challenge: &str) -> Vec<u8> {
    format!("calternal-machine-erasure-status-v1\n{machine_id}\n{challenge}").into_bytes()
}

/// A machine's result for a durable workspace discard or owner erasure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscardReport {
    /// Unique deletion attempt UUID; its session binds the target machine.
    pub request_id: Uuid,
    /// True after the requested workspace or account tree was removed.
    pub success: bool,
}

/// Messages for the machine, in order. Empty after the hold time elapsed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncResponse {
    /// Unacknowledged messages; the same message may arrive twice after a
    /// cancelled request, so the machine applies each `seq` at most once.
    pub messages: Vec<Envelope>,
}

/// A numbered message. Numbers restart at 1 in every session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    /// Position in this session's stream.
    pub seq: u64,
    /// The message.
    pub message: Down,
}

/// Resource limits for one sandbox. `None` means the backend's default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    /// Memory ceiling in bytes.
    pub memory_bytes: Option<u64>,
    /// CPU ceiling in thousandths of a CPU.
    pub cpu_millis: Option<u32>,
    /// Process count ceiling.
    pub pids: Option<u32>,
}

/// What to start for a thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxSpec {
    /// Local development fixture code directory, mounted read-only by the same
    /// backend. Never serialized: a remote assignment can carry source only.
    #[serde(skip)]
    pub code_directory: Option<std::path::PathBuf>,
    /// Space from the enclosing assignment, or supplied by a local backend caller.
    #[serde(skip)]
    pub space_id: Option<SpaceId>,
    /// Start a disposable login home with write access to shared provider login state.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub provider_login: bool,
    /// Account owning the persistent shared login; validated UUID from the server.
    pub account_id: Uuid,
    /// The thread; one sandbox per thread, reused across leases.
    pub thread_id: Uuid,
    /// Base image by reference; `None` uses the machine's configured default.
    pub image: Option<String>,
    /// Verified worker source and its space; absent for agent sandboxes.
    pub worker: Option<PluginWorker>,
    /// Resource limits.
    pub limits: Limits,
}

/// A single verified, self-contained JavaScript bundle, never a host path.
/// Limited to 1 MiB; transferred over the authenticated machine connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginWorker {
    /// Space whose broker owns this worker; never inferred from a file path.
    pub space_id: SpaceId,
    /// UTF-8 JavaScript source, already checked against the registry digest.
    pub source: String,
}

/// A message from the server to a machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Down {
    /// Explicit owner discard; never inferred from lease expiry or elapsed time.
    Discard {
        /// Durable request identity, scoped to this enrolled machine.
        request_id: Uuid,
        /// Account whose workspace is being discarded.
        account_id: Uuid,
        /// Thread workspace to discard; its persistent account home is retained.
        thread_id: Uuid,
    },
    /// Remove a deleted owner's machine-role data and acknowledge durable completion.
    Erase {
        /// Durable request identity, scoped to this enrolled machine.
        request_id: Uuid,
        /// Account directory that contains the target.
        account_id: Uuid,
        /// A workspace within the account. Absent means the entire account tree.
        thread_id: Option<Uuid>,
    },
    /// Run work for (space, account, thread) in a site-local sandbox.
    Assign {
        /// The lease.
        lease_id: Uuid,
        /// Fencing token; output with an older fence is refused.
        fence: u64,
        /// Space the work belongs to.
        space_id: SpaceId,
        /// Account the work runs for.
        account_id: AccountId,
        /// Sandbox to start.
        spec: SandboxSpec,
    },
    /// Run a command in the lease's sandbox and stream its output.
    Exec {
        /// Keep stdin open for bounded, ordered Input messages.
        interactive: bool,
        /// The lease.
        lease_id: Uuid,
        /// Its current fence.
        fence: u64,
        /// Identifies the output stream.
        exec_id: Uuid,
        /// Program and arguments; no shell is implied.
        argv: Vec<String>,
        /// Bounded UTF-8 stdin, delivered once then closed; at most 128 KiB; empty for interactive execs.
        stdin: String,
        /// Extra environment; the sandbox starts from an empty environment.
        /// Never a credential: secret values reach a sandbox only as
        /// `calternal_secrets::Consumer::SandboxEnv { thread, variable }`,
        /// released by `SecretStore::use_value` when the server builds this
        /// message (see `MachineService::exec` for the seam).
        env: Vec<(String, String)>,
    },
    /// One bounded write to an interactive exec; envelope replay protection applies.
    Input {
        /// Lease owning the exec.
        lease_id: Uuid,
        /// Current lease fence.
        fence: u64,
        /// Interactive exec identity.
        exec_id: Uuid,
        /// At most 256 KiB plus a newline, never placed on a command line.
        data: Vec<u8>,
    },
    /// Stop and remove the lease's sandbox (the lease ended or was fenced).
    Stop {
        /// The lease.
        lease_id: Uuid,
        /// Short reason for logs.
        reason: String,
        /// Provider turns retain workspace files and the account login home.
        keep_files: bool,
    },
}

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

/// Format of the private enrollment handoff and confirmation.
pub const LOCAL_ENROLLMENT_FORMAT: &str = "calternal-machine-enrollment/v1";

/// File written by the co-located machine before the first owner registers.
/// The server receives only public material; the signing key stays on the host.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalEnrollment {
    /// Durable handoff format tag.
    pub format: String,
    /// Machine-generated UUID, persisted with its signing key before publication.
    pub machine_id: Uuid,
    /// Base64 Ed25519 public key.
    pub public_key: String,
    /// Display label.
    pub name: String,
    /// Operating system.
    pub os: String,
    /// CPU architecture.
    pub arch: String,
}
