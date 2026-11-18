#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
//! Pure v1 preview contract. No networking, async runtime or platform effects.
mod jobs;
pub use jobs::*;
mod models;
mod note_operations;
mod palette;
mod search;
pub use palette::*;
pub use search::*;
mod notes;
pub use note_operations::*;
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

mod provider;
pub use provider::*;

mod agent;
pub use agent::*;
mod readiness;
pub use readiness::*;

mod live;
pub use live::*;

mod account_input;
pub use account_input::*;

mod people;
pub use people::*;

mod requests;
pub use requests::*;

/// Trusted HTTP account deletion route, using current installation credentials.
pub const ACCOUNT_ERASURE_PATH: &str = "/api/v1/auth/account/delete";

/// CLI and HTTP action identity for permanent account erasure.
pub const ACCOUNT_ERASURE_ACTION: &str = "account.delete";
