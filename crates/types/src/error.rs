//! The caller-safe error shared by every layer that can answer a request.
//!
//! A message and its details are returned to the caller verbatim, so they never
//! contain paths, SQL, stack traces, tokens or another account's data. Private
//! diagnostics are logged where the failure happens, never carried here.
use serde_json::Value;
use std::fmt;

/// Stable error categories used by every operation failure; each maps to one HTTP status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "contract", derive(schemars::JsonSchema, ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Authentication is missing or invalid (HTTP 401).
    Unauthorized,
    /// Current authority does not permit the operation (HTTP 403).
    Forbidden,
    /// An authenticated action needs a fresh passkey proof (HTTP 403).
    PasskeyReverificationRequired,
    /// The requested resource or operation is unavailable (HTTP 404).
    NotFound,
    /// The expected revision or durable retry input conflicts (HTTP 409).
    Conflict,
    /// Input violates the contract (HTTP 400 in operation errors).
    InvalidInput,
    /// The serialized body or document exceeds its byte budget (HTTP 413).
    TooLarge,
    /// A short-lived condition prevents this attempt. Retry after the advised delay (HTTP 429).
    RetryAfter,
    /// The operation cannot currently be served (HTTP 503).
    Unavailable,
    /// An internal failure occurred; expose only safe diagnostics (HTTP 500 in operation errors).
    Internal,
}

impl ErrorCode {
    /// The HTTP status for this category. Cost: one match.
    pub fn http_status(self) -> u16 {
        match self {
            Self::Unauthorized => 401,
            Self::Forbidden | Self::PasskeyReverificationRequired => 403,
            Self::NotFound => 404,
            Self::Conflict => 409,
            Self::InvalidInput => 400,
            Self::TooLarge => 413,
            Self::RetryAfter => 429,
            Self::Unavailable => 503,
            Self::Internal => 500,
        }
    }
}

/// A failure whose every field may be shown to the caller who caused it.
///
/// Cost: the message (and details, when present) are heap allocations; building
/// one does no I/O and takes no lock. It serializes as `{code, message, details}`
/// for local channels such as the admin socket.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Error {
    /// Category callers branch on; never parse the message instead.
    pub code: ErrorCode,
    /// Short human-readable reason, safe to return.
    pub message: String,
    /// Optional structured reason (for example `{"reason":"stale_revision"}`), safe to return.
    pub details: Option<Value>,
}

/// Result whose error is caller-safe.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Describe an invalid input field without including any submitted value.
    /// Bounds and escapes the caller-visible JSON pointer. Cost is bounded by
    /// the length of the returned diagnostic.
    pub fn input_field(pointer: &str, expectation: &str) -> Self {
        let path: String = pointer
            .chars()
            .take(160)
            .flat_map(char::escape_debug)
            .collect();
        let label = if path.is_empty() { "/" } else { &path };
        Self::invalid_input(format!("Input {label}: {expectation}.")).with_details(
            serde_json::json!({"reason":"input_schema","path":label,"expected":expectation}),
        )
    }
    /// A classified error with a caller-safe message and no details.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }
    /// Attaches structured, caller-safe details.
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
    /// Missing or invalid authentication.
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, message)
    }
    /// An authenticated caller without sufficient authority.
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, message)
    }
    /// Require the caller to reverify with a passkey before retrying.
    pub fn passkey_reverification_required() -> Self {
        Self::new(
            ErrorCode::PasskeyReverificationRequired,
            "Verify with your passkey to continue.",
        )
    }
    /// An absent resource, or one the caller may not know exists.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }
    /// A conflict with current state or an earlier request.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, message)
    }
    /// Input outside the operation contract.
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, message)
    }
    /// A temporarily unavailable operation or dependency.
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unavailable, message)
    }
    /// An internal failure; the message must already be caller-safe.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }
    /// The answer to a request ID reused with different input, identical everywhere.
    pub fn request_reused() -> Self {
        Self::conflict("request_id was already used with different input")
            .with_details(serde_json::json!({ "reason": "request_id_reused" }))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}
