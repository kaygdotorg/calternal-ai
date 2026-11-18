//! Trusted same-origin sealed documents. The general API guard handles identity.

use axum::{
    Extension,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use super::{asset_signature, current_frame, frame_path, missing};
use crate::{
    api::Shared,
    auth::{Directory, Identity},
};

pub(crate) const FRAME_ROUTE: &str = "/api/v1/plugin-content/{frame}";

/// Typed response extension inspected by the general header guard.
#[derive(Clone)]
pub(crate) struct SealedFrame(pub(crate) HeaderValue);

/// Render sanitized markup only for its authorized viewer. Reads current
/// activation and revision before returning bytes. Performs no outbound fetch.
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
