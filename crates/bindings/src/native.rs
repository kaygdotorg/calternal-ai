//! Native async objects reuse the shared HTTP client, retry and cancellation policy.
use calternal_client::{CancellationToken, Client, ClientError};
use calternal_core::*;
use serde_json::json;
use std::sync::Arc;

/// Safe native failure categories. No transport URL or credential enters diagnostics.
/// Carries at most one serialized diagnostic; no I/O or synchronization.
#[derive(Debug, thiserror::Error)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Error))]
pub enum NativeError {
    /// The trusted sign-in flow must obtain a fresh session credential.
    #[error("sign in again")]
    ReauthenticationRequired,
    /// Waiting stopped locally; a mutation may already have committed.
    #[error("local wait cancelled; server outcome may be unknown")]
    Cancelled,
    /// Transport attempts exhausted; retain the request for reconciliation.
    #[error("transport failed; server outcome may be unknown")]
    Transport,
    /// Configuration, validation, protocol or server failure, with safe JSON details.
    #[error("{details}")]
    Failure {
        /// Serialized portable ApiError with category, message and optional details.
        details: String,
    },
}
impl From<ClientError> for NativeError {
    fn from(error: ClientError) -> Self {
        match error {
            ClientError::ReauthenticationRequired => Self::ReauthenticationRequired,
            ClientError::Cancelled => Self::Cancelled,
            ClientError::Transport => Self::Transport,
            other => Self::Failure {
                details: serde_json::to_string(&other.api_error())
                    .expect("API error serialization"),
            },
        }
    }
}
fn invalid(message: &str) -> NativeError {
    NativeError::Failure {
        details: json!({"code":"invalid_input","message":message}).to_string(),
    }
}

/// One-way cancellation shared by explicit Swift control and Rust async waits.
/// Bounded watch state; cloning the Arc is constant cost, no persistent I/O.
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct NativeCancellation {
    pub(crate) token: CancellationToken,
}
#[cfg_attr(feature = "uniffi", uniffi::export)]
impl NativeCancellation {
    /// Allocate one Arc and watch channel; constant cost, no I/O or errors.
    /// Reuse only within one operation group.
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            token: CancellationToken::new(),
        })
    }
    /// Stop local waits; synchronizes one watch value and wakes O(waiters), no I/O.
    /// Does not undo a committed write.
    pub fn cancel(&self) {
        self.token.cancel();
    }
}

/// Validated immutable operation envelope with one request identity, retained for retry.
/// No credential is stored in this object. Persist identity/input before durable use;
/// the connection's in-memory outbox does not provide platform durability. Holds
/// O(input bytes) in memory;
/// no credential, persistent writes or per-frame use.
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct NativeRequest {
    pending: PendingRequest,
}
#[cfg_attr(feature = "uniffi", uniffi::export)]
impl NativeRequest {
    /// Prepare one request from generated-contract JSON input, optionally restoring a
    /// retained identity. A restored identity must always use identical input/operation.
    /// O(input bytes) parsing/validation/allocation plus UUID entropy for fresh IDs;
    /// no network/persistent I/O. Invalid input returns Failure.
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(
        operation: String,
        input_json: String,
        request_id: Option<String>,
    ) -> Result<Arc<Self>, NativeError> {
        let input: serde_json::Value =
            serde_json::from_str(&input_json).map_err(|_| invalid("invalid input JSON"))?;
        Ok(Arc::new(Self {
            pending: Client::pending(&operation, &input, request_id)?,
        }))
    }
    /// Clone the stable identity, O(ID bytes) allocation; no locks, I/O or errors.
    pub fn request_id(&self) -> String {
        self.pending.request().request_id.clone()
    }
    /// Serialize the envelope for platform retention; O(input bytes) allocation, no I/O.
    /// The platform must persist it atomically before claiming durable offline intent.
    pub fn envelope_json(&self) -> String {
        serde_json::to_string(self.pending.request()).expect("request serialization")
    }
}

/// Reusable async HTTP session; platform code owns credential acquisition and storage.
/// No Debug implementation exposes the sensitive bearer header. Construction allocates
/// a connection pool; requests use bounded transport, no local persistent writes.
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct NativeClient {
    client: Client,
}
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl NativeClient {
    /// Follow this signed-in account's drafts, pending input and confirmation
    /// requests. Cost: bounded memory bookmark load; next opens the socket.
    /// A reset requires authorized reload and a new stream with no bookmark.
    pub async fn subscribe_account(
        &self,
        bookmark: Option<String>,
        cancellation: Arc<NativeCancellation>,
    ) -> Result<Arc<crate::NativeAccountLive>, NativeError> {
        crate::NativeAccountLive::open(&self.client, bookmark, cancellation).await
    }

    /// Follow up to 16 spaces with platform-restored bookmarks (`{spaceUUID: token}`).
    /// Returns a pull-based async stream with shared cancellation. Loads bounded
    /// memory state, then next opens the socket. No platform persistence occurs.
    pub async fn subscribe(
        &self,
        spaces: Vec<String>,
        bookmarks_json: String,
        cancellation: Arc<NativeCancellation>,
    ) -> Result<Arc<crate::NativeLive>, NativeError> {
        crate::NativeLive::open(&self.client, spaces, bookmarks_json, cancellation).await
    }

    /// Accept a session credential from trusted passkey/OIDC endpoints, or an explicitly
    /// selected disposable development token. Current transport accepts loopback HTTP.
    /// Allocates Arc/header/HTTP state without network I/O; invalid origins/tokens fail.
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(base_url: String, credential: String) -> Result<Arc<Self>, NativeError> {
        Ok(Arc::new(Self {
            client: Client::new(&base_url, &credential)?,
        }))
    }
    /// Discover contracts asynchronously; at most three round trips, 128 MiB buffering
    /// per attempt plus O(result bytes) JSON serialization. No local writes/fsync.
    /// Returns typed cancellation/authentication/transport or safe protocol failures.
    pub async fn describe(
        &self,
        cancellation: Arc<NativeCancellation>,
    ) -> Result<String, NativeError> {
        let result = self
            .client
            .describe_cancellable(&cancellation.token)
            .await?;
        Ok(serde_json::to_string(&result).expect("describe serialization"))
    }
    /// Resolve a kind and UUID; up to three round trips, bounded response buffering and
    /// O(result bytes) JSON allocation. No local writes/fsync; missing/inaccessible
    /// resources share the server not-found failure, with typed local failures.
    pub async fn resolve(
        &self,
        kind: String,
        id: String,
        cancellation: Arc<NativeCancellation>,
    ) -> Result<String, NativeError> {
        let result = self.client.resolve(&kind, &id, &cancellation.token).await?;
        Ok(serde_json::to_string(&result).expect("resolve serialization"))
    }
    /// Execute a retained validated operation asynchronously; transport retries reuse
    /// its identity. Dynamic plugin results remain JSON validated by the server.
    /// Up to three round trips, O(input/result bytes) copying and bounded buffering;
    /// no local storage/fsync. Typed local failures preserve uncertain outcome semantics.
    pub async fn call(
        &self,
        request: Arc<NativeRequest>,
        cancellation: Arc<NativeCancellation>,
    ) -> Result<String, NativeError> {
        let result: serde_json::Value = self
            .client
            .execute_cancellable(request.pending.request(), &cancellation.token)
            .await?;
        Ok(result.to_string())
    }
}
