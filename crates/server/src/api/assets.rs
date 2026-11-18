//! HTTP responses and rolling admission budgets for same-origin binary assets.
use axum::{
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use std::{
    collections::VecDeque,
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
