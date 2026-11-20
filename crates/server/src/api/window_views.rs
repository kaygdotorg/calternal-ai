//! Authenticated read-only v2 Notes windows. The route owns a declared Bind and
//! its leases. It never takes a renderer-supplied kind, filter, sort or audience.

use super::*;
use axum::{
    Extension, Json,
    extract::{DefaultBodyLimit, Path, Query},
    routing::get,
};
use calternal_content::{
    WindowAccess, WindowAudience, WindowCountMode, WindowReply, WindowRequest, WindowResult,
    WindowScope,
};
use calternal_types::{Capability, item_access_revision_handle};
use ring::hmac;
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

const SHARDS: usize = 32;
const MAX_BOOK_ENTRIES_PER_SHARD: usize = 128;
const LEASE_HEADER: &str = "x-calternal-view-lease";
const PARTITION_HEADER: &str = "x-calternal-cache-partition";

#[derive(Deserialize)]
struct Limits {
    subscriber_session_lease_seconds: u64,
    subscriptions_per_actor: usize,
    subscriptions_per_space: usize,
    pending_windows_per_actor: usize,
    query_concurrency_per_space: usize,
    query_starts_per_actor_per_second: usize,
    query_starts_per_view_per_second: usize,
    query_starts_per_space_per_second: usize,
    count_starts_per_space_per_second: usize,
    binds_per_view: usize,
}

fn limits() -> &'static Limits {
    static LIMITS: OnceLock<Limits> = OnceLock::new();
    LIMITS.get_or_init(|| {
        let catalogue: Value =
            serde_json::from_str(include_str!("../../../../contract/host-catalogue-v2.json"))
                .expect("sealed catalogue");
        serde_json::from_value(catalogue["query"]["live_limits"].clone())
            .expect("sealed view limits")
    })
}

#[derive(Default)]
struct Rate(Mutex<VecDeque<Instant>>);

impl Rate {
    fn admit(&self, limit: usize) -> bool {
        let now = Instant::now();
        let mut starts = self.0.lock().unwrap_or_else(|e| e.into_inner());
        while starts
            .front()
            .is_some_and(|at| now.saturating_duration_since(*at) >= Duration::from_secs(1))
        {
            starts.pop_front();
        }
        if starts.len() >= limit {
            return false;
        }
        starts.push_back(now);
        true
    }
}

struct ActorShard {
    leases: Mutex<HashMap<(AccountId, SpaceId, Uuid), Arc<Lease>>>,
    rate: Rate,
    work: Arc<Semaphore>,
}

struct SpaceShard {
    subscriptions: Arc<Semaphore>,
    work: Arc<Semaphore>,
    rate: Rate,
    count_rate: Rate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum View {
    NotesView,
    ReadingListView,
}

pub(super) struct Binding {
    pub(super) bind: String,
    pub(super) fields: Vec<String>,
    pub(super) scope: WindowScope,
    pub(super) description: Option<Value>,
}

struct Views {
    notes: Binding,
    reading_list: Option<Binding>,
    actors: [ActorShard; SHARDS],
    spaces: [SpaceShard; SHARDS],
    revision: Arc<AtomicU64>,
}

impl Views {
    fn new(reading_list: Option<Binding>) -> Self {
        let limits = limits();
        Self {
            notes: Binding {
                bind: "notes".into(),
                fields: vec!["title".into()],
                scope: WindowScope::SpaceNotes,
                description: None,
            },
            reading_list,
            actors: std::array::from_fn(|_| ActorShard {
                leases: Mutex::new(HashMap::new()),
                rate: Rate::default(),
                work: Arc::new(Semaphore::new(limits.pending_windows_per_actor)),
            }),
            spaces: std::array::from_fn(|_| SpaceShard {
                subscriptions: Arc::new(Semaphore::new(limits.subscriptions_per_space)),
                work: Arc::new(Semaphore::new(limits.query_concurrency_per_space)),
                rate: Rate::default(),
                count_rate: Rate::default(),
            }),
            revision: item_access_revision_handle(),
        }
    }

    fn binding(&self, view: View) -> Result<&Binding, Error> {
        match view {
            View::NotesView => Ok(&self.notes),
            View::ReadingListView => self.reading_list.as_ref().ok_or_else(hidden),
        }
    }

    fn actor(&self, account: AccountId) -> &ActorShard {
        &self.actors[usize::from(account.as_uuid().as_bytes()[15]) % SHARDS]
    }

    fn space(&self, space: SpaceId) -> &SpaceShard {
        &self.spaces[usize::from(space.as_uuid().as_bytes()[15]) % SHARDS]
    }
}

struct Lease {
    reference: Uuid,
    partition: Uuid,
    view: View,
    revision: u64,
    expires: Instant,
    session: Session,
    audience: WindowAudience,
    count: Count,
    replaced: AtomicBool,
    work: Arc<Semaphore>,
    rate: Arc<Rate>,
    previous: Mutex<Option<Arc<WindowResult>>>,
    _space_quota: Arc<OwnedSemaphorePermit>,
}

impl Lease {
    fn valid(&self, revision: &AtomicU64) -> bool {
        self.revision == revision.load(Ordering::Acquire)
            && self.revision & 1 == 0
            && !self.replaced.load(Ordering::Acquire)
            && Instant::now() < self.expires
    }

    fn wire(&self) -> Value {
        json!({
            "lease_ref":self.reference,
            "cache_partition":self.partition,
            "grant_revision":format!("{:x}", self.revision),
            "lease_seconds":limits().subscriber_session_lease_seconds,
        })
    }
}

#[derive(Debug, Default, Clone, Copy, serde::Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Count {
    #[default]
    Exact,
    None,
}

impl Count {
    fn policy(self) -> WindowCountMode {
        match self {
            Self::Exact => WindowCountMode::Exact,
            Self::None => WindowCountMode::None,
        }
    }

    fn tag(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::None => "none",
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Discovery {
    #[serde(default)]
    count: Count,
}

pub(super) fn routes<D: Directory>(reading_list: Option<Binding>) -> Router<Shared<D>> {
    Router::new()
        .route(
            "/api/v2/spaces/{space}/{view}",
            get(discover::<D>).post(open::<D>),
        )
        .route("/api/v2/spaces/{space}/{view}/window", post(window::<D>))
        .route("/api/v2/spaces/{space}/{view}/renew", post(renew::<D>))
        .layer(DefaultBodyLimit::max(32 * 1024))
        .layer(Extension(Arc::new(Views::new(reading_list))))
}

fn hidden() -> Error {
    Error::not_found("window view is unavailable")
}

fn rate_limited() -> Error {
    Error::new(ErrorCode::RetryAfter, "too many active window requests")
}

fn private(value: impl serde::Serialize, status: StatusCode) -> Response {
    (
        status,
        [(header::CACHE_CONTROL, "private, no-store")],
        Json(value),
    )
        .into_response()
}

fn view_key(
    who: &Identity,
    session: &Session,
    space: SpaceId,
    view: View,
    count: Count,
) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(&("v2/space-view", who.id, session, space, view, count))
        .map_err(|_| Error::internal("encode view identity"))
}

async fn admit<D: Directory>(
    state: &Shared<D>,
    who: &Identity,
    space: SpaceId,
) -> Result<(), Error> {
    super::authorization::capabilities(state, who.id, space)
        .await?
        .filter(|caps| caps.contains(Capability::SpaceRead))
        .map(|_| ())
        .ok_or_else(hidden)
}

async fn discover<D: Directory>(
    State(state): State<Shared<D>>,
    Extension(who): Extension<Identity>,
    Extension(session): Extension<Session>,
    Extension(views): Extension<Arc<Views>>,
    Path((space, view)): Path<(SpaceId, View)>,
    Query(discovery): Query<Discovery>,
) -> Response {
    let answer = async {
        admit(&state, &who, space).await?;
        let binding = views.binding(view)?;
        let reference = hmac::sign(&state.store.cursor_key, &view_key(&who, &session, space, view, discovery.count)?);
        let mut result = json!({"view_ref": format!("{}.{}",discovery.count.tag(),hex::encode(reference.as_ref())), "bind":binding.bind, "fields":binding.fields, "count":discovery.count});
        if let Some(description) = &binding.description {
            result["description"] = description.clone();
        }
        Ok(result)
    }.await;
    match answer {
        Ok(value) => private(value, StatusCode::OK),
        Err(error) => failure(None, error),
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Open {
    ViewOpen { view_ref: String },
}

async fn open<D: Directory>(
    State(state): State<Shared<D>>,
    Extension(who): Extension<Identity>,
    Extension(session): Extension<Session>,
    Extension(views): Extension<Arc<Views>>,
    Path((space, view)): Path<(SpaceId, View)>,
    Json(Open::ViewOpen { view_ref }): Json<Open>,
) -> Response {
    let answer = async {
        let revision = views.revision.load(Ordering::Acquire);
        if revision & 1 != 0 {
            return Err(hidden());
        }
        admit(&state, &who, space).await?;
        views.binding(view)?;
        let (mode, signature) = view_ref.split_once('.').ok_or_else(hidden)?;
        let count = match mode {
            "exact" => Count::Exact,
            "none" => Count::None,
            _ => return Err(hidden()),
        };
        let signature = hex::decode(signature).map_err(|_| hidden())?;
        hmac::verify(
            &state.store.cursor_key,
            &view_key(&who, &session, space, view, count)?,
            &signature,
        )
        .map_err(|_| hidden())?;
        let actor = who.id;
        let directory = state.clone();
        let audience = blocking(&state, move |_| {
            // Directory ownership classifies the view after membership admission.
            let own = directory
                .directory
                .protected_spaces()
                .map(|spaces| spaces.is_personal_space(actor, space))
                .transpose()
                .map_err(crate::auth::auth_error)?
                .unwrap_or(false);
            Ok(if own {
                WindowAudience::OwnSpaceMember
            } else {
                WindowAudience::Member
            })
        })
        .await?;
        if views.revision.load(Ordering::Acquire) != revision {
            return Err(hidden());
        }
        // A quiet actor's expired lease must not strand a space permit.
        if views.space(space).subscriptions.available_permits() == 0 {
            for shard in &views.actors {
                if let Ok(mut leases) = shard.leases.try_lock() {
                    leases.retain(|_, lease| lease.valid(&views.revision));
                }
            }
        }
        let actor_shard = views.actor(actor);
        let mut leases = actor_shard.leases.lock().unwrap_or_else(|e| e.into_inner());
        leases.retain(|_, lease| lease.valid(&views.revision));
        if leases.len() >= MAX_BOOK_ENTRIES_PER_SHARD
            || leases
                .keys()
                .filter(|(account, _, _)| *account == actor)
                .count()
                >= limits().subscriptions_per_actor
        {
            return Err(rate_limited());
        }
        let quota = views
            .space(space)
            .subscriptions
            .clone()
            .try_acquire_owned()
            .map_err(|_| rate_limited())?;
        let lease = Arc::new(Lease {
            reference: Uuid::new_v4(),
            partition: Uuid::new_v4(),
            view,
            revision,
            expires: Instant::now()
                + Duration::from_secs(limits().subscriber_session_lease_seconds),
            session,
            audience,
            count,
            replaced: AtomicBool::new(false),
            work: Arc::new(Semaphore::new(1)),
            rate: Arc::new(Rate::default()),
            previous: Mutex::new(None),
            _space_quota: Arc::new(quota),
        });
        leases.insert((actor, space, lease.reference), lease.clone());
        Ok(json!({"lease":lease.wire(),"audience":audience,"activation_contexts":{}}))
    }
    .await;
    match answer {
        Ok(value) => private(value, StatusCode::OK),
        Err(error) => failure(None, error),
    }
}

fn header_id(headers: &HeaderMap, name: &str) -> Result<Uuid, Error> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(hidden)
}

async fn window<D: Directory>(
    State(state): State<Shared<D>>,
    Extension(who): Extension<Identity>,
    Extension(session): Extension<Session>,
    Extension(views): Extension<Arc<Views>>,
    Path((space, view)): Path<(SpaceId, View)>,
    headers: HeaderMap,
    Json(request): Json<WindowRequest>,
) -> Response {
    let answer = async {
        let actor = views.actor(who.id);
        let space_shard = views.space(space);
        let reference = header_id(&headers, LEASE_HEADER)?;
        let lease = actor
            .leases
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&(who.id, space, reference))
            .cloned()
            .ok_or_else(hidden)?;
        if !lease.valid(&views.revision)
            || lease.session != session
            || lease.view != view
            || lease.partition != header_id(&headers, PARTITION_HEADER)?
        {
            return Err(hidden());
        }
        let work = (
            actor
                .work
                .clone()
                .try_acquire_owned()
                .map_err(|_| rate_limited())?,
            space_shard
                .work
                .clone()
                .try_acquire_owned()
                .map_err(|_| rate_limited())?,
            lease
                .work
                .clone()
                .try_acquire_owned()
                .map_err(|_| rate_limited())?,
        );
        if !actor.rate.admit(limits().query_starts_per_actor_per_second)
            || !lease.rate.admit(limits().query_starts_per_view_per_second)
            || !space_shard
                .rate
                .admit(limits().query_starts_per_space_per_second)
            || (lease.count == Count::Exact
                && !space_shard
                    .count_rate
                    .admit(limits().count_starts_per_space_per_second))
        {
            return Err(rate_limited());
        }
        let previous = lease
            .previous
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let runtime = tokio::runtime::Handle::current();
        let request_state = state.clone();
        let read_lease = lease.clone();
        let revision = views.revision.clone();
        let binding = views.binding(view)?;
        let WindowRequest::Window { request: range } = &request;
        let scope = if range.bind == binding.bind {
            binding.scope.clone()
        } else {
            WindowScope::Unqualified
        };
        let (response, _work) = blocking(&state, move |store| {
            let response = store
                .content()
                .window(space, &request, previous.as_deref(), || {
                    if !read_lease.valid(&revision)
                        || !session.is_live_blocking(&request_state.directory, &runtime)
                        || !request_state
                            .directory
                            .capabilities(who.id, space)
                            .is_some_and(|caps| caps.contains(Capability::SpaceRead))
                        || !read_lease.valid(&revision)
                    {
                        return Err(hidden());
                    }
                    Ok(WindowAccess {
                        scope: scope.clone(),
                        audience: read_lease.audience,
                        count: read_lease.count.policy(),
                        view: read_lease.partition,
                    })
                })?;
            // The blocking query owns admission even if HTTP disconnects.
            Ok((response, work))
        })
        .await?;
        if !lease.valid(&views.revision) {
            return Err(hidden());
        }
        let encoded = match response {
            WindowReply::Result(result) => {
                let encoded = private(&result, StatusCode::OK);
                *lease.previous.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(result));
                encoded
            }
            WindowReply::Loading(result) => {
                *lease.previous.lock().unwrap_or_else(|e| e.into_inner()) = None;
                private(result, StatusCode::OK)
            }
            WindowReply::PlanError(error) => private(error, StatusCode::UNPROCESSABLE_ENTITY),
        };
        if !lease.valid(&views.revision) {
            return Err(hidden());
        }
        Ok(encoded)
    }
    .await;
    match answer {
        Ok(response) => response,
        Err(error) => failure(None, error),
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Renew {
    LeaseRenew {
        lease_ref: Uuid,
        cache_partition: Uuid,
        grant_revision: String,
        renewal_id: String,
        result_versions: HashMap<String, String>,
    },
}

async fn renew<D: Directory>(
    State(state): State<Shared<D>>,
    Extension(who): Extension<Identity>,
    Extension(session): Extension<Session>,
    Extension(views): Extension<Arc<Views>>,
    Path((space, view)): Path<(SpaceId, View)>,
    Json(Renew::LeaseRenew {
        lease_ref,
        cache_partition,
        grant_revision,
        renewal_id,
        result_versions,
    }): Json<Renew>,
) -> Response {
    let answer = async {
        let valid_ref = |reference: &str| !reference.is_empty() && reference.chars().count() <= 128;
        if !valid_ref(&renewal_id) || result_versions.len() > limits().binds_per_view
            || result_versions.iter().any(|(key, value)| !valid_ref(key) || !valid_ref(value))
        {
            return Err(Error::invalid_input("invalid lease renewal range"));
        }
        admit(&state, &who, space).await?;
        if !session.is_live(&state.directory).await { return Err(hidden()); }
        let mut leases = views.actor(who.id).leases.lock().unwrap_or_else(|e| e.into_inner());
        let lease = leases.get(&(who.id, space, lease_ref)).cloned().ok_or_else(hidden)?;
        if !lease.valid(&views.revision)
            || lease.session != session
            || lease.view != view
            || lease.reference != lease_ref
            || lease.partition != cache_partition
            || format!("{:x}", lease.revision) != grant_revision
        {
            return Err(hidden());
        }
        let previous = lease.previous.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let renewed = Arc::new(Lease {
            reference: Uuid::new_v4(), partition: lease.partition, view, revision: lease.revision,
            expires: Instant::now() + Duration::from_secs(limits().subscriber_session_lease_seconds),
            session, audience: lease.audience, count: lease.count, replaced: AtomicBool::new(false),
            work: lease.work.clone(), rate: lease.rate.clone(), previous: Mutex::new(previous),
            _space_quota: lease._space_quota.clone(),
        });
        lease.replaced.store(true, Ordering::Release);
        leases.remove(&(who.id, space, lease_ref));
        leases.insert((who.id, space, renewed.reference), renewed.clone());
        // The renderer must reload rows. The private continuation is checked
        // against the current scope version when processing the next window.
        Ok(json!({
            "lease":renewed.wire(), "retained_results":[], "windows":[], "activation_contexts":{},
            "renewal_id":renewal_id, "request_lease_ref":lease_ref, "cache_partition":cache_partition,
        }))
    }.await;
    match answer {
        Ok(value) => private(value, StatusCode::OK),
        Err(error) => failure(None, error),
    }
}
