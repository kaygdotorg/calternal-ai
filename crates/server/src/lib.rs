//! Calternal-AI localhost development-preview host.
//!
//! Disposable data only. Not production auth, not a plugin sandbox, not the
//! inherited storage/recovery guarantees. See crates/server/README.md.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

pub mod account_http;
pub mod api;
mod asset_cache;
pub mod auth;
pub mod authz;
mod automations;
pub mod blocking;
pub mod erasure;
pub mod fault;
pub mod machines;
mod notes;
pub mod owner_setup;
pub mod plugins;
#[cfg(feature = "rebuild-probe")]
pub mod rebuild_probe;
pub mod resolve;
pub mod secrets;
pub mod store;

pub mod turns;

pub mod live;
mod live_rooms;

pub mod account_input;
mod netguard;
mod onboarding;
mod provider_sign_in;
mod requests;

pub(crate) mod threads;

/// Local release-coordinator commands and startup fencing.
pub mod upgrades;
