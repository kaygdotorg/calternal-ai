//! HTTP surface: `/api/v1/describe`, `/api/v1/operations`, same-origin static
//! web assets with SPA fallback, Host/Origin checks, account sessions, optional development bearers
//! and security headers.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Request, State};
use axum::handler::HandlerWithoutStateExt;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use calternal_core::{
    DescribeResult, ErrorCode, GENUI_VERSION, JsonObject, MAX_BODY_BYTES, NoteResult,
    NotesCreateInput, NotesGetInput, NotesHistoryInput, NotesHistoryResult, NotesListInput,
    NotesListResult, NotesUpdateInput, OperationResponse, PROTOCOL_VERSION, PluginInput,
    PluginPreviewResult, PluginResult, PluginState, PluginsListInput, PluginsListResult, True,
    WorkerResource, canonical_request_input, operation_catalog, parse_request,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tower_http::services::ServeDir;

use crate::auth::{Directory, Identity, TokenTable};
use crate::authz::{self, Constraint};
use crate::error::HostError;
use crate::fault::{self, FaultPoint};
use crate::plugins::{LoadedPlugin, NodeRuntime, Registry, run_summarize};
use crate::store::{PluginRow, Store, sha256_hex};

/// Same-origin development content security policy applied to every response.
pub const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'";

/// Fixed fixture text for plugin preview. Never user data.
pub const PREVIEW_FIXTURE: &str = "# Preview fixture\n\nThis is fixed sample text used to preview a plugin.\nIt is not one of your notes.\n";
/// Stable synthetic resource identifier used only for plugin previews.
pub const PREVIEW_RESOURCE_ID: &str = "00000000-0000-4000-8000-000000000000";

/// Shared host services; `D` is the protected source of current identity and grants.
/// The default preserves fixture integrations; stored directories plug in without
/// changing operation handlers or caching membership in authenticated identities.
pub struct AppState<D: Directory = TokenTable> {
    /// Serialized durable preview store; authorization precedes taking this lock.
    pub store: Mutex<Store>,
    /// Identity and membership directory (legacy field name preserves startup callers).
    pub tokens: D,
    /// Immutable registry of trusted development plugins.
    pub registry: Registry,
    /// Optional bounded Node worker runtime.
    pub node: Option<NodeRuntime>,
    /// Exact loopback Host allowlist for this listener.
    pub allowed_hosts: Vec<String>,
}

/// Reference-counted host state; cloning never clones the directory or its grants.
pub type Shared<D = TokenTable> = Arc<AppState<D>>;

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
    let api = Router::new()
        .route("/api/v1/describe", get(describe::<D>))
        .route("/api/v1/operations", post(operations::<D>))
        .route("/api", any(api_not_found))
        .route("/api/", any(api_not_found))
        .route("/api/{*rest}", any(api_not_found));
    let http_auth = state.tokens.http_auth().cloned();
    let mut app = Router::new().merge(api);
    if let Some(service) = &http_auth {
        app = app.merge(crate::account_http::routes(service.clone()));
    }

    app = match web_dir {
        Some(dir) if dir.join("index.html").is_file() => {
            let index = dir.join("index.html");
            let spa = move |uri: axum::http::Uri| {
                let index = index.clone();
                async move { spa_fallback(uri, index).await }
            };
            app.fallback_service(ServeDir::new(dir).fallback(spa.into_service()))
        }
        _ => app.fallback(web_not_built),
    };
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

fn failure(request_id: Option<String>, e: HostError) -> Response {
    let status =
        StatusCode::from_u16(e.code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut resp = OperationResponse::<Value>::failure(request_id, e.code, e.message);
    if let (OperationResponse::Failure(f), Some(Value::Object(d))) = (&mut resp, e.details) {
        f.error.details = Some(d.into_iter().collect());
    }
    json_response(status, &resp)
}

fn json_response<T: serde::Serialize>(status: StatusCode, body: &T) -> Response {
    let bytes = serde_json::to_vec(body).expect("serializable");
    let mut r = (status, bytes).into_response();
    r.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    r
}

async fn api_not_found() -> Response {
    failure(None, HostError::not_found("unknown API route"))
}

/// Client-side routes (e.g. `/notes/<id>`) get index.html; a
/// missing asset (`/assets/...` or any last segment with an extension) is a
/// plain 404, never SPA HTML.
async fn spa_fallback(uri: axum::http::Uri, index: PathBuf) -> Response {
    let path = uri.path();
    let last = path.rsplit('/').next().unwrap_or("");
    if path.starts_with("/assets/") || last.contains('.') {
        return (StatusCode::NOT_FOUND, "not found\n").into_response();
    }
    match tokio::fs::read(&index).await {
        Ok(bytes) => {
            let mut r = (StatusCode::OK, bytes).into_response();
            r.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            );
            r.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            r
        }
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "web UI index missing\n").into_response(),
    }
}

async fn web_not_built() -> Response {
    let mut r = (
        StatusCode::SERVICE_UNAVAILABLE,
        "Calternal-AI development preview: web UI is not built. Run `npm run build:web`, then restart with --web-dir apps/web/dist.\n",
    )
        .into_response();
    r.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    r
}

// --------------------------------------------------------------- guard

fn is_api(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/")
}

/// Host allowlist (DNS-rebinding defense), configured Origin checks, current session
/// or explicitly enabled development bearer auth, and security headers on every response.
/// Successful operation/discovery delivery revalidates the originating credential
/// after the handler finishes, discarding work whose session was revoked meanwhile.
async fn guard<D: Directory>(
    State(state): State<Shared<D>>,
    mut req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let api = is_api(&path);
    let deny = |e: HostError| -> Response {
        if api {
            failure(None, e)
        } else {
            (
                StatusCode::from_u16(e.code.http_status()).unwrap(),
                e.message,
            )
                .into_response()
        }
    };
    let resp = 'resp: {
        let host = req
            .headers()
            .get(header::HOST)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        if !state.allowed_hosts.iter().any(|h| h == host) {
            break 'resp deny(HostError::forbidden("host not allowed"));
        }
        let account_route = path.starts_with("/api/v1/auth/");
        if api
            && !account_route
            && bearer(req.headers()).is_none()
            && let Some(service) = state.tokens.http_auth()
            && req
                .headers()
                .get(header::ORIGIN)
                .is_some_and(|v| v.as_bytes() != service.config.origin_header().as_bytes())
            {
                break 'resp deny(HostError::unauthorized("missing or invalid session"));
            }

        if !account_route && let Some(origin) = req.headers().get(header::ORIGIN) {
            let expected = if bearer(req.headers())
                .and_then(|t| state.tokens.authenticate(t))
                .is_some()
            {
                format!("http://{host}")
            } else {
                state
                    .tokens
                    .http_auth()
                    .map(|s| s.config.origin_header())
                    .unwrap_or_else(|| format!("http://{host}"))
            };
            if origin.to_str().ok() != Some(expected.as_str()) {
                break 'resp deny(HostError::forbidden("cross-origin request denied"));
            }
        }
        if req
            .headers()
            .get("sec-fetch-site")
            .and_then(|v| v.to_str().ok())
            == Some("cross-site")
        {
            break 'resp deny(HostError::forbidden("cross-site request denied"));
        }
        if api && !account_route {
            let session = if let Some(service) = state.tokens.http_auth() {
                crate::account_http::identify(service, req.headers())
                    .await
                    .map(|(id, _)| Identity {
                        id: Arc::from(id.to_string()),
                    })
            } else {
                None
            };
            match session
                .or_else(|| bearer(req.headers()).and_then(|t| state.tokens.authenticate(t)))
            {
                Some(p) => {
                    req.extensions_mut().insert(p);
                }
                None => {
                    break 'resp deny(HostError::unauthorized("missing or invalid session"));
                }
            }
        }
        let credentials = req.headers().clone();
        let admitted = req.extensions().get::<Identity>().cloned();
        let response = next.run(req).await;
        if api && !account_route && response.status().is_success() {
            let session = if let Some(service) = state.tokens.http_auth() {
                crate::account_http::identify(service, &credentials)
                    .await
                    .map(|(id, _)| Identity {
                        id: Arc::from(id.to_string()),
                    })
            } else {
                None
            };
            let current =
                session.or_else(|| bearer(&credentials).and_then(|t| state.tokens.authenticate(t)));
            if current.as_ref().map(|p| &p.id) != admitted.as_ref().map(|p| &p.id)
                || current.is_none()
            {
                break 'resp deny(HostError::unauthorized("missing or invalid session"));
            }
        }
        response
    };
    let mut resp = resp;
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    if api {
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    resp
}

fn bearer(headers: &HeaderMap) -> Option<&[u8]> {
    let v = headers.get(header::AUTHORIZATION)?.as_bytes();
    let rest = v.strip_prefix(b"Bearer ")?;
    if rest.is_empty() { None } else { Some(rest) }
}

// ------------------------------------------------------------- handlers

async fn describe<D: Directory>(State(state): State<Shared<D>>, req: Request) -> Response {
    let p = req
        .extensions()
        .get::<Identity>()
        .cloned()
        .expect("guard inserts principal");
    let st = state.clone();
    let principal = match tokio::task::spawn_blocking(move || st.tokens.describe(&p)).await {
        Ok(p) => p,
        Err(_) => return failure(None, HostError::internal("directory task failed")),
    };
    let result = DescribeResult {
        protocol_version: PROTOCOL_VERSION,
        genui_version: GENUI_VERSION,
        principal,
        operations: operation_catalog(),
    };
    json_response(StatusCode::OK, &result)
}

async fn operations<D: Directory>(State(state): State<Shared<D>>, req: Request) -> Response {
    let p = req
        .extensions()
        .get::<Identity>()
        .cloned()
        .expect("guard inserts principal");
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
            HostError::invalid_input("Content-Type must be application/json"),
        );
    }
    if req.method() != Method::POST {
        return failure(None, HostError::invalid_input("POST required"));
    }
    let bytes: Bytes = match axum::body::to_bytes(req.into_body(), MAX_BODY_BYTES + 1).await {
        Ok(b) => b,
        Err(_) => {
            return failure(
                None,
                HostError::new(ErrorCode::TooLarge, "request exceeds 2 MiB"),
            );
        }
    };
    let request = match parse_request(&bytes) {
        Ok(r) => r,
        Err(e) => return failure(salvage_request_id(&bytes), e.into()),
    };
    let rid = request.request_id.clone();
    let result = dispatch(
        state,
        p,
        &request.request_id,
        &request.operation,
        request.input,
    )
    .await;
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

fn decode<T: DeserializeOwned>(input: JsonObject) -> Result<T, HostError> {
    serde_json::from_value(Value::Object(input.into_iter().collect()))
        .map_err(|_| HostError::invalid_input("input does not match operation schema"))
}

fn fingerprint(op: &str, input: &JsonObject) -> String {
    sha256_hex(format!("{op}\n{}", canonical_request_input(input)).as_bytes())
}

async fn blocking<D: Directory, T: Send + 'static>(
    state: &Shared<D>,
    f: impl FnOnce(&mut Store) -> Result<T, HostError> + Send + 'static,
) -> Result<T, HostError> {
    let state = state.clone();
    tokio::task::spawn_blocking(move || {
        let mut store = state
            .store
            .lock()
            .map_err(|_| HostError::internal("store poisoned"))?;
        f(&mut store)
    })
    .await
    .map_err(|_| HostError::internal("storage task failed"))?
}

fn to_value<T: serde::Serialize>(v: T) -> Result<Value, HostError> {
    serde_json::to_value(v).map_err(|_| HostError::internal("encode"))
}

/// Dispatch through the mandatory policy gate before any handler or receipt replay.
/// The caller supplies a verified identity; supplied fixture grants are discarded
/// by conversion for legacy development callers. Each attempt reads current capability sets.
/// Summarize additionally rechecks membership after the worker before delivery.
/// Stored directory lookups run on blocking tasks; identities contain no grants.
pub async fn dispatch<D: Directory>(
    state: Shared<D>,
    p: impl Into<Identity>,
    request_id: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, HostError> {
    let p = p.into();
    let mut input = input;
    if op == "resolve" || authz::policy(op).is_some_and(|p| p.scope == authz::Scope::Note) {
        calternal_core::validate_operation_input(op, &input)?;
        let kind = if op == "resolve" {
            input.get("kind").and_then(Value::as_str).unwrap_or("")
        } else {
            "notes"
        }
        .to_owned();
        let id = input
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let op_required = authz::policy(op)
            .map(|p| p.required)
            .unwrap_or(authz::Capability::SpaceRead);
        let st = state.clone();
        let account = p.clone();
        let resolved = tokio::task::spawn_blocking(move || {
            let store = st
                .store
                .lock()
                .map_err(|_| HostError::internal("store poisoned"))?;
            crate::resolve::resolve_for(&st.tokens, &store, &account, &kind, &id, op_required)
        })
        .await
        .map_err(|_| HostError::internal("resolver task failed"))??;
        if op == "resolve" {
            return to_value(resolved);
        }
        if let Some(hint) = input
            .get("space_id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            && state.tokens.wire_space(hint) != resolved.space_id {
                return Err(HostError::not_found("item not found"));
            }
        input.insert("space_id".into(), Value::String(resolved.space_id));
    }
    let Some(policy) = authz::policy(op) else {
        return Err(if state.registry.owner_of_operation(op).is_some() {
            HostError::unavailable("operation has no host binding in this preview")
        } else {
            HostError::not_found("unknown operation")
        });
    };
    let st = state.clone();
    let identity = p.clone();
    let checked = input.clone();
    tokio::task::spawn_blocking(move || policy.authorize(&st.tokens, &identity, &checked))
        .await
        .map_err(|_| HostError::internal("directory task failed"))??;
    if op.starts_with("spaces.") {
        let st = state.clone();
        let rid = request_id.to_owned();
        let operation = op.to_owned();
        return tokio::task::spawn_blocking(move || {
            // Serialize receipt namespace checks across directory/content DBs.
            // Each mutation still commits its own change and receipt atomically.
            let store = st
                .store
                .lock()
                .map_err(|_| HostError::internal("store poisoned"))?;
            if !matches!(operation.as_str(), "spaces.list" | "spaces.members")
                && store.receipt(&p.id, &rid)?.is_some()
            {
                return Err(HostError::conflict(
                    "request_id was already used with different input",
                ));
            }
            st.tokens.space_operation(&p, &rid, &operation, &input)
        })
        .await
        .map_err(|_| HostError::internal("directory task failed"))?;
    }
    let mut input = input;
    if let Some(Value::String(space)) = input.get_mut("space_id") {
        *space = state.tokens.content_space(space);
    }
    if policy.constraint == Constraint::PluginInvocation {
        let i: NotesGetInput = decode(input)?;
        let lp = state
            .registry
            .owner_of_operation(op)
            .ok_or_else(|| HostError::not_found("unknown operation"))?
            .clone();
        let result = summarize(state.clone(), lp, &i).await?;
        let st = state.clone();
        let space = i.space_id.clone();
        tokio::task::spawn_blocking(move || policy.authorize_space(&st.tokens, &p, &space))
            .await
            .map_err(|_| HostError::internal("directory task failed"))??;
        return Ok(wire_spaces(&state.tokens, result));
    }
    let delivery_space = input
        .get("space_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let value = dispatch_checked(state.clone(), p.clone(), request_id, op, input).await?;
    if let Some(space) = delivery_space {
        let st = state.clone();
        tokio::task::spawn_blocking(move || policy.authorize_space(&st.tokens, &p, &space))
            .await
            .map_err(|_| HostError::internal("directory task failed"))??;
    }
    Ok(wire_spaces(&state.tokens, value))
}

// Expose UUIDs while the development store retains its explicit root bindings.
fn wire_spaces(directory: &impl Directory, mut value: Value) -> Value {
    match &mut value {
        Value::Object(object) => {
            for (key, item) in object {
                if key == "space_id" {
                    if let Value::String(space) = item {
                        *space = directory.wire_space(space);
                    }
                } else {
                    *item = wire_spaces(directory, item.take());
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                *item = wire_spaces(directory, item.take());
            }
        }
        _ => {}
    }
    value
}

/// Private handler entry: called exclusively after the dispatch policy gate.
async fn dispatch_checked<D: Directory>(
    state: Shared<D>,
    p: Identity,
    request_id: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, HostError> {
    match op {
        "notes.list" => {
            let i: NotesListInput = decode(input)?;
            let notes = blocking(&state, move |s| s.list(&i.space_id, 100)).await?;
            to_value(NotesListResult { notes })
        }
        "notes.get" => {
            let i: NotesGetInput = decode(input)?;
            let note = blocking(&state, move |s| s.get(&i.space_id, &i.id)).await?;
            to_value(NoteResult { note })
        }
        "notes.history" => {
            let i: NotesHistoryInput = decode(input)?;
            let limit = i.limit.unwrap_or(20);
            let (versions, next_cursor) = blocking(&state, move |s| {
                s.history(&i.space_id, &i.id, limit, i.cursor.as_deref())
            })
            .await?;
            to_value(NotesHistoryResult {
                versions,
                next_cursor,
            })
        }
        "notes.create" | "notes.update" => mutate_note(state, p, request_id, op, input).await,
        "plugins.list" => {
            let i: PluginsListInput = decode(input)?;
            let st = state.clone();
            let plugins = blocking(&state, move |s| {
                st.registry
                    .plugins
                    .iter()
                    .map(|lp| plugin_state(s, &i.space_id, lp))
                    .collect::<Result<Vec<_>, _>>()
            })
            .await?;
            to_value(PluginsListResult { plugins })
        }
        "plugins.enable" | "plugins.disable" => {
            let fp = fingerprint(op, &input);
            let i: PluginInput = decode(input)?;
            let enable = op == "plugins.enable";
            let st = state.clone();
            let (rid, op, principal) = (request_id.to_string(), op.to_string(), p.id);
            blocking(&state, move |s| {
                authz::policy(&op).expect("bound policy").authorize_space(
                    &st.tokens,
                    &Identity {
                        id: principal.clone(),
                    },
                    &i.space_id,
                )?;
                if st.tokens.has_space_receipt(
                    &Identity {
                        id: principal.clone(),
                    },
                    &rid,
                )? {
                    return Err(HostError::conflict(
                        "request_id was already used with different input",
                    ));
                }
                if let Some(v) = replay_receipt(s, &principal, &rid, &op, &fp)? {
                    return Ok(v);
                }
                let lp = st
                    .registry
                    .get(&i.plugin_id)
                    .ok_or_else(|| HostError::not_found("plugin is not registered"))?;
                if enable {
                    if let Some(why) = &lp.unavailable {
                        return Err(HostError::unavailable(format!(
                            "plugin cannot be enabled: {why}"
                        )));
                    }
                    // Dependencies must be enabled in this space first.
                    for dep in &lp.manifest.dependencies {
                        let dlp = st
                            .registry
                            .get(&dep.id)
                            .ok_or_else(|| HostError::unavailable("dependency missing"))?;
                        if !plugin_state(s, &i.space_id, dlp)?.enabled {
                            return Err(HostError::conflict(format!(
                                "dependency {} is disabled",
                                dep.id
                            )));
                        }
                    }
                } else {
                    // Refuse to disable a plugin an enabled dependent needs.
                    for other in &st.registry.plugins {
                        if other
                            .manifest
                            .dependencies
                            .iter()
                            .any(|d| d.id == lp.manifest.id)
                            && plugin_state(s, &i.space_id, other)?.enabled
                        {
                            return Err(HostError::conflict(format!(
                                "plugin {} depends on it; disable that first",
                                other.manifest.id
                            )));
                        }
                    }
                }
                let manifest = lp.manifest.clone();
                let (_, value) = s.set_plugin_enabled(
                    &principal,
                    &rid,
                    &op,
                    &fp,
                    &i.space_id,
                    &lp.manifest.id,
                    default_row(lp),
                    enable,
                    |row| {
                        serde_json::to_value(PluginResult {
                            plugin: PluginState {
                                manifest,
                                enabled: row.enabled,
                                generation: row.generation,
                            },
                        })
                        .expect("encode")
                    },
                )?;
                Ok(value)
            })
            .await
        }
        "plugins.preview" => {
            let i: PluginInput = decode(input)?;
            let lp = state
                .registry
                .get(&i.plugin_id)
                .ok_or_else(|| HostError::not_found("plugin is not registered"))?
                .clone();
            // Preview never activates and never touches user data: the worker
            // sees only fixed fixture text.
            let view = if lp.worker.is_some() {
                let node = state
                    .node
                    .as_ref()
                    .ok_or_else(|| HostError::unavailable("Node runtime unavailable"))?;
                let r = run_summarize(
                    node,
                    &lp,
                    WorkerResource {
                        id: PREVIEW_RESOURCE_ID.into(),
                        space_id: i.space_id.clone(),
                        revision: "preview-fixture".into(),
                        title: "Preview fixture".into(),
                        markdown: PREVIEW_FIXTURE.into(),
                    },
                )
                .await?;
                r.view
            } else {
                lp.manifest
                    .views
                    .first()
                    .map(|v| v.view.clone())
                    .ok_or_else(|| HostError::not_found("plugin has no views"))?
            };
            to_value(PluginPreviewResult {
                view,
                preview: True,
            })
        }
        _ => Err(HostError::unavailable(
            "operation has no host binding in this preview",
        )),
    }
}

/// Retry handling shared by every mutation: same principal + request_id with
/// the same canonical fingerprint returns the stored result (finishing any
/// pending materialization); a different fingerprint is a conflict.
fn replay_receipt(
    s: &mut Store,
    principal: &str,
    rid: &str,
    op: &str,
    fp: &str,
) -> Result<Option<Value>, HostError> {
    let Some(r) = s.receipt(principal, rid)? else {
        return Ok(None);
    };
    if r.operation != op || r.fingerprint != fp {
        return Err(
            HostError::conflict("request_id was already used with different input")
                .with_details(serde_json::json!({ "reason": "request_id_reused" })),
        );
    }
    if let Some(n) = &r.note_id {
        s.materialize(n).map_err(|e| {
            if e.code == ErrorCode::Conflict {
                e
            } else {
                HostError::unavailable("committed but not yet materialized; retry")
            }
        })?;
    }
    Ok(Some(r.result))
}

fn default_row(lp: &LoadedPlugin) -> PluginRow {
    PluginRow {
        enabled: lp.entry.default_enabled && lp.unavailable.is_none(),
        generation: 1,
    }
}

fn plugin_state(s: &Store, space: &str, lp: &LoadedPlugin) -> Result<PluginState, HostError> {
    let row = s
        .plugin_row(space, &lp.manifest.id)?
        .unwrap_or_else(|| default_row(lp));
    Ok(PluginState {
        manifest: lp.manifest.clone(),
        enabled: row.enabled && lp.unavailable.is_none(),
        generation: row.generation,
    })
}

async fn summarize<D: Directory>(
    state: Shared<D>,
    lp: LoadedPlugin,
    i: &NotesGetInput,
) -> Result<Value, HostError> {
    let node = state
        .node
        .clone()
        .ok_or_else(|| HostError::unavailable("Node runtime unavailable"))?;
    let (space, id) = (i.space_id.clone(), i.id.clone());
    let lp2 = lp.clone();
    // Authorized canonical read + generation capture under the same lock.
    let (note, generation) = blocking(&state, move |s| {
        let ps = plugin_state(s, &space, &lp2)?;
        if !ps.enabled {
            return Err(HostError::forbidden("plugin is disabled in this space"));
        }
        Ok((s.get(&space, &id)?, ps.generation))
    })
    .await?;
    let result = run_summarize(
        &node,
        &lp,
        WorkerResource {
            id: note.id.clone(),
            space_id: note.space_id.clone(),
            revision: note.revision.clone(),
            title: note.title.clone(),
            markdown: note.markdown,
        },
    )
    .await?;
    // Reauthorize delivery: plugin must still be enabled at the same generation.
    let space = i.space_id.clone();
    let lp3 = lp.clone();
    let still = blocking(&state, move |s| plugin_state(s, &space, &lp3)).await?;
    if !still.enabled || still.generation != generation {
        return Err(HostError::conflict(
            "plugin state changed during invocation; result discarded",
        )
        .with_details(serde_json::json!({ "reason": "plugin_generation_changed" })));
    }
    to_value(result)
}

async fn mutate_note<D: Directory>(
    state: Shared<D>,
    p: Identity,
    request_id: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, HostError> {
    let fp = fingerprint(op, &input);
    let (create, update) = if op == "notes.create" {
        let i: NotesCreateInput = decode(input)?;
        (Some(i), None)
    } else {
        let i: NotesUpdateInput = decode(input)?;
        (None, Some(i))
    };
    let rid = request_id.to_string();
    let op = op.to_string();
    let principal = p.id;
    let st = state.clone();
    let space = create
        .as_ref()
        .map(|i| i.space_id.clone())
        .or_else(|| update.as_ref().map(|i| i.space_id.clone()))
        .expect("note scope");
    blocking(&state, move |s| {
        authz::policy(&op).expect("bound policy").authorize_space(
            &st.tokens,
            &Identity {
                id: principal.clone(),
            },
            &space,
        )?;
        if st.tokens.has_space_receipt(
            &Identity {
                id: principal.clone(),
            },
            &rid,
        )? {
            return Err(HostError::conflict(
                "request_id was already used with different input",
            ));
        }
        if let Some(v) = replay_receipt(s, &principal, &rid, &op, &fp)? {
            return Ok(v);
        }
        let note = if let Some(i) = create {
            s.create(
                &principal,
                &rid,
                &op,
                &fp,
                &i.space_id,
                &i.title,
                &i.markdown,
            )?
        } else {
            let i = update.expect("update input");
            s.update(
                &principal,
                &rid,
                &op,
                &fp,
                &i.space_id,
                &i.id,
                &i.base_revision,
                i.title.as_deref(),
                &i.markdown,
            )?
        };
        to_value(NoteResult { note })
    })
    .await
}
