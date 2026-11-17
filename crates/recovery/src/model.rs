use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// A refused recovery proof or a private storage failure. Never expose diagnostics to tenants.
#[derive(Debug, thiserror::Error)]
#[error("protected recovery: {0}")]
pub struct Error(String);
/// A validated result; failures preserve all evidence for explicit operator recovery.
pub type Result<T> = std::result::Result<T, Error>;
/// Converts a host-private diagnostic without exposing it through tenant APIs.
pub(crate) fn err(e: impl std::fmt::Display) -> Error {
    Error(e.to_string())
}
/// Refuses an incomplete or inconsistent proof without attempting fallback.
pub(crate) fn invalid() -> Error {
    err("incomplete, corrupt or mismatched recovery proof")
}

/// Crash boundaries exposed for deterministic host tests. Errors simulate loss of
/// execution; production uses an observer that always succeeds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    /// Before writing and syncing the proposed-state intent.
    BeforeIntent,
    /// After the atomic intent file and directory have been synced.
    AfterIntent,
    /// Last boundary before the caller commits the SQLite transaction.
    BeforeCommit,
    /// First boundary after the transaction committed.
    AfterCommit,
    /// Before materializing an account or space record.
    BeforeRecord,
    /// After atomic record placement, including fsio's directory flush.
    AfterRecord,
    /// Before the atomic fsio writer, which includes its placement directory barrier.
    BeforeDirectorySync,
    /// After the atomic writer has durably synced the placement directory.
    AfterDirectorySync,
    /// Before the completion marker is durably placed.
    BeforeCompletion,
    /// After durable completion; an injected failure here means an unacknowledged commit.
    AfterCompletion,
    /// Before replay resolves an unfinished transaction.
    Replay,
    /// Before making a durable head point to an unresolved intent.
    BeforePendingHead,
    /// After the pending head is durable, before SQLite may commit.
    AfterPendingHead,
    /// Before publishing an immutable authenticated index node.
    BeforeIndex,
    /// After an index node and its directory are durable.
    AfterIndex,
    /// Before unlinking a retired immutable file.
    BeforeCompact,
    /// After unlinking one retired file; deletion may still need directory sync.
    AfterCompact,
    /// Before syncing directories containing retired files.
    BeforeCompactSync,
    /// After reclamation directory syncs are durable.
    AfterCompactSync,
    /// One file fsync completed through the atomic fsio writer.
    FileSynced,
    /// One directory fsync completed through fsio.
    DirectorySynced,
}

/// Serializable SQLite cell preserving blobs, integers, nulls and real bit patterns.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Cell {
    /// SQL NULL.
    Null,
    /// Signed SQLite integer.
    Integer(i64),
    /// IEEE-754 bits, preserving every representable database value.
    Real(u64),
    /// UTF-8 SQL text.
    Text(String),
    /// Opaque SQL blob; always in account-private metadata.
    Blob(Vec<u8>),
}
/// Complete logical table image for a later owner-controlled rebuild, never SQL to execute blindly.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Table {
    /// Original CREATE TABLE declaration; rebuild must validate against its own schema.
    pub schema: String,
    /// Ordered column names corresponding to every row.
    pub columns: Vec<String>,
    /// Current protected rows, including credential/session revocations.
    pub rows: Vec<Vec<Cell>>,
}
/// Fully validated installation-bound snapshot. It contains protected credentials;
/// use only within the host security owner, never a space operation or export API.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    /// Format version; unsupported versions are rejected.
    pub version: u32,
    /// UUID pinned externally by the host, distinct from account installations.
    pub installation: Uuid,
    /// Unique immutable transaction identity.
    pub transaction: Uuid,
    /// Monotonic local publication order, independent of wall-clock time.
    pub sequence: u64,
    /// Current protected account/space/policy tables reconstructed from the index;
    /// sharing delivery/reset bookkeeping is absent.
    pub tables: BTreeMap<String, Table>,
}
/// Checksummed canonical payload envelope; not an administrator-authentication MAC.
#[derive(Serialize, Deserialize)]
pub(crate) struct Envelope {
    /// Exact serialized payload bytes bound by the checksum.
    pub(crate) bytes: Vec<u8>,
    checksum: String,
}
/// Serializes a deterministic value with an integrity-checking BLAKE3 envelope.
pub(crate) fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value).map_err(err)?;
    serde_json::to_vec(&Envelope {
        checksum: blake3::hash(&bytes).to_hex().to_string(),
        bytes,
    })
    .map_err(err)
}
/// Verifies exact payload bytes before deserializing a protected value.
pub(crate) fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(err)?;
    if envelope.checksum != blake3::hash(&envelope.bytes).to_hex().as_str() {
        return Err(invalid());
    }
    serde_json::from_slice(&envelope.bytes).map_err(err)
}
