//! Static serving with a fallback limited to Calternal application addresses.
use crate::plugins::Registry;
use axum::{
    Router,
    extract::Request,
    http::{HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};
use tower_http::services::{ServeDir, ServeFile};
use uuid::Uuid;

/// Serve existing files first. A missing GET or HEAD at an application address
/// serves the shell, leaving API handlers and missing asset paths independent.
/// App shells and unversioned entry files revalidate on every use. Vite-hashed
/// assets are immutable for a year. Other files stay fresh for five minutes
/// and must revalidate afterward. Build timestamps cannot validate mutable files.
/// Cost: one static lookup and, for an app fallback, one index.html lookup.
pub(super) fn assets(dir: PathBuf, registry: &Registry) -> Router {
    let kinds: Arc<HashSet<_>> = Arc::new(
        registry
            .snapshot()
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
    Router::new()
        .fallback_service(ServeDir::new(dir).fallback(fallback))
        .layer(middleware::from_fn(cache_headers))
}

async fn cache_headers(mut req: Request, next: Next) -> Response {
    let path = req.uri().path();
    let hashed = is_hashed_asset(path);
    let entry = matches!(
        Path::new(path).extension().and_then(|ext| ext.to_str()),
        Some("html" | "js" | "mjs" | "css" | "webmanifest")
    ) || path == "/";
    if !hashed {
        // The same reproducible mtime can describe different bytes after a deploy.
        req.headers_mut().remove(header::IF_MODIFIED_SINCE);
    }

    let mut response = next.run(req).await;
    let status = response.status();
    let headers = response.headers_mut();
    if !hashed {
        headers.remove(header::LAST_MODIFIED);
    }
    let html = headers
        .get(header::CONTENT_TYPE)
        .is_some_and(|value| value.as_bytes().starts_with(b"text/html"));
    let policy = if !status.is_success() && status != StatusCode::NOT_MODIFIED {
        "no-store"
    } else if hashed {
        "public, max-age=31536000, immutable"
    } else if entry || html {
        "no-cache"
    } else {
        "public, max-age=300, must-revalidate"
    };
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(policy));
    response
}

fn is_hashed_asset(path: &str) -> bool {
    let Some(asset) = path.strip_prefix("/assets/") else {
        return false;
    };
    let file = asset.rsplit('/').next().unwrap_or_default();
    let Some((stem, _)) = file.rsplit_once('.') else {
        return false;
    };
    // Vite uses a hyphen followed by an eight-character URL-safe hash.
    let (name, hash) = stem.as_bytes().split_at(stem.len().saturating_sub(8));
    name.ends_with(b"-")
        && hash
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(*b, b'-' | b'_'))
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
        ("assets", _, _) => false,
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
