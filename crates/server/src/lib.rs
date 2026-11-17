//! Calternal-AI localhost development-preview host.
//!
//! Disposable data only. Not production auth, not a plugin sandbox, not the
//! inherited storage/recovery guarantees. See crates/server/README.md.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

pub mod account_http;
pub mod api;
pub mod auth;
pub mod authz;
pub mod fault;
pub mod machines;
mod notes;
pub mod plugins;
pub mod resolve;
pub mod secrets;
pub mod store;

pub mod turns;

pub mod live;

pub mod account_input;

pub(crate) mod threads;
