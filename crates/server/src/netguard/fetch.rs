//! HTTPS URL admission, explicit redirect handling, and limited response reads.

use reqwest::{RequestBuilder, Response, StatusCode, Url, header};
use std::io;

/// Accept a canonical HTTPS hostname on the default port with no URL credentials
/// or fragment. Cost: validation of at most 2 KiB of input, with no DNS lookup.
/// The checked-address client separately verifies every DNS answer at connect.
pub(crate) fn check_url(url: &Url) -> io::Result<()> {
    if url.as_str().len() > 2048
        || url.scheme() != "https"
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !url
            .domain()
            .is_some_and(|name| name.contains('.') && !name.ends_with('.'))
    {
        return Err(super::refused());
    }
    Ok(())
}

/// Parse and check one of the exact approved hostnames. No network access.
/// A caller must still use the checked-address client when sending the request.
pub(crate) fn checked_url(raw: &str, hosts: &[&str]) -> Option<Url> {
    if raw.len() > 2048 {
        return None;
    }
    let url = Url::parse(raw).ok()?;
    check_url(&url).ok()?;
    url.domain().filter(|name| hosts.contains(name))?;
    Some(url)
}

/// Send a GET through a checked-address client with automatic redirects disabled.
/// Revalidate the canonical URL and await caller authorization before each hop.
/// Build every request anew so the caller can bind credentials to its own origin.
/// Exhausting the redirect allowance refuses another hop. One hop costs
/// one grant check and one request. Callers may wrap the entire transfer in a
/// deadline that includes redirects and the final bounded body read.
pub(crate) async fn get_with_redirects(
    mut url: Url,
    mut redirects: u8,
    mut authorize: impl AsyncFnMut(&Url) -> io::Result<()>,
    mut request: impl FnMut(Url) -> RequestBuilder,
) -> io::Result<Response> {
    loop {
        check_url(&url)?;
        authorize(&url).await?;
        let response = request(url.clone())
            .send()
            .await
            .map_err(io::Error::other)?;
        if !matches!(
            response.status(),
            StatusCode::MOVED_PERMANENTLY
                | StatusCode::FOUND
                | StatusCode::SEE_OTHER
                | StatusCode::TEMPORARY_REDIRECT
                | StatusCode::PERMANENT_REDIRECT
        ) {
            return Ok(response);
        }
        if redirects == 0 {
            return Err(super::refused());
        }
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .filter(|value| value.len() <= 2048)
            .ok_or_else(super::refused)?;
        url = url.join(location).map_err(|_| super::refused())?;
        redirects -= 1;
    }
}

/// Read a response body without retaining more than the caller's byte ceiling.
/// Refuse oversized Content-Length early and enforce the same limit for a
/// chunked or unknown-length body. Return allocation or body errors to the
/// protocol adapter so it can choose its own caller-safe status.
pub(crate) async fn read_bounded(mut response: Response, cap: usize) -> io::Result<Vec<u8>> {
    let oversized = || io::Error::other("upstream response exceeds its byte limit");
    if response
        .content_length()
        .is_some_and(|bytes| bytes > cap as u64)
    {
        return Err(oversized());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(io::Error::other)? {
        if chunk.len() > cap - bytes.len() {
            return Err(oversized());
        }
        let needed = bytes.len() + chunk.len();
        if needed > bytes.capacity() {
            let capacity = bytes.capacity().saturating_mul(2).max(needed).min(cap);
            bytes
                .try_reserve_exact(capacity - bytes.len())
                .map_err(io::Error::other)?;
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        response::{IntoResponse, Redirect},
        routing::get,
    };

    #[tokio::test]
    async fn redirect_requires_fresh_authorization_before_contacting_the_next_host() {
        let app = Router::new()
            .route(
                "/initial",
                get(|| async { Redirect::temporary("https://assets.example.org/next") }),
            )
            .route("/next", get(|| async { "unused" }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let mut checked = Vec::new();
        let mut sent = Vec::new();
        let result = get_with_redirects(
            Url::parse("https://site.example.org/initial").unwrap(),
            3,
            async |url| {
                let host = url.domain().unwrap().to_owned();
                checked.push(host.clone());
                if host == "site.example.org" {
                    Ok(())
                } else {
                    Err(super::super::refused())
                }
            },
            |url| {
                sent.push(url.path().to_owned());
                client.get(format!("http://{origin}{}", url.path()))
            },
        )
        .await;
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(checked, ["site.example.org", "assets.example.org"]);
        assert_eq!(sent, ["/initial"]);
        task.abort();
    }

    #[tokio::test]
    async fn bounded_reader_enforces_the_limit_without_content_length() {
        let app = Router::new().route(
            "/",
            get(|| async {
                let chunks = futures_util::stream::iter([
                    Ok::<_, std::convert::Infallible>("four"),
                    Ok("more"),
                ]);
                axum::body::Body::from_stream(chunks).into_response()
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let fetch = || client.get(format!("http://{origin}/")).send();
        assert!(read_bounded(fetch().await.unwrap(), 7).await.is_err());
        assert_eq!(
            read_bounded(fetch().await.unwrap(), 8).await.unwrap(),
            b"fourmore"
        );
        task.abort();
    }
}
