//! Store errors: a stable code plus a private diagnostic for trusted logs.
use std::fmt;

/// Store failures suitable for adaptation into the host's operation errors.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ErrorCode {
    /// Invalid typed binding or input.
    InvalidInput,
    /// A request key has already been used within its retention.
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
