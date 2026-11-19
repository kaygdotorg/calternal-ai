//! Disposable host-side implementation of the sealed activation protocol.
mod activation;
mod views;
mod wire;

use crate::fixture;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use calternal_server::{
    api::Shared,
    auth::{Directory, Identity},
};
use calternal_types::{AccountId, SpaceId};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{Mutex as Gate, Notify};
use uuid::Uuid;
use wire::Wire;

const LIFETIME: Duration = Duration::from_secs(15);
const NOTES: &str = "notes";
const CHOICE: &str = "collection";

#[derive(Clone)]
struct View {
    space: SpaceId,
    home: bool,
    rows: Vec<Uuid>,
    options: Vec<(Uuid, String)>,
    gate: Arc<Gate<()>>,
}

#[derive(Clone)]
struct Lease {
    id: Uuid,
    view: Uuid,
    actor: AccountId,
    session: Uuid,
    partition: Uuid,
    grant: Uuid,
    audience: &'static str,
    deadline: Instant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Pin,
    Prefill,
    Choice,
    Bulk,
    Ordinary,
}
impl Action {
    fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "pin" => Self::Pin,
            "rename" => Self::Prefill,
            "choose-destination" => Self::Choice,
            "trash-selected" => Self::Bulk,
            "remind" => Self::Ordinary,
            _ => return None,
        })
    }
    fn name(self) -> &'static str {
        match self {
            Self::Pin => "pin",
            Self::Prefill => "rename",
            Self::Choice => "choose-destination",
            Self::Bulk => "trash-selected",
            Self::Ordinary => "remind",
        }
    }
    fn challenge(self) -> bool {
        matches!(self, Self::Prefill | Self::Choice | Self::Bulk)
    }
}

#[derive(Clone)]
struct Context {
    lease: Uuid,
    row: Option<Uuid>,
    revision: Option<String>,
    action: Action,
}

#[derive(Clone)]
struct IssuedWindow {
    lease: Uuid,
    version: Uuid,
    order: u64,
    rows: Vec<(Uuid, String)>,
}

#[derive(Clone)]
struct Choice {
    context: Uuid,
    destination: Uuid,
    label: String,
}

#[derive(Clone)]
struct Resolution {
    context: Uuid,
    activation: Uuid,
    interaction: Value,
    targets: Vec<(Uuid, String)>,
    material: Vec<Value>,
    challenge: Option<Uuid>,
    proof: Option<Uuid>,
    selection: Option<Uuid>,
    destination: Option<Uuid>,
}

#[derive(Default)]
struct Volatile {
    leases: HashMap<Uuid, Lease>,
    contexts: HashMap<Uuid, Context>,
    windows: HashMap<Uuid, IssuedWindow>,
    window_order: u64,
    choices: HashMap<Uuid, Choice>,
    resolutions: HashMap<Uuid, Resolution>,
    activations: HashMap<(AccountId, Uuid), Uuid>,
    grants: HashMap<(AccountId, Uuid), Uuid>,
}

#[derive(Default)]
struct Hooks {
    stage: AtomicUsize,
    entered: Notify,
    resume: Notify,
}
impl Hooks {
    async fn hit(&self, stage: usize) {
        if self
            .stage
            .compare_exchange(stage, 0, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.entered.notify_one();
            self.resume.notified().await;
        }
    }
}

pub(crate) struct Host {
    state: Shared,
    wire: Wire,
    views: HashMap<Uuid, View>,
    owner: AccountId,
    viewer: AccountId,
    shared: Uuid,
    home: Uuid,
    live: Mutex<Volatile>,
    hooks: Hooks,
}

#[derive(Clone, Copy)]
pub(crate) struct ResultRow(pub(crate) StatusCode, pub(crate) &'static str);
type Reply<T> = Result<T, ResultRow>;
const HIDDEN: ResultRow = ResultRow(StatusCode::NOT_FOUND, "not_found");
const INVALID: ResultRow = ResultRow(StatusCode::UNPROCESSABLE_ENTITY, "invalid_input");
const STALE: ResultRow = ResultRow(StatusCode::CONFLICT, "stale");
const FORBIDDEN: ResultRow = ResultRow(StatusCode::FORBIDDEN, "forbidden");
const CONFLICT: ResultRow = ResultRow(StatusCode::CONFLICT, "request_reused");

fn ref_at(value: &Value, key: &str) -> Reply<Uuid> {
    value[key]
        .as_str()
        .and_then(|r| Uuid::parse_str(r).ok())
        .ok_or(HIDDEN)
}

impl Host {
    pub(crate) async fn create(roots: &calternal_roots::Roots) -> Arc<Self> {
        let packages = fixture::packages();
        let (state, ids) = tokio::task::block_in_place(|| fixture::open(roots, &packages));
        let mut make = Vec::new();
        for (index, space) in [ids.shared, ids.personal].into_iter().enumerate() {
            let mut rows = Vec::new();
            for label in [
                "Project brief",
                "Second note",
                "Private fixture material",
                "Unrelated churn",
            ] {
                let result = fixture::run(
                    &state,
                    fixture::identity(ids.owner),
                    "notes.create",
                    json!({"space_id":space,"title":label,"markdown":"owned by sealed fixture"}),
                )
                .await
                .unwrap();
                rows.push(Uuid::parse_str(result["note"]["id"].as_str().unwrap()).unwrap());
            }
            let mut options = Vec::new();
            for label in ["Review", "Archive"] {
                let result = fixture::run(
                    &state,
                    fixture::identity(ids.owner),
                    "collections.create",
                    json!({"space_id":space,"title":label,"members":[]}),
                )
                .await
                .unwrap();
                options.push((
                    Uuid::parse_str(result["id"].as_str().unwrap()).unwrap(),
                    label.into(),
                ));
            }
            make.push((
                Uuid::new_v4(),
                View {
                    space,
                    home: index == 1,
                    rows,
                    options,
                    gate: Arc::new(Gate::new(())),
                },
            ));
        }
        let shared = make[0].0;
        let home = make[1].0;
        Arc::new(Self {
            state,
            wire: Wire::new(),
            views: make.into_iter().collect(),
            owner: ids.owner,
            viewer: ids.guest,
            shared,
            home,
            live: Mutex::default(),
            hooks: Hooks::default(),
        })
    }

    fn session(&self, headers: &HeaderMap) -> Reply<(Identity, Uuid)> {
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(HIDDEN)?;
        let who = self
            .state
            .directory
            .authenticate(token.as_bytes())
            .ok_or(HIDDEN)?;
        // The fixture directory has one bearer per account. The salted
        // derivation is only an opaque in-memory session namespace.
        let digest = ring::digest::digest(&ring::digest::SHA256, token.as_bytes());
        let session = Uuid::from_bytes(digest.as_ref()[..16].try_into().unwrap());
        Ok((who, session))
    }

    pub(crate) fn router(self: Arc<Self>) -> Router {
        Router::new()
            .route("/step0/v2/{route}", post(http))
            .route("/step0/v2/fixture", get(bootstrap))
            .layer(DefaultBodyLimit::max(96 * 1024))
            .layer(axum::middleware::from_fn(local_cors))
            .with_state(self)
    }

    async fn call(
        &self,
        who: Identity,
        session: Uuid,
        lease_header: Option<Uuid>,
        route: &str,
        input: Value,
    ) -> Reply<(&'static str, Value)> {
        let now = Instant::now();
        {
            let mut live = self.live.lock().unwrap();
            live.leases.retain(|_, l| l.deadline > now);
            let Volatile {
                leases,
                contexts,
                windows,
                choices,
                resolutions,
                activations,
                ..
            } = &mut *live;
            contexts.retain(|_, c| leases.contains_key(&c.lease));
            windows.retain(|_, w| leases.contains_key(&w.lease));
            choices.retain(|_, c| contexts.contains_key(&c.context));
            resolutions.retain(|_, r| contexts.contains_key(&r.context));
            activations.retain(|_, r| resolutions.contains_key(r));
        }
        let schema = match route {
            "view.open" => "view_open",
            "view.renew" => "renew_request",
            "window" => "window_request",
            "selection.settled" => "selection",
            "choice.options" => "choice_request",
            "activation.resolve" => "resolve",
            "activation.confirm" => "confirm",
            "activation.dispatch" => "dispatch",
            _ => return Err(HIDDEN),
        };
        if !self.wire.valid(schema, &input) {
            return Err(INVALID);
        }
        let result = match route {
            "view.open" => (
                "view",
                self.open_view(who, session, ref_at(&input, "view_ref")?)
                    .await?,
            ),
            "view.renew" => ("renew", self.renew(who, session, &input).await?),
            "window" => (
                "window",
                self.window(who, session, lease_header.ok_or(HIDDEN)?, &input["request"])
                    .await?,
            ),
            "selection.settled" => (
                "selection_context",
                self.selection(who, session, lease_header.ok_or(HIDDEN)?, &input)
                    .await?,
            ),
            "choice.options" => ("choices", self.choices(who, session, &input).await?),
            "activation.resolve" => self.resolve(who, session, &input).await?,
            "activation.confirm" => {
                return Ok(("proof", self.confirm(who, session, &input).await?));
            }
            "activation.dispatch" => ("outcome", self.dispatch(who, session, &input).await?),
            _ => unreachable!(),
        };
        assert!(
            self.wire.valid(result.0, &result.1),
            "invalid fixture reply {}",
            result.0
        );
        Ok(result)
    }
}

async fn http(
    State(host): State<Arc<Host>>,
    Path(route): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let operation = async {
        let (actor, session) = host.session(&headers)?;
        let lease = headers
            .get("x-step0-view-lease")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| Uuid::parse_str(value).ok());
        host.call(actor, session, lease, &route, body)
            .await
            .map(|(_, out)| out)
    };
    reply(operation.await)
}

async fn bootstrap(State(host): State<Arc<Host>>, headers: HeaderMap) -> Response {
    let operation = async {
        let (who, _) = host.session(&headers)?;
        let mut views = serde_json::Map::new();
        for (label, id) in [("shared", host.shared), ("home", host.home)] {
            if host.audience(who, &host.views[&id]).await.is_some() {
                views.insert(label.into(), json!(id));
            }
        }
        Ok(
            json!({"views":views,"template":serde_json::from_str::<Value>(include_str!("template.json")).unwrap()}),
        )
    };
    reply(operation.await)
}

fn reply(result: Reply<Value>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(ResultRow(status, code)) => (status, Json(json!({"error":code}))).into_response(),
    }
}

async fn local_cors(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let origin = req
        .headers()
        .get(header::ORIGIN)
        .filter(|value| {
            value
                .to_str()
                .ok()
                .and_then(|o| url::Url::parse(o).ok())
                .is_some_and(|o| {
                    o.scheme() == "http"
                        && matches!(o.host_str(), Some("localhost" | "127.0.0.1"))
                        && o.path() == "/"
                })
        })
        .cloned();
    let mut out = if req.method() == Method::OPTIONS && origin.is_some() {
        StatusCode::NO_CONTENT.into_response()
    } else {
        next.run(req).await
    };
    if let Some(origin) = origin {
        out.headers_mut()
            .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        out.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET,POST,OPTIONS"),
        );
        out.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("authorization,content-type,x-step0-view-lease"),
        );
        out.headers_mut()
            .insert(header::VARY, HeaderValue::from_static("Origin"));
    }
    out
}

impl Host {
    pub(crate) fn arm(&self, stage: usize) {
        assert_eq!(self.hooks.stage.swap(stage, Ordering::AcqRel), 0);
    }

    pub(crate) async fn wait_for_race(&self) {
        tokio::time::timeout(Duration::from_secs(3), self.hooks.entered.notified())
            .await
            .unwrap();
    }

    pub(crate) fn finish_race(&self) {
        self.hooks.resume.notify_one();
    }

    pub(crate) fn end_lease(&self, open: &Value) {
        let id = ref_at(&open["lease"], "lease_ref").ok().unwrap();
        self.live.lock().unwrap().leases.remove(&id);
    }

    pub(crate) async fn change_fixture_row(&self, index: usize, serial: usize) {
        let view = &self.views[&self.shared];
        let _fence = view.gate.lock().await;
        let state = self.state.clone();
        let space = view.space;
        let item = view.rows[index];
        let actor = self.owner;
        tokio::task::spawn_blocking(move || {
            let content = state.store.content();
            let old = content.get(space, item).unwrap();
            let mut metadata = old.metadata;
            metadata["spike_churn"] = json!(serial);
            if index == 0 {
                metadata["pinned"] = json!(false);
            }
            if index == 1 {
                metadata["title"] = json!(format!("Draft prefill {serial}"));
            }
            let id = Uuid::new_v4().to_string();
            content
                .write(
                    space,
                    Some(item),
                    NOTES,
                    Some(&old.revision),
                    metadata,
                    &old.bytes,
                    calternal_content::Request {
                        account: actor,
                        via: None,
                        id: &id,
                        operation: "spike.churn",
                        fingerprint: &id,
                    },
                    |_| json!({}),
                )
                .unwrap();
        })
        .await
        .unwrap();
    }

    pub(crate) async fn revision(&self, index: usize) -> String {
        let view = &self.views[&self.shared];
        self.current_head(view, view.rows[index])
            .await
            .ok()
            .unwrap()
    }

    pub(crate) async fn committed_effects(&self, rid: Uuid) -> usize {
        let view = &self.views[&self.shared];
        let root = self
            .state
            .store
            .appdata()
            .join(format!("spaces/{}/index.db", view.space));
        let key = format!("step0/{rid}");
        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(root).unwrap();
            conn.query_row(
                "SELECT count(*) FROM journal WHERE request_id=?1",
                [key],
                |r| r.get::<_, i64>(0),
            )
            .unwrap() as usize
        })
        .await
        .unwrap()
    }
}
