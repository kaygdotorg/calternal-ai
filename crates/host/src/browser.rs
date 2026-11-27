//! Passkey and cookie HTTP transport for the personal-space host.

use crate::{Call, Host, WRITE_BYTES, internal};
use axum::body::to_bytes;
use axum::extract::{ConnectInfo, Path, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::{
    Json, Router,
    routing::{get, post},
};
use calternal_auth::AuthConfig;
use calternal_types::{AccountId, Error, ErrorCode, SpaceId};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

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

const BODY_DEADLINE: Duration = Duration::from_secs(10);
const MAX_LOGIN_WORK: usize = 8;

fn admit_login_work() -> Result<OwnedSemaphorePermit, Failure> {
    static ADMISSION: std::sync::LazyLock<Arc<Semaphore>> =
        std::sync::LazyLock::new(|| Arc::new(Semaphore::new(MAX_LOGIN_WORK)));
    ADMISSION.clone().try_acquire_owned().map_err(|_| {
        Error::new(
            ErrorCode::RetryAfter,
            "The host is processing too many requests.",
        )
        .into()
    })
}

async fn admit_http_call(
    host: &Arc<Host>,
    account: AccountId,
    space: SpaceId,
) -> Result<OwnedSemaphorePermit, Failure> {
    let host = host.clone();
    let slot = tokio::task::spawn_blocking(move || {
        // An authenticated caller can reserve work only for its personal space.
        // Avoid opening storage or occupying a slot for arbitrary path input.
        if !host
            .auth
            .spaces()
            .is_personal_space(account, space)
            .map_err(internal)?
        {
            return Err(Error::not_found("Personal space was not found."));
        }
        host.space_slot(space)
    })
    .await
    .map_err(internal)??;
    slot.http_work.clone().try_acquire_owned().map_err(|_| {
        Error::new(
            ErrorCode::RetryAfter,
            "This space is processing too many requests.",
        )
        .into()
    })
}

async fn call(State(host): State<Arc<Host>>, Path(space): Path<String>, req: Request) -> Reply {
    let headers = req.headers();
    let credential = if headers.contains_key(header::COOKIE) {
        cookie(headers)
    } else {
        bearer(headers)
    }
    .ok_or_else(|| Error::unauthorized("Sign in to use this space."))?;
    let session = host.authenticate(credential).await?;
    let space = space.parse()?;
    let permit = admit_http_call(&host, session.account, space).await?;
    let (call, permit) = {
        if !is_json(&req) {
            return Err(Error::invalid_input("Expected a JSON call envelope.").into());
        }
        let bytes = bounded_body(req, WRITE_BYTES, "The call exceeds 16 MiB.").await?;
        tokio::task::spawn_blocking(move || {
            let call = serde_json::from_slice::<Call>(&bytes)
                .map_err(|_| Error::invalid_input("Expected a JSON call envelope."))?;
            if call.call.len() > 256
                || call.slot_selection.as_ref().is_some_and(|selection| {
                    selection.slot.len() > 128 || selection.filler.len() > 128
                })
            {
                return Err(Error::invalid_input("Call or slot selector is too long."));
            }
            Ok((call, permit))
        })
        .await
        .map_err(internal)??
    };
    let value = host.authorized_call(session, space, call).await?;
    let encoded = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        serde_json::to_vec(&json!({"result":value})).map_err(internal)
    })
    .await
    .map_err(internal)??;
    Ok(([(header::CONTENT_TYPE, "application/json")], encoded).into_response())
}

async fn session_payload(host: &Arc<Host>, account: AccountId) -> Result<Value, Failure> {
    let (name, space) = personal_space(host, account).await?;
    let mut payload = json!({"session":{"account":account,"name":name,"space":space}});
    if !host.development_plugins.is_empty() {
        let host = host.clone();
        let seeded =
            tokio::task::spawn_blocking(move || host.ensure_development_plugins(account, space))
                .await;
        if !matches!(seeded, Ok(Ok(()))) {
            eprintln!("calternal-host: development bundle initialization is pending");
            payload["initialization"] = json!({
                "status":"pending",
                "retry":"Refresh your sign-in status to retry preview initialization."
            });
        }
    }
    Ok(payload)
}

async fn personal_space(
    host: &Host,
    account: AccountId,
) -> Result<(String, calternal_types::SpaceId), Failure> {
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
    Ok((name, space.id))
}

async fn status(State(browser): State<Browser>, headers: HeaderMap) -> Reply {
    let account = match cookie(&headers) {
        Some(token) => browser.host.identify_session(token).await,
        None => None,
    };
    let setup = browser.host.auth.setup_needed().await.map_err(auth_error)?;
    let mut payload = match account {
        Some(account) => session_payload(&browser.host, account).await?,
        None => json!({"session":null}),
    };
    payload["setup"] = json!(setup);
    Ok(Json(payload).into_response())
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

fn is_json(req: &Request) -> bool {
    req.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
}

async fn bounded_body(
    req: Request,
    limit: usize,
    too_large: &'static str,
) -> Result<axum::body::Bytes, Failure> {
    tokio::time::timeout(BODY_DEADLINE, to_bytes(req.into_body(), limit))
        .await
        .map_err(|_| Error::invalid_input("Request body did not arrive in time."))?
        .map_err(|_| Error::new(ErrorCode::TooLarge, too_large).into())
}

async fn body<T: serde::de::DeserializeOwned>(req: Request) -> Result<T, Failure> {
    if !is_json(&req) {
        return Err(Error::invalid_input("Sign-in requests must be JSON.").into());
    }
    let bytes = bounded_body(req, AUTH_BYTES, "Sign-in body exceeds 64 KiB.").await?;
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
    let permit = admit_login_work()?;
    let auth = browser.host.auth.clone();
    let (auth, source, permit) = tokio::task::spawn_blocking(move || {
        auth.admit_http_login(&source).map_err(auth_error)?;
        Ok::<_, Failure>((auth, source, permit))
    })
    .await
    .map_err(internal)??;
    let Empty {} = body(req).await?;
    let (ceremony, options) = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        auth.start_http_login(None, &source).map_err(auth_error)
    })
    .await
    .map_err(internal)??;
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
    let mut payload = session_payload(&browser.host, account).await?;
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
        && let Ok(session) = browser.host.authenticate(token).await
    {
        browser.host.logout(session).await.map_err(auth_error)?;
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

    #[tokio::test]
    async fn unauthenticated_call_rejects_before_deserializing_envelope() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let root = tempfile::tempdir().unwrap();
        let config = AuthConfig::new("http://localhost").unwrap();
        let auth_path = root.path().join("auth");
        let auth_config = config.clone();
        let auth = tokio::task::spawn_blocking(move || {
            calternal_auth::Auth::open(&auth_path, auth_config)
        })
        .await
        .unwrap()
        .unwrap();
        let host = Arc::new(Host::new(root.path().into(), auth, 7).unwrap());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = router(host, config);
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut client = tokio::net::TcpStream::connect(address).await.unwrap();
        client.write_all(format!(
            "POST /v1/spaces/{}/call HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer invalid\r\nContent-Type: application/json\r\nContent-Length: 1\r\nConnection: close\r\n\r\n{{",
            calternal_types::SpaceId::new(),
        ).as_bytes()).await.unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).await.unwrap();
        assert!(response.starts_with("HTTP/1.1 401 "), "{response}");
        server.abort();
    }
}

#[cfg(test)]
#[path = "browser/tests.rs"]
mod bundle_tests;
