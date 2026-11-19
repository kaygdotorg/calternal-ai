#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
//! Data-only plugin registry and bounded worker-v2 session engine.
//!
//! This library never starts a process or grants authority. An execution adapter must
//! isolate each (plugin, space) worker, deny ambient filesystem/network/credentials,
//! enforce CPU/memory/deadline limits and kill unresponsive cancelled workers.
//! Host brokers must recheck current authority at every effect and delivery boundary.
mod bounded;
mod claims;
mod input_diagnostics;
mod registry;
mod registry_v2;
mod runtime;
mod slots;
/// Private wire protocol for an enrolled space runtime.
pub mod space_channel;
mod strict_json;
pub use claims::DeclarationIndex;
pub use registry::*;
pub use registry_v2::ManifestV2;
pub use runtime::*;

/// Safe boundary failure; never includes plugin input or secret values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Manifest or wire shape/value is invalid.
    #[error("invalid contract")]
    Invalid,
    /// A declaration collides with an existing registration.
    #[error("declaration collision")]
    Collision,
    /// A queue is full. Admission/receive can retry; cancellation has already invalidated authority.
    #[error("back pressure")]
    BackPressure,
    /// The bounded frame size was exceeded.
    #[error("frame too large")]
    TooLarge,
    /// An unknown, cancelled or mismatched invocation was referenced.
    #[error("unknown invocation")]
    Invocation,
    /// A worker requested an undeclared or unauthorized broker call.
    #[error("host call refused")]
    Refused,
}
