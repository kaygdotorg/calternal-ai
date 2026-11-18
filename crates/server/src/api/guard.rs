//! Host allowlist, Origin and session checks and security headers for every response.
use super::*;

#[cfg(feature = "perf-timings")]
tokio::task_local! {
    pub(super) static OPERATION_ENTRY_NS: u64;
}

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
    #[cfg(feature = "perf-timings")]
    let entry_ns = (path == "/api/v1/operations").then(calternal_types::perf_monotonic_ns);
    let api = is_api(&path);
    let guest_read = path.starts_with("/s/") && matches!(*req.method(), Method::GET | Method::HEAD);
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

        if !account_route
            && !guest_read
            && let Some(origin) = req.headers().get(header::ORIGIN)
        {
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
        if !guest_read
            && req
                .headers()
                .get("sec-fetch-site")
                .and_then(|v| v.to_str().ok())
                == Some("cross-site")
        {
            break 'resp deny(Error::forbidden("cross-site request denied"));
        }
        let sign_in_wallpaper = super::wallpapers::is_sign_in_request(req.method(), &path);
        if api && !account_route && !sign_in_wallpaper {
            // The installation travels with the request so plugin work can recheck it.
            let session = if let Some(service) = state.directory.http_auth() {
                crate::account_http::identify(service, req.headers())
                    .await
                    .map(|(id, installation)| {
                        (Identity { id }, Session::installation(id, installation))
                    })
            } else {
                None
            };
            let thread_token = if let Some(raw) = bearer(req.headers())
                .and_then(|b| std::str::from_utf8(b).ok())
                .filter(|t| t.starts_with("ct1_"))
            {
                if path != "/api/v1/operations"
                    && path != "/api/v1/describe"
                    && !(path.starts_with("/api/v1/jobs/") && path.ends_with("/events"))
                {
                    break 'resp deny(Error::forbidden(
                        "thread tokens may only call operations or discovery",
                    ));
                }
                let (st, token) = (state.clone(), raw.to_owned());
                crate::blocking::spawn(move || {
                    st.jobs
                        .agent_authenticate(&token)
                        .map(|a| a.map(|a| (a, calternal_auth::token::hash32(&token).to_vec())))
                })
                .await
                .ok()
                .and_then(Result::ok)
                .flatten()
            } else {
                None
            };
            if let Some((a, _)) = &thread_token {
                req.extensions_mut().insert(a.clone());
            }
            match thread_token
                .map(|(a, hash)| {
                    (
                        Identity {
                            id: a.actor.account,
                        },
                        Session::Agent {
                            space: a.space,
                            job: a.job,
                            token_hash: hash,
                        },
                    )
                })
                .or(session)
                .or(
                    super::people::development_identity(&state.directory, req.headers())
                        .await
                        .map(|p| (p, Session::Static)),
                ) {
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
        let agent_admitted = req
            .extensions()
            .get::<crate::turns::access::Authority>()
            .cloned();
        let response = next.run(req);
        #[cfg(feature = "perf-timings")]
        let response = async {
            if let Some(entry_ns) = entry_ns {
                OPERATION_ENTRY_NS.scope(entry_ns, response).await
            } else {
                response.await
            }
        };
        let response = response.await;
        if api
            && !account_route
            && !sign_in_wallpaper
            && (response.status().is_success()
                || response
                    .extensions()
                    .get::<super::wallpapers::ImageResponse>()
                    .is_some())
            && !(agent_admitted.is_some()
                && response
                    .extensions()
                    .get::<ParkedAcknowledgement>()
                    .is_some())
        {
            let session = if let Some(service) = state.directory.http_auth() {
                crate::account_http::identify(service, &credentials)
                    .await
                    .map(|(id, _)| Identity { id })
            } else {
                None
            };
            let agent_current = if agent_admitted.is_some() {
                let token = bearer(&credentials)
                    .and_then(|b| std::str::from_utf8(b).ok())
                    .unwrap_or("")
                    .to_owned();
                let st = state.clone();
                crate::blocking::spawn(move || st.jobs.agent_authenticate(&token))
                    .await
                    .ok()
                    .and_then(Result::ok)
                    .flatten()
                    .map(|a| Identity {
                        id: a.actor.account,
                    })
            } else {
                None
            };
            let current = agent_current.or(session);
            let current = match current {
                Some(current) => Some(current),
                None => super::people::development_identity(&state.directory, &credentials).await,
            };
            if current.as_ref().map(|p| &p.id) != admitted.as_ref().map(|p| &p.id)
                || current.is_none()
            {
                break 'resp deny(Error::unauthorized("missing or invalid session"));
            }
        }
        response
    };
    let mut resp = resp;
    let cacheable_image = resp
        .extensions()
        .get::<super::wallpapers::ImageResponse>()
        .is_some();
    let h = resp.headers_mut();
    if !h.contains_key(header::CONTENT_SECURITY_POLICY) {
        h.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(CSP),
        );
    }
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    if api && !cacheable_image {
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    resp
}

/// A response containing only the request id and pending status after the
/// operation itself ended the issuing run. All other responses recheck the
/// original credential normally before delivering data.
#[derive(Clone)]
pub(super) struct ParkedAcknowledgement;

pub(super) fn bearer(headers: &HeaderMap) -> Option<&[u8]> {
    let v = headers.get(header::AUTHORIZATION)?.as_bytes();
    let rest = v.strip_prefix(b"Bearer ")?;
    if rest.is_empty() { None } else { Some(rest) }
}
