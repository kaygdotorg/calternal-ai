//! Typed account/space bindings and store errors; move IDs to shared types later.
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use uuid::Uuid;

macro_rules! identifier {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
        #[serde(try_from = "Uuid", into = "Uuid")]
        pub struct $name(Uuid);
        impl $name {
            /// Allocates a fresh UUIDv7. No I/O, writer lock or SQLite work.
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }
            /// Validates an existing UUIDv7 without I/O or allocation.
            pub fn from_uuid(value: Uuid) -> Result<Self> {
                if value.get_version_num() != 7 || value.get_variant() != uuid::Variant::RFC4122 {
                    return Err(Error::new(
                        ErrorCode::InvalidInput,
                        "identifier must be an RFC UUIDv7",
                    ));
                }
                Ok(Self(value))
            }
            /// Returns the underlying UUID by value, without allocation or I/O.
            pub fn as_uuid(self) -> Uuid {
                self.0
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
        impl TryFrom<Uuid> for $name {
            type Error = Error;
            fn try_from(value: Uuid) -> Result<Self> {
                Self::from_uuid(value)
            }
        }
        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.0
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
        impl FromStr for $name {
            type Err = Error;
            fn from_str(value: &str) -> Result<Self> {
                Self::from_uuid(Uuid::parse_str(value).map_err(Error::internal)?)
            }
        }
    };
}
identifier!(
    SpaceId,
    "Stable space UUIDv7, distinct from account identity. Temporarily owned by this crate until shared types are available."
);
identifier!(
    AccountId,
    "Stable acting account UUIDv7. Never used to select a space journal; move to shared types alongside SpaceId."
);

/// Store failures suitable for adaptation into the host's operation errors.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ErrorCode {
    /// Invalid typed binding or input.
    InvalidInput,
    /// A request or permanent publication key has already been used.
    IdempotencyConflict,
    /// The cursor is invalid or requires a complete resynchronization.
    SyncTokenExpired,
    /// Filesystem, SQLite, worker or serialization failure.
    Internal,
}
/// Store error retaining a stable code and diagnostic detail.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{code:?}: {detail}")]
pub struct Error {
    /// Stable failure category.
    pub code: ErrorCode,
    /// Diagnostic detail for trusted logs.
    pub detail: String,
}
impl Error {
    /// Allocates a diagnostic string without taking a writer lock or doing I/O.
    pub fn new(code: ErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
    /// Wraps an internal failure, allocating its display text without I/O.
    pub fn internal(detail: impl fmt::Display) -> Self {
        Self::new(ErrorCode::Internal, detail.to_string())
    }
}
/// A result returned by the per-space store.
pub type Result<T> = std::result::Result<T, Error>;
