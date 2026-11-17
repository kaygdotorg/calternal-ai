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
pub mod error;
pub mod fault;
// These two pre-existing modules belong to the storage/plugin lanes. Preserve
// their existing warning level while enforcing documentation in owned modules.
#[warn(missing_docs)]
pub mod plugins;
pub mod resolve;
#[warn(missing_docs)]
pub mod store;
