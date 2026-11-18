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
mod sharing;
pub use sharing::*;
mod spaces;
pub use spaces::*;
mod appearance;
pub use appearance::*;
mod onboarding;
pub use onboarding::*;
mod wallpapers;
pub use wallpapers::*;
mod secrets;
pub use secrets::*;
mod validation;
pub use calternal_types::ErrorCode;
pub use models::*;
pub use validation::*;

mod plugin_diagnostics;
mod plugin_manifest;
mod plugin_settings;
pub use plugin_diagnostics::*;
pub use plugin_manifest::*;
pub use plugin_settings::*;

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

mod provider_sign_in;
pub use provider_sign_in::*;
mod requests;
pub use requests::*;

/// Trusted HTTP account deletion route, using current installation credentials.
pub const ACCOUNT_ERASURE_PATH: &str = "/api/v1/auth/account/delete";

/// CLI and HTTP action identity for permanent account erasure.
pub const ACCOUNT_ERASURE_ACTION: &str = "account.delete";

/// IANA time-zone validation and civil offsets for account settings and clients.
pub mod time_zone;
