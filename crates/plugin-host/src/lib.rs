#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
//! Plugin registry and worker transports. With the `operations` feature, the
//! primitives-only transport starts a process through a trusted launcher.
//! The launcher enforces the process boundary and the call route checks
//! current authority at each effect and delivery boundary.
mod bounded;
mod claims;
mod input_diagnostics;
/// Process transport for the primitives-only plugin API.
#[cfg(feature = "operations")]
pub mod operations;
mod registry;
mod registry_v2;
mod runtime;
mod slots;
/// Private wire protocol for an enrolled space runtime.
pub mod space_channel;
mod strict_json;
pub use claims::{DeclarationIndex, PackageClaims};
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
