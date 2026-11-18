//! Shared bounded I/O and outbound admission for same-origin binary assets.
use axum::{
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use calternal_types::Error;
use reqwest::{RequestBuilder, Url, header};
use std::{
    collections::VecDeque,
    fs::File,
    io::{self, Read},
    path::Path,
    time::{Duration, Instant},
};

/// An admitted binary response or 304 carrying its own cache policy.
#[derive(Clone)]
pub(crate) struct CacheResponse;

pub(super) fn respond(
    bytes: Vec<u8>,
    content_type: &'static str,
    etag: &str,
    policy: &'static str,
    vary_by_credentials: bool,
    request: &HeaderMap,
) -> Response {
    let unchanged = request
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|h| h.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .any(|tag| tag == "*" || tag.strip_prefix("W/").unwrap_or(tag) == etag);
    let mut response = if unchanged {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        ([(header::CONTENT_TYPE, content_type)], bytes).into_response()
    };
    response.extensions_mut().insert(CacheResponse);
    let headers = response.headers_mut();
    headers.insert(header::ETAG, HeaderValue::from_str(etag).expect("hex ETag"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(policy));
    headers.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    if vary_by_credentials {
        headers.insert(
            header::VARY,
            HeaderValue::from_static("Cookie, Authorization"),
        );
    }
    response
}

pub(super) fn read(path: &Path, cap: usize) -> io::Result<Option<Vec<u8>>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut bytes = Vec::new();
    file.take(cap as u64 + 1).read_to_end(&mut bytes)?;
    Ok((bytes.len() <= cap).then_some(bytes))
}

pub(super) fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    calternal_fsio::write_atomic(path, bytes)
}

pub(super) async fn io_job<T: Send + 'static>(
    f: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> Result<T, Error> {
    crate::blocking::spawn(f)
        .await
        .map_err(|_| Error::internal("asset cache task stopped"))?
        .map_err(|error| {
            tracing::warn!(%error, "asset cache I/O");
            Error::internal("asset cache could not be accessed")
        })
}

pub(super) fn checked_url(text: &str, hosts: &[&str]) -> Option<Url> {
    let url = Url::parse(text).ok()?;
    (text.len() <= 2048
        && url.scheme() == "https"
        && url.host_str().is_some_and(|host| hosts.contains(&host))
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none())
    .then_some(url)
}

pub(super) async fn get(
    request: RequestBuilder,
    cap: usize,
    content_type: Option<&str>,
    failure: fn() -> Error,
) -> Result<Vec<u8>, Error> {
    let mut reply = request.send().await.map_err(|_| failure())?;
    if reply.status() != reqwest::StatusCode::OK
        || reply.content_length().is_some_and(|n| n > cap as u64)
        || content_type.is_some_and(|expected| {
            reply
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                != Some(expected)
        })
    {
        return Err(failure());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = reply.chunk().await.map_err(|_| failure())? {
        if chunk.len() > cap - bytes.len() {
            return Err(failure());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Rolling installation budget, with optional per-caller ceiling. Its memory
/// stays bounded by the installation ceiling and it charges failed attempts.
pub(super) struct Budget<K>(VecDeque<(Instant, K)>);

impl<K: PartialEq> Budget<K> {
    pub(super) fn new() -> Self {
        Self(VecDeque::new())
    }

    pub(super) fn admit(&mut self, key: K, total: usize, per_key: usize) -> bool {
        self.admit_at(Instant::now(), key, total, per_key)
    }

    pub(in crate::api) fn admit_at(
        &mut self,
        now: Instant,
        key: K,
        total: usize,
        per_key: usize,
    ) -> bool {
        while self
            .0
            .front()
            .is_some_and(|(at, _)| now.duration_since(*at) >= Duration::from_secs(3600))
        {
            self.0.pop_front();
        }
        if self.0.len() >= total
            || self.0.iter().filter(|(_, item)| *item == key).count() >= per_key
        {
            return false;
        }
        self.0.push_back((now, key));
        true
    }
}
