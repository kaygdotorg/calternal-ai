//! Client surface of plugin jobs: `jobs.get`, `jobs.cancel` and the live stream.
//!
//! The stream is server-sent events at `GET /api/v1/jobs/<job-id>/events`. SSE was
//! chosen over a WebSocket because the stream is one-way, the server's HTTP stack
//! (axum 0.8 with only `http1`, `tokio` and `json`) needs no upgrade feature or new
//! dependency for it, the same guard (Host, Origin, cookie session) applies as to
//! any API request, and browsers reconnect by themselves with `Last-Event-ID`.
//!
//! Events, each `id: <epoch>:<seq>` where it applies:
//! - `snapshot` — the durable record ([`calternal_core::JobView`]) plus the stream
//!   `epoch`; always first.
//! - `chunk` — `{ seq, data }`, in order.
//! - `reset` — `{ epoch, from }`: the chunks the client asked for are gone (another
//!   run started, or they were evicted from the bounded buffer). The client keeps
//!   the snapshot's checkpoint and continues from `from`.
//! - `state` — manual turns only: retry state and pending approval requests.
//! - `end` — the final record; the stream then closes.
//!
//! A client resumes with `?after=<epoch>:<seq>` (or `Last-Event-ID`). Every delivery
//! rechecks the subscriber's sign-in, its `space.read` in the job's space and the
//! plugin's activation at the job's generation; on failure the stream closes. A job
//! the caller cannot see is the same not-found as a missing one.

use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use calternal_core::{JobInput, JobResult};
use serde_json::{Value, json};

use super::jobs::Job;
use super::live::Subscription;
use super::{Session, activation, runner};
use crate::api::Shared;
use crate::auth::{Directory, Identity};
use calternal_types::Error;
use calternal_types::{Capability, SpaceId};

/// Longest wait between deliveries; an idle stream sends a comment and rechecks access.
pub const KEEPALIVE: Duration = Duration::from_secs(15);

fn hidden() -> Error {
    Error::not_found("job not found")
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    crate::blocking::spawn(f)
        .await
        .map_err(|_| Error::internal("job task failed"))?
}

/// The space UUID of a job, for policy admission. Cost: one shared map read.
pub fn route<D: Directory>(
    state: &Shared<D>,
    input: &calternal_core::JsonObject,
) -> Result<String, Error> {
    let id = input.get("job_id").and_then(Value::as_str).unwrap_or("");
    state.jobs.route(id).ok_or_else(hidden)
}

/// Whether `identity` may still see `job`: live sign-in, `required` in its space and
/// the plugin enabled at the job's generation. Cost: one session check, one
/// directory read and one activation read on a blocking task.
async fn visible<D: Directory>(
    state: &Shared<D>,
    identity: &Identity,
    session: &Session,
    job: &Job,
    required: Capability,
) -> bool {
    if job.operation == "agent.turn" && job.input["check"] == true && job.account != identity.id {
        return false;
    }
    if let Session::Agent { space, job: id, .. } = session {
        if space != &job.space || id != &job.id || identity.id != job.account {
            return false;
        }
    }
    if !session.is_live_in(state).await {
        return false;
    }
    let Ok(space) = SpaceId::parse(&job.space) else {
        return false;
    };
    let (st, account, plugin, generation) = (
        state.clone(),
        identity.id,
        job.plugin.clone(),
        job.generation,
    );
    blocking(move || {
        let caps = st.directory.capabilities(account, space);
        let Some(plugin) = st.registry.get(&plugin) else {
            return Ok(false);
        };
        let row = activation(&st.store, space, plugin)?;
        Ok(caps.is_some_and(|c| c.contains(required))
            && row.enabled
            && row.generation == generation)
    })
    .await
    .unwrap_or(false)
}

async fn load<D: Directory>(state: &Shared<D>, space: &str, id: &str) -> Result<Job, Error> {
    let (st, sp, jid) = (state.clone(), space.to_owned(), id.to_owned());
    blocking(move || st.jobs.get(&sp, &jid))
        .await?
        .ok_or_else(hidden)
}

/// `jobs.get` and `jobs.cancel` after policy admission in `space` (a UUID).
/// Cancelling needs the starting account or `plugins.manage`; it cancels the job and
/// its active descendants and wakes a parent they unblock.
/// Cost: one indexed read; cancel adds one FULL transaction, and a caller
/// other than the starter one capability read on a blocking worker.
pub async fn dispatch<D: Directory>(
    state: Shared<D>,
    identity: Identity,
    op: &str,
    space: String,
    input: calternal_core::JsonObject,
) -> Result<Value, Error> {
    let i: JobInput = serde_json::from_value(Value::Object(input.into_iter().collect()))
        .map_err(|_| Error::invalid_input("input does not match operation schema"))?;
    let job = load(&state, &space, &i.job_id).await?;
    if job.operation == "agent.turn" && job.input["check"] == true && job.account != identity.id {
        return Err(hidden());
    }
    if op == "jobs.cancel" {
        let owner = job.account == identity.id || {
            // The directory is SQLite-backed: read it on a blocking worker.
            let space_id = SpaceId::parse(&space).map_err(|_| Error::not_found("job not found"))?;
            let (st, account) = (state.clone(), identity.id);
            blocking(move || {
                Ok(st
                    .directory
                    .capabilities(account, space_id)
                    .is_some_and(|c| c.contains(Capability::PluginsManage)))
            })
            .await?
        };
        if !owner {
            return Err(Error::forbidden(
                "only the job's starter or a plugin manager can cancel it",
            ));
        }
        let (st, sp, jid) = (state.clone(), space.clone(), i.job_id.clone());
        for woken in blocking(move || st.jobs.cancel(&sp, &jid)).await? {
            runner::schedule(&state, space.clone(), woken);
        }
    }
    let job = load(&state, &space, &i.job_id).await?;
    serde_json::to_value(JobResult {
        job: job.view_for(identity.id),
    })
    .map_err(|_| Error::internal("encode"))
}

/// Parse `<epoch>:<seq>` from the query or `Last-Event-ID`.
fn cursor(req: &Request) -> Option<(String, u64)> {
    let from_query = req.uri().query().and_then(|q| {
        q.split('&')
            .find_map(|kv| kv.strip_prefix("after="))
            .map(str::to_owned)
    });
    let raw = from_query.or_else(|| {
        req.headers()
            .get("last-event-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    })?;
    let (epoch, seq) = raw.split_once(':')?;
    Some((epoch.to_owned(), seq.parse().ok()?))
}

fn event(name: &str, id: Option<String>, data: &Value) -> String {
    let mut out = String::new();
    if let Some(id) = id {
        out.push_str(&format!("id: {id}\n"));
    }
    out.push_str(&format!("event: {name}\ndata: {data}\n\n"));
    out
}

/// `GET /api/v1/jobs/<id>/events`: the job's server-sent event stream.
pub async fn events<D: Directory>(
    State(state): State<Shared<D>>,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    let read_workers = state.blocking.clone();
    let identity = req.extensions().get::<Identity>().cloned();
    let session = req.extensions().get::<Session>().cloned();
    let (Some(identity), Some(session)) = (identity, session) else {
        return hidden_response();
    };
    let after = cursor(&req);
    let Some(space) = state.jobs.route(&id) else {
        return hidden_response();
    };
    let Ok(job) = load(&state, &space, &id).await else {
        return hidden_response();
    };
    if !visible(&state, &identity, &session, &job, Capability::SpaceRead).await {
        return hidden_response();
    }
    let sub = match state.jobs.subscribe(&id) {
        Ok(sub) => sub,
        Err(e) => return crate::api::error_response(e),
    };
    let stream = Stream {
        state,
        identity,
        session,
        space,
        id,
        sub,
        after,
        next: None,
        epoch: String::new(),
        started: false,
        done: false,
        last_state: Value::Null,
    };
    let body = Body::from_stream(read_workers.read_stream(futures_util::stream::unfold(
        stream,
        |mut s| async move {
            let frame = s.next_frame().await?;
            Some((Ok::<_, std::convert::Infallible>(frame), s))
        },
    )));
    let mut r = (StatusCode::OK, body).into_response();
    let h = r.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

fn hidden_response() -> Response {
    crate::api::error_response(hidden())
}

/// One subscriber's position in a job stream.
struct Stream<D: Directory> {
    state: Shared<D>,
    identity: Identity,
    session: Session,
    space: String,
    id: String,
    sub: Subscription,
    after: Option<(String, u64)>,
    next: Option<u64>,
    epoch: String,
    started: bool,
    done: bool,
    last_state: Value,
}

impl<D: Directory> Stream<D> {
    /// The next SSE frame, or `None` to close. Each call is one delivery: it rechecks
    /// access, then sends what is new. Cost: one indexed job read, one access check
    /// and a copy of the newly buffered chunks.
    async fn next_frame(&mut self) -> Option<String> {
        if self.done {
            return None;
        }
        let mut watch = self.sub.live.watch();
        loop {
            let job = load(&self.state, &self.space, &self.id).await.ok()?;
            if !visible(
                &self.state,
                &self.identity,
                &self.session,
                &job,
                Capability::SpaceRead,
            )
            .await
            {
                return None;
            }
            let batch = self.sub.live.read(0);
            let mut out = String::new();
            if !self.started {
                self.started = true;
                let snapshot =
                    json!({ "job": job.view_for(self.identity.id), "epoch": batch.epoch });
                out.push_str(&event("snapshot", None, &snapshot));
                self.next = match self.after.take() {
                    Some((epoch, seq)) if epoch == batch.epoch && seq + 1 >= batch.oldest => {
                        Some(seq + 1)
                    }
                    Some(_) => None,
                    None => Some(if job.operation == "agent.turn" {
                        batch.oldest
                    } else {
                        job.checkpoint_seq
                    }),
                };
                self.epoch = batch.epoch.clone();
            }
            if self.epoch != batch.epoch {
                self.epoch = batch.epoch.clone();
                self.next = None;
            }
            let want = match self.next {
                Some(n) if n >= batch.oldest => n,
                _ => {
                    // Asked-for chunks are gone: continue from the checkpoint if it is
                    // still buffered, else from the oldest chunk kept.
                    let from = job.checkpoint_seq.max(batch.oldest);
                    out.push_str(&event(
                        "reset",
                        None,
                        &json!({ "epoch": batch.epoch, "from": from }),
                    ));
                    from
                }
            };
            let mut next = want;
            for chunk in batch.chunks.iter().filter(|c| c.seq >= want) {
                let mut data: Value = serde_json::from_str(&chunk.data).unwrap_or(Value::Null);
                if job.operation == "agent.turn" && job.account != self.identity.id {
                    data = crate::turns::reason::public_event(data);
                }
                out.push_str(&event(
                    "chunk",
                    Some(format!("{}:{}", batch.epoch, chunk.seq)),
                    &json!({ "seq": chunk.seq, "data": data }),
                ));
                next = chunk.seq + 1;
            }
            self.next = Some(next);
            let current = json!({"job":job.view_for(self.identity.id)});
            if job.operation == "agent.turn" && current != self.last_state {
                self.last_state = current.clone();
                out.push_str(&event("state", None, &current));
            }
            if job.state.is_final() || job.state == calternal_core::JobState::Interrupted {
                self.done = true;
                out.push_str(&event(
                    "end",
                    None,
                    &json!({ "job": job.view_for(self.identity.id) }),
                ));
                return Some(out);
            }
            if !out.is_empty() {
                return Some(out);
            }
            match tokio::time::timeout(KEEPALIVE, watch.changed()).await {
                Ok(Ok(())) => continue,
                Ok(Err(_)) => return None,
                Err(_) => return Some(": keepalive\n\n".into()),
            }
        }
    }
}
