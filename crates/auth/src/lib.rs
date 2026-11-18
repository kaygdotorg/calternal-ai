//! Protected identity storage primitives ported from Calternal's auth crate.
//!
//! `auth.db` has one current schema (`schema.sql`); there is no migration
//! chain and no sharing, DAV or host-cookie state.
//! It supplies protected account and space authority to the host HTTP adapter.
//! Callers must validate and lock installation roots before opening the DB;
//! opening a missing path creates it and is not safe recovery of lost state.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

mod account;
mod account_details;
mod account_email;
mod account_recovery;
pub mod agents;
mod ceremony;
mod config;
pub mod db;
pub mod erasure;
mod http_facing;
mod invite_key;
mod oidc;
pub use oidc::{OidcProvider, OidcSession, OidcSignup};
pub mod invites;
pub mod limit;
pub mod machines;
mod passkey;
mod passkeys;
pub mod people;
mod secret_keys;
mod session;
mod setup;
pub mod spaces;
use calternal_types::{AccountId, InstallationId, SpaceId};
pub mod token;
pub use account::{Auth, LoginPurpose};
pub use account_email::AccountMail;
pub use config::{AuthConfig, Clock};
pub use passkeys::PasskeyInfo;
pub use session::{Installation, InstallationKind};
pub use setup::{Created, FirstOwnerCode};

struct Inner {
    db: db::Db,
    config: AuthConfig,
    invite_key: invite_key::Key,
    webauthn: webauthn_rs::prelude::Webauthn,
    ceremonies: ceremony::Ceremonies,
    sessions: session::Sessions,
    account_failures: limit::Failures,
    setup_failures: limit::Limiter,
    setup_targets: limit::TargetLimiter,
    login_starts: limit::Limiter,
    email_sends: limit::Limiter,
    recovery_proofs: limit::Limiter,
}

/// Stable refusal categories for account and stored-space operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// A request identifier was reused with different input.
    Conflict,
    /// Invalid configuration, name or identifier.
    InvalidInput,
    /// No matching account-owned object exists.
    NotFound,
    /// Interactive proof or current scoped authority is absent or unsuitable.
    Forbidden,
    /// WebAuthn verification failed or its ceremony is unavailable.
    PasskeyFailed,
    /// Admission/back-off limit exceeded.
    TooManyRequests,
    /// The space already holds the maximum number of live invitations.
    InviteLimit,
    /// Setup token is unknown, used or expired.
    SetupTokenInvalid,
    /// Removing this key would leave the account without a passkey.
    LastPasskey,
    /// A new passkey assertion is required.
    PasskeyReverificationRequired,
    /// Private storage/entropy/worker failure.
    Internal,
}

/// Failure from a storage/entropy operation or bounded worker admission.
/// Internal diagnostics are private host data; do not return them over the wire.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The dedicated worker already has 64 queued jobs; retry with backpressure.
    #[error("auth database queue full")]
    QueueFull,
    /// An account or space operation refused with a stable code and private diagnostic.
    #[error("account operation refused ({code:?}): {detail}")]
    Refused {
        /// Machine-readable refusal category.
        code: ErrorCode,
        /// Private host diagnostic; transport adapters must sanitize it.
        detail: String,
    },
    /// Private diagnostic from SQLite, filesystem, entropy or worker failure.
    #[error("auth internal error: {0}")]
    Internal(String),
}
impl Error {
    /// Returns a stable account refusal category; worker saturation is rate-limited.
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Refused { code, .. } => *code,
            Self::QueueFull => ErrorCode::TooManyRequests,
            Self::Internal(_) => ErrorCode::Internal,
        }
    }
    pub(crate) fn new(code: ErrorCode, detail: impl std::fmt::Display) -> Self {
        Self::Refused {
            code,
            detail: detail.to_string(),
        }
    }
    pub(crate) fn invalid(detail: impl std::fmt::Display) -> Self {
        Self::new(ErrorCode::InvalidInput, detail)
    }

    pub(crate) fn internal(error: impl std::fmt::Display) -> Self {
        Self::Internal(error.to_string())
    }
}
/// Result returned by identity storage, stored spaces and token primitives.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod test_support {
    // Preserve the source testkit constructor shape without importing its engine.
    pub(crate) struct TempDir(tempfile::TempDir);
    impl TempDir {
        pub(crate) fn new() -> Self {
            Self(tempfile::tempdir().unwrap())
        }
        pub(crate) fn path(&self) -> &std::path::Path {
            self.0.path()
        }
    }
}
