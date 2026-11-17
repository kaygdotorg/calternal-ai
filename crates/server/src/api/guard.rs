//! Host allowlist, Origin and session checks and security headers for every response.
use super::*;


pub(super) fn is_api(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/")
}

/// Host allowlist (DNS-rebinding defense), configured Origin checks, current session
/// or explicitly enabled development bearer auth, and security headers on every response.
/// Successful operation/discovery delivery revalidates the originating credential
/// after the handler finishes, discarding work whose session was revoked meanwhile.
pub(super) async fn guard<D: Directory>(
    State(state): State<Shared<D>>,
    mut req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let api = is_api(&path);
    let deny = |e: Error| -> Response {
        if api {
            failure(None, e)
        } else {
            (
                StatusCode::from_u16(e.code.http_status()).unwrap(),
                e.message,
            )
                .into_response()
        }
    };
    let resp = 'resp: {
        let host = req
            .headers()
            .get(header::HOST)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        if !state.allowed_hosts.iter().any(|h| h == host) {
            break 'resp deny(Error::forbidden("host not allowed"));
        }
        // Machine protocol routes authenticate the machine themselves.
        let account_route = path.starts_with("/api/v1/auth/")
            || path.starts_with(calternal_machine::protocol::PATH_PREFIX);
        if api
            && !account_route
            && bearer(req.headers()).is_none()
            && let Some(service) = state.directory.http_auth()
            && req
                .headers()
                .get(header::ORIGIN)
                .is_some_and(|v| v.as_bytes() != service.config.origin_header().as_bytes())
        {
            break 'resp deny(Error::unauthorized("missing or invalid session"));
        }

        if !account_route && let Some(origin) = req.headers().get(header::ORIGIN) {
            let expected = if bearer(req.headers())
                .and_then(|t| state.directory.authenticate(t))
                .is_some()
            {
                format!("http://{host}")
            } else {
                state
                    .directory
                    .http_auth()
                    .map(|s| s.config.origin_header())
                    .unwrap_or_else(|| format!("http://{host}"))
            };
            if origin.to_str().ok() != Some(expected.as_str()) {
                break 'resp deny(Error::forbidden("cross-origin request denied"));
            }
        }
        if req
            .headers()
            .get("sec-fetch-site")
            .and_then(|v| v.to_str().ok())
            == Some("cross-site")
        {
            break 'resp deny(Error::forbidden("cross-site request denied"));
        }
        if api && !account_route {
            // The installation travels with the request so plugin work can recheck it.
            let session = if let Some(service) = state.directory.http_auth() {
                crate::account_http::identify(service, req.headers())
                    .await
                    .map(|(id, installation)| (Identity { id }, Session::installation(id, installation)))
            } else {
                None
            };
            match session.or_else(|| {
                bearer(req.headers())
                    .and_then(|t| state.directory.authenticate(t))
                    .map(|p| (p, Session::Static))
            }) {
                Some((p, session)) => {
                    req.extensions_mut().insert(p);
                    req.extensions_mut().insert(session);
                }
                None => {
                    break 'resp deny(Error::unauthorized("missing or invalid session"));
                }
            }
        }
        let credentials = req.headers().clone();
        let admitted = req.extensions().get::<Identity>().cloned();
        let response = next.run(req).await;
        if api && !account_route && response.status().is_success() {
            let session = if let Some(service) = state.directory.http_auth() {
                crate::account_http::identify(service, &credentials)
                    .await
                    .map(|(id, _)| Identity { id })
            } else {
                None
            };
            let current =
                session.or_else(|| bearer(&credentials).and_then(|t| state.directory.authenticate(t)));
            if current.as_ref().map(|p| &p.id) != admitted.as_ref().map(|p| &p.id)
                || current.is_none()
            {
                break 'resp deny(Error::unauthorized("missing or invalid session"));
            }
        }
        response
    };
    let mut resp = resp;
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    if api {
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    resp
}

pub(super) fn bearer(headers: &HeaderMap) -> Option<&[u8]> {
    let v = headers.get(header::AUTHORIZATION)?.as_bytes();
    let rest = v.strip_prefix(b"Bearer ")?;
    if rest.is_empty() { None } else { Some(rest) }
}

