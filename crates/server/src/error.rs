use calternal_core::{ErrorCode, ValidationError};

/// Host error mapped 1:1 onto the wire error envelope. Messages are safe to
/// return: never paths, stacks or tokens.
#[derive(Debug, Clone)]
pub struct HostError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl HostError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
    pub fn unauthorized(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, m)
    }
    pub fn forbidden(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, m)
    }
    pub fn not_found(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, m)
    }
    pub fn conflict(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, m)
    }
    pub fn invalid_input(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, m)
    }
    pub fn unavailable(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unavailable, m)
    }
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
