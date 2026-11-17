#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
//! Async HTTP effects adapter. The pure contract and pending identities live in core.
//! Session credentials come from the trusted passkey/OIDC flow; platform adapters own
//! secret storage and supply the credential without logging it. A 401 requires sign-in.
//! Dropping a request future or cancelling a token stops waiting locally; it does not
//! undo server commits. Retain pending identities after an uncertain mutation outcome.
//!
//! UniFFI is a background operation boundary: editor composition, selection, keystrokes,
//! layout, animation and per-frame rendering stay native. Submit coalesced transactions
//! and consume batched results; never copy an entire document per keystroke. Journal
//! following and offline storage are deliberately unbuilt; see `SYNC.md`.
use calternal_core::*;
use reqwest::{
    Client as HttpClient, Url,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::time::Duration;
mod cancellation;
pub use cancellation::CancellationToken;
mod typed;
pub use typed::{Operation, PendingCall, operations};

const MAX_RESPONSE_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
/// Safe local or remote failure; transport errors can leave a mutation outcome unknown.
pub enum ClientError {
    #[error("{0}")]
    /// Local contract validation failed before dispatch.
    Validation(#[from] ValidationError),
    #[error("session expired or revoked; sign in again")]
    /// Authentication is no longer valid; no automatic retry or credential refresh occurs.
    ReauthenticationRequired,
    #[error("local wait cancelled; server outcome may be unknown")]
    /// Local cancellation ends transport/retry waiting, without requesting server rollback.
    Cancelled,
    #[error("invalid client configuration: {0}")]
    /// The loopback URL, credential or HTTP client could not be configured.
    Configuration(&'static str),
    #[error("transport failed; server outcome may be unknown")]
    /// The transport failed; the server may already have committed a mutation.
    Transport,
    #[error("server response violates the preview protocol: {0}")]
    /// The reply failed content-type, size, schema, version or correlation checks.
    Protocol(&'static str),
    #[error("{code:?}: {message}", code = .error.code, message = .error.message)]
    /// A safe server error whose category matches the HTTP status.
    Api {
        /// HTTP response status, consistent with the API error category.
        status: u16,
        /// Safe server failure payload.
        error: ApiError,
        /// Recovered correlation identity, absent when the server could not recover it.
        request_id: Option<String>,
    },
}
impl ClientError {
    /// Convert to a portable error; O(diagnostic bytes) allocations, no locks or I/O.
    /// Authentication retains Unauthorized; local transport/protocol failures map to Unavailable.
    pub fn api_error(&self) -> ApiError {
        match self {
            Self::Validation(e) => ApiError {
                code: e.code,
                message: e.message.clone(),
                details: None,
            },
            Self::Api { error, .. } => error.clone(),
            Self::ReauthenticationRequired => ApiError {
                code: ErrorCode::Unauthorized,
                message: self.to_string(),
                details: None,
            },
            Self::Configuration(_) => ApiError {
                code: ErrorCode::InvalidInput,
                message: self.to_string(),
                details: None,
            },
            _ => ApiError {
                code: ErrorCode::Unavailable,
                message: self.to_string(),
                details: None,
            },
        }
    }
}

// Deliberately no Debug: the HTTP adapter owns a sensitive Authorization header.
#[derive(Clone)]
/// Reusable HTTP adapter holding a sensitive bearer header; cloning shares the connection pool.
/// Construction allocates HTTP state; calls buffer bounded bodies and never persist credentials.
/// Deliberately omits Debug to avoid exposing credentials.
pub struct Client {
    http: HttpClient,
    base: Url,
}
impl Client {
    /// Build a reusable session client for an HTTPS origin or HTTP loopback origin and ASCII graphic
    /// credential. Allocates URL/header/connection-pool state, O(URL + token bytes),
    /// without network or persistent I/O. Uses 15-second request and 3-second connection
    /// timeouts; disables proxies and redirects. No credential appears in Debug/errors.
    ///
    /// # Errors
    /// Returns configuration errors for invalid origins/tokens or HTTP initialization failure.
    pub fn new(base_url: &str, token: &str) -> Result<Self, ClientError> {
        let base =
            Url::parse(base_url).map_err(|_| ClientError::Configuration("invalid base URL"))?;
        let loopback = base.host_str().is_some_and(|h| {
            h == "localhost"
                || h.trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|a| a.is_loopback())
        });
        if (base.scheme() != "https" && (base.scheme() != "http" || !loopback))
            || base.host_str().is_none()
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || base.path() != "/"
        {
            return Err(ClientError::Configuration(
                "server URL must be HTTPS (or loopback HTTP) without credentials, path, query or fragment",
            ));
        }
        if token.is_empty() || token.bytes().any(|b| !b.is_ascii_graphic()) {
            return Err(ClientError::Configuration("invalid bearer token"));
        }
        let mut headers = HeaderMap::new();
        let mut auth = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| ClientError::Configuration("invalid bearer token"))?;
        auth.set_sensitive(true);
        headers.insert(AUTHORIZATION, auth);
        let http = HttpClient::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .map_err(|_| ClientError::Configuration("HTTP client initialization failed"))?;
        Ok(Self { http, base })
    }
    /// Run a contract-associated operation with at most three transport attempts and
    /// 100/200 ms backoff. One pending identity is allocated before the first attempt.
    /// Keep `pending` to reconcile a cancelled or exhausted mutation; API/protocol errors
    /// are never retried. Cancellation drops local HTTP work, not server effects.
    /// Cost: up to three HTTP round trips; serialization/validation O(input bytes),
    /// at most 128 MiB buffered per attempt; no durable writes or fsyncs.
    pub async fn call<O: Operation>(
        &self,
        pending: &PendingCall<O>,
        cancellation: &CancellationToken,
    ) -> Result<O::Output, ClientError> {
        self.execute_cancellable(pending.request(), cancellation)
            .await
    }

    /// Execute a retained envelope with bounded transport retries and prompt local
    /// cancellation, including during backoff and response-body reads. Reuses the exact
    /// envelope on every attempt; validation, authentication and protocol failures stop.
    /// Cost: at most three HTTP round trips and 128 MiB buffered per attempt,
    /// O(input bytes) validation/serialization; no persistent storage or fsync.
    pub async fn execute_cancellable<T: DeserializeOwned>(
        &self,
        request: &OperationRequest,
        cancellation: &CancellationToken,
    ) -> Result<T, ClientError> {
        retry_transport(cancellation, || self.execute_once(request)).await
    }

    /// Discover compatible contracts with at most three transport attempts and local
    /// cancellation. Buffers at most 128 MiB per attempt; no durable storage or fsync.
    /// Authentication failure requires trusted sign-in; credentials are not refreshed here.
    pub async fn describe_cancellable(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<DescribeResult, ClientError> {
        retry_transport(cancellation, || self.describe_once()).await
    }

    /// Resolve a kind and UUID under current authority, with bounded transport retries.
    /// Inaccessible and missing items have the same server-defined not-found result.
    /// Cost: one request allocation, up to three HTTP round trips, bounded buffering;
    /// errors match [`Self::execute_cancellable`]. No local storage or fsync.
    pub async fn resolve(
        &self,
        kind: &str,
        id: &str,
        cancellation: &CancellationToken,
    ) -> Result<ResolveResult, ClientError> {
        self.call(
            &PendingCall::<operations::Resolve>::new(&ResolveInput {
                kind: kind.into(),
                id: id.into(),
            })?,
            cancellation,
        )
        .await
    }

    async fn body(response: reqwest::Response) -> Result<(u16, Vec<u8>), ClientError> {
        let status = response.status().as_u16();
        let json = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|s| {
                s.split(';')
                    .next()
                    .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
            });
        if !json {
            return Err(ClientError::Protocol("expected application/json"));
        }
        if response
            .content_length()
            .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
        {
            return Err(ClientError::Protocol("response exceeds 128 MiB"));
        }
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| ClientError::Transport)? {
            if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err(ClientError::Protocol("response exceeds 128 MiB"));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok((status, bytes))
    }
    /// Fetch `GET /api/v1/describe` with version checks and a 128 MiB response cap. One network round trip; response bytes are buffered.
    ///
    /// # Errors
    /// Returns transport, API or protocol errors for failed or incompatible replies.
    async fn describe_once(&self) -> Result<DescribeResult, ClientError> {
        let response = self
            .http
            .get(self.base.join("api/v1/describe").unwrap())
            .send()
            .await
            .map_err(|_| ClientError::Transport)?;
        let (status, bytes) = Self::body(response).await?;
        if !(200..300).contains(&status) {
            return Err(Self::failure(status, &bytes, None)?);
        }
        let describe: DescribeResult = serde_json::from_slice(&bytes)
            .map_err(|_| ClientError::Protocol("invalid discovery result"))?;
        if describe.protocol_version != PROTOCOL_VERSION || describe.genui_version != GENUI_VERSION
        {
            return Err(ClientError::Protocol("unsupported server version"));
        }
        Ok(describe)
    }
    /// Discover contracts with bounded transport retries; up to three HTTP round trips
    /// and 128 MiB buffered per attempt. No persistent writes. Returns safe protocol,
    /// transport or re-authentication errors; drop the future to stop local waiting.
    pub async fn describe(&self) -> Result<DescribeResult, ClientError> {
        self.describe_cancellable(&CancellationToken::new()).await
    }
    fn failure(
        status: u16,
        bytes: &[u8],
        expected: Option<&str>,
    ) -> Result<ClientError, ClientError> {
        if status == 401 {
            return Ok(ClientError::ReauthenticationRequired);
        }
        let failure: Failure = serde_json::from_slice(bytes)
            .map_err(|_| ClientError::Protocol("invalid error envelope"))?;
        if status != failure.error.code.http_status() {
            return Err(ClientError::Protocol("HTTP/error code mismatch"));
        }
        if failure.request_id.is_some()
            && expected.is_some()
            && failure.request_id.as_deref() != expected
        {
            return Err(ClientError::Protocol("response request_id mismatch"));
        }
        Ok(ClientError::Api {
            status,
            error: failure.error,
            request_id: failure.request_id,
        })
    }
    /// Validate and send one operation, buffering at most 128 MiB and checking request correlation. No automatic retry occurs; retain the same request after uncertain outcomes. Dropping the future does not undo a server commit.
    ///
    /// # Errors
    /// Returns validation, transport, API or protocol errors, including result decoding errors.
    async fn execute_once<T: DeserializeOwned>(
        &self,
        request: &OperationRequest,
    ) -> Result<T, ClientError> {
        validate_client_request(request)?;
        let response = self
            .http
            .post(self.base.join("api/v1/operations").unwrap())
            .json(request)
            .send()
            .await
            .map_err(|_| ClientError::Transport)?;
        let (status, bytes) = Self::body(response).await?;
        if !(200..300).contains(&status) {
            return Err(Self::failure(status, &bytes, Some(&request.request_id))?);
        }
        let result: Success<T> = serde_json::from_slice(&bytes)
            .map_err(|_| ClientError::Protocol("invalid success envelope"))?;
        if result.request_id != request.request_id {
            return Err(ClientError::Protocol("response request_id mismatch"));
        }
        Ok(result.data)
    }
    /// Execute a validated envelope with three transport attempts, 100/200 ms backoff,
    /// and one retained request identity. Dropping the future stops local waiting only.
    /// Use [`Self::execute_cancellable`] for an explicit cancellation handle.
    /// Cost: O(input bytes) encoding/validation, at most 128 MiB response buffering
    /// per attempt, no persistent storage or fsync; errors match that method.
    pub async fn execute<T: DeserializeOwned>(
        &self,
        request: &OperationRequest,
    ) -> Result<T, ClientError> {
        self.execute_cancellable(request, &CancellationToken::new())
            .await
    }
    /// Execute with an untyped result; up to three round trips, O(input/result bytes)
    /// allocation, 128 MiB response cap per attempt, no persistent I/O or fsync.
    /// Inherits cancellation-by-drop and errors from [`Self::execute`].
    ///
    /// # Errors
    /// Returns validation, transport, API or protocol errors.
    pub async fn invoke(&self, request: &OperationRequest) -> Result<Value, ClientError> {
        self.execute(request).await
    }
    /// Serialize a typed input into an immutable pending request, allocating O(input bytes). Supply the prior identity for retries or omit it for a new UUID.
    ///
    /// # Errors
    /// Returns protocol errors for non-object/unserializable input or validation errors.
    pub fn pending<I: Serialize>(
        operation: &str,
        input: &I,
        request_id: Option<String>,
    ) -> Result<PendingRequest, ClientError> {
        let input = serde_json::to_value(input)
            .map_err(|_| ClientError::Protocol("input serialization failed"))?;
        let input: JsonObject = serde_json::from_value(input)
            .map_err(|_| ClientError::Protocol("operation input must be object"))?;
        Ok(match request_id {
            Some(id) => PendingRequest::with_id(id, operation, input)?,
            None => PendingRequest::new(operation, input)?,
        })
    }
    /// Call `notes.list` with typed input/output and a buffered response capped at 128 MiB per attempt (up to three round trips; O(input/result bytes) allocation, no local writes/fsync). Creates one fresh request identity, reused on bounded transport retry.
    ///
    /// # Errors
    /// Returns validation, transport, API or protocol errors; dispatch checks current space access.
    pub async fn notes_list(&self, input: &NotesListInput) -> Result<NotesListResult, ClientError> {
        self.execute(Self::pending("notes.list", input, None)?.request())
            .await
    }
    /// Call `notes.get` with typed input/output and a buffered response capped at 128 MiB per attempt (up to three round trips; O(input/result bytes) allocation, no local writes/fsync). Creates one fresh request identity, reused on bounded transport retry.
    ///
    /// # Errors
    /// Returns validation, transport, API or protocol errors; dispatch checks current space access.
    pub async fn notes_get(&self, input: &NotesGetInput) -> Result<NoteResult, ClientError> {
        self.execute(Self::pending("notes.get", input, None)?.request())
            .await
    }
    /// Call `notes.history` with typed input/output and a buffered response capped at 128 MiB per attempt (up to three round trips; O(input/result bytes) allocation, no local writes/fsync). Creates one fresh request identity, reused on bounded transport retry.
    ///
    /// # Errors
    /// Returns validation, transport, API or protocol errors; dispatch checks current space access.
    pub async fn notes_history(
        &self,
        input: &NotesHistoryInput,
    ) -> Result<NotesHistoryResult, ClientError> {
        self.execute(Self::pending("notes.history", input, None)?.request())
            .await
    }
    /// Call `notes.create` with typed input/output and a buffered response capped at 128 MiB per attempt (up to three round trips; O(input/result bytes) allocation, no local writes/fsync). Reuse `request_id` and identical input after uncertain outcomes; bounded transport retries retain it.
    ///
    /// # Errors
    /// Returns validation, transport, API or protocol errors; dispatch checks current space access.
    pub async fn notes_create(
        &self,
        input: &NotesCreateInput,
        request_id: &str,
    ) -> Result<NoteResult, ClientError> {
        self.execute(Self::pending("notes.create", input, Some(request_id.into()))?.request())
            .await
    }
    /// Call `notes.update` with typed input/output and a buffered response capped at 128 MiB per attempt (up to three round trips; O(input/result bytes) allocation, no local writes/fsync). Reuse `request_id` and identical input after uncertain outcomes; bounded transport retries retain it.
    ///
    /// # Errors
    /// Returns validation, transport, API or protocol errors; dispatch checks current space access.
    pub async fn notes_update(
        &self,
        input: &NotesUpdateInput,
        request_id: &str,
    ) -> Result<NoteResult, ClientError> {
        self.execute(Self::pending("notes.update", input, Some(request_id.into()))?.request())
            .await
    }
    /// Call `plugins.list` with typed input/output and a buffered response capped at 128 MiB per attempt (up to three round trips; O(input/result bytes) allocation, no local writes/fsync). Creates one fresh request identity, reused on bounded transport retry.
    ///
    /// # Errors
    /// Returns validation, transport, API or protocol errors; dispatch checks current space access.
    pub async fn plugins_list(
        &self,
        input: &PluginsListInput,
    ) -> Result<PluginsListResult, ClientError> {
        self.execute(Self::pending("plugins.list", input, None)?.request())
            .await
    }
    /// Call `plugins.enable` with typed input/output and a buffered response capped at 128 MiB per attempt (up to three round trips; O(input/result bytes) allocation, no local writes/fsync). Reuse `request_id` and identical input after uncertain outcomes; bounded transport retries retain it.
    ///
    /// # Errors
    /// Returns validation, transport, API or protocol errors; dispatch checks current space access.
    pub async fn plugins_enable(
        &self,
        input: &PluginInput,
        request_id: &str,
    ) -> Result<PluginResult, ClientError> {
        self.execute(Self::pending("plugins.enable", input, Some(request_id.into()))?.request())
            .await
    }
    /// Call `plugins.disable` with typed input/output and a buffered response capped at 128 MiB per attempt (up to three round trips; O(input/result bytes) allocation, no local writes/fsync). Reuse `request_id` and identical input after uncertain outcomes; bounded transport retries retain it.
    ///
    /// # Errors
    /// Returns validation, transport, API or protocol errors; dispatch checks current space access.
    pub async fn plugins_disable(
        &self,
        input: &PluginInput,
        request_id: &str,
    ) -> Result<PluginResult, ClientError> {
        self.execute(Self::pending("plugins.disable", input, Some(request_id.into()))?.request())
            .await
    }
    /// Call `plugins.preview` with typed input/output and a buffered response capped at 128 MiB per attempt (up to three round trips; O(input/result bytes) allocation, no local writes/fsync). Creates one fresh request identity, reused on bounded transport retry.
    ///
    /// # Errors
    /// Returns validation, transport, API or protocol errors; dispatch checks current space access.
    pub async fn plugins_preview(
        &self,
        input: &PluginInput,
    ) -> Result<PluginPreviewResult, ClientError> {
        self.execute(Self::pending("plugins.preview", input, None)?.request())
            .await
    }
}

// One policy for discovery and operations; API/protocol errors never enter backoff.
async fn retry_transport<T, F, Fut>(
    cancellation: &CancellationToken,
    mut attempt: F,
) -> Result<T, ClientError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, ClientError>>,
{
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(ClientError::Cancelled),
        result = async {
            for index in 0..3 {
                match attempt().await {
                    Err(ClientError::Transport) if index < 2 => tokio::time::sleep(Duration::from_millis(100 << index)).await,
                    result => return result,
                }
            }
            unreachable!()
        } => result,
    }
}
