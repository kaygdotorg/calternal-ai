//! HTTP endpoints for machines, merged into the API router by [`crate::api`].
//!
//! Machine-facing, under [`calternal_machine::protocol::PATH_PREFIX`]
//! (`/api/v1/machine/`): the request guard does not require an account session
//! there; `enroll` and `session/*` are authenticated by the code and the machine
//! key, `sync` and `output` by the machine's bearer token.
//!
//! Account-facing, under `/api/v1/machines` (signed-in account required, bare
//! JSON results, operation error envelope on failure):
//!
//! | Method and path | Body → result |
//! | --- | --- |
//! | `POST /api/v1/machines/codes` | `{}` → `{code, expires_ms}`: enroll one of your machines |
//! | `GET /api/v1/machines` | → `{machines: [MachineView]}`: your machines |
//! | `POST /api/v1/machines/{id}/revoke` | `{}` → `{}` |
//! | `POST /api/v1/machines/{id}/drain` | `{draining}` → `{}` |
//! | `POST /api/v1/machines/runs` | `{space_id, thread_id, image?}` → `RunView` |
//! | `GET /api/v1/machines/runs/{lease}` | → `RunView` |
//! | `POST /api/v1/machines/runs/{lease}/stop` | `{}` → `RunView` |
//! | `POST /api/v1/machines/runs/{lease}/execs` | `{argv, env?}` → `{exec_id}` |
//! | `GET /api/v1/machines/execs/{exec}/output?after=N&wait_ms=M` | → `OutputView` |
//!
//! Runs need `plugins.use` in the space, re-checked on every call; a run or exec
//! of another account, or in a space the caller lost, is "not found". Every
//! handler takes the registry and directory on blocking tasks.
//!
//! These endpoints are the minimal seam the jobs lane replaces with plugin jobs;
//! they are not in the operation catalog yet.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Router};
use calternal_machine::protocol::{
    EnrolRequest, OutputPost, Owner, SessionChallenge, SessionFinish, SessionStart, SyncRequest,
    SyncResponse,
};
use calternal_types::{AccountId, SpaceId};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use uuid::Uuid;

use super::{MachineService, blocking};
use crate::api::{Shared, failure, json_response};
use crate::auth::{Directory, Identity};
use calternal_types::Capability;
use calternal_types::Error;

/// The service as a request extension; absent when machines are disabled.
pub(super) type Svc = Option<Extension<Arc<MachineService>>>;

/// Every machine route. Handlers answer 404 when no [`MachineService`]
/// extension is installed (tests that build the router without one).
pub fn routes<D: Directory>() -> Router<Shared<D>> {
    let release_path = calternal_machine::release::PATH;
    Router::new()
        .route("/install", get(super::install::installer))
        .route(
            &format!("{release_path}/version"),
            get(super::install::version),
        )
        .route(
            &format!("{release_path}/{{version}}/{{platform}}/{{asset}}"),
            get(super::install::asset),
        )
        .route("/api/v1/machine/enroll", post(enrol))
        .route("/api/v1/machine/session/start", post(session_start))
        .route("/api/v1/machine/session/finish", post(session_finish))
        .route("/api/v1/machine/sync", post(sync))
        .route("/api/v1/machine/status", post(erasure_status))
        .route("/api/v1/machine/output", post(output))
        .route("/api/v1/machine/preview-output", post(preview_output))
        .route("/api/v1/machines", get(list))
        .route("/api/v1/machines/codes", post(code))
        .route("/api/v1/machines/{id}/revoke", post(revoke))
        .route("/api/v1/machines/{id}/drain", post(drain))
        .route("/api/v1/machines/runs", post(create_run::<D>))
        .route("/api/v1/machines/runs/{lease}", get(get_run::<D>))
        .route(
            "/api/v1/machines/runs/{lease}/previews",
            post(super::preview::publish::<D>),
        )
        .route(
            "/api/v1/machine-previews/{preview}/",
            get(super::preview::view::<D>),
        )
        .route(
            "/api/v1/machine-previews/{preview}/{*tail}",
            get(super::preview::view::<D>),
        )
        .route("/api/v1/machines/runs/{lease}/stop", post(stop_run::<D>))
        .route("/api/v1/machines/runs/{lease}/execs", post(exec::<D>))
        .route(
            "/api/v1/machines/execs/{exec}/output",
            get(read_output::<D>),
        )
}

pub(super) fn reply<T: serde::Serialize>(r: Result<T, Error>) -> Response {
    match r {
        Ok(v) => json_response(StatusCode::OK, &v),
        Err(e) => failure(None, e),
    }
}

pub(super) fn service(svc: Svc) -> Result<Arc<MachineService>, Error> {
    svc.map(|Extension(s)| s)
        .ok_or_else(|| Error::not_found("machines are not enabled"))
}

pub(super) fn body<T: DeserializeOwned>(bytes: &Bytes) -> Result<T, Error> {
    serde_json::from_slice(bytes)
        .map_err(|_| Error::invalid_input("body does not match the schema"))
}

fn bearer(headers: &HeaderMap) -> Result<String, Error> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_owned)
        .ok_or_else(|| Error::unauthorized("machine session required"))
}

// ------------------------------------------------------------ machine side

async fn enrol(svc: Svc, bytes: Bytes) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let req: EnrolRequest = body(&bytes)?;
            blocking(move || svc.enrol(&req)).await
        }
        .await,
    )
}

async fn session_start(svc: Svc, bytes: Bytes) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let req: SessionStart = body(&bytes)?;
            let challenge = blocking(move || svc.session_start(req.machine_id)).await?;
            Ok(SessionChallenge { challenge })
        }
        .await,
    )
}

async fn session_finish(svc: Svc, bytes: Bytes) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let req: SessionFinish = body(&bytes)?;
            blocking(move || svc.session_finish(&req)).await
        }
        .await,
    )
}

async fn erasure_status(svc: Svc, bytes: Bytes) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let request = body(&bytes)?;
            blocking(move || svc.erasure_status(&request)).await
        }
        .await,
    )
}

async fn sync(
    svc: Svc,
    shutdown: Option<Extension<crate::shutdown::Shutdown>>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let token = bearer(&headers)?;
            let req: SyncRequest = body(&bytes)?;
            let (s, t) = (svc.clone(), token.clone());
            let machine = blocking(move || s.authenticate(&t)).await?;
            let messages = svc
                .sync_until(machine, &token, req, shutdown.as_ref().map(|ext| &ext.0))
                .await?;
            Ok(SyncResponse { messages })
        }
        .await,
    )
}

async fn output(svc: Svc, headers: HeaderMap, bytes: Bytes) -> Response {
    receive_output(svc, headers, bytes, false).await
}

async fn preview_output(svc: Svc, headers: HeaderMap, bytes: Bytes) -> Response {
    receive_output(svc, headers, bytes, true).await
}

async fn receive_output(svc: Svc, headers: HeaderMap, bytes: Bytes, preview: bool) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let token = bearer(&headers)?;
            let post: OutputPost = body(&bytes)?;
            blocking(move || {
                let machine = svc.authenticate(&token)?;
                if preview {
                    svc.preview_output(machine, post)
                } else {
                    svc.output(machine, post)
                }
            })
            .await
        }
        .await,
    )
}

// ------------------------------------------------------------ owner side

async fn code(svc: Svc, Extension(who): Extension<Identity>) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let owner = Owner::Account { id: who.id };
            let (code, expires_ms) = blocking(move || svc.issue_code(owner)).await?;
            Ok(json!({ "code": code, "expires_ms": expires_ms }))
        }
        .await,
    )
}

async fn list(svc: Svc, Extension(who): Extension<Identity>) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let me = who.id;
            let machines = blocking(move || svc.machines_of(me)).await?;
            Ok(json!({ "machines": machines }))
        }
        .await,
    )
}

async fn revoke(svc: Svc, Extension(who): Extension<Identity>, Path(id): Path<Uuid>) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let me = who.id;
            blocking(move || svc.revoke(me, id)).await?;
            Ok(json!({}))
        }
        .await,
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DrainInput {
    draining: bool,
}

async fn drain(
    svc: Svc,
    Extension(who): Extension<Identity>,
    Path(id): Path<Uuid>,
    bytes: Bytes,
) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let me = who.id;
            let input: DrainInput = body(&bytes)?;
            blocking(move || svc.set_draining(me, id, input.draining)).await?;
            Ok(json!({}))
        }
        .await,
    )
}

/// Check `plugins.use` for the caller in `space` on a blocking task; a missing
/// grant is "not found" so a caller cannot probe spaces or runs.
async fn authorize<D: Directory>(
    state: &Shared<D>,
    who: &Identity,
    space: SpaceId,
) -> Result<(), Error> {
    let (state, id) = (state.clone(), who.id);
    blocking(move || match state.directory.capabilities(id, space) {
        Some(caps) if caps.contains(Capability::PluginsUse) => Ok(()),
        _ => Err(Error::not_found("space not found")),
    })
    .await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunInput {
    space_id: SpaceId,
    thread_id: Uuid,
    #[serde(default)]
    image: Option<String>,
}

async fn create_run<D: Directory>(
    State(state): State<Shared<D>>,
    svc: Svc,
    Extension(who): Extension<Identity>,
    bytes: Bytes,
) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let me = who.id;
            let input: RunInput = body(&bytes)?;
            if input
                .image
                .as_ref()
                .is_some_and(|i| i.is_empty() || i.len() > 255)
            {
                return Err(Error::invalid_input("image must be 1 to 255 characters"));
            }
            authorize(&state, &who, input.space_id).await?;
            blocking(move || svc.request_run(input.space_id, me, input.thread_id, input.image))
                .await
        }
        .await,
    )
}

/// Resolve a lease's space and re-check the caller's authority there.
pub(super) async fn authorize_run<D: Directory>(
    state: &Shared<D>,
    svc: &Arc<MachineService>,
    who: &Identity,
    lease: Uuid,
) -> Result<AccountId, Error> {
    let me = who.id;
    let s = svc.clone();
    let space = blocking(move || s.run_space(lease))
        .await?
        .ok_or_else(|| Error::not_found("run not found"))?;
    authorize(state, who, space)
        .await
        .map_err(|_| Error::not_found("run not found"))?;
    Ok(me)
}

async fn get_run<D: Directory>(
    State(state): State<Shared<D>>,
    svc: Svc,
    Extension(who): Extension<Identity>,
    Path(lease): Path<Uuid>,
) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let me = authorize_run(&state, &svc, &who, lease).await?;
            blocking(move || svc.run(me, lease)).await
        }
        .await,
    )
}

async fn stop_run<D: Directory>(
    State(state): State<Shared<D>>,
    svc: Svc,
    Extension(who): Extension<Identity>,
    Path(lease): Path<Uuid>,
) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let me = authorize_run(&state, &svc, &who, lease).await?;
            blocking(move || svc.stop_run(me, lease)).await
        }
        .await,
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecInput {
    argv: Vec<String>,
    #[serde(default)]
    env: Vec<(String, String)>,
}

async fn exec<D: Directory>(
    State(state): State<Shared<D>>,
    svc: Svc,
    Extension(who): Extension<Identity>,
    Path(lease): Path<Uuid>,
    bytes: Bytes,
) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let me = authorize_run(&state, &svc, &who, lease).await?;
            let input: ExecInput = body(&bytes)?;
            let exec_id = blocking(move || svc.exec(me, lease, input.argv, input.env)).await?;
            Ok(json!({ "exec_id": exec_id }))
        }
        .await,
    )
}

/// `after` and `wait_ms` from the query string; other keys are ignored and a
/// malformed number is an input error. Parsed by hand: two integers do not
/// justify axum's `query` feature and its dependencies.
fn output_query(uri: &Uri) -> Result<(u64, u64), Error> {
    let (mut after, mut wait) = (0, 0);
    for pair in uri
        .query()
        .unwrap_or("")
        .split('&')
        .filter(|p| !p.is_empty())
    {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let n = || {
            v.parse::<u64>()
                .map_err(|_| Error::invalid_input("after and wait_ms are integers"))
        };
        match k {
            "after" => after = n()?,
            "wait_ms" => wait = n()?,
            _ => {}
        }
    }
    Ok((after, wait))
}

async fn read_output<D: Directory>(
    State(state): State<Shared<D>>,
    svc: Svc,
    Extension(who): Extension<Identity>,
    Path(exec): Path<Uuid>,
    shutdown: Option<Extension<crate::shutdown::Shutdown>>,
    uri: Uri,
) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let (after, wait_ms) = output_query(&uri)?;
            let me = who.id;
            let s = svc.clone();
            let space = blocking(move || s.exec_space(exec))
                .await?
                .ok_or_else(|| Error::not_found("exec not found"))?;
            authorize(&state, &who, space)
                .await
                .map_err(|_| Error::not_found("exec not found"))?;
            tokio::select! {
                result = svc.read_output(me, exec, after, Duration::from_millis(wait_ms)) => result,
                () = crate::shutdown::wait(shutdown.as_ref().map(|ext| &ext.0)) => {
                    Err(Error::unavailable("the server is shutting down"))
                }
            }
        }
        .await,
    )
}
