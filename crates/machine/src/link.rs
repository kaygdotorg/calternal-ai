//! HTTP plumbing to the server and the current session token.
//!
//! One `reqwest` client keeps its connections alive, so the held `sync` request
//! and output uploads reuse at most a few sockets to the server.

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::watch;

use crate::{Error, Result};

/// Connection to one server plus the machine's current session token.
pub struct Link {
    http: reqwest::Client,
    base: String,
    token: watch::Sender<Option<Arc<str>>>,
}

impl Link {
    /// A link to `base` (scheme, host and port; no trailing slash needed).
    pub fn new(base: &str) -> Result<Self> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(90))
            .user_agent(concat!("calternal-machine/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
            token: watch::channel(None).0,
        })
    }

    /// Server base URL.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// POST `body` as JSON to `path` and decode the JSON answer.
    ///
    /// With `bearer` the current session token is sent; without a session that
    /// fails at once with status 401. Non-2xx answers become [`Error::Status`],
    /// except 403 on a bearer request, which is [`Error::Revoked`].
    pub async fn post<B: Serialize + ?Sized, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        bearer: bool,
        timeout: Duration,
    ) -> Result<R> {
        let mut req = self
            .http
            .post(format!("{}{}", self.base, path))
            .timeout(timeout)
            .json(body);
        if bearer {
            let Some(token) = self.token.borrow().clone() else {
                return Err(Error::Status { status: 401, message: "no session".into() });
            };
            req = req.bearer_auth(&*token);
        }
        let resp = req.send().await?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp.json().await?);
        }
        if bearer && status.as_u16() == 403 {
            return Err(Error::Revoked);
        }
        let message = resp
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
            .unwrap_or_else(|| status.to_string());
        Err(Error::Status { status: status.as_u16(), message })
    }

    /// Install a new session token, waking every waiter.
    pub fn set_token(&self, token: Option<Arc<str>>) {
        self.token.send_replace(token);
    }

    /// Wait until a session is open. Output uploads park here while reconnecting.
    pub async fn wait_for_session(&self) {
        let mut rx = self.token.subscribe();
        let _ = rx.wait_for(Option::is_some).await;
    }

    /// Whether a session is currently open.
    pub fn has_session(&self) -> bool {
        self.token.borrow().is_some()
    }
}
