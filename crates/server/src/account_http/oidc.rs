//! Protected OIDC transport. The host owns the browser ceremony and identity proof.

mod exchange;

use super::*;
use axum::{Form, extract::Query, response::Redirect};
use calternal_auth::{OidcProvider, OidcSignup};
use openidconnect::{Nonce, PkceCodeVerifier, core::CoreProviderMetadata};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

pub(crate) const PATH: &str = "/api/v1/auth/oidc/callback";
const COOKIE: &str = "__Host-calternal_oidc";
const TTL: Duration = Duration::from_secs(600);
type Caller = (AccountId, InstallationId);

pub(super) struct Flows {
    states: Mutex<HashMap<String, (Instant, Flow)>>,
    client: reqwest::Client,
    network: Semaphore,
}

impl Flows {
    pub(super) fn new() -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
            client: Self::client_builder()
                .build()
                .expect("OIDC HTTP client configuration"),
            network: Semaphore::new(16),
        }
    }

    fn client_builder() -> reqwest::ClientBuilder {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(8))
    }

    pub(super) fn with_ca(pem: &[u8]) -> std::io::Result<Self> {
        let invalid = || {
            std::io::Error::other("CALTERNAL_OIDC_CA_FILE must contain trusted PEM certificates")
        };
        let roots = reqwest::Certificate::from_pem_bundle(pem).map_err(|_| invalid())?;
        if roots.is_empty() {
            return Err(invalid());
        }
        let mut builder = Self::client_builder();
        for root in roots {
            builder = builder.add_root_certificate(root);
        }
        Ok(Self {
            states: Mutex::new(HashMap::new()),
            client: builder.build().map_err(|_| invalid())?,
            network: Semaphore::new(16),
        })
    }

    fn insert(&self, id: String, flow: Flow, began: Instant) -> Result<(), Response> {
        let mut states = self.states.lock().unwrap_or_else(|e| e.into_inner());
        states.retain(|_, (opened, _)| opened.elapsed() < TTL);
        if states.len() >= 1024 {
            return Err(failure(429, "too_many_requests", "OIDC flow pool"));
        }
        if states.contains_key(&id) {
            return Err(failed());
        }
        states.insert(id, (began, flow));
        Ok(())
    }

    fn take(
        &self,
        id: &str,
        cookie: Option<&str>,
        pending: bool,
    ) -> Result<(Instant, Flow), Response> {
        let mut states = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let Some((at, flow)) = states.get(id) else {
            return Err(failed());
        };
        if at.elapsed() >= TTL
            || cookie != Some(flow.browser())
            || pending != matches!(flow, Flow::Pending(_))
        {
            return Err(failed());
        }
        states.remove(id).ok_or_else(failed)
    }
}

struct Intent {
    provider: OidcProvider,
    revision: String,
    browser: String,
    signup: Option<OidcSignup>,
    link: Option<Caller>,
    reverify: Option<Caller>,
}

struct Pending {
    intent: Intent,
    metadata: CoreProviderMetadata,
    nonce: Nonce,
    verifier: Option<PkceCodeVerifier>,
}

struct Verified {
    subject: String,
    display_name: Option<String>,
}

enum Flow {
    Pending(Box<Pending>),
    Completed {
        intent: Intent,
        proof: Option<Verified>,
    },
}

impl Flow {
    fn browser(&self) -> &str {
        match self {
            Self::Pending(p) => &p.intent.browser,
            Self::Completed { intent, .. } => &intent.browser,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    provider_id: String,
    purpose: String,
    signup: Option<OidcSignup>,
}

#[derive(Deserialize)]
struct Callback {
    state: String,
    code: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FinishFlow {
    flow_id: String,
    device: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configure {
    provider: OidcProvider,
    client_secret: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderState {
    provider_id: String,
    action: String,
    #[serde(default)]
    operator_recovery: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Link {
    provider_id: String,
}

pub(super) fn routes(router: Router<HttpAuth>) -> Router<HttpAuth> {
    router
        .route("/api/v1/auth/oidc/providers", get(providers))
        .route(
            "/api/v1/auth/oidc/admin/providers",
            get(admin_providers).post(configure),
        )
        .route("/api/v1/auth/oidc/admin/state", post(provider_state))
        .route("/api/v1/auth/oidc/start", post(start))
        .route(PATH, get(callback_get).post(callback_post))
        .route("/api/v1/auth/oidc/finish", post(finish))
        .route("/api/v1/auth/oidc/unlink", post(unlink))
}

fn failed() -> Response {
    failure(401, "provider_failed", "OIDC verification failed")
}

fn browser_cookie(headers: &HeaderMap) -> Option<&str> {
    let mut matched = cookies(headers, COOKIE);
    let first = matched.next()?;
    matched.next().is_none().then_some(first)
}

fn cookie(response: &mut Response, browser: &str, ttl: u64) {
    if let Ok(value) = HeaderValue::from_str(&format!(
        "{COOKIE}={browser}; Path=/; HttpOnly; Secure; SameSite=None; Max-Age={ttl}"
    )) {
        response.headers_mut().append(SET_COOKIE, value);
    }
}

async fn providers(State(s): State<HttpAuth>) -> Reply {
    let providers = s
        .auth
        .oidc_providers(None)
        .await
        .map_err(error)?
        .into_iter()
        .map(|(p, _)| json!({"id":p.id,"label":p.label,"icon":p.preset}))
        .collect::<Vec<_>>();
    Ok(onboarding::private(json!({"providers":providers})))
}

async fn admin_providers(State(s): State<HttpAuth>, req: Request) -> Reply {
    let caller = session(&s, req.headers()).await?;
    let providers = s
        .auth
        .oidc_providers(Some(caller))
        .await
        .map_err(error)?
        .into_iter()
        .map(|(provider, enabled)| json!({"provider":provider,"enabled":enabled}))
        .collect::<Vec<_>>();
    Ok(onboarding::private(
        json!({"providers":providers,"presets":[
        {"id":"apple","issuer":"https://appleid.apple.com","label":"Sign in with Apple","scopes":[]},
        {"id":"google","issuer":"https://accounts.google.com","label":"Sign in with Google","scopes":["profile"]},
        {"id":"generic","label":"OpenID Connect","scopes":["profile"]}]}),
    ))
}

async fn configure(State(s): State<HttpAuth>, req: Request) -> Reply {
    let caller = session(&s, req.headers()).await?;
    let Configure {
        mut provider,
        client_secret,
    } = body(&s, req).await?;
    if provider.issuer.is_empty() {
        provider.issuer = match provider.preset.as_str() {
            "apple" => "https://appleid.apple.com".into(),
            "google" => "https://accounts.google.com".into(),
            _ => return Err(failure(400, "invalid_input", "an issuer is required")),
        };
    }
    let secret = client_secret
        .map(|v| calternal_secrets::SecretValue::new(v.into_bytes()))
        .transpose()
        .map_err(|_| failure(400, "invalid_input", "invalid client credential"))?;
    s.auth
        .configure_oidc(caller, provider, secret)
        .await
        .map_err(error)?;
    Ok(onboarding::private(json!({"saved":true})))
}

async fn provider_state(State(s): State<HttpAuth>, req: Request) -> Reply {
    let caller = session(&s, req.headers()).await?;
    let request: ProviderState = body(&s, req).await?;
    s.auth
        .set_oidc_provider_state(
            caller,
            &request.provider_id,
            &request.action,
            request.operator_recovery,
        )
        .await
        .map_err(error)?;
    Ok(onboarding::private(json!({"saved":true})))
}

async fn start(State(s): State<HttpAuth>, req: Request) -> Reply {
    let source = source_key(&s, &req);
    s.auth.admit_http_login(&source).map_err(error)?;
    let caller = identify(&s, req.headers()).await;
    let request: Start = body(&s, req).await?;
    let (provider, revision) = s
        .auth
        .oidc_provider(&request.provider_id)
        .await
        .map_err(error)?;
    let (link, reverify) = match request.purpose.as_str() {
        "sign_in" => (None, None),
        "link" if request.signup.is_none() => {
            let caller = caller.ok_or_else(unauthorized)?;
            s.auth
                .require_fresh(caller.0, caller.1, false)
                .await
                .map_err(error)?;
            (Some(caller), None)
        }
        "reverify" if request.signup.is_none() => {
            let caller = caller.ok_or_else(unauthorized)?;
            if !s
                .auth
                .oidc_session(caller)
                .await
                .map_err(error)?
                .is_some_and(|s| s.provider == provider.id)
            {
                return Err(failed());
            }
            (None, Some(caller))
        }
        _ => return Err(failed()),
    };
    let browser = calternal_auth::token::new_session_token().map_err(error)?;
    let intent = Intent {
        provider,
        revision,
        browser: browser.clone(),
        signup: request.signup,
        link,
        reverify,
    };
    let _network = s
        .oidc
        .network
        .try_acquire()
        .map_err(|_| failure(429, "too_many_requests", "OIDC network"))?;
    let (id, url, flow) = exchange::begin(&s, intent).await?;
    s.oidc
        .insert(id.clone(), Flow::Pending(Box::new(flow)), Instant::now())?;
    let mut response = onboarding::private(json!({"flow_id":id,"redirect_url":url.as_str()}));
    cookie(&mut response, &browser, TTL.as_secs());
    Ok(response)
}

async fn callback_get(
    State(s): State<HttpAuth>,
    headers: HeaderMap,
    Query(data): Query<Callback>,
) -> Reply {
    callback(s, headers, data).await
}

async fn callback_post(
    State(s): State<HttpAuth>,
    headers: HeaderMap,
    Form(data): Form<Callback>,
) -> Reply {
    callback(s, headers, data).await
}

async fn callback(s: HttpAuth, headers: HeaderMap, data: Callback) -> Reply {
    let _network = s
        .oidc
        .network
        .try_acquire()
        .map_err(|_| failure(429, "too_many_requests", "OIDC network"))?;
    let (opened, Flow::Pending(pending)) =
        s.oidc.take(&data.state, browser_cookie(&headers), true)?
    else {
        return Err(failed());
    };
    let Pending {
        intent,
        metadata,
        nonce,
        verifier,
    } = *pending;
    let proof = match data.code {
        Some(code) if code.len() <= 4096 => {
            exchange::check(&s, &intent, metadata, nonce, verifier, code)
                .await
                .ok()
        }
        _ => None,
    };
    s.oidc
        .insert(data.state, Flow::Completed { intent, proof }, opened)?;
    let mut response = Redirect::to("/?account-oidc=complete").into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    Ok(response)
}

async fn finish(State(s): State<HttpAuth>, req: Request) -> Reply {
    let browser = browser_cookie(req.headers()).map(str::to_owned);
    let caller = identify(&s, req.headers()).await;
    let request: FinishFlow = body(&s, req).await?;
    let (
        _,
        Flow::Completed {
            intent,
            proof: Some(proof),
        },
    ) = s.oidc.take(&request.flow_id, browser.as_deref(), false)?
    else {
        return Err(failed());
    };
    if let Some(expected) = intent.link.or(intent.reverify)
        && caller != Some(expected)
    {
        return Err(failed());
    }
    let offered = intent.link.is_none() && intent.reverify.is_none();
    let invitation = intent.signup.is_some();
    let result = s
        .auth
        .finish_oidc(
            intent.provider.id,
            intent.revision,
            proof.subject,
            proof.display_name,
            intent.signup,
            intent.link,
            intent.reverify,
            request.device.unwrap_or_default(),
        )
        .await
        .map_err(|e| {
            if offered && !invitation && e.code() == ErrorCode::SetupTokenInvalid {
                failure(403, "invite_required", "unlinked identity")
            } else {
                error(e)
            }
        })?;
    let mut response = if let Some((account, _, secret, codes)) = result {
        let offer = offered && s.auth.offer_first_passkey(account).await.map_err(error)?;
        signed_in(
            &s,
            &secret,
            json!({"user":{"id":account},"offer_passkey":offer,"recovery_codes":codes}),
        )
    } else {
        onboarding::private(json!({"linked":true}))
    };
    cookie(&mut response, "", 0);
    Ok(response)
}

async fn unlink(State(s): State<HttpAuth>, req: Request) -> Reply {
    let caller = session(&s, req.headers()).await?;
    let request: Link = body(&s, req).await?;
    s.auth
        .unlink_oidc(caller, &request.provider_id)
        .await
        .map_err(error)?;
    Ok(onboarding::private(json!({"unlinked":true})))
}
