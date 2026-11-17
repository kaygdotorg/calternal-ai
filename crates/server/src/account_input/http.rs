//! One private account SSE route, account inferred exclusively from authentication.
use calternal_types::Error;
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
use serde::Deserialize;
use std::collections::VecDeque;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FollowQuery {
    bookmark: Option<String>,
}
struct Follow<D: Directory> {
    state: Shared<D>,
    p: Identity,
    session: Session,
    after: String,
    page: VecDeque<String>,
    ready: bool,
    done: bool,
    _admission: std::fs::File,
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
    let st = state.clone();
    let identity = p.clone();
    let head = tokio::task::spawn_blocking(move || {
        let mut c = st.inputs.open(identity.id)?;
        st.inputs.clean(&mut c, &st.directory, &identity)?;
        Ok::<_, Error>((Inputs::head(&c)?, st.inputs.admit(identity.id)?))
    })
    .await;
    let (head, admission) = match head {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return error_response(e),
        Err(e) => return error_response(internal(e)),
    };
    let follow = Follow {
        state,
        p,
        session,
        after: q.bookmark.unwrap_or(head),
        page: VecDeque::new(),
        ready: false,
        done: false,
        _admission: admission,
    };
    let body = Body::from_stream(futures_util::stream::unfold(follow, |mut f| async move {
        if f.done {
            return None;
        }
        loop {
            if !f.session.is_live(&f.state.directory).await {
                return None;
            }
            if let Some(bookmark) = f.page.pop_front() {
                f.after = bookmark.clone();
                return Some((
                    Ok::<_, std::convert::Infallible>(encode(AccountFrame::Changed { bookmark })),
                    f,
                ));
            }
            let st = f.state.clone();
            let p = f.p.clone();
            let after = f.after.clone();
            let page = tokio::task::spawn_blocking(move || {
                let mut c = st.inputs.open(p.id)?;
                st.inputs.clean(&mut c, &st.directory, &p)?;
                Inputs::page(&c, &after)
            })
            .await
            .ok()?
            .ok()?;
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
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
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
