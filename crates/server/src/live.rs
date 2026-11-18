//! Multiplexed SSE journal following, ported from the reference live route.
//!
//! SSE matches the existing jobs transport: this is a one-way feed, while writes
//! retain ordinary operation receipts. Subscribe before replay; broadcasts are
//! hints only. Every wake replays the database so lag cannot discard entries.
//! A connection pins at most 16 existing journals and buffers one 64-entry page.
//! Each journal admits at most 128 listeners (operator safety ceiling). Dropping
//! the HTTP body drops receivers and journal handles, releasing admission.
use crate::{
    api::Shared,
    auth::{Directory, Identity},
    plugins::Session,
};
use axum::{
    body::Body,
    extract::{Query, Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use calternal_core::{LiveFrame, LiveSubscription};
use calternal_store::{JournalEntry, SpaceStore};
use calternal_types::Error;
use calternal_types::{Capability, SpaceId};
use futures_util::future::select_all;
use serde::Deserialize;
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::sync::{broadcast, watch};

#[derive(Deserialize)]
pub(crate) struct LiveQuery {
    subscriptions: String,
}
struct Follow {
    space: String,
    journal: SpaceStore,
    receiver: broadcast::Receiver<Arc<JournalEntry>>,
    restored: watch::Receiver<u64>,
    after: i64,
    ready: bool,
    content: Arc<calternal_content::Content>,
    content_space: SpaceId,
}
struct Stream<D: Directory> {
    state: Shared<D>,
    identity: Identity,
    session: Session,
    follows: Vec<Follow>,
    changes: Vec<watch::Receiver<u64>>,
    page: VecDeque<(usize, JournalEntry)>,
    next_space: usize,
    done: bool,
}
fn hidden() -> Error {
    Error::not_found("space not found")
}
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    crate::blocking::spawn(f)
        .await
        .map_err(|_| Error::internal("live task failed"))?
}
async fn authorized<D: Directory>(
    state: &Shared<D>,
    identity: &Identity,
    session: &Session,
    space: SpaceId,
) -> bool {
    if !session.is_live(&state.directory).await {
        return false;
    }
    let (state, account) = (state.clone(), identity.id);
    blocking(move || {
        Ok(state
            .directory
            .capabilities(account, space)
            .is_some_and(|c| c.contains(Capability::SpaceRead)))
    })
    .await
    .unwrap_or(false)
}

/// Opens one authenticated SSE feed for JSON `subscriptions` query values.
/// Refuses absent and inaccessible spaces identically, before opening journals.
/// Cost: one authority read and one lazy handle lookup per space. Reads and
/// blocking bridges run off executor threads. Bookmarks are checked once at admit;
/// every delivery rechecks session, membership and identifier-only item routing.
pub(crate) async fn events<D: Directory>(
    State(state): State<Shared<D>>,
    axum::Extension(content): axum::Extension<Arc<calternal_content::Content>>,
    Query(query): Query<LiveQuery>,
    req: Request,
) -> Response {
    let read_workers = state.blocking.clone();
    let Some(identity) = req.extensions().get::<Identity>().cloned() else {
        return crate::api::error_response(hidden());
    };
    let Some(session) = req.extensions().get::<Session>().cloned() else {
        return crate::api::error_response(hidden());
    };
    let subscriptions =
        if query.subscriptions.len() <= state.inputs.limits().live_spaces.saturating_mul(2048) {
            serde_json::from_str::<Vec<LiveSubscription>>(&query.subscriptions).ok()
        } else {
            None
        };
    let Some(subscriptions) =
        subscriptions.filter(|s| !s.is_empty() && s.len() <= state.inputs.limits().live_spaces)
    else {
        return crate::api::error_response(Error::invalid_input(
            "live subscriptions exceed CALTERNAL_LIVE_SPACES or are empty",
        ));
    };
    let mut follows = Vec::new();
    let changes = state.directory.changes();
    for sub in subscriptions {
        let Ok(content_space) = SpaceId::parse(&sub.space_id) else {
            return crate::api::error_response(Error::invalid_input(
                "distinct space UUIDs required",
            ));
        };
        if follows.iter().any(|f: &Follow| f.space == sub.space_id) {
            return crate::api::error_response(Error::invalid_input(
                "distinct space UUIDs required",
            ));
        }
        if !authorized(&state, &identity, &session, content_space).await {
            return crate::api::error_response(hidden());
        }
        let service = content.clone();
        let journal = match blocking(move || {
            service
                .live_journal(content_space)
                .map_err(|_| Error::internal("journal open failed"))
        })
        .await
        {
            Ok(journal) => journal,
            Err(e) => return crate::api::error_response(e),
        };
        let receiver = match journal.subscribe_bounded(state.inputs.limits().listeners) {
            Ok(receiver) => receiver,
            Err(_) => {
                return crate::api::error_response(Error::unavailable(
                    "space live subscriber limit reached",
                ));
            }
        };
        let restored = journal.restored();
        let after = match sub.bookmark {
            Some(token) => journal.position(&token).await.unwrap_or(-1),
            None => journal.head().seq,
        };
        follows.push(Follow {
            space: sub.space_id,
            journal,
            receiver,
            restored,
            after,
            ready: false,
            content: content.clone(),
            content_space,
        });
    }
    let stream = Stream {
        state,
        identity,
        session,
        follows,
        changes,
        page: VecDeque::new(),
        next_space: 0,
        done: false,
    };
    let body = Body::from_stream(read_workers.read_stream(futures_util::stream::unfold(
        stream,
        |mut s| async move {
            let frame = s.next_frame().await?;
            Some((Ok::<_, std::convert::Infallible>(frame), s))
        },
    )));
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/event-stream"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response()
}
fn encode(frame: LiveFrame) -> String {
    format!(
        "data: {}\n\n",
        serde_json::to_string(&frame).expect("live frame")
    )
}
impl<D: Directory> Stream<D> {
    /// Replay uses one indexed journal read per 64 scanned entries, zero flushes,
    /// and one current session/member/item check per entry. Empty-page wakes check
    /// all subscribed spaces. Filtering may scan several pages before yielding;
    /// only a server-issued ready bookmark covers those hidden positions. No
    /// content body is loaded. Memory remains bounded to one page.
    async fn next_frame(&mut self) -> Option<String> {
        if self.done {
            return None;
        }
        loop {
            // Check all followed spaces on every wake, including an idle revoke.
            if self.page.is_empty() {
                for f in &self.follows {
                    if !authorized(&self.state, &self.identity, &self.session, f.content_space)
                        .await
                    {
                        return None;
                    }
                }
            }
            while let Some((index, mut entry)) = self.page.pop_front() {
                let follow = &mut self.follows[index];
                if follow.restored.has_changed().unwrap_or(true) {
                    self.done = true;
                    return Some(encode(LiveFrame::Reset {
                        space_id: follow.space.clone(),
                    }));
                }
                if !authorized(
                    &self.state,
                    &self.identity,
                    &self.session,
                    follow.content_space,
                )
                .await
                {
                    return None;
                }
                if !entry.item.is_empty() {
                    let content = follow.content.clone();
                    let space = follow.content_space;
                    let kind = crate::notes::journal_kind(&entry.kind)
                        .unwrap_or_else(|| entry.kind.split('.').next().unwrap_or(""))
                        .to_owned();
                    let id = uuid::Uuid::parse_str(&entry.item).ok()?;
                    let visible = blocking(move || {
                        content
                            .contains(space, &kind, id)
                            .map_err(|_| Error::internal("item check failed"))
                    })
                    .await
                    .ok()?;
                    if !visible {
                        follow.after = entry.seq;
                        continue;
                    }
                }
                if entry.kind == crate::threads::turn_events::KIND
                    && entry.author != self.identity.id
                {
                    entry.data["turn_reason"] = crate::turns::reason::public(std::mem::take(
                        &mut entry.data["turn_reason"],
                    ));
                }
                follow.after = entry.seq;
                let authored_by_deleted_account =
                    follow.content.lifecycle().is_closed(entry.author.as_uuid());
                let mut entry = serde_json::to_value(entry).ok()?;
                if authored_by_deleted_account {
                    entry["author_label"] = serde_json::Value::String("a deleted account".into());
                }
                return Some(encode(LiveFrame::Entry {
                    space_id: follow.space.clone(),
                    entry,
                }));
            }
            // Discard stale hints before indexed replay, so an already replayed
            // burst does not cause one empty database read per broadcast entry.
            for f in &mut self.follows {
                while matches!(
                    f.receiver.try_recv(),
                    Ok(_) | Err(broadcast::error::TryRecvError::Lagged(_))
                ) {}
            }
            for _ in 0..self.follows.len() {
                let index = self.next_space;
                self.next_space = (index + 1) % self.follows.len();
                let f = &mut self.follows[index];
                if f.after < 0 || f.restored.has_changed().unwrap_or(true) {
                    self.done = true;
                    return Some(encode(LiveFrame::Reset {
                        space_id: f.space.clone(),
                    }));
                }
                let entries = f.journal.since(f.after, 64).await.ok()?;
                if !entries.is_empty() {
                    f.ready = false;
                    self.page = entries.into_iter().map(|e| (index, e)).collect();
                    break;
                }
                if !f.ready {
                    f.ready = true;
                    return Some(encode(LiveFrame::Ready {
                        space_id: f.space.clone(),
                        bookmark: f.journal.token_at(f.after).await.ok()?,
                    }));
                }
            }
            if !self.page.is_empty() {
                continue;
            }
            // Broadcast lag is just another hint: the next pass reads the database.
            let journal = select_all(self.follows.iter_mut().map(|f| Box::pin(f.receiver.recv())));
            let authority = async {
                if self.changes.is_empty() {
                    std::future::pending::<()>().await;
                }
                let _ = select_all(self.changes.iter_mut().map(|c| Box::pin(c.changed()))).await;
            };
            tokio::select! {
                _ = journal => {},
                _ = authority => {},
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    // Also bounds expiry and separately opened directory detection.
                    return Some(": keepalive\n\n".into());
                }
            }
        }
    }
}
