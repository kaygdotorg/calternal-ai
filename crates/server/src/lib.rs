//! Calternal-AI localhost development-preview host.
//!
//! Disposable data only. Not production auth, not a plugin sandbox, not the
//! inherited storage/recovery guarantees. See crates/server/README.md.
#![warn(missing_docs)]

pub mod api;
pub mod auth;
pub mod authz;
pub mod error;
pub mod fault;
pub mod plugins;
pub mod store;
