//! The machine role of the `calternal` binary.
//!
//! A machine (the user's own Mac or Linux box, or a cloud VM the operator enrolled)
//! connects out to the server, reports its capacity and runs each assigned thread
//! in a per-thread sandbox it starts itself. The server never connects to a
//! machine and never starts containers or VMs.
//!
//! - [`protocol`]: the wire types shared with the server role.
//! - [`identity`]: the machine key and its 0600 credentials file.
//! - [`enroll`]: exchange a one-time code for a machine identity.
//! - [`connection`]: the outbound session, long-poll loop and reconnect backoff.
//! - [`runner`]: applies server messages to sandboxes and streams exec output.
//! - [`sandbox`]: the sandbox interface with Podman and direct backends.
//! - [`cli`]: `calternal machine enroll | run | status | discard`.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

mod bootstrap;
pub mod cli;
pub mod connection;
pub mod enroll;
pub mod identity;
pub mod link;
mod ownership;
pub use ownership::{OwnerLock, lock_for_upgrade};
pub mod protocol;
mod ready;
pub mod runner;
pub mod sandbox;

/// Machine-role failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The request did not reach the server or its answer was unreadable.
    #[error("cannot reach the server: {0}")]
    Http(#[from] reqwest::Error),
    /// The server answered with an error status.
    #[error("server refused ({status}): {message}")]
    Status {
        /// HTTP status.
        status: u16,
        /// The server's caller-safe message.
        message: String,
    },
    /// The server revoked this machine; stop all work and enroll again.
    #[error("this machine was revoked; enroll it again to use it")]
    Revoked,
    /// Local file failure.
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    /// Bad local state or an unexpected answer.
    #[error("{0}")]
    Protocol(String),
}

impl Error {
    /// HTTP status of a server refusal, if this is one.
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Status { status, .. } => Some(*status),
            _ => None,
        }
    }
}

/// Result of machine-role operations.
pub type Result<T> = std::result::Result<T, Error>;
