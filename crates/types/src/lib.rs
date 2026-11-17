//! Types every Calternal-AI crate shares: typed identifiers, the caller-safe
//! error, the closed space capability vocabulary and the millisecond clock.
//!
//! The crate does no I/O, holds no locks and depends only on `serde`,
//! `serde_json` and `uuid` (plus `schemars`/`ts-rs` behind the `contract`
//! feature for the wire contract exporter). Crates that need an identifier or
//! a caller-facing error import it from here instead of defining their own.
#![deny(missing_docs, rustdoc::broken_intra_doc_links)]

mod capability;
mod error;
mod id;

pub use capability::{Capability, CapabilitySet};
pub use error::{Error, ErrorCode, Result};
pub use id::{AccountId, InstallationId, SpaceId};

/// Milliseconds since the Unix epoch, or 0 if the clock reads before 1970.
///
/// Cost: one system clock read; no allocation, lock or I/O.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}
