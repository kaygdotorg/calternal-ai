//! Source HTTP fixture adapted to this workspace's trusted account router.
#![allow(dead_code)]
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use calternal_auth::{Auth, AuthConfig};
use calternal_server::{
    api::{self, AppState},
    auth::{Directory, StoredDirectory},
    plugins::Registry,
    store::Store,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;
use url::Url;
use webauthn_authenticator_rs::{WebauthnAuthenticator, softpasskey::SoftPasskey};
use webauthn_rs::prelude::{CreationChallengeResponse, RequestChallengeResponse};
pub const ORIGIN: &str = "http://localhost:8080";
pub struct App {
    pub dir: tempfile::TempDir,
    pub auth: Auth,
    pub router: Router,
    pub state: Arc<AppState>,
}
pub fn app() -> App {
    app_with(|_| {})
}
pub fn app_with(tweak: impl FnOnce(&mut AuthConfig)) -> App {
    app_in(tempfile::tempdir().unwrap(), tweak)
}

pub fn app_in(dir: tempfile::TempDir, tweak: impl FnOnce(&mut AuthConfig)) -> App {
    build_app(dir, tweak, false)
}

pub fn app_with_jobs() -> App {
    build_app(tempfile::tempdir().unwrap(), |_| {}, true)
}

fn build_app(dir: tempfile::TempDir, tweak: impl FnOnce(&mut AuthConfig), with_jobs: bool) -> App {
    let mut config = AuthConfig::new(ORIGIN).unwrap();
    tweak(&mut config);
    let roots = calternal_roots::Roots {
        appdata: dir.path().join("appdata"),
        userdata_ssd: dir.path().join("userdata-ssd"),
        userdata_hdd: dir.path().join("userdata-hdd"),
    };
    let store = Store::open(
        &roots,
        uuid::Uuid::from_u128(1),
        true,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
    )
    .unwrap();
    let tokens = StoredDirectory::open(&roots.appdata, config, false, None).unwrap();
    let auth = tokens.http_auth().unwrap().auth.clone();
    let jobs = if with_jobs {
        calternal_server::plugins::Jobs::open(&roots.appdata)
            .unwrap()
            .with_security(tokens.spaces.agents())
    } else {
        calternal_server::plugins::Jobs::disabled()
    };
    let state = Arc::new(AppState {
        blocking: calternal_server::blocking::RequestWorkers::new().unwrap(),
        inputs: store.account_inputs(),
        store,
        directory: tokens,
        registry: Registry::load(std::path::Path::new("/nonexistent-test-plugins")),
        workers: calternal_server::plugins::WorkerPool::new(None),
        jobs,
        allowed_hosts: api::allowed_hosts(8080),
    });
    let router = api::router(state.clone(), None);
    App {
        dir,
        auth,
        router,
        state,
    }
}
/// One answer: status, the Set-Cookie value (if any) and the JSON body.
pub struct Res {
    pub status: StatusCode,
    pub cookie: Option<String>,
    pub body: Value,
}

impl Res {
    /// The error code, for refusals.
    pub fn code(&self) -> &str {
        self.body["error"]["code"].as_str().unwrap_or("")
    }

    /// Just `name=value` from the Set-Cookie header.
    pub fn session(&self) -> String {
        let c = self.cookie.as_deref().expect("no cookie set");
        c.split(';').next().unwrap().to_owned()
    }
}

/// Sends a request. JSON content type and our own origin by default;
/// `headers` add or override.
pub async fn send(
    app: &App,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Body,
) -> Res {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "localhost:8080");
    let mut has_ct = false;
    let mut has_origin = false;
    for (k, v) in headers {
        has_ct |= k.eq_ignore_ascii_case("content-type");
        has_origin |= k.eq_ignore_ascii_case("origin");
        b = b.header(*k, *v);
    }
    if !has_ct {
        b = b.header("content-type", "application/json");
    }
    if !has_origin {
        b = b.header("origin", ORIGIN);
    }
    let res = app
        .router
        .clone()
        .oneshot(b.body(body).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let cookie = res
        .headers()
        .get("set-cookie")
        .map(|v| v.to_str().unwrap().to_owned());
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Res {
        status,
        cookie,
        body,
    }
}

pub async fn post(app: &App, path: &str, cookie: Option<&str>, body: Value) -> Res {
    let mut h = vec![];
    if let Some(c) = cookie {
        h.push(("cookie", c));
    }
    send(app, "POST", path, &h, Body::from(body.to_string())).await
}

pub async fn get(app: &App, path: &str, cookie: Option<&str>) -> Res {
    let mut h = vec![];
    if let Some(c) = cookie {
        h.push(("cookie", c));
    }
    send(app, "GET", path, &h, Body::empty()).await
}

/// Runs an action as a signed-in browser.
pub async fn action(app: &App, cookie: &str, name: &str, key: &str, input: Value) -> Res {
    let (method, path) = match name {
        "account.installations" => ("GET", "/api/v1/auth/installations"),
        "account.revoke" => ("POST", "/api/v1/auth/installations/revoke"),
        "account.passkeys" => ("GET", "/api/v1/auth/passkeys"),
        "account.passkey-add-start" => ("POST", "/api/v1/auth/passkeys/add/start"),
        "account.passkey-add-finish" => ("POST", "/api/v1/auth/passkeys/add/finish"),
        "account.passkey-rename" => ("POST", "/api/v1/auth/passkeys/rename"),
        "account.passkey-remove" => ("POST", "/api/v1/auth/passkeys/remove"),
        _ => ("GET", "/api/v1/describe"),
    };
    let mut r = send(
        app,
        method,
        path,
        &[("cookie", cookie), ("idempotency-key", key)],
        Body::from(if input.is_null() { json!({}) } else { input }.to_string()),
    )
    .await;
    if r.status.is_success() {
        let replayed = r.body.get("replayed").cloned();
        r.body = json!({"result":r.body});
        if let Some(replayed) = replayed {
            r.body["replayed"] = replayed;
        }
    }
    r
}

/// A passkey on a pretend device.
pub struct Device {
    pub key: WebauthnAuthenticator<SoftPasskey>,
    /// The credential id this device registered, base64url.
    pub cred_id: Option<String>,
    pub user: Option<String>,
}

impl Device {
    pub fn new() -> Self {
        Self {
            key: WebauthnAuthenticator::new(SoftPasskey::new(true)),
            cred_id: None,
            user: None,
        }
    }

    /// Answers a `setup/start` reply like `navigator.credentials.create`.
    pub fn create(&mut self, start: &Res) -> Value {
        let mut opts = start.body["options"].clone();
        let sel = &mut opts["publicKey"]["authenticatorSelection"];
        assert_eq!(
            sel["residentKey"], "required",
            "server must ask for a discoverable passkey"
        );
        // The software key can't store discoverable credentials; real
        // passkeys can. The server's check doesn't depend on it.
        sel["residentKey"] = json!("discouraged");
        sel["requireResidentKey"] = json!(false);
        let ccr: CreationChallengeResponse = serde_json::from_value(opts).unwrap();
        self.user = Some(
            start.body["options"]["publicKey"]["user"]["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
        let cred = self
            .key
            .do_registration(Url::parse(ORIGIN).unwrap(), ccr)
            .unwrap();
        let v = serde_json::to_value(&cred).unwrap();
        self.cred_id = Some(v["rawId"].as_str().unwrap().to_owned());
        v
    }

    /// Answers a `login/start` reply like `navigator.credentials.get`. The
    /// software key can't discover its own credentials, so the test tells it
    /// which one to use and adds the user handle a real passkey would send.
    pub fn sign(&mut self, start: &Res) -> Value {
        let mut opts = start.body["options"].clone();
        opts["publicKey"]["allowCredentials"] =
            json!([{ "type": "public-key", "id": self.cred_id.clone().unwrap() }]);
        let rcr: RequestChallengeResponse = serde_json::from_value(opts).unwrap();
        let cred = self
            .key
            .do_authentication(Url::parse(ORIGIN).unwrap(), rcr)
            .unwrap();
        let mut v = serde_json::to_value(&cred).unwrap();
        v["response"]["userHandle"] = json!(self.user.clone().unwrap());
        v
    }
}

/// Creates the first owner and returns their session cookie and device.
pub async fn first_owner(app: &App, name: &str) -> (String, Device) {
    let token = app.auth.first_owner_token().await.unwrap().unwrap();
    let start = post(
        app,
        "/api/v1/auth/setup/start",
        None,
        json!({ "token": token, "name": name }),
    )
    .await;
    assert_eq!(start.status, 200, "{:?}", start.body);
    let mut dev = Device::new();
    let cred = dev.create(&start);
    let fin = post(
        app,
        "/api/v1/auth/setup/finish",
        None,
        json!({ "ceremony": start.body["ceremony"], "credential": cred, "device": "Test browser" }),
    )
    .await;
    assert_eq!(fin.status, 200, "{:?}", fin.body);
    // Setup sessions are not a fresh sign-in; most tests need one, so sign in.
    let signed_in = login(app, &mut dev).await;
    assert_eq!(signed_in.status, 200, "{:?}", signed_in.body);
    (signed_in.session(), dev)
}

/// Like `first_owner` but returns the session made by setup itself.
pub async fn first_owner_setup_session(app: &App, name: &str) -> (String, Device) {
    let token = app.auth.first_owner_token().await.unwrap().unwrap();
    let start = post(
        app,
        "/api/v1/auth/setup/start",
        None,
        json!({ "token": token, "name": name }),
    )
    .await;
    let mut dev = Device::new();
    let cred = dev.create(&start);
    let fin = post(
        app,
        "/api/v1/auth/setup/finish",
        None,
        json!({ "ceremony": start.body["ceremony"], "credential": cred, "device": "Test browser" }),
    )
    .await;
    assert_eq!(fin.status, 200, "{:?}", fin.body);
    (fin.session(), dev)
}

/// Signs in with a device and returns the new session cookie.
pub async fn login(app: &App, dev: &mut Device) -> Res {
    let start = post(app, "/api/v1/auth/login/start", None, json!({})).await;
    assert_eq!(start.status, 200, "{:?}", start.body);
    let cred = dev.sign(&start);
    post(
        app,
        "/api/v1/auth/login/finish",
        None,
        json!({ "ceremony": start.body["ceremony"], "credential": cred, "device": "Phone" }),
    )
    .await
}

pub fn remove_v11_sharing_schema(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "PRAGMA foreign_keys=OFF;
         DROP TRIGGER installation_notes_scopes_insert;
         DROP TABLE recipient_control_acks;
         DROP TABLE recipient_control;
         DROP TABLE sharing_installation_resets;
         DROP TABLE share_fanout_acks;
         DROP TABLE sharing_reset_cursors;
         DROP TABLE share_fanout_deliveries;
         DROP TABLE recipient_share_fences;
         DROP TABLE recipient_share_state;
         DROP TABLE share_fanout;
         DROP TABLE note_journal;
         DROP TABLE share_grants;
         DROP TABLE note_shares;
         DROP TABLE installation_notes_scopes;",
    )
}
