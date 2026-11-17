//! Safe host errors translated into the shared protocol envelope.

use calternal_core::{ErrorCode, ValidationError};

/// Host error mapped 1:1 onto the wire error envelope. Messages are safe to
/// return: never paths, stacks or tokens.
#[derive(Debug, Clone)]
pub struct HostError {
    /// Stable protocol error classification.
    pub code: ErrorCode,
    /// Caller-safe explanation without paths, stacks, or credentials.
    pub message: String,
    /// Optional structured caller-safe diagnostic details.
    pub details: Option<serde_json::Value>,
}

impl HostError {
    /// Construct a classified error with a caller-safe message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }
    /// Attach structured details that are safe to return to this caller.
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
    /// Reject missing or invalid authentication.
    pub fn unauthorized(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, m)
    }
    /// Reject an authenticated caller without sufficient authority.
    pub fn forbidden(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, m)
    }
    /// Report an absent resource only after its scope was authorized.
    pub fn not_found(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, m)
    }
    /// Report a conflict with current state or a previous request.
    pub fn conflict(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, m)
    }
    /// Reject input outside the operation contract.
    pub fn invalid_input(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, m)
    }
    /// Report a temporarily unavailable operation or dependency.
    pub fn unavailable(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unavailable, m)
    }
    /// Report an internal failure using a caller-safe message.
    pub fn internal(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, m)
    }
}

impl From<ValidationError> for HostError {
    fn from(e: ValidationError) -> Self {
        Self::new(e.code, e.message)
    }
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}
impl std::error::Error for HostError {}
