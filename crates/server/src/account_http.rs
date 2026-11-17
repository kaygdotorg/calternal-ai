//! Trusted account HTTP endpoints ported from the original auth transport.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use axum::{
    Json, Router,
    body::to_bytes,
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header::*},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use calternal_auth::{AccountId, Auth, AuthConfig, Error, ErrorCode, InstallationId};
use serde::Deserialize;
use serde_json::{Value, json};
use std::net::SocketAddr;

/// Account service and exact origin/proxy settings shared by transport guards.
#[derive(Clone)]
pub struct HttpAuth {
    /// Protected account service; clones share ceremony pools and revocation fences.
    pub auth: Auth,
    /// Origin, lifetime and explicit proxy trust configuration used to open `auth`.
    pub config: AuthConfig,
}
const HOST_COOKIE_NAME: &str = "__Host-calternal_session";
type Reply = Result<Response, Response>;

// Catalogue text and retry semantics from source core/error.rs:106–223.
fn failure(status: u16, code: &str, _diagnostic: &str) -> Response {
    let (message, retryable) = match code {
        "invalid_input" => ("Something in that request doesn't look right.", false),
        "unauthorized" => ("Please sign in to continue.", false),
        "not_found" => ("We couldn't find that.", false),
        "forbidden" => ("That request isn't allowed from here.", false),
        "passkey_failed" => ("That didn't work. Please try your passkey again.", true),
        "too_many_requests" => ("Too many tries. Please wait a moment and try again.", true),
        "invite_invalid" => ("That invite link doesn't work. Ask for a new one.", false),
        "setup_token_invalid" => (
            "That setup token doesn't work. It may be used up or expired.",
            false,
        ),
        "last_passkey" => (
            "That's your only passkey. Add another one before removing it.",
            false,
        ),
        "passkey_reverification_required" => ("Verify with your passkey to continue.", false),
        "too_large" => ("That's too big to send.", false),
        "conflict" => (
            "That request key was already used for a different change.",
            false,
        ),
        _ => (
            "Something went wrong on our side. Trying again might help.",
            true,
        ),
    };
    (StatusCode::from_u16(status).unwrap(),Json(json!({"error":{"code":code,"message":message,"retryable":retryable,"request_id":uuid::Uuid::now_v7().to_string()}}))).into_response()
}

fn error(e: Error) -> Response {
    let (status, code, message) = match e.code() {
        ErrorCode::InvalidInput => (400, "invalid_input", "Check the request and try again."),
        ErrorCode::NotFound => (404, "not_found", "That was not found."),
        ErrorCode::Forbidden => (403, "forbidden", "This request is not allowed."),
        ErrorCode::PasskeyFailed => (401, "passkey_failed", "The passkey could not be verified."),
        ErrorCode::TooManyRequests => (429, "too_many_requests", "Wait a moment and try again."),
        ErrorCode::SetupTokenInvalid => (
            400,
            "setup_token_invalid",
            "That setup link is invalid or expired.",
        ),
        ErrorCode::LastPasskey => (409, "last_passkey", "Keep at least one passkey."),
        ErrorCode::PasskeyReverificationRequired => (
            403,
            "passkey_reverification_required",
            "Verify your passkey again.",
        ),
        ErrorCode::Conflict => (
            409,
            "conflict",
            "This request conflicts with an earlier request.",
        ),
        ErrorCode::Internal => (500, "internal", "Something went wrong. Try again."),
    };
    failure(status, code, message)
}
fn unauthorized() -> Response {
    failure(401, "unauthorized", "Sign in to continue.")
}

/// Builds trusted account endpoints; merge before the API fallback.
pub fn routes<S: Clone + Send + Sync + 'static>(service: HttpAuth) -> Router<S> {
    Router::new()
        .route("/api/v1/auth/status", get(status))
        .route("/api/v1/auth/setup/start", post(setup_start))
        .route("/api/v1/auth/setup/finish", post(setup_finish))
        .route("/api/v1/auth/login/start", post(login_start))
        .route("/api/v1/auth/login/finish", post(login_finish))
        .route("/api/v1/auth/reverify/start", post(reverify_start))
        .route("/api/v1/auth/reverify/finish", post(reverify_finish))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/me", get(me))
        .route("/api/v1/auth/installations", get(installations))
        .route("/api/v1/auth/installations/revoke", post(revoke))
        .route("/api/v1/auth/passkeys", get(passkeys))
        .route("/api/v1/auth/passkeys/add/start", post(add_start))
        .route("/api/v1/auth/passkeys/add/finish", post(add_finish))
        .route("/api/v1/auth/passkeys/rename", post(rename))
        .route("/api/v1/auth/passkeys/remove", post(remove))
        .with_state(service)
}
fn guard(service: &HttpAuth, headers: &HeaderMap) -> Result<(), Response> {
    if !headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(';')
                .next()
                .is_some_and(|m| m.trim().eq_ignore_ascii_case("application/json"))
        })
    {
        return Err(failure(
            400,
            "invalid_input",
            "Sign-in requests must be JSON.",
        ));
    }
    if foreign(service, headers) {
        return Err(failure(403, "forbidden", "Request from another origin."));
    }
    Ok(())
}
fn foreign(service: &HttpAuth, headers: &HeaderMap) -> bool {
    headers
        .get(ORIGIN)
        .is_some_and(|o| o.as_bytes() != service.config.origin_header().as_bytes())
}
async fn body<T: serde::de::DeserializeOwned>(
    service: &HttpAuth,
    req: Request,
) -> Result<T, Response> {
    guard(service, req.headers())?;
    let bytes = to_bytes(req.into_body(), 64 * 1024)
        .await
        .map_err(|_| failure(413, "too_large", "Sign-in body exceeds 64 KiB."))?;
    serde_json::from_slice(&bytes)
        .map_err(|_| failure(400, "invalid_input", "Check the JSON request."))
}
fn cookies(headers: &HeaderMap) -> Vec<(&str, &str)> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|h| h.to_str().ok())
        .flat_map(|h| h.split(';'))
        .filter_map(|p| p.trim().split_once('='))
        .filter(|(n, _)| *n == HOST_COOKIE_NAME)
        .collect()
}
fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}
/// Identifies a live session only; grants always come from the request directory.
pub async fn identify(
    service: &HttpAuth,
    headers: &HeaderMap,
) -> Option<(AccountId, InstallationId)> {
    let token = if let Some(b) = bearer(headers) {
        b.trim()
    } else {
        if foreign(service, headers) {
            return None;
        }
        let c: Vec<_> = cookies(headers)
            .into_iter()
            .filter(|(n, _)| *n == HOST_COOKIE_NAME)
            .collect();
        if c.len() != 1 {
            return None;
        }
        c[0].1
    };
    service.auth.identify(token).await
}
async fn session(
    service: &HttpAuth,
    headers: &HeaderMap,
) -> Result<(AccountId, InstallationId), Response> {
    identify(service, headers).await.ok_or_else(unauthorized)
}
fn signed_in(service: &HttpAuth, account: AccountId, token: &str) -> Response {
    let mut response = Json(json!({"user":{"id":account}})).into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{HOST_COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}; Secure",
            service.config.session_ttl.as_secs()
        ))
        .unwrap(),
    );
    response
}
/// Refuse duplicate host cookies across the merged router, without clearing session cookies.
pub async fn cookie_middleware(
    State(service): State<HttpAuth>,
    req: Request,
    next: Next,
) -> Response {
    let api = req.uri().path() == "/api" || req.uri().path().starts_with("/api/");
    let c = cookies(req.headers());
    let browser = bearer(req.headers()).is_none() && !foreign(&service, req.headers());
    let ambiguous = browser && c.iter().filter(|(n, _)| *n == HOST_COOKIE_NAME).count() > 1;
    let mut response = if ambiguous {
        unauthorized()
    } else {
        next.run(req).await
    };
    if api || ambiguous {
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}
/// Transport peer rate-limit key with source proxy-chain and IPv6 /64 rules.
pub fn source_key(service: &HttpAuth, req: &Request) -> String {
    let Some(peer) = req.extensions().get::<ConnectInfo<SocketAddr>>() else {
        return "unknown".into();
    };
    let mut client = peer.0.ip();
    if service.config.trusts_proxy(client) {
        let values = req.headers().get_all("x-forwarded-for");
        let parsed = values
            .iter()
            .map(|v| v.to_str().ok())
            .collect::<Option<Vec<_>>>()
            .and_then(|v| {
                v.iter()
                    .flat_map(|v| v.split(','))
                    .map(|v| v.trim().parse::<std::net::IpAddr>().ok())
                    .collect::<Option<Vec<_>>>()
            });
        if let Some(chain) = parsed {
            for hop in chain.into_iter().rev() {
                if !service.config.trusts_proxy(client) {
                    break;
                }
                client = hop;
            }
        }
    }
    match client {
        std::net::IpAddr::V4(ip) => ip.to_string(),
        std::net::IpAddr::V6(ip) => {
            ip.to_ipv4_mapped()
                .map(|ip| ip.to_string())
                .unwrap_or_else(|| {
                    format!(
                        "{}/64",
                        std::net::Ipv6Addr::from(u128::from(ip) & (u128::MAX << 64))
                    )
                })
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetupStart {
    token: String,
    #[serde(default)]
    name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Finish {
    ceremony: String,
    credential: Value,
    #[serde(default)]
    device: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddFinish {
    ceremony: String,
    credential: Value,
    #[serde(default)]
    name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Key {
    id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rename {
    id: String,
    name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revoke {
    installation: InstallationId,
}
async fn status(State(s): State<HttpAuth>, req: Request) -> Reply {
    Ok(Json(json!({"setup":s.auth.setup_needed().await.map_err(error)?,"signed_in":identify(&s,req.headers()).await.is_some()})).into_response())
}
async fn setup_start(State(s): State<HttpAuth>, req: Request) -> Reply {
    let source = source_key(&s, &req);
    let i: SetupStart = body(&s, req).await?;
    let (_, ceremony, options) = s
        .auth
        .start_setup(&i.token, &i.name, &source)
        .await
        .map_err(|e| {
            if e.code() == ErrorCode::SetupTokenInvalid {
                failure(400, "invite_invalid", "invalid setup candidate")
            } else {
                error(e)
            }
        })?;
    Ok(Json(json!({"ceremony":ceremony,"options":options})).into_response())
}
async fn setup_finish(State(s): State<HttpAuth>, req: Request) -> Reply {
    let i: Finish = body(&s, req).await?;
    let (a, _, t) = s
        .auth
        .finish_http_setup(&i.ceremony, i.credential, &i.device)
        .await
        .map_err(error)?;
    Ok(signed_in(&s, a, &t))
}
async fn start(s: HttpAuth, req: Request, reverify: bool) -> Reply {
    let source = source_key(&s, &req);
    s.auth.admit_http_login(&source).map_err(error)?;
    let expected = if reverify {
        Some(session(&s, req.headers()).await?.0)
    } else {
        None
    };
    // The source accepts an empty body or any valid JSON on login/start.
    guard(&s, req.headers())?;
    let bytes = to_bytes(req.into_body(), 64 * 1024)
        .await
        .map_err(|_| failure(413, "too_large", "Sign-in body exceeds 64 KiB."))?;
    if !bytes.is_empty() {
        serde_json::from_slice::<Value>(&bytes)
            .map_err(|_| failure(400, "invalid_input", "Check the JSON request."))?;
    }
    let (ceremony, options) = s.auth.start_http_login(expected, &source).map_err(error)?;
    Ok(Json(json!({"ceremony":ceremony,"options":options})).into_response())
}
async fn login_start(State(s): State<HttpAuth>, req: Request) -> Reply {
    start(s, req, false).await
}
async fn reverify_start(State(s): State<HttpAuth>, req: Request) -> Reply {
    start(s, req, true).await
}
async fn finish(s: HttpAuth, req: Request, reverify: bool) -> Reply {
    let expected = if reverify {
        Some(session(&s, req.headers()).await?.0)
    } else {
        None
    };
    let i: Finish = body(&s, req).await?;
    let (a, _, t) = s
        .auth
        .finish_http_login(expected, &i.ceremony, i.credential, &i.device)
        .await
        .map_err(error)?;
    Ok(signed_in(&s, a, &t))
}
async fn login_finish(State(s): State<HttpAuth>, req: Request) -> Reply {
    finish(s, req, false).await
}
async fn reverify_finish(State(s): State<HttpAuth>, req: Request) -> Reply {
    finish(s, req, true).await
}
async fn logout(State(s): State<HttpAuth>, req: Request) -> Reply {
    guard(&s, req.headers())?;
    if let Some((a, i)) = identify(&s, req.headers()).await {
        s.auth.revoke(a, i).await.map_err(error)?;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert("clear-site-data", HeaderValue::from_static("\"cache\""));
    Ok(response)
}
async fn me(State(s): State<HttpAuth>, req: Request) -> Reply {
    let (a, i) = session(&s, req.headers()).await?;
    Ok(
        Json(json!({"user":s.auth.account_profile(a).await.map_err(error)?,"installation":i}))
            .into_response(),
    )
}
async fn installations(State(s): State<HttpAuth>, req: Request) -> Reply {
    let (a, current) = session(&s, req.headers()).await?;
    let rows = s
        .auth
        .installations(a)
        .await
        .map_err(error)?
        .into_iter()
        .map(|i| {
            let mut v = serde_json::to_value(&i).unwrap();
            v["current"] = json!(i.id == current);
            v
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({"installations":rows})).into_response())
}
async fn revoke(State(s): State<HttpAuth>, req: Request) -> Reply {
    let (a, _) = session(&s, req.headers()).await?;
    let i: Revoke = body(&s, req).await?;
    s.auth.revoke(a, i.installation).await.map_err(error)?;
    Ok(Json(json!({"revoked":i.installation})).into_response())
}
async fn passkeys(State(s): State<HttpAuth>, req: Request) -> Reply {
    let (a, _) = session(&s, req.headers()).await?;
    Ok(Json(json!({"passkeys":s.auth.passkeys(a).await.map_err(error)?})).into_response())
}
async fn add_start(State(s): State<HttpAuth>, req: Request) -> Reply {
    let (a, i) = session(&s, req.headers()).await?;
    let _: Value = body(&s, req).await?;
    let (ceremony, options) = s.auth.start_add_passkey(a, i).await.map_err(error)?;
    Ok(Json(json!({"ceremony":ceremony,"options":options})).into_response())
}
async fn add_finish(State(s): State<HttpAuth>, req: Request) -> Reply {
    let (a, install) = session(&s, req.headers()).await?;
    let i: AddFinish = body(&s, req).await?;
    let row = s
        .auth
        .finish_add_passkey(a, install, &i.ceremony, i.credential, &i.name)
        .await
        .map_err(error)?;
    Ok(Json(json!({"passkey":row})).into_response())
}
async fn rename(State(s): State<HttpAuth>, req: Request) -> Reply {
    let (a, _) = session(&s, req.headers()).await?;
    let i: Rename = body(&s, req).await?;
    Ok(
        Json(
            json!({"id":i.id,"name":s.auth.rename_passkey(a,&i.id,&i.name).await.map_err(error)?}),
        )
        .into_response(),
    )
}
async fn remove(State(s): State<HttpAuth>, req: Request) -> Reply {
    let (a, install) = session(&s, req.headers()).await?;
    let i: Key = body(&s, req).await?;
    s.auth
        .remove_passkey(a, install, &i.id)
        .await
        .map_err(error)?;
    Ok(Json(json!({"removed":i.id})).into_response())
}
