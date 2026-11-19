//! Trusted same-origin sealed documents. The general API guard handles identity.

use axum::{
    Extension,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use super::{
    asset_signature, current_frame, db,
    eligibility::{self, Trigger},
    fetch, frame_path, missing,
};
use crate::{
    api::Shared,
    auth::{Directory, Identity},
};
use subtle::ConstantTimeEq;

/// Authenticated document GET and explicit-load POST.
pub(crate) const FRAME_ROUTE: &str = "/api/v1/plugin-content/{frame}";

/// Signed asset read for a sandboxed frame with an opaque browser origin.
pub(crate) const ASSET_ROUTE: &str = "/api/v1/plugin-content/{frame}/{signature}/{asset}";

/// Match only the media route shape for the narrow signed-read guard exception.
pub(crate) fn is_asset_read(method: &axum::http::Method, path: &str) -> bool {
    if !matches!(*method, axum::http::Method::GET | axum::http::Method::HEAD) {
        return false;
    }
    let Some(path) = path.strip_prefix("/api/v1/plugin-content/") else {
        return false;
    };
    let mut segments = path.split('/');
    matches!((segments.next(), segments.next(), segments.next(), segments.next()),
        (Some(frame), Some(signature), Some(asset), None)
        if Uuid::parse_str(frame).is_ok() && signature.len() == 64
            && signature.bytes().all(|byte| byte.is_ascii_hexdigit())
            && asset.parse::<usize>().is_ok())
}

/// Typed response extension inspected by the general header guard.
#[derive(Clone)]
pub(crate) struct SealedFrame(pub(crate) HeaderValue);

/// Render sanitized markup only for its authorized viewer. Reads current
/// activation and revision before returning bytes. An old-source open may
/// queue background work without waiting for any upstream request.
pub(crate) async fn document<D: Directory>(
    State(state): State<Shared<D>>,
    Extension(viewer): Extension<Identity>,
    Path(frame): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    let result = async {
        let st = state.clone();
        let frame_data = crate::blocking::spawn(move || current_frame(&st, frame, viewer.id))
            .await
            .map_err(|_| missing())??;
        if !frame_data.source.session.is_live_in(&state).await {
            return Err(missing());
        }
        schedule(&state, &frame_data, Trigger::Open).await;
        let origin = if let Some(http) = state.directory.http_auth() {
            http.config.origin_header()
        } else {
            let host = headers.get(header::HOST).and_then(|h| h.to_str().ok()).ok_or_else(missing)?;
            format!("http://{host}")
        };
        let assets = format!("{origin}{}/{}/", frame_path(frame), asset_signature(&state.store, frame));
        let csp = HeaderValue::from_str(&format!(
            "default-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors {origin}; sandbox; style-src 'unsafe-inline'; img-src {assets}; font-src {assets}"
        )).map_err(|_| missing())?;
        let html = format!("<!doctype html><html><head><meta charset=\"utf-8\"></head><body>{}</body></html>", frame_data.content.html);
        let mut response = ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response();
        response.extensions_mut().insert(SealedFrame(csp));
        Ok::<_, calternal_types::Error>(response)
    }.await;
    result.unwrap_or_else(crate::api::error_response)
}

async fn schedule<D: Directory>(
    state: &Shared<D>,
    frame: &std::sync::Arc<super::frames::Frame>,
    trigger: Trigger,
) {
    let (st, source) = (state.clone(), frame.source.clone());
    let eligible = crate::blocking::spawn(move || {
        db::preference(st.store.appdata(), source.space, source.account)
            .is_ok_and(|preference| eligibility::may_enqueue(preference, source.age, trigger))
    })
    .await
    .unwrap_or(false);
    if eligible {
        if matches!(trigger, Trigger::ExplicitLoad) {
            frame
                .load_approved
                .store(true, std::sync::atomic::Ordering::Release);
        }
        fetch::enqueue(
            &tokio::runtime::Handle::current(),
            state,
            std::sync::Arc::new(frame.source.clone()),
            &frame.content.assets,
            trigger,
        );
    }
}

/// Queue the viewer's explicit media load. The document and proxy reads
/// remain independent of the outbound worker's lifetime.
pub(crate) async fn load<D: Directory>(
    State(state): State<Shared<D>>,
    Extension(viewer): Extension<Identity>,
    Path(frame): Path<Uuid>,
) -> Response {
    let st = state.clone();
    let result = async {
        let current = crate::blocking::spawn(move || current_frame(&st, frame, viewer.id))
            .await
            .map_err(|_| missing())??;
        if !current.source.session.is_live_in(&state).await {
            return Err(missing());
        }
        schedule(&state, &current, Trigger::ExplicitLoad).await;
        Ok::<_, calternal_types::Error>(axum::http::StatusCode::ACCEPTED.into_response())
    }
    .await;
    result.unwrap_or_else(crate::api::error_response)
}

/// Serve only cached, typed media. The signature is scoped to this short-lived
/// frame and contains no upstream URL. Recheck live access after any cache wait.
pub(crate) async fn asset<D: Directory>(
    State(state): State<Shared<D>>,
    Path((frame, signature, index)): Path<(Uuid, String, usize)>,
) -> Response {
    let result = async {
        let expected = asset_signature(&state.store, frame);
        if !bool::from(expected.as_bytes().ct_eq(signature.as_bytes())) {
            return Err(missing());
        }
        let (current, entry) = ready_entry(&state, frame, index).await?;
        let asset = entry.read().await.ok_or_else(missing)?;
        let (after, _) = ready_entry(&state, frame, index).await?;
        if !std::sync::Arc::ptr_eq(&current, &after) {
            return Err(missing());
        }
        Ok::<_, calternal_types::Error>(
            (
                [
                    (header::CONTENT_TYPE, asset.kind),
                    (
                        header::CONTENT_SECURITY_POLICY,
                        "default-src 'none'; sandbox",
                    ),
                    (header::ACCESS_CONTROL_ALLOW_ORIGIN, "null"),
                ],
                asset.bytes,
            )
                .into_response(),
        )
    }
    .await;
    result.unwrap_or_else(crate::api::error_response)
}

async fn ready_entry<D: Directory>(
    state: &Shared<D>,
    id: Uuid,
    index: usize,
) -> Result<
    (
        std::sync::Arc<super::frames::Frame>,
        std::sync::Arc<super::cache::Entry>,
    ),
    calternal_types::Error,
> {
    let st = state.clone();
    let (frame, entry) = crate::blocking::spawn(move || {
        let frame = st.store.plugin_frames.get(id).ok_or_else(missing)?;
        let source = &frame.source;
        let lifecycle = st.store.content().lifecycle();
        let _space = lifecycle.step(source.space.as_uuid())?;
        let _account = lifecycle.step(source.account.as_uuid())?;
        super::check_source(&st, source)?;
        let preference = db::preference(st.store.appdata(), source.space, source.account)?;
        if matches!(source.age, eligibility::SourceAge::Junk)
            || preference == calternal_core::ContentFetchPreference::Never
            || (preference == calternal_core::ContentFetchPreference::Ask
                && !frame
                    .load_approved
                    .load(std::sync::atomic::Ordering::Acquire))
        {
            return Err(missing());
        }
        let url = frame.content.assets.get(index).ok_or_else(missing)?;
        let entry = st.store.plugin_fetch.get(source, url).ok_or_else(missing)?;
        Ok::<_, calternal_types::Error>((frame, entry))
    })
    .await
    .map_err(|_| missing())??;
    if !frame.source.session.is_live_in(state).await {
        return Err(missing());
    }
    Ok((frame, entry))
}
