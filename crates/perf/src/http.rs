//! Synchronous real-HTTP driver; blocking I/O never runs on an async executor.
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use webauthn_authenticator_rs::{WebauthnAuthenticator, softpasskey::SoftPasskey};
use webauthn_rs::prelude::{CreationChallengeResponse, RequestChallengeResponse};

pub(crate) const ORIGIN: &str = "http://localhost:18080";
pub(crate) type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub(crate) struct Http {
    pub(crate) client: Client,
    pub(crate) base: String,
    pub(crate) cookie: String,
    pub(crate) bearer: Option<String>,
    counter: u64,
    // A fresh namespace prevents copied fixture receipts from colliding with
    // subsequent client mutations whose local counter starts at zero.
    nonce: uuid::Uuid,
}
impl Http {
    pub(crate) fn new(port: u16) -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(120))
                .build()?,
            base: format!("http://127.0.0.1:{port}"),
            cookie: String::new(),
            bearer: None,
            counter: 0,
            nonce: uuid::Uuid::new_v4(),
        })
    }
    /// Reuse this test session and client pool with an independent request namespace.
    pub(crate) fn peer(&self) -> Self {
        Self {
            client: self.client.clone(),
            base: self.base.clone(),
            cookie: self.cookie.clone(),
            bearer: self.bearer.clone(),
            counter: 0,
            nonce: uuid::Uuid::new_v4(),
        }
    }

    pub(crate) fn request(&self, path: &str, body: Option<Value>) -> Result<(Value, String, f64)> {
        let mut r = if body.is_some() {
            self.client.post(format!("{}{path}", self.base))
        } else {
            self.client.get(format!("{}{path}", self.base))
        };
        r = r.header("host", "localhost:18080");
        r = if let Some(token) = &self.bearer {
            // Match the people-lane development fixture: no cookie or Origin.
            r.bearer_auth(token)
        } else {
            r.header("origin", ORIGIN).header("cookie", &self.cookie)
        };
        if let Some(body) = body {
            r = r.json(&body);
        }
        let t = Instant::now();
        let r = r.send()?;
        let status = r.status();
        let cookie = r
            .headers()
            .get("set-cookie")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .to_owned();
        let bytes = r.bytes()?;
        let ms = t.elapsed().as_secs_f64() * 1000.;
        let v: Value = serde_json::from_slice(&bytes)?;
        if !status.is_success() || v.get("ok") == Some(&json!(false)) {
            return Err(format!("{path}: {status} {v}").into());
        }
        Ok((v, cookie, ms))
    }
    pub(crate) fn op_id(&self, name: &str, input: Value, id: &str) -> Result<(Value, f64)> {
        let (v, _, ms) = self.request(
            "/api/v1/operations",
            Some(json!({"request_id":id,"operation":name,"input":input})),
        )?;
        Ok((v["data"].clone(), ms))
    }
    pub(crate) fn op(&mut self, name: &str, input: Value) -> Result<(Value, f64)> {
        self.counter += 1;
        self.op_id(
            name,
            input,
            &format!("perf-{}-{}", self.nonce, self.counter),
        )
    }
    // Port of the conformance software passkey fixture: signed registration and login,
    // with explicit credential discovery because SoftPasskey has no resident store.
    pub(crate) fn enroll(&mut self, token: &str) -> Result<()> {
        let (s, _, _) = self.request(
            "/api/v1/auth/setup/start",
            Some(json!({"token":token,"name":"Performance fixture"})),
        )?;
        let mut opts = s["options"].clone();
        let user = opts["publicKey"]["user"]["id"].clone();
        opts["publicKey"]["authenticatorSelection"]["residentKey"] = json!("discouraged");
        opts["publicKey"]["authenticatorSelection"]["requireResidentKey"] = json!(false);
        let mut key = WebauthnAuthenticator::new(SoftPasskey::new(true));
        let challenge: CreationChallengeResponse = serde_json::from_value(opts)?;
        let cred = serde_json::to_value(
            key.do_registration(url::Url::parse(ORIGIN)?, challenge)
                .map_err(|e| format!("registration: {e:?}"))?,
        )?;
        self.request(
            "/api/v1/auth/setup/finish",
            Some(json!({"ceremony":s["ceremony"],"credential":cred,"device":"perf"})),
        )?;
        let (s, _, _) = self.request("/api/v1/auth/login/start", Some(json!({})))?;
        let mut opts = s["options"].clone();
        opts["publicKey"]["allowCredentials"] = json!([{"type":"public-key","id":cred["rawId"]}]);
        let challenge: RequestChallengeResponse = serde_json::from_value(opts)?;
        let mut signed = serde_json::to_value(
            key.do_authentication(url::Url::parse(ORIGIN)?, challenge)
                .map_err(|e| format!("login: {e:?}"))?,
        )?;
        signed["response"]["userHandle"] = user;
        let (_, cookie, _) = self.request(
            "/api/v1/auth/login/finish",
            Some(json!({"ceremony":s["ceremony"],"credential":signed,"device":"perf"})),
        )?;
        if cookie.is_empty() {
            return Err("login returned no cookie".into());
        }
        self.cookie = cookie;
        Ok(())
    }
}
