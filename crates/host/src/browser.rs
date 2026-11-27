//! Passkey and cookie HTTP transport for the personal-space host.

use crate::{Call, Host, WRITE_BYTES, internal};
use axum::body::to_bytes;
use axum::extract::{
    ConnectInfo, DefaultBodyLimit, Path, Request, State, rejection::JsonRejection,
};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::{
    Json, Router,
    routing::{get, post},
};
use calternal_auth::AuthConfig;
use calternal_types::{AccountId, Error, ErrorCode};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{net::SocketAddr, sync::Arc};

const SESSION: &str = "__Host-calternal_session";
const AUTH_BYTES: usize = 64 * 1024;
type Reply = Result<Response, Failure>;

/// Mount the standalone host's JSON and WebSocket routes. Configuration sets
/// the single trusted browser origin; serving HTTPS is the proxy's job.
pub fn router(host: Arc<Host>, config: AuthConfig) -> Router {
    let origin = config.origin_header();
    let auth = Router::new()
        .route("/v1/auth/status", get(status))
        .route("/v1/auth/setup/start", post(setup_start))
        .route("/v1/auth/setup/finish", post(setup_finish))
        .route("/v1/auth/login/start", post(login_start))
        .route("/v1/auth/login/finish", post(login_finish))
        .route("/v1/auth/logout", post(logout))
        .with_state(Browser {
            host: host.clone(),
            config: Arc::new(config),
        });
    Router::new()
        .route("/v1/spaces/{space}/call", post(call))
        .route("/v1/spaces/{space}/live", get(crate::live::ws_route))
        .layer(DefaultBodyLimit::max(WRITE_BYTES))
        .with_state(host)
        .merge(auth)
        .layer(middleware::from_fn_with_state(origin, origin_guard))
        .layer(middleware::from_fn(private))
}

#[derive(Clone)]
struct Browser {
    host: Arc<Host>,
    config: Arc<AuthConfig>,
}

async fn private(req: Request, next: Next) -> Response {
    let mut response = if cookies(req.headers()).nth(1).is_some() {
        Failure(Error::unauthorized("Sign in to use this host.")).into_response()
    } else {
        next.run(req).await
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn same_origin(headers: &HeaderMap, origin: &str, required: bool) -> bool {
    let mut origins = headers.get_all(header::ORIGIN).iter();
    let valid_origin = match origins.next() {
        Some(value) => value.as_bytes() == origin.as_bytes() && origins.next().is_none(),
        None => !required,
    };
    let mut sites = headers.get_all("sec-fetch-site").iter();
    valid_origin
        && match sites.next() {
            Some(value) => value == "same-origin" && sites.next().is_none(),
            None => true,
        }
}

async fn origin_guard(State(origin): State<String>, req: Request, next: Next) -> Response {
    let is_call = req.uri().path().ends_with("/call");
    let headers = req.headers();
    let cli_call = is_call
        && bearer(headers).is_some()
        && !headers.contains_key(header::COOKIE)
        && !headers.contains_key("sec-fetch-site");
    if !same_origin(
        headers,
        &origin,
        !req.method().is_safe() && !cli_call || req.uri().path().ends_with("/live"),
    ) {
        return Failure(Error::forbidden("Request from another origin.")).into_response();
    }
    next.run(req).await
}

fn cookies(headers: &HeaderMap) -> impl Iterator<Item = &str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|header| header.to_str().ok())
        .flat_map(|header| header.split(';'))
        .filter_map(|part| part.trim().split_once('='))
        .filter_map(|(key, value)| (key == SESSION).then_some(value))
}

pub(crate) fn cookie(headers: &HeaderMap) -> Option<&str> {
    let mut values = cookies(headers);
    let value = values.next()?;
    values.next().is_none().then_some(value)
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

async fn call(
    State(host): State<Arc<Host>>,
    Path(space): Path<String>,
    headers: HeaderMap,
    body: Result<Json<Call>, JsonRejection>,
) -> Reply {
    let credential = if headers.contains_key(header::COOKIE) {
        cookie(&headers)
    } else {
        bearer(&headers)
    }
    .ok_or_else(|| Error::unauthorized("Sign in to use this space."))?;
    let Json(call) = body.map_err(|error| {
        if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
            Error::new(ErrorCode::TooLarge, "The call exceeds 16 MiB.")
        } else {
            Error::invalid_input("Expected a JSON call envelope.")
        }
    })?;
    let value = host.session_call(credential, space.parse()?, call).await?;
    Ok(Json(json!({"result":value})).into_response())
}

async fn session_info(host: &Host, account: AccountId) -> Result<Value, Failure> {
    let (_, name) = host
        .auth
        .find(&account.to_string())
        .await
        .map_err(auth_error)?;
    let directory = host.auth.spaces();
    let space = tokio::task::spawn_blocking(move || directory.list(account))
        .await
        .map_err(internal)?
        .map_err(auth_error)?
        .into_iter()
        .find(|space| space.personal)
        .ok_or_else(|| Error::not_found("Personal space was not found."))?;
    Ok(json!({"account":account,"name":name,"space":space.id}))
}

async fn status(State(browser): State<Browser>, headers: HeaderMap) -> Reply {
    let account = match cookie(&headers) {
        Some(token) => browser.host.identify_session(token).await,
        None => None,
    };
    let setup = browser.host.auth.setup_needed().await.map_err(auth_error)?;
    let session = match account {
        Some(account) => session_info(&browser.host, account).await?,
        None => Value::Null,
    };
    Ok(Json(json!({"setup":setup,"session":session})).into_response())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Setup {
    token: String,
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
struct Empty {}

async fn body<T: serde::de::DeserializeOwned>(req: Request) -> Result<T, Failure> {
    if !req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
    {
        return Err(Error::invalid_input("Sign-in requests must be JSON.").into());
    }
    let bytes = to_bytes(req.into_body(), AUTH_BYTES)
        .await
        .map_err(|_| Error::new(ErrorCode::TooLarge, "Sign-in body exceeds 64 KiB."))?;
    Ok(serde_json::from_slice(&bytes)
        .map_err(|_| Error::invalid_input("Check the JSON request."))?)
}

async fn setup_start(
    State(browser): State<Browser>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request,
) -> Reply {
    let source = source(&browser.config, peer, req.headers());
    let Setup { token, name } = body(req).await?;
    let (_, ceremony, options) = browser
        .host
        .auth
        .start_setup(&token, &name, &source)
        .await
        .map_err(auth_error)?;
    Ok(Json(json!({"ceremony":ceremony,"options":options})).into_response())
}

async fn login_start(
    State(browser): State<Browser>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request,
) -> Reply {
    let source = source(&browser.config, peer, req.headers());
    let auth = &browser.host.auth;
    auth.admit_http_login(&source).map_err(auth_error)?;
    let Empty {} = body(req).await?;
    let (ceremony, options) = auth.start_http_login(None, &source).map_err(auth_error)?;
    Ok(Json(json!({"ceremony":ceremony,"options":options})).into_response())
}

fn source(config: &AuthConfig, peer: SocketAddr, headers: &HeaderMap) -> String {
    config.transport_source(
        peer.ip(),
        headers
            .get_all("x-forwarded-for")
            .iter()
            .map(|header| header.to_str().ok()),
    )
}

async fn setup_finish(State(browser): State<Browser>, req: Request) -> Reply {
    let Finish {
        ceremony,
        credential,
        device,
    } = body(req).await?;
    let (account, _, token, codes) = browser
        .host
        .auth
        .finish_http_setup(&ceremony, credential, &device)
        .await
        .map_err(auth_error)?;
    signed_in(&browser, account, token, codes).await
}

async fn login_finish(State(browser): State<Browser>, req: Request) -> Reply {
    let Finish {
        ceremony,
        credential,
        device,
    } = body(req).await?;
    let (account, _, token, codes) = browser
        .host
        .auth
        .finish_http_login(None, &ceremony, credential, &device)
        .await
        .map_err(auth_error)?;
    signed_in(&browser, account, token, codes).await
}

async fn signed_in(
    browser: &Browser,
    account: AccountId,
    token: String,
    codes: Vec<String>,
) -> Reply {
    let session = session_info(&browser.host, account).await?;
    let mut payload = json!({"session":session});
    if !codes.is_empty() {
        payload["recovery_codes"] = json!(codes);
    }
    Ok(session_cookie(
        Json(payload).into_response(),
        &token,
        browser.config.session_ttl.as_secs(),
    ))
}

async fn logout(State(browser): State<Browser>, headers: HeaderMap) -> Reply {
    if let Some(token) = cookie(&headers)
        && let Some((account, installation)) = browser.host.auth.identify(token).await
        && let Err(error) = browser.host.auth.revoke(account, installation).await
        && error.code() != calternal_auth::ErrorCode::NotFound
    {
        return Err(auth_error(error));
    }
    let mut response = session_cookie(StatusCode::NO_CONTENT.into_response(), "", 0);
    response
        .headers_mut()
        .insert("clear-site-data", HeaderValue::from_static(r#""cache""#));
    Ok(response)
}

fn session_cookie(mut response: Response, value: &str, max_age: u64) -> Response {
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{SESSION}={value}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age={max_age}"
        ))
        .expect("auth generates HTTP-safe session credentials"),
    );
    response
}

fn auth_error(error: calternal_auth::Error) -> Failure {
    use calternal_auth::ErrorCode::*;
    match error.code() {
        InvalidInput => Error::invalid_input("Check the sign-in request."),
        SetupTokenInvalid => Error::invalid_input("The setup code is invalid or expired."),
        PasskeyFailed => Error::unauthorized("The passkey could not be verified."),
        TooManyRequests => Error::new(ErrorCode::RetryAfter, "Wait a moment and try again."),
        NotFound => Error::not_found("The account or session was not found."),
        Forbidden | PasskeyReverificationRequired => {
            Error::forbidden("This request is not allowed.")
        }
        Conflict | InviteLimit | LastPasskey => {
            Error::conflict("The sign-in request conflicts with an earlier request.")
        }
        Internal => Error::internal("The host could not complete sign-in."),
    }
    .into()
}

struct Failure(Error);
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self(error)
    }
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        let error = self.0;
        let (code, next) = match error.code {
            ErrorCode::Unauthorized => ("needs-sign-in", "Sign in and try again."),
            ErrorCode::Forbidden | ErrorCode::PasskeyReverificationRequired => {
                ("not-allowed", "Check your access.")
            }
            ErrorCode::NotFound => ("not-found", "Check the requested ID."),
            ErrorCode::Conflict => ("conflict", "Refresh before retrying."),
            ErrorCode::InvalidInput => ("invalid", "Check the call arguments."),
            ErrorCode::TooLarge => ("too-large", "Reduce the request size."),
            ErrorCode::EffectiveScopeLimitExceeded | ErrorCode::RetryAfter => {
                ("limit", "Reduce the work or retry later.")
            }
            ErrorCode::Internal | ErrorCode::Unavailable => ("unreachable", "Try again later."),
        };
        let status = StatusCode::from_u16(error.code.http_status()).expect("shared HTTP status");
        let detail = |key| {
            error
                .details
                .as_ref()
                .and_then(|value| value.get(key))
                .and_then(Value::as_str)
        };
        (
            status,
            Json(json!({"error":{
                "code":detail("operation_code").unwrap_or(code),
                "message":error.message,
                "next":detail("next").unwrap_or(next)
            }})),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_mutations_require_the_exact_origin() {
        let mut headers = HeaderMap::new();
        assert!(!same_origin(&headers, "https://localhost", true));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://localhost"),
        );
        assert!(same_origin(&headers, "https://localhost", true));
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-site"));
        assert!(!same_origin(&headers, "https://localhost", true));
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        headers.append(
            header::ORIGIN,
            HeaderValue::from_static("https://localhost"),
        );
        assert!(!same_origin(&headers, "https://localhost", true));
    }

    #[test]
    fn rejects_ambiguous_session_cookies_and_clears_with_the_same_scope() {
        let mut headers = HeaderMap::new();
        headers.append(
            header::COOKIE,
            HeaderValue::from_static("__Host-calternal_session=first"),
        );
        assert_eq!(cookie(&headers), Some("first"));
        headers.append(
            header::COOKIE,
            HeaderValue::from_static("__Host-calternal_session=second"),
        );
        assert_eq!(cookie(&headers), None);
        let response = session_cookie(StatusCode::NO_CONTENT.into_response(), "", 0);
        assert_eq!(
            response.headers()[header::SET_COOKIE],
            "__Host-calternal_session=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0"
        );
    }
}
