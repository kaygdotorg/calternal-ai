//! Same-origin images for the fixed wallpaper catalog. The only anonymous
//! requests admitted by the API guard are the default's sign-in previews.

mod cache;
mod catalog;
mod upstream;

use axum::{
    Extension,
    extract::Path,
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use calternal_core::{JsonObject, WallpapersListInput};
use calternal_types::Error;
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, Semaphore};

const PREFIX: &str = "/api/v1/wallpapers/";
pub(super) const ROUTE: &str = "/api/v1/wallpapers/{id}/{rendition}";

/// Server-owned wallpaper configuration and cache. Clones share the outbound
/// budget, cache miss coordination and active request bound.
#[derive(Clone)]
pub struct Wallpapers(Option<Arc<Configured>>);

struct Configured {
    dir: PathBuf,
    upstream: upstream::Upstream,
    photos: [Mutex<PhotoState>; catalog::PHOTOS.len()],
    active: Arc<Semaphore>,
}

/// Only fills of one photo serialize across network requests and cache writes.
/// Warm reads bypass the lock. Publishing while holding it prevents concurrent
/// callers from fetching and flushing the same rendition or tracking twice.
#[derive(Default)]
struct PhotoState {
    source: Option<upstream::Source>,
    tracked: bool,
}

impl Wallpapers {
    /// Loads the access key from `CALTERNAL_UNSPLASH_KEY` and opens the bounded
    /// appdata cache off the async executor. An absent or blank key disables
    /// fetches. An invalid key header or inaccessible cache fails initialization
    /// with a redacted message. Startup costs one bounded-directory scan.
    pub async fn from_env(appdata: &std::path::Path) -> Result<Self, Error> {
        let key = match std::env::var("CALTERNAL_UNSPLASH_KEY") {
            Ok(key) if !key.trim().is_empty() => key,
            Ok(_) | Err(std::env::VarError::NotPresent) => return Ok(Self(None)),
            Err(_) => {
                return Err(Error::invalid_input(
                    "invalid Unsplash access key configuration",
                ));
            }
        };
        Self::open(appdata, upstream::Upstream::new(key.trim())?).await
    }

    async fn open(appdata: &std::path::Path, upstream: upstream::Upstream) -> Result<Self, Error> {
        Ok(Self(Some(Arc::new(Configured {
            dir: cache::open(appdata).await?,
            upstream,
            photos: std::array::from_fn(|_| Mutex::default()),
            active: Arc::new(Semaphore::new(16)),
        }))))
    }

    async fn rendition(&self, slot: usize, r: catalog::Rendition) -> Result<cache::Image, Error> {
        let service = self.0.clone().ok_or_else(not_configured)?;
        let permit = service
            .active
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::unavailable("wallpaper service is busy"))?;
        let id = catalog::PHOTOS[slot].0;
        let path = service.dir.join(format!("{id}.{}", r.name));
        if let Some(image) = cache::read(path.clone(), r.max_bytes).await? {
            return Ok(image);
        }
        // Once a fill starts, finish publication even if the HTTP client leaves.
        // The owned permit also bounds fills waiting for a photo's lock.
        tokio::spawn(async move {
            let _permit = permit;
            service.fill(slot, r, path).await
        })
        .await
        .map_err(|_| Error::internal("wallpaper fetch task stopped"))?
    }
}

impl Configured {
    async fn fill(
        &self,
        slot: usize,
        r: catalog::Rendition,
        path: PathBuf,
    ) -> Result<cache::Image, Error> {
        let mut photo = self.photos[slot].lock().await;
        if let Some(image) = cache::read(path.clone(), r.max_bytes).await? {
            return Ok(image);
        }
        let id = catalog::PHOTOS[slot].0;
        if photo.source.is_none() {
            photo.source = Some(self.upstream.source(id).await?);
        }
        let PhotoState { source, tracked } = &mut *photo;
        let source = source.as_ref().expect("resolved source");
        if r.name != "thumb" && !*tracked {
            let marker = self.dir.join(format!("{id}.tracked"));
            if !cache::tracked(marker.clone()).await? {
                self.upstream.track(source).await?;
                cache::mark_tracked(marker).await?;
            }
            *tracked = true;
        }
        let bytes = self.upstream.image(source, r).await?;
        cache::put(path, bytes).await
    }
}

fn not_configured() -> Error {
    Error::not_found("Unsplash wallpapers are not configured")
}

pub(super) fn list(input: JsonObject) -> Result<Value, Error> {
    let _: WallpapersListInput = super::dispatch::decode(input)?;
    Ok(catalog::LIST.clone())
}

pub(super) fn is_sign_in_request(method: &Method, path: &str) -> bool {
    matches!(*method, Method::GET | Method::HEAD)
        && path
            .strip_prefix(PREFIX)
            .and_then(|rest| rest.split_once('/'))
            .is_some_and(|(id, rendition)| catalog::public(id, rendition))
}

pub(super) async fn image(
    Path((id, name)): Path<(String, String)>,
    service: Option<Extension<Wallpapers>>,
    headers: HeaderMap,
) -> Response {
    let result = async {
        let (slot, rendition) = catalog::find(&id, &name)
            .ok_or_else(|| Error::not_found("wallpaper rendition not found"))?;
        let Extension(service) = service.ok_or_else(not_configured)?;
        let image = service.rendition(slot, rendition).await?;
        Ok::<_, Error>(respond(image, catalog::public(&id, &name), &headers))
    }
    .await;
    result.unwrap_or_else(super::error_response)
}

/// Marks image responses and their 304 replies for the API cache policy.
#[derive(Clone)]
pub(crate) struct ImageResponse;

fn respond(image: cache::Image, public: bool, request: &HeaderMap) -> Response {
    let unchanged = request
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|h| h.to_str().ok())
        .flat_map(|h| h.split(','))
        .map(str::trim)
        .any(|tag| tag == "*" || tag.strip_prefix("W/").unwrap_or(tag) == image.etag);
    let mut response = if unchanged {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        ([(header::CONTENT_TYPE, "image/jpeg")], image.bytes).into_response()
    };
    response.extensions_mut().insert(ImageResponse);
    let headers = response.headers_mut();
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&image.etag).expect("hex ETag"),
    );
    let scope = if public {
        "public, max-age=2592000"
    } else {
        "private, max-age=2592000"
    };
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(scope));
    headers.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    if !public {
        headers.insert(
            header::VARY,
            HeaderValue::from_static("Cookie, Authorization"),
        );
    }
    response
}

#[cfg(test)]
mod tests;
