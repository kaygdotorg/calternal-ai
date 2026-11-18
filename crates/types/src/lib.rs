//! Types every Calternal-AI crate shares: typed identifiers, the acting
//! [`Actor`], the caller-safe error, the closed host capability vocabulary and the
//! millisecond clock and shared worker-frame bound.
//!
//! The crate does no file or database I/O and depends only on `serde`,
//! `serde_json` and `uuid` (plus `schemars`/`ts-rs` behind the `contract`
//! feature for the wire contract exporter and `rustix` behind `perf-clock`).
//! Crates that need an identifier or
//! a caller-facing error import it from here instead of defining their own.
#![deny(missing_docs, rustdoc::broken_intra_doc_links)]

mod actor;
mod capability;
mod error;
mod id;
mod owner;
mod owner_lifecycle;
mod page;
mod space_paths;

pub use actor::{Actor, Via};
pub use capability::{Capability, CapabilitySet};
pub use error::{Error, ErrorCode, Result};
pub use id::{AccountId, GroupId, InstallationId, InviteId, SpaceId};
pub use owner::Owner;
pub use owner_lifecycle::{OwnerLifecycle, OwnerScoped, OwnerStep};
pub use page::Page;
pub use space_paths::{SPACE_INSTRUCTIONS_PATH, is_reserved_space_path};

/// Maximum JSON bytes in a plugin worker frame, excluding its newline (256 KiB).
/// Host validation and machine stdin admission share this bound; no I/O or allocation.
pub const MAX_FRAME_BYTES: usize = 256 * 1024;

/// Milliseconds since the Unix epoch, or 0 if the clock reads before 1970.
///
/// Cost: one system clock read; no allocation, lock or I/O.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// Nanoseconds on the host monotonic clock, shared by processes in the same
/// time namespace. Use differences within one boot for performance measurements.
/// Reads the clock once without allocating or taking a lock.
#[cfg(feature = "perf-clock")]
pub fn perf_monotonic_ns() -> u64 {
    let time = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    time.tv_sec as u64 * 1_000_000_000 + time.tv_nsec as u64
}
