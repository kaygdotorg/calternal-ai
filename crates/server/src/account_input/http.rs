//! One private account SSE route, account inferred exclusively from authentication.
use super::{Inputs, internal};
use crate::{
    api::{Shared, error_response},
    auth::{Directory, Identity},
    plugins::Session,
};
use axum::{
    body::Body,
    extract::{Query, Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use calternal_core::AccountFrame;
use calternal_types::Error;
use serde::Deserialize;
use std::collections::VecDeque;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FollowQuery {
    bookmark: Option<String>,
    #[serde(default)]
    readiness: bool,
}
struct Follow<D: Directory> {
    state: Shared<D>,
    p: Identity,
    session: Session,
    after: String,
    page: VecDeque<String>,
    ready: bool,
    wake: super::wake::Listener,
    changes: Vec<tokio::sync::watch::Receiver<u64>>,
    done: bool,
    reader: Option<super::reader::Reader<D>>,
    maintenance: bool,
    maintenance_at: tokio::time::Instant,
    readiness: Option<crate::machines::readiness::Listener>,
    readiness_pending: bool,
}
fn encode(frame: AccountFrame) -> String {
    format!(
        "data: {}\n\n",
        serde_json::to_string(&frame).expect("frame")
    )
}
pub(crate) async fn events<D: Directory>(
    State(state): State<Shared<D>>,
    Query(q): Query<FollowQuery>,
    req: Request,
) -> Response {
    let Some(p) = req.extensions().get::<Identity>().cloned() else {
        return error_response(Error::unauthorized("sign in required"));
    };
    let Some(session) = req.extensions().get::<Session>().cloned() else {
        return error_response(Error::unauthorized("sign in required"));
    };
    if q.bookmark.as_ref().is_some_and(|b| b.len() > 128) {
        return error_response(Error::invalid_input("invalid bookmark"));
    }
    // Subscribe before reading the head: a concurrent commit is either in replay
    // or leaves a wake hint. Every hint, including lag, triggers durable replay.
    let wake = state.inputs.wakes.listen(p.id);
    let machines = q
        .readiness
        .then(|| {
            req.extensions()
                .get::<std::sync::Arc<crate::machines::MachineService>>()
                .cloned()
        })
        .flatten();
    let st = state.clone();
    let identity = p.clone();
    let head = tokio::task::spawn_blocking(move || {
        let _step = st.inputs.step(identity.id)?;
        let mut c = st.inputs.open(identity.id)?;
        st.inputs.clean(&mut c, &st.directory, &identity)?;
        let head = Inputs::head(&c)?;
        let admission = st.inputs.admit(identity.id)?;
        let reader = super::reader::Reader::new(c, admission, st)?;
        let readiness = machines
            .map(|m| m.subscribe_readiness(identity.id))
            .transpose()?;
        Ok::<_, Error>((head, reader, readiness))
    })
    .await;
    let (head, reader, readiness) = match head {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return error_response(e),
        Err(e) => return error_response(internal(e)),
    };
    let changes = state.directory.changes();
    let follow = Follow {
        state,
        p,
        session,
        after: q.bookmark.unwrap_or(head),
        page: VecDeque::new(),
        ready: false,
        wake,
        changes,
        done: false,
        reader: Some(reader),
        maintenance: false,
        maintenance_at: tokio::time::Instant::now() + std::time::Duration::from_secs(1),
        readiness,
        readiness_pending: false,
    };
    let body = Body::from_stream(futures_util::stream::unfold(follow, |mut f| async move {
        if f.done {
            return None;
        }
        loop {
            if !f.session.is_live(&f.state.directory).await {
                return None;
            }
            if f.readiness_pending
                || f.readiness
                    .as_ref()
                    .is_some_and(|listener| listener.changes.has_changed().unwrap_or(false))
            {
                if let Some(listener) = &mut f.readiness {
                    listener.changes.borrow_and_update();
                }
                f.readiness_pending = false;
                return Some((Ok(encode(AccountFrame::ReadinessChanged)), f));
            }
            if let Some(bookmark) = f.page.pop_front() {
                f.after = bookmark.clone();
                return Some((
                    Ok::<_, std::convert::Infallible>(encode(AccountFrame::Changed { bookmark })),
                    f,
                ));
            }
            while matches!(
                f.wake.receiver.try_recv(),
                Ok(_) | Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_))
            ) {}
            let p = f.p.clone();
            let after = f.after.clone();
            let maintenance = f.maintenance
                || tokio::time::Instant::now() >= f.maintenance_at
                || f.changes
                    .iter()
                    .any(|change| change.has_changed().unwrap_or(true));
            if maintenance {
                for change in &mut f.changes {
                    let _ = change.borrow_and_update();
                }
            }
            let mut reader = f.reader.take()?;
            let (reader, page) = tokio::task::spawn_blocking(move || {
                let page = reader.page(&p, &after, maintenance);
                (reader, page)
            })
            .await
            .ok()?;
            f.reader = Some(reader);
            if maintenance {
                f.maintenance_at = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
            }
            f.maintenance = false;
            let page = page.ok()?;
            let Some(page) = page else {
                f.done = true;
                return Some((Ok(encode(AccountFrame::Reset)), f));
            };
            if !page.is_empty() {
                f.page = page.into();
                f.ready = false;
                continue;
            }
            if !f.ready {
                f.ready = true;
                let bookmark = f.after.clone();
                return Some((Ok(encode(AccountFrame::Ready { bookmark })), f));
            }
            let authority = async {
                if f.changes.is_empty() {
                    std::future::pending::<()>().await;
                }
                let _ = futures_util::future::select_all(
                    f.changes.iter_mut().map(|c| Box::pin(c.changed())),
                )
                .await;
            };
            let readiness = async {
                match &mut f.readiness {
                    Some(listener) => listener.changes.changed().await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                result = readiness => {
                    if result.is_err() { return None; }
                    // Loop through current-session validation before emitting.
                    f.readiness_pending = true;
                },
                _ = f.wake.receiver.recv() => {},
                _ = authority => { f.maintenance = true; },
                _ = tokio::time::sleep_until(f.maintenance_at) => {
                    f.maintenance = true;
                    // Bounds session expiry and age-based confirmation checks;
                    // commits wake immediately, independently of this keepalive.
                    return Some((Ok(": keepalive\n\n".into()), f));
                }
            }
        }
    }));
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
