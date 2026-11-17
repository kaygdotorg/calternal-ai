//! Static serving with a fallback limited to Calternal application addresses.
use crate::plugins::Registry;
use axum::{
    Router,
    extract::Request,
    http::{Method, StatusCode},
    response::IntoResponse,
};
use std::{collections::HashSet, path::PathBuf, sync::Arc};
use tower_http::services::{ServeDir, ServeFile};
use uuid::Uuid;

/// Serve existing files first. A missing GET or HEAD at an application address
/// serves the shell, leaving API handlers and missing asset paths independent.
/// Cost: one static lookup and, for an app fallback, one index.html lookup.
pub(super) fn assets(dir: PathBuf, registry: &Registry) -> ServeDir<Router> {
    let kinds: Arc<HashSet<_>> = Arc::new(
        registry
            .plugins()
            .iter()
            .flat_map(|plugin| plugin.manifest.kinds.iter().map(|kind| kind.id.clone()))
            .collect(),
    );
    let index = ServeFile::new(dir.join("index.html"));
    let fallback = Router::new().fallback(move |req: Request| {
        let kinds = kinds.clone();
        let mut index = index.clone();
        async move {
            if !matches!(*req.method(), Method::GET | Method::HEAD)
                || !app_address(req.uri().path(), &kinds)
            {
                return StatusCode::NOT_FOUND.into_response();
            }
            match index.try_call(req).await {
                Ok(response) => response.into_response(),
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            }
        }
    });
    ServeDir::new(dir).fallback(fallback)
}

fn app_address(path: &str, kinds: &HashSet<String>) -> bool {
    let Some(path) = path.strip_prefix('/') else {
        return false;
    };
    let mut segments = path.split('/');
    let kind = segments.next().unwrap_or("");
    let id = segments.next();
    let child = segments.next();
    if segments.next().is_some() {
        return false;
    }
    let uuid = |value: &str| value.len() == 36 && Uuid::parse_str(value).is_ok();
    match (kind, id, child) {
        ("activity" | "settings" | "notes" | "threads", None, None) => true,
        ("spaces" | "people" | "collections" | "files", Some(id), None) => uuid(id),
        ("spaces", Some(id), Some("members" | "plugins" | "settings")) => uuid(id),
        ("invites", Some(secret), None) => !secret.is_empty(),
        ("settings", Some(id), child) => {
            let key = |value: &str| {
                !value.is_empty()
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            };
            key(id) && child.is_none_or(key)
        }
        (kind, Some(id), block) if kinds.contains(kind) || matches!(kind, "notes" | "threads") => {
            uuid(id) && block.is_none_or(uuid)
        }
        _ => false,
    }
}
