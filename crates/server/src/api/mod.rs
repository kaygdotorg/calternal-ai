//! HTTP surface: `/api/v1/describe`, `/api/v1/operations`, same-origin static
//! files and application addresses (`--web-dir`), Host/Origin checks, account sessions, optional development bearers
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
    NotesGetInput, NotesUpdateInput, OperationResponse, PROTOCOL_VERSION, PluginInput,
    PluginPreviewResult, PluginResult, PluginState, PluginsListInput, True,
    canonical_request_input, operation_catalog, parse_request,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::auth::{Directory, Identity, StoredDirectory};
use crate::authz::{self, Constraint};
use crate::fault::{self, FaultPoint};
use crate::plugins::worker::Outcome;
use crate::plugins::{
    Call, Jobs, PREVIEW_ITEM_ID, Plugin, PreviewHost, Registry, Session, WorkerPool, activation,
    broker::SpaceHost,
};
use crate::store::{Store, sha256_hex};
use calternal_types::{AccountId, Error, SpaceId};

pub(crate) mod authorization;
pub(crate) mod cursors;
mod dispatch;
mod guard;
mod note_pages;
mod notes;
pub(crate) mod people;
mod plugin_slots;
mod plugins;
mod request;
mod search;
mod search_skills;
mod sharing;

pub(crate) mod appearance;
pub(crate) mod assets;
mod request_action;
mod spaces;
pub mod wallpapers;
mod web;
pub use dispatch::dispatch;
pub(crate) use dispatch::fingerprint;
use dispatch::*;
use guard::*;
use plugins::*;

/// Same-origin development content security policy applied to every response.
pub const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data: blob:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'";

/// Shared host services; `D` is the protected source of current identity and grants.
/// The default preserves fixture integrations; stored directories plug in without
/// changing operation handlers or caching membership in authenticated identities.
pub struct AppState<D: Directory = StoredDirectory> {
    /// Two foreground read slots and up to 32 mutation slots, each isolated
    /// from the listener's unscoped background executor.
    /// Operation declarations select scheduling; no security decision is cached.
    pub blocking: crate::blocking::RequestWorkers,
    /// Per-account input databases (drafts, pending submissions, the account feed).
    pub inputs: crate::account_input::Inputs,
    /// Durable preview store, shared without an outer lock. Content, plugin state
    /// and receipts take only their space's lock; a new item's first catalog route
    /// takes the one catalog writer (see [`Store`]).
    pub store: Store,
    /// Identity and membership directory, read for each authorization. The
    /// stored directory has one bounded auth.db worker for ordered authority
    /// reads, security writes and their recovery publication. Each content
    /// space keeps its own store and journal. See the auth database worker's
    /// documentation for the exact serialized interval.
    pub directory: D,
    /// Atomically published snapshots of trusted plugins and their derived policies.
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

/// Start the background search worker against the live plugin catalog.
/// The worker only holds a weak state reference and exits after the state drops.
/// Startup spawns one thread and performs no content I/O.
pub fn configure_search<D: Directory>(
    state: &Shared<D>,
) -> Result<std::thread::JoinHandle<()>, Error> {
    let owner = Arc::downgrade(state);
    let source: calternal_content::SearchConfigSource = Box::new(move || {
        let state = owner.upgrade()?;
        Some(state.registry.snapshot().search_configuration().clone())
    });
    state
        .store
        .shared_content()
        .configure_search(source, crate::threads::search::decode)
}

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
        .route(
            crate::plugins::signals::PATH,
            post(crate::plugins::signals::webhook::<D>),
        )
        .merge(crate::machines::http::routes::<D>())
        .route(wallpapers::ROUTE, get(wallpapers::image))
        .route(
            crate::plugins::network::http::FRAME_ROUTE,
            get(crate::plugins::network::http::document::<D>),
        )
        .merge(appearance::routes::<D>())
        .route("/api", any(api_not_found))
        .route("/api/", any(api_not_found))
        .route("/api/{*rest}", any(api_not_found))
        .layer(axum::Extension(content));
    let http_auth = state.directory.http_auth().cloned();
    let mut app = Router::new().merge(api);
    if let Some(service) = &http_auth {
        app = app.merge(crate::account_http::routes(service.clone()));
        app = app.merge(sharing::routes::<D>());
        app = app.merge(Router::new().route(
            calternal_core::ACCOUNT_ERASURE_PATH,
            post(crate::account_http::erasure::delete::<D>),
        ));
        app = app.merge(crate::secrets::routes::<D>());
    }

    if let Some(dir) = web_dir {
        app = app.fallback_service(web::assets(dir, &state.registry));
    }
    let app = app
        .layer(middleware::from_fn_with_state(
            state.clone(),
            schedule_http::<D>,
        ))
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

/// HTTP mutations outside the operation endpoint use writer scheduling too.
/// Operations choose their declared class after parsing; authentication remains
/// outside this scope so admission reads never queue as mutations.
async fn schedule_http<D: Directory>(
    State(state): State<Shared<D>>,
    req: Request,
    next: Next,
) -> Response {
    let mutates = req.uri().path() != "/api/v1/operations"
        && !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    state.blocking.scope(mutates, next.run(req)).await
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
    let (principal, operations) = match crate::blocking::spawn(move || {
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

/// Scheduling derives from the same immutable operation declarations as policy.
/// Unknown operations get writer scheduling until normal admission rejects them.
fn operation_mutates(catalog: &crate::plugins::Catalog, op: &str) -> bool {
    catalog
        .owner_of_operation(op)
        .and_then(|p| p.operation(op))
        .map(|p| p.mutates)
        .or_else(|| calternal_core::cached_operation(op).map(|p| p.mutates))
        .unwrap_or(true)
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
    let scheduling_space = request
        .input
        .get("space_id")
        .and_then(Value::as_str)
        .and_then(|s| SpaceId::parse(s).ok())
        .map(|s| s.to_string());
    let thread_account = p.id;
    let state_for_threads = state.clone();
    let thread_scheduler = machines.clone();
    let workers = state.blocking.clone();
    let catalog = state.registry.snapshot();
    let mutates = operation_mutates(&catalog, &request.operation);
    let result = workers
        .scope(mutates, async {
            let mut result =
                if let Some(row) = calternal_core::request_operation(&request.operation) {
                    crate::requests::dispatch(
                        state,
                        machines,
                        p,
                        session,
                        agent,
                        &rid,
                        row,
                        request.input,
                    )
                    .await
                } else if calternal_core::AGENT_OPERATIONS
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
                    dispatch_scoped_inner(
                        state,
                        p,
                        session,
                        &request.request_id,
                        &request.operation,
                        request.input,
                        agent,
                        machines,
                        catalog,
                    )
                    .await
                };
            if request.operation == "threads.stop" {
                if let (Some(machines), Ok(value)) = (&thread_scheduler, &result) {
                    if let Err(error) =
                        crate::turns::finish_thread_stop(&state_for_threads, machines, value).await
                    {
                        result = Err(error);
                    }
                }
            }
            if matches!(
                request.operation.as_str(),
                "threads.send" | "pending.submit" | "pending.confirm"
            ) {
                if let (Some(machines), Ok(value)) = (thread_scheduler, result.as_ref()) {
                    let value = if request.operation.starts_with("pending.") {
                        &value["result"]
                    } else {
                        value
                    };
                    if let (Some(space), Some(id)) = (
                        value["job"]["space_id"].as_str(),
                        value["job"]["id"].as_str(),
                    ) {
                        if scheduling_space
                            .as_ref()
                            .is_none_or(|expected| expected == space)
                        {
                            crate::turns::schedule_thread(
                                &state_for_threads,
                                &machines,
                                space.to_owned(),
                                id.to_owned(),
                                thread_account,
                            );
                        }
                    }
                }
            }
            result
        })
        .await;
    fault::hit(FaultPoint::BeforeReply);
    match result {
        Ok(data) => {
            let mut response =
                json_response(StatusCode::OK, &OperationResponse::success(rid, data));
            if request.operation == "requests.ask" || request.operation == "access.request" {
                response
                    .extensions_mut()
                    .insert(guard::ParkedAcknowledgement);
            }
            response
        }
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
