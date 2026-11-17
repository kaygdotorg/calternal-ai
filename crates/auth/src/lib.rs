//! Protected identity storage primitives ported from Calternal's auth crate.
//!
//! This crate preserves the v1–15 legacy schema, including sharing tables.
//! It supplies stored space authority, but no login endpoints or recovery coordinator.
//! Callers must validate and lock installation roots before opening the DB;
//! opening a missing path creates it and is not safe recovery of lost state.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

mod account;
mod ceremony;
mod config;
pub mod db;
pub mod limit;
mod passkey;
mod passkeys;
mod session;
mod setup;
pub mod spaces;
pub use spaces::SpaceId;
pub mod token;
pub use account::{Auth, LoginPurpose};
pub use config::AuthConfig;
pub use passkeys::PasskeyInfo;
pub use session::{Installation, InstallationKind};
pub use setup::Created;

struct Inner {
    db: db::Db,
    config: AuthConfig,
    webauthn: webauthn_rs::prelude::Webauthn,
    ceremonies: ceremony::Ceremonies,
    sessions: session::Sessions,
    account_failures: limit::Failures,
    setup_failures: limit::Limiter,
    setup_targets: limit::TargetLimiter,
    login_starts: limit::Limiter,
}

/// Stable account identity; never selects a content root or grants space access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct AccountId(
    /// The account UUID, separate from every content/space identifier.
    pub uuid::Uuid,
);

/// Stable installation identity, owned by an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct InstallationId(
    /// The installation UUID; its account ownership is held in auth.db.
    pub uuid::Uuid,
);

macro_rules! identity_impl {
    ($name:ident) => {
        impl $name {
            /// Allocates a fresh version-seven identity.
            pub fn new() -> Self {
                Self(uuid::Uuid::now_v7())
            }
            /// Parses UUID syntax; this does not establish authority.
            pub fn parse(value: &str) -> Option<Self> {
                uuid::Uuid::parse_str(value).ok().map(Self)
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}
identity_impl!(AccountId);
identity_impl!(InstallationId);

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
