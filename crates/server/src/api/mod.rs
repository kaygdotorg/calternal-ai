//! HTTP surface: `/api/v1/describe`, `/api/v1/operations`, same-origin static
//! files (`--web-dir`, no SPA fallback), Host/Origin checks, account sessions, optional development bearers
//! and security headers. Split by concern: `guard` (Host/Origin/session checks),
//! `dispatch` (operation admission and host handlers) and `plugins` (plugin
//! invocation and preview).

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use calternal_core::{
    ErrorCode, GENUI_VERSION, JsonObject, MAX_BODY_BYTES, NoteResult, NotesCreateInput,
    NotesGetInput, NotesHistoryInput, NotesHistoryResult, NotesListInput, NotesListResult,
    NotesUpdateInput, OperationResponse, PROTOCOL_VERSION, PluginInput, PluginPreviewResult,
    PluginResult, PluginState, PluginsListInput, PluginsListResult, True, canonical_request_input,
    operation_catalog, parse_request,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tower_http::services::ServeDir;

use crate::auth::{Directory, Identity, StoredDirectory};
use crate::authz::{self, Constraint};
use crate::fault::{self, FaultPoint};
use crate::plugins::worker::Outcome;
use crate::plugins::{
    Call, Jobs, PREVIEW_ITEM_ID, Plugin, PreviewHost, Registry, Session, WorkerPool, activation,
    broker::SpaceHost,
};
use crate::store::{PluginRow, Store, sha256_hex};
use calternal_types::{AccountId, Error, SpaceId};

mod dispatch;
mod guard;
mod plugins;
pub use dispatch::dispatch;
pub(crate) use dispatch::fingerprint;
use dispatch::*;
use guard::*;
use plugins::*;

/// Same-origin development content security policy applied to every response.
pub const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'";

/// Shared host services; `D` is the protected source of current identity and grants.
/// The default preserves fixture integrations; stored directories plug in without
/// changing operation handlers or caching membership in authenticated identities.
pub struct AppState<D: Directory = StoredDirectory> {
    /// Per-account input databases (drafts, pending submissions, the account feed).
    pub inputs: crate::account_input::Inputs,
    /// Durable preview store, shared without an outer lock. Content, plugin state
    /// and receipts take only their space's lock; a new item's first catalog route
    /// takes the one catalog writer (see [`Store`]).
    pub store: Store,
    /// Identity and membership directory; read for every authorization. The stored
    /// directory runs every read and write on one bounded auth worker (one
    /// connection), so authorization queues behind other auth work, including
    /// recovery fsyncs: it is installation-wide, not per space.
    pub directory: D,
    /// Immutable registry of trusted plugins and their manifest-derived policy rows.
    pub registry: Registry,
    /// Bounded pool of long-lived (plugin, space) workers routed through enrolled
    /// machines; explicit direct adapters serve disposable development and tests.
    pub workers: WorkerPool,
    /// Durable plugin jobs and their live streams; [`Jobs::disabled`] refuses jobs.
    pub jobs: Jobs,
    /// Exact loopback Host allowlist for this listener.
    pub allowed_hosts: Vec<String>,
}

/// Reference-counted host state; cloning never clones the directory or its grants.
pub type Shared<D = StoredDirectory> = Arc<AppState<D>>;

/// Construct the exact loopback hostnames accepted for a bound listener port.
pub fn allowed_hosts(port: u16) -> Vec<String> {
    vec![
        format!("127.0.0.1:{port}"),
        format!("localhost:{port}"),
        format!("[::1]:{port}"),
    ]
}

/// Build the authenticated API and optional static asset routes.
pub fn router<D: Directory>(state: Shared<D>, web_dir: Option<PathBuf>) -> Router {
    let content = state.store.shared_content();
    let api = Router::new()
        .route("/api/v1/describe", get(describe::<D>))
        .route(
            "/api/v1/account/live",
            get(crate::account_input::events::<D>),
        )
        .route("/api/v1/live", get(crate::live::events::<D>))
        .route("/api/v1/operations", post(operations::<D>))
        .route(
            "/api/v1/jobs/{id}/events",
            get(crate::plugins::job_api::events::<D>),
        )
        .merge(crate::machines::http::routes::<D>())
        .route("/api", any(api_not_found))
        .route("/api/", any(api_not_found))
        .route("/api/{*rest}", any(api_not_found))
        .layer(axum::Extension(content));
    let http_auth = state.directory.http_auth().cloned();
    let mut app = Router::new().merge(api);
    if let Some(service) = &http_auth {
        app = app.merge(crate::account_http::routes(service.clone()));
        app = app.merge(crate::secrets::routes::<D>());
    }

    if let Some(dir) = web_dir {
        app = app.fallback_service(ServeDir::new(dir));
    }
    let app = app
        .layer(middleware::from_fn_with_state(state.clone(), guard::<D>))
        .with_state(state);
    if let Some(service) = http_auth {
        app.layer(middleware::from_fn_with_state(
            service,
            crate::account_http::cookie_middleware,
        ))
    } else {
        app
    }
}

// ------------------------------------------------------------- envelope

/// Operation error envelope with the error's HTTP status.
pub(crate) fn failure(request_id: Option<String>, e: Error) -> Response {
    let status =
        StatusCode::from_u16(e.code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut resp = OperationResponse::<Value>::failure(request_id, e.code, e.message);
    if let (OperationResponse::Failure(f), Some(Value::Object(d))) = (&mut resp, e.details) {
        f.error.details = Some(d.into_iter().collect());
    }
    json_response(status, &resp)
}

/// Serialize `body` as a JSON response with `status`.
pub(crate) fn json_response<T: serde::Serialize>(status: StatusCode, body: &T) -> Response {
    let bytes = serde_json::to_vec(body).expect("serializable");
    let mut r = (status, bytes).into_response();
    r.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    r
}

/// A failure envelope without a request ID, for routes outside `/api/v1/operations`.
/// Cost: one small serialization.
pub(crate) fn error_response(e: Error) -> Response {
    failure(None, e)
}

async fn api_not_found() -> Response {
    failure(None, Error::not_found("unknown API route"))
}

// ------------------------------------------------------------- handlers

async fn describe<D: Directory>(State(state): State<Shared<D>>, req: Request) -> Response {
    let p = req
        .extensions()
        .get::<Identity>()
        .cloned()
        .expect("guard inserts principal");
    let st = state.clone();
    let agent = req
        .extensions()
        .get::<crate::turns::access::Authority>()
        .cloned();
    let scoped = agent.is_some();
    let (principal, operations) = match tokio::task::spawn_blocking(move || {
        let mut principal = st.directory.describe(&p);
        let mut operations = calternal_core::cached_operation_catalog().to_vec();
        if let Some(a) = agent {
            principal.spaces.retain(|s| s.id == a.space);
            let sample =
                serde_json::from_value(serde_json::json!({"space_id":a.space,"job_id":a.job}))
                    .unwrap_or_default();
            operations.retain(|o| a.check(&o.id, &sample).is_ok());
        }
        (principal, operations)
    })
    .await
    {
        Ok(v) => v,
        Err(_) => return failure(None, Error::internal("directory task failed")),
    };
    if !scoped {
        return json_response(
            StatusCode::OK,
            &DescribeReply {
                protocol_version: PROTOCOL_VERSION,
                genui_version: GENUI_VERSION,
                principal,
                operations: operation_catalog_json(),
            },
        );
    }
    let result = calternal_core::DescribeResult {
        protocol_version: PROTOCOL_VERSION,
        genui_version: GENUI_VERSION,
        principal,
        operations,
    };
    json_response(StatusCode::OK, &result)
}

/// The wire form of [`calternal_core::DescribeResult`], with the operation catalog
/// spliced in pre-serialized: the same JSON, without rebuilding 38 schemas per call.
#[derive(serde::Serialize)]
struct DescribeReply {
    protocol_version: u32,
    genui_version: u32,
    principal: calternal_core::Principal,
    operations: &'static serde_json::value::RawValue,
}

/// The operation catalog serialized once per process; it depends only on the build.
/// Cost: first call generates every schema (milliseconds); later calls are free.
fn operation_catalog_json() -> &'static serde_json::value::RawValue {
    static CATALOG: std::sync::OnceLock<Box<serde_json::value::RawValue>> =
        std::sync::OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::value::to_raw_value(&operation_catalog()).expect("catalog serializes")
    })
}

async fn operations<D: Directory>(State(state): State<Shared<D>>, req: Request) -> Response {
    let machines = req
        .extensions()
        .get::<Arc<crate::machines::MachineService>>()
        .cloned();
    let agent = req
        .extensions()
        .get::<crate::turns::access::Authority>()
        .cloned();
    let p = req
        .extensions()
        .get::<Identity>()
        .cloned()
        .expect("guard inserts principal");
    let session = req
        .extensions()
        .get::<Session>()
        .cloned()
        .expect("guard inserts session");
    let ct_ok = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case("application/json")
        })
        .unwrap_or(false);
    if !ct_ok {
        return failure(
            None,
            Error::invalid_input("Content-Type must be application/json"),
        );
    }
    if req.method() != Method::POST {
        return failure(None, Error::invalid_input("POST required"));
    }
    let bytes: Bytes = match axum::body::to_bytes(req.into_body(), MAX_BODY_BYTES + 1).await {
        Ok(b) => b,
        Err(_) => {
            return failure(
                None,
                Error::new(ErrorCode::TooLarge, "request exceeds 2 MiB"),
            );
        }
    };
    let request = match parse_request(&bytes) {
        Ok(r) => r,
        Err(e) => return failure(salvage_request_id(&bytes), e.into()),
    };
    let rid = request.request_id.clone();
    let result = if calternal_core::AGENT_OPERATIONS
        .iter()
        .any(|(id, _, _)| *id == request.operation)
    {
        match machines {
            Some(m) => {
                crate::turns::dispatch(
                    state,
                    m,
                    p,
                    session,
                    agent,
                    &request.request_id,
                    &request.operation,
                    request.input,
                )
                .await
            }
            None => Err(Error::unavailable("machines are not enabled")),
        }
    } else {
        dispatch_scoped(
            state,
            p,
            session,
            &request.request_id,
            &request.operation,
            request.input,
            agent,
        )
        .await
    };
    fault::hit(FaultPoint::BeforeReply);
    match result {
        Ok(data) => json_response(StatusCode::OK, &OperationResponse::success(rid, data)),
        Err(e) => failure(Some(rid), e),
    }
}

/// Echo a request_id from a well-formed envelope whose input was rejected.
fn salvage_request_id(bytes: &[u8]) -> Option<String> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    let obj = v.as_object()?;
    if obj
        .keys()
        .any(|k| !matches!(k.as_str(), "request_id" | "operation" | "input"))
    {
        return None;
    }
    let id = obj.get("request_id")?.as_str()?;
    (!id.is_empty() && id.chars().count() <= 128).then(|| id.to_string())
}
