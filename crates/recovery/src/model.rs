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
    /// Before appending a proposed delta.
    BeforeIntent,
    /// After the proposed delta's file fsync.
    AfterIntent,
    /// Before SQLite may commit its witness and authority.
    BeforeCommit,
    /// After SQLite commit, before marker publication.
    AfterCommit,
    /// Before appending a resolution marker.
    BeforeCompletion,
    /// After the marker's file fsync, the acknowledgement barrier.
    AfterCompletion,
    /// Before resolving a proposal from committed database visibility.
    Replay,
    /// Before writing bytes to the current segment.
    BeforeAppend,
    /// After writing a frame length prefix; permits torn-tail tests.
    AfterPrefix,
    /// After writing frame bytes, before their fsync.
    AfterAppend,
    /// Before repairing a database-proven uncommitted torn tail.
    BeforeTailRepair,
    /// After the repaired tail is synced.
    AfterTailRepair,
    /// Before creating the next empty segment.
    BeforeSegment,
    /// After the segment and its directory entry are durable.
    AfterSegment,
    /// Before replacing the full snapshot.
    BeforeSnapshot,
    /// After the snapshot and its placement directory are durable.
    AfterSnapshot,
    /// Before unlinking an obsolete segment.
    BeforeCompact,
    /// After unlinking an obsolete segment.
    AfterCompact,
    /// Before a segment creation/removal directory flush.
    BeforeCompactSync,
    /// After a segment creation/removal directory flush.
    AfterCompactSync,
    /// One completed file fsync, including snapshot/segment creation.
    FileSynced,
    /// One completed directory fsync.
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
    /// Logical table image version, independent of the on-disk log version.
    pub version: u32,
    /// UUID pinned externally by the host, distinct from account installations.
    pub installation: Uuid,
    /// Unique immutable transaction identity.
    pub transaction: Uuid,
    /// Monotonic local publication order, independent of wall-clock time.
    pub sequence: u64,
    /// Current protected account/space/policy tables reconstructed from snapshot and log;
    /// sharing delivery/reset bookkeeping is absent.
    pub tables: BTreeMap<String, Table>,
}
/// Serializes canonical JSON preceded by its fixed-width BLAKE3 checksum.
pub(crate) fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value).map_err(err)?;
    let mut out = blake3::hash(&bytes).as_bytes().to_vec();
    out.extend(bytes);
    Ok(out)
}
/// Checks exact payload bytes before decoding; a checksum is not an operator MAC.
pub(crate) fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    if bytes.len() < 32 || bytes[..32] != *blake3::hash(&bytes[32..]).as_bytes() {
        return Err(invalid());
    }
    serde_json::from_slice(&bytes[32..]).map_err(err)
}
