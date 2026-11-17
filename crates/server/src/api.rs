//! HTTP surface: `/api/v1/describe`, `/api/v1/operations`, same-origin static
//! files (`--web-dir`, no SPA fallback), Host/Origin checks, account sessions, optional development bearers
//! and security headers.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

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
    ErrorCode, GENUI_VERSION, JsonObject, MAX_BODY_BYTES, NoteResult,
    NotesCreateInput, NotesGetInput, NotesHistoryInput, NotesHistoryResult, NotesListInput,
    NotesListResult, NotesUpdateInput, OperationResponse, PROTOCOL_VERSION, PluginInput,
    PluginPreviewResult, PluginResult, PluginState, PluginsListInput, PluginsListResult, True,
    canonical_request_input, operation_catalog, parse_request,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tower_http::services::ServeDir;

use crate::auth::{Directory, Identity, TokenTable};
use crate::authz::{self, Constraint};
use calternal_types::{AccountId, Error, SpaceId};
use crate::fault::{self, FaultPoint};
use crate::plugins::worker::Outcome;
use crate::plugins::{
    Call, Jobs, PREVIEW_ITEM_ID, Plugin, PreviewHost, Registry, Session, WorkerPool, activation,
    broker::SpaceHost,
};
use crate::store::{PluginRow, Store, sha256_hex};

/// Same-origin development content security policy applied to every response.
pub const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'";

/// Shared host services; `D` is the protected source of current identity and grants.
/// The default preserves fixture integrations; stored directories plug in without
/// changing operation handlers or caching membership in authenticated identities.
pub struct AppState<D: Directory = TokenTable> {
    /// Durable preview store, shared without an outer lock: work in one space never
    /// waits for another (see [`Store`] for the locks it takes).
    pub store: Store,
    /// Identity and membership directory (legacy field name preserves startup callers).
    pub tokens: D,
    /// Immutable registry of trusted plugins and their manifest-derived policy rows.
    pub registry: Registry,
    /// Bounded pool of long-lived (plugin, space) workers; refuses calls without Node.
    pub workers: WorkerPool,
    /// Durable plugin jobs and their live streams; [`Jobs::disabled`] refuses jobs.
    pub jobs: Jobs,
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
        .route(
            "/api/v1/jobs/{id}/events",
            get(crate::plugins::job_api::events::<D>),
        )
        .merge(crate::machines::http::routes::<D>())
        .route("/api", any(api_not_found))
        .route("/api/", any(api_not_found))
        .route("/api/{*rest}", any(api_not_found));
    let http_auth = state.tokens.http_auth().cloned();
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
    let deny = |e: Error| -> Response {
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
            break 'resp deny(Error::forbidden("host not allowed"));
        }
        // Machine protocol routes authenticate the machine themselves.
        let account_route = path.starts_with("/api/v1/auth/")
            || path.starts_with(calternal_machine::protocol::PATH_PREFIX);
        if api
            && !account_route
            && bearer(req.headers()).is_none()
            && let Some(service) = state.tokens.http_auth()
            && req
                .headers()
                .get(header::ORIGIN)
                .is_some_and(|v| v.as_bytes() != service.config.origin_header().as_bytes())
        {
            break 'resp deny(Error::unauthorized("missing or invalid session"));
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
                break 'resp deny(Error::forbidden("cross-origin request denied"));
            }
        }
        if req
            .headers()
            .get("sec-fetch-site")
            .and_then(|v| v.to_str().ok())
            == Some("cross-site")
        {
            break 'resp deny(Error::forbidden("cross-site request denied"));
        }
        if api && !account_route {
            // The installation travels with the request so plugin work can recheck it.
            let session = if let Some(service) = state.tokens.http_auth() {
                crate::account_http::identify(service, req.headers())
                    .await
                    .map(|(id, installation)| (Identity { id }, Session::installation(id, installation)))
            } else {
                None
            };
            match session.or_else(|| {
                bearer(req.headers())
                    .and_then(|t| state.tokens.authenticate(t))
                    .map(|p| (p, Session::Static))
            }) {
                Some((p, session)) => {
                    req.extensions_mut().insert(p);
                    req.extensions_mut().insert(session);
                }
                None => {
                    break 'resp deny(Error::unauthorized("missing or invalid session"));
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
                    .map(|(id, _)| Identity { id })
            } else {
                None
            };
            let current =
                session.or_else(|| bearer(&credentials).and_then(|t| state.tokens.authenticate(t)));
            if current.as_ref().map(|p| &p.id) != admitted.as_ref().map(|p| &p.id)
                || current.is_none()
            {
                break 'resp deny(Error::unauthorized("missing or invalid session"));
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
        Err(_) => return failure(None, Error::internal("directory task failed")),
    };
    let result = DescribeReply {
        protocol_version: PROTOCOL_VERSION,
        genui_version: GENUI_VERSION,
        principal,
        operations: operation_catalog_json(),
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
    let result = dispatch(
        state,
        p,
        session,
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

fn decode<T: DeserializeOwned>(input: JsonObject) -> Result<T, Error> {
    serde_json::from_value(Value::Object(input.into_iter().collect()))
        .map_err(|_| Error::invalid_input("input does not match operation schema"))
}

fn fingerprint(op: &str, input: &JsonObject) -> String {
    sha256_hex(format!("{op}\n{}", canonical_request_input(input)).as_bytes())
}

async fn blocking<D: Directory, T: Send + 'static>(
    state: &Shared<D>,
    f: impl FnOnce(&Store) -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    let state = state.clone();
    tokio::task::spawn_blocking(move || f(&state.store))
    .await
    .map_err(|_| Error::internal("storage task failed"))?
}

/// A `space_id` that already passed authorization. Cost: one 36-byte parse.
fn space_of(text: &str) -> Result<SpaceId, Error> {
    SpaceId::parse(text).map_err(|_| Error::not_found("space not found"))
}

fn to_value<T: serde::Serialize>(v: T) -> Result<Value, Error> {
    serde_json::to_value(v).map_err(|_| Error::internal("encode"))
}

/// Dispatch through the mandatory policy gate before any handler or receipt replay.
/// The caller supplies a verified identity; supplied fixture grants are discarded
/// by conversion for legacy development callers. Each attempt reads current capability sets.
/// Plugin operations carry `session` into every worker host call, which rechecks it;
/// in-process callers without a revocable sign-in pass [`Session::Static`].
/// Summarize additionally rechecks membership after the worker before delivery.
/// Stored directory lookups run on blocking tasks; identities contain no grants.
pub async fn dispatch<D: Directory>(
    state: Shared<D>,
    p: Identity,
    session: Session,
    request_id: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let mut input = input;
    let item_scoped = state
        .registry
        .policy(op)
        .is_some_and(|p| p.scope == authz::Scope::Note);
    if op == "resolve" || item_scoped {
        // A plugin operation names its target in the manifest; host item operations
        // and `resolve` carry the item ID in `id`.
        let target = state
            .registry
            .owner_of_operation(op)
            .and_then(|p| p.target(op));
        let field = target.map_or("id", |t| t.field.as_str());
        if calternal_core::is_known_operation(op) {
            calternal_core::validate_operation_input(op, &input)?;
        } else {
            validate_item_input(&input, field)?;
        }
        let kind = if op == "resolve" {
            input.get("kind").and_then(Value::as_str).unwrap_or("")
        } else {
            target.map_or("notes", |t| t.kind.as_str())
        }
        .to_owned();
        let id = input
            .get(field)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let op_required = state
            .registry
            .policy(op)
            .map(|p| p.required)
            .unwrap_or(calternal_types::Capability::SpaceRead);
        let st = state.clone();
        let account = p.clone();
        let resolved = tokio::task::spawn_blocking(move || {
            crate::resolve::resolve_for(&st.tokens, &st.store, &account, &kind, &id, op_required)
        })
        .await
        .map_err(|_| Error::internal("resolver task failed"))??;
        if op == "resolve" {
            return to_value(resolved);
        }
        if let Some(hint) = input
            .get("space_id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            && hint != resolved.space_id
        {
            return Err(Error::not_found("item not found"));
        }
        input.insert("space_id".into(), Value::String(resolved.space_id));
    }
    // A job's space comes from the job store, never from the caller.
    let job_scoped = state
        .registry
        .policy(op)
        .is_some_and(|p| p.scope == authz::Scope::Job);
    if job_scoped {
        calternal_core::validate_operation_input(op, &input)?;
        let space = crate::plugins::job_api::route(&state, &input)?;
        input.insert("space_id".into(), Value::String(space));
    }
    // One authorization path: a host row or a row derived from a plugin manifest.
    // Without either, answer as a hidden target would, so a caller cannot tell an
    // unregistered plugin operation from an item or space it may not see.
    let Some(policy) = state.registry.policy(op).cloned() else {
        return Err(Error::not_found(if input.contains_key("id") {
            "item not found"
        } else if input.contains_key("space_id") {
            "space not found"
        } else {
            "unknown operation"
        }));
    };
    let st = state.clone();
    let identity = p.clone();
    let checked = input.clone();
    let admission = policy.clone();
    tokio::task::spawn_blocking(move || admission.authorize(&st.tokens, &identity, &checked))
        .await
        .map_err(|_| Error::internal("directory task failed"))??;
    if job_scoped {
        let space = match input.remove("space_id") {
            Some(Value::String(space)) => space,
            _ => return Err(Error::not_found("job not found")),
        };
        return crate::plugins::job_api::dispatch(state, p, op, space, input).await;
    }
    if op.starts_with("secrets.") {
        return crate::secrets::dispatch(state, p, policy, input).await;
    }
    if op.starts_with("spaces.") {
        let st = state.clone();
        let rid = request_id.to_owned();
        let operation = op.to_owned();
        // Directory changes keep their own receipts in auth.db.
        return tokio::task::spawn_blocking(move || {
            st.tokens.space_operation(&p, &rid, &operation, &input)
        })
        .await
        .map_err(|_| Error::internal("directory task failed"))?;
    }
    if policy.constraint == Constraint::PluginInvocation {
        return invoke_plugin(state, p, session, request_id, op, policy, input).await;
    }
    let delivery_space = input
        .get("space_id")
        .and_then(Value::as_str)
        .and_then(|s| SpaceId::parse(s).ok());
    let value = dispatch_checked(state.clone(), p.clone(), request_id, op, input).await?;
    if let Some(space) = delivery_space {
        let st = state.clone();
        tokio::task::spawn_blocking(move || policy.authorize_space(&st.tokens, &p, space))
            .await
            .map_err(|_| Error::internal("directory task failed"))??;
    }
    Ok(value)
}

/// Private handler entry: called exclusively after the dispatch policy gate.
async fn dispatch_checked<D: Directory>(
    state: Shared<D>,
    p: Identity,
    request_id: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    match op {
        "notes.list" => {
            let i: NotesListInput = decode(input)?;
            let space = space_of(&i.space_id)?;
            let notes = blocking(&state, move |s| s.list(space, 100)).await?;
            to_value(NotesListResult { notes })
        }
        "notes.get" => {
            let i: NotesGetInput = decode(input)?;
            let space = space_of(&i.space_id)?;
            let note = blocking(&state, move |s| s.get(space, &i.id)).await?;
            to_value(NoteResult { note })
        }
        "notes.history" => {
            let i: NotesHistoryInput = decode(input)?;
            let limit = i.limit.unwrap_or(20);
            let space = space_of(&i.space_id)?;
            let (versions, next_cursor) = blocking(&state, move |s| {
                s.history(space, &i.id, limit, i.cursor.as_deref())
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
            let space = space_of(&i.space_id)?;
            let st = state.clone();
            let plugins = blocking(&state, move |s| {
                st.registry
                    .plugins()
                    .iter()
                    .map(|plugin| plugin_state(s, space, plugin))
                    .collect::<Result<Vec<_>, _>>()
            })
            .await?;
            to_value(PluginsListResult { plugins })
        }
        "plugins.enable" | "plugins.disable" => {
            let fp = fingerprint(op, &input);
            let i: PluginInput = decode(input)?;
            let space = space_of(&i.space_id)?;
            let enable = op == "plugins.enable";
            let st = state.clone();
            let (rid, op, principal) = (request_id.to_string(), op.to_string(), p.id);
            blocking(&state, move |s| {
                st.registry
                    .policy(&op)
                    .expect("bound policy")
                    .authorize_space(&st.tokens, &Identity { id: principal }, space)?;
                if let Some(v) = replay_receipt(s, principal, space, &rid, &op, &fp)? {
                    return Ok(v);
                }
                let plugin = st
                    .registry
                    .get(&i.plugin_id)
                    .ok_or_else(|| Error::not_found("plugin is not registered"))?;
                if enable {
                    if let Some(why) = &plugin.unavailable {
                        return Err(Error::unavailable(format!(
                            "plugin cannot be enabled: {why}"
                        )));
                    }
                    // Provision before activation commits, so a failure leaves it disabled.
                    let collections: Vec<_> = plugin
                        .manifest
                        .collections
                        .iter()
                        .map(|c| (c.kind.clone(), c.id.clone(), c.name.clone()))
                        .collect();
                    s.provision_plugin(
                        space,
                        principal,
                        &plugin.kind_registrations(),
                        &collections,
                    )?;
                }
                let manifest = plugin.wire.clone();
                let (_, value) = s.set_plugin_enabled(
                    principal,
                    &rid,
                    &op,
                    &fp,
                    space,
                    &plugin.manifest.id,
                    default_row(plugin),
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
            let plugin = state
                .registry
                .get(&i.plugin_id)
                .ok_or_else(|| Error::not_found("plugin is not registered"))?
                .clone();
            preview(state, p, plugin, space_of(&i.space_id)?).await
        }
        _ => Err(Error::unavailable(
            "operation has no host binding in this preview",
        )),
    }
}

/// Retry handling shared by every mutation in an authorized space: the same
/// (account, space, request_id) with the same canonical fingerprint returns the
/// stored result (finishing any pending materialization); a different fingerprint
/// is a conflict. Another account or space never sees this receipt.
fn replay_receipt(
    s: &Store,
    principal: AccountId,
    space: SpaceId,
    rid: &str,
    op: &str,
    fp: &str,
) -> Result<Option<Value>, Error> {
    let Some(r) = s.receipt(principal, space, rid)? else {
        return Ok(None);
    };
    if r.operation != op || r.fingerprint != fp {
        return Err(Error::request_reused());
    }
    if let Some(n) = &r.note_id {
        s.materialize(n).map_err(|e| {
            if e.code == ErrorCode::Conflict {
                e
            } else {
                Error::unavailable("committed but not yet materialized; retry")
            }
        })?;
    }
    Ok(Some(r.result))
}

fn default_row(plugin: &Plugin) -> PluginRow {
    crate::plugins::default_row(plugin)
}

fn plugin_state(s: &Store, space: SpaceId, plugin: &Plugin) -> Result<PluginState, Error> {
    let row = activation(s, space, plugin)?;
    Ok(PluginState {
        manifest: plugin.wire.clone(),
        enabled: row.enabled,
        generation: row.generation,
    })
}

/// Host-checked part of an item-targeted plugin operation without a core-contract
/// entry: `field` holds a bounded string ID and the optional `space_id` hint is a
/// string. Every other property is checked by the declared input schema before the
/// worker sees it. Cost: O(1) map lookups.
fn validate_item_input(input: &JsonObject, field: &str) -> Result<(), Error> {
    let shape = Error::invalid_input("input does not match operation schema");
    if !input
        .get(field)
        .and_then(Value::as_str)
        .is_some_and(|id| !id.is_empty() && id.len() <= 128)
        || input.get("space_id").is_some_and(|v| !v.is_string())
    {
        return Err(shape);
    }
    Ok(())
}

/// Map a worker outcome onto the wire, after the caller's delivery checks.
fn outcome_value(outcome: Outcome) -> Result<Value, Error> {
    match outcome {
        Outcome::Complete(value) => Ok(value),
        Outcome::InvalidInput => Err(Error::invalid_input(
            "input does not match operation schema",
        )),
        Outcome::Refused => Err(Error::unavailable("extension request was refused")),
        Outcome::Failed(e) => Err(e),
        Outcome::Cancelled => Err(Error::unavailable("extension request was cancelled")),
    }
}

/// A portable view in a plugin result must pass the same checks as the v0 host:
/// bounded tree, unique IDs, and actions only to declared read-only operations.
/// Cost: one view decode and validation when the result carries `view`.
fn check_view(plugin: &Plugin, value: &Value) -> Result<(), Error> {
    let Some(view) = value.get("view") else {
        return Ok(());
    };
    let rejected = || {
        tracing::warn!(plugin = %plugin.manifest.id, "worker view rejected");
        Error::unavailable("extension returned an invalid view")
    };
    let view: calternal_core::GenUiView =
        serde_json::from_value(view.clone()).map_err(|_| rejected())?;
    calternal_core::validate_extension_view(&view, &plugin.wire).map_err(|_| rejected())
}

/// Invoke a manifest-declared operation on its (plugin, space) worker.
///
/// A mutating operation is receipted like every other write: a retry with the same
/// request ID and input returns the original result without invoking the worker, and
/// different input under that ID is a conflict. The worker's own `write_item` calls
/// carry per-call receipts, so a retry after an undelivered result replays them.
/// Cost for mutating operations: one receipt lookup before, one journal append after.
///
/// Admission (already done by the caller) checked the manifest-derived policy.
/// Here: the plugin must be enabled; its activation generation is captured; every
/// host call and the delivery are rechecked by the broker; then, as before, the
/// result is discarded if the generation changed (conflict) or the caller's space
/// access no longer holds. The HTTP guard rechecks the session on delivery.
async fn invoke_plugin<D: Directory>(
    state: Shared<D>,
    p: Identity,
    session: Session,
    request_id: &str,
    op: &str,
    policy: authz::Policy,
    mut input: JsonObject,
) -> Result<Value, Error> {
    let plugin = state
        .registry
        .owner_of_operation(op)
        .ok_or_else(|| Error::not_found("unknown operation"))?
        .clone();
    let fp = fingerprint(op, &input);
    let mutates = plugin.operation(op).is_some_and(|o| o.mutates);
    let space = match input.remove("space_id") {
        Some(Value::String(space)) => space_of(&space)?,
        _ => {
            return Err(Error::invalid_input(
                "input does not match operation schema",
            ));
        }
    };
    // The resolved space travels separately; the plugin sees only its declared input.
    let plugin_input = Value::Object(input.into_iter().collect());
    let (plugin2, account) = (plugin.clone(), p.id);
    let (rid, operation, fp2) = (request_id.to_owned(), op.to_owned(), fp.clone());
    let admitted = blocking(&state, move |s| {
        // A mutating operation's retry replays its receipt and runs nothing again.
        if mutates {
            if let Some(v) = replay_receipt(s, account, space, &rid, &operation, &fp2)? {
                return Ok(Err(v));
            }
        }
        let row = activation(s, space, &plugin2)?;
        if !row.enabled {
            return Err(Error::forbidden("plugin is disabled in this space"));
        }
        Ok(Ok(row.generation))
    })
    .await?;
    let generation = match admitted {
        Ok(generation) => generation,
        Err(replayed) => return Ok(replayed),
    };
    let host = SpaceHost::new(
        state.clone(),
        p.clone(),
        session.clone(),
        space,
        plugin.clone(),
        generation,
        request_id.to_owned(),
    );
    let outcome = state
        .workers
        .invoke(
            &plugin,
            space,
            Call {
                operation: op.to_owned(),
                input: plugin_input,
                account: p.id,
                host: Arc::new(host),
                timeout: crate::plugins::WORKER_TIMEOUT,
                cancel: None,
                job: None,
            },
        )
        .await;
    // Delivery checks run whatever the worker answered, so a refusal caused by a
    // revocation reports the same error the revocation itself would.
    if !session.is_live(&state.tokens).await {
        return Err(Error::unauthorized("missing or invalid session"));
    }
    let plugin2 = plugin.clone();
    let still = blocking(&state, move |s| activation(s, space, &plugin2)).await?;
    if !still.enabled || still.generation != generation {
        return Err(Error::conflict(
            "plugin state changed during invocation; result discarded",
        )
        .with_details(serde_json::json!({ "reason": "plugin_generation_changed" })));
    }
    let st = state.clone();
    let caller = p.id;
    tokio::task::spawn_blocking(move || policy.authorize_space(&st.tokens, &p, space))
        .await
        .map_err(|_| Error::internal("directory task failed"))??;
    let value = outcome_value(outcome)?;
    check_view(&plugin, &value)?;
    if mutates {
        // Only a delivered result is receipted: a refused or failed call may be
        // retried, and its brokered writes replay by their own receipts.
        let (principal, rid, op, result) =
            (caller, request_id.to_owned(), op.to_owned(), value.clone());
        blocking(&state, move |s| {
            s.record_plugin_receipt(principal, &rid, &op, &fp, space, result)
        })
        .await?;
    }
    Ok(value)
}

/// Preview a plugin without activating it or touching user data: its first
/// item-targeted operation (manifest `target`) runs against the fixed fixture item, served by a
/// broker that refuses every other host call. A plugin with no worker or no
/// item-targeted operation has nothing to preview (not found).
async fn preview<D: Directory>(
    state: Shared<D>,
    p: Identity,
    plugin: Arc<Plugin>,
    space: SpaceId,
) -> Result<Value, Error> {
    let (operation, target) = plugin
        .manifest
        .operations
        .iter()
        .find_map(|op| Some((op.id.clone(), op.target.clone()?)))
        .filter(|_| plugin.entry.worker.is_some())
        .ok_or_else(|| Error::not_found("plugin has no views"))?;
    let outcome = state
        .workers
        .invoke(
            &plugin,
            space,
            Call {
                operation,
                input: serde_json::json!({ target.field: PREVIEW_ITEM_ID }),
                account: p.id,
                host: Arc::new(PreviewHost { kind: target.kind }),
                timeout: crate::plugins::WORKER_TIMEOUT,
                cancel: None,
                job: None,
            },
        )
        .await;
    let value = outcome_value(outcome)?;
    check_view(&plugin, &value)?;
    let view = value
        .get("view")
        .cloned()
        .ok_or_else(|| Error::not_found("plugin has no views"))?;
    let view = serde_json::from_value(view)
        .map_err(|_| Error::unavailable("extension returned an invalid view"))?;
    to_value(PluginPreviewResult {
        view,
        preview: True,
    })
}

async fn mutate_note<D: Directory>(
    state: Shared<D>,
    p: Identity,
    request_id: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
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
    let space = space_of(
        create
            .as_ref()
            .map(|i| i.space_id.as_str())
            .or_else(|| update.as_ref().map(|i| i.space_id.as_str()))
            .expect("note scope"),
    )?;
    blocking(&state, move |s| {
        authz::policy(&op)
            .expect("bound policy")
            .authorize_space(&st.tokens, &Identity { id: principal }, space)?;
        if let Some(v) = replay_receipt(s, principal, space, &rid, &op, &fp)? {
            return Ok(v);
        }
        let note = if let Some(i) = create {
            s.create(
                principal,
                &rid,
                &op,
                &fp,
                space,
                &i.title,
                &i.markdown,
            )?
        } else {
            let i = update.expect("update input");
            s.update(
                principal,
                &rid,
                &op,
                &fp,
                space,
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
