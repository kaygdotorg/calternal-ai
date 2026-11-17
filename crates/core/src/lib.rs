#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
//! Pure v1 preview contract. No networking, async runtime or platform effects.
mod jobs;
pub use jobs::*;
mod models;
mod notes;
pub use notes::*;
mod plugin_wire;
pub use plugin_wire::*;
mod spaces;
pub use spaces::*;
mod secrets;
pub use secrets::*;
mod validation;
pub use calternal_types::ErrorCode;
pub use models::*;
pub use validation::*;

mod plugin_manifest;
pub use plugin_manifest::*;
