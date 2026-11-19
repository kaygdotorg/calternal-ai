//! HTTP plumbing to the server and the current session token.
//!
//! One `reqwest` client keeps its connections alive, so the held `sync` request
//! and output uploads reuse at most a few sockets to the server.

use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::watch;

use crate::{Error, Result};

/// A private IP to dial for one configured server hostname and port.
#[derive(Clone, Debug)]
pub struct ConnectAddress {
    host: String,
    port: u16,
    address: IpAddr,
}

impl FromStr for ConnectAddress {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        let invalid = || Error::Protocol("Use --connect-address HOST:PORT=IP.".into());
        let (authority, address) = value.split_once('=').ok_or_else(invalid)?;
        let (host, port) = authority.rsplit_once(':').ok_or_else(invalid)?;
        if host.is_empty()
            || host.parse::<IpAddr>().is_ok()
            || !host
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        {
            return Err(invalid());
        }
        let port = port
            .parse::<u16>()
            .ok()
            .filter(|&port| port != 0)
            .ok_or_else(invalid)?;
        let address = address
            .strip_prefix('[')
            .and_then(|address| address.strip_suffix(']'))
            .unwrap_or(address)
            .parse()
            .map_err(|_| invalid())?;
        Ok(Self {
            host: host.to_ascii_lowercase(),
            port,
            address,
        })
    }
}

/// Connection to one server plus the machine's current session token.
pub struct Link {
    http: reqwest::Client,
    space_http: reqwest::Client,
    base: String,
    token: watch::Sender<Option<Arc<str>>>,
}

impl Link {
    /// A link to `base` (scheme, host and port; no trailing slash needed).
    pub fn new(base: &str) -> Result<Self> {
        Self::with_connect_address(base, None)
    }

    /// Connect to the given IP for the configured origin. The host and port
    /// must match the URL. HTTPS still uses the URL hostname for SNI and
    /// certificate verification. With an override, use a direct connection.
    pub fn with_connect_address(base: &str, address: Option<&ConnectAddress>) -> Result<Self> {
        Self::build(base, address, reqwest::Client::builder())
    }

    /// Trust a disposable test CA for verified space upgrades in integration fixtures.
    /// Cost: builds two HTTP clients. The regular origin and binding checks still apply.
    #[cfg(feature = "test-space-carrier")]
    #[doc(hidden)]
    pub fn with_space_test_ca(base: &str, root: reqwest::Certificate) -> Result<Self> {
        let mut link = Self::new(base)?;
        link.space_http = Self::configure(base, None, reqwest::Client::builder())?
            .no_proxy()
            .add_root_certificate(root)
            .redirect(reqwest::redirect::Policy::none())
            .http1_only()
            .build()?;
        Ok(link)
    }

    fn configure(
        base: &str,
        address: Option<&ConnectAddress>,
        mut builder: reqwest::ClientBuilder,
    ) -> Result<reqwest::ClientBuilder> {
        if let Some(address) = address {
            let url = reqwest::Url::parse(base)
                .map_err(|_| Error::Protocol("Set the server to an HTTP or HTTPS URL.".into()))?;
            if url.host_str() != Some(&address.host)
                || url.port_or_known_default() != Some(address.port)
                || !matches!(url.scheme(), "http" | "https")
            {
                return Err(Error::Protocol(
                    "--connect-address must match the server URL's host and port.".into(),
                ));
            }
            builder = builder
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .resolve(
                    &address.host,
                    SocketAddr::new(address.address, address.port),
                );
        }
        Ok(builder
            .connect_timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(90))
            .user_agent(concat!("calternal-machine/", env!("CARGO_PKG_VERSION"))))
    }

    fn build(
        base: &str,
        address: Option<&ConnectAddress>,
        builder: reqwest::ClientBuilder,
    ) -> Result<Self> {
        let http = Self::configure(base, address, builder)?.build()?;
        let space_http = Self::configure(base, address, reqwest::Client::builder())?
            .redirect(reqwest::redirect::Policy::none())
            .http1_only()
            .build()?;
        Ok(Self {
            http,
            space_http,
            base: base.trim_end_matches('/').to_owned(),
            token: watch::channel(None).0,
        })
    }

    pub(crate) async fn upgrade_space(
        &self,
        channel: &crate::protocol::SpaceChannelAssignment,
    ) -> Result<(reqwest::Upgraded, watch::Receiver<Option<Arc<str>>>)> {
        let invalid =
            || Error::Protocol("space carrier requires a current enrolled TLS origin".into());
        let url = reqwest::Url::parse(&format!(
            "{}/api/v1/machine/spaces/{}/channels/{}",
            self.base, channel.space, channel.assignment,
        ))
        .map_err(|_| invalid())?;
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
            return Err(invalid());
        }
        let mut session = self.token.subscribe();
        let token = session.borrow_and_update().clone().ok_or_else(invalid)?;
        let protocol = crate::protocol::SPACE_CHANNEL_UPGRADE;
        let upgrade = async {
            let response = self
                .space_http
                .get(url)
                .bearer_auth(&*token)
                .header(reqwest::header::CONNECTION, "Upgrade")
                .header(reqwest::header::UPGRADE, protocol)
                .send()
                .await?;
            if response.status() != reqwest::StatusCode::SWITCHING_PROTOCOLS
                || response
                    .headers()
                    .get(reqwest::header::UPGRADE)
                    .is_none_or(|value| value != protocol)
            {
                return Err(invalid());
            }
            Ok(response.upgrade().await?)
        };
        let io = tokio::time::timeout(Duration::from_secs(2), upgrade)
            .await
            .map_err(|_| invalid())??;
        if session.has_changed().unwrap_or(true) {
            return Err(invalid());
        }
        Ok((io, session))
    }

    /// Server base URL.
    pub fn base(&self) -> &str {
        &self.base
    }

    pub(crate) async fn needs_first_owner(&self) -> Result<bool> {
        #[derive(serde::Deserialize)]
        struct AuthStatus {
            setup: bool,
        }
        let status = self
            .http
            .get(format!("{}/api/v1/auth/status", self.base))
            .timeout(Duration::from_secs(5))
            .send()
            .await?
            .error_for_status()?
            .json::<AuthStatus>()
            .await?;
        Ok(status.setup)
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
                return Err(Error::Status {
                    status: 401,
                    message: "no session".into(),
                });
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
        Err(Error::Status {
            status: status.as_u16(),
            message,
        })
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
}

#[cfg(test)]
mod tls_tests {
    use super::*;
    use std::process::{Child, Command, Stdio};

    struct TestServer(Child);
    impl Drop for TestServer {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[tokio::test]
    async fn override_keeps_the_tls_name_and_rejects_untrusted_certificates() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path();
        let openssl = |arguments: &[&str]| {
            let output = Command::new("openssl")
                .current_dir(path)
                .args(arguments)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        openssl(&[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-noenc",
            "-days",
            "1",
            "-subj",
            "/CN=Calternal temporary test CA",
            "-keyout",
            "ca.key",
            "-out",
            "ca.pem",
        ]);
        openssl(&[
            "req",
            "-new",
            "-newkey",
            "rsa:2048",
            "-noenc",
            "-subj",
            "/CN=calternal-private-route.invalid",
            "-keyout",
            "server.key",
            "-out",
            "server.csr",
        ]);
        std::fs::write(
            path.join("extensions"),
            b"subjectAltName=DNS:calternal-private-route.invalid\nextendedKeyUsage=serverAuth\n",
        )
        .unwrap();
        openssl(&[
            "x509",
            "-req",
            "-in",
            "server.csr",
            "-CA",
            "ca.pem",
            "-CAkey",
            "ca.key",
            "-CAcreateserial",
            "-days",
            "1",
            "-extfile",
            "extensions",
            "-out",
            "server.pem",
        ]);
        let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reservation.local_addr().unwrap();
        drop(reservation);
        let _server = TestServer(
            Command::new("openssl")
                .current_dir(path)
                .args([
                    "s_server",
                    "-quiet",
                    "-www",
                    "-accept",
                    &address.to_string(),
                    "-cert",
                    "server.pem",
                    "-key",
                    "server.key",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        tokio::time::timeout(Duration::from_secs(3), async {
            while tokio::net::TcpStream::connect(address).await.is_err() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();

        let root =
            reqwest::Certificate::from_pem(&std::fs::read(path.join("ca.pem")).unwrap()).unwrap();
        let route = |host| {
            let base = format!("https://{host}:{}", address.port());
            let override_address = format!("{host}:{}=127.0.0.1", address.port())
                .parse()
                .unwrap();
            (base, override_address)
        };
        let (base, resolved) = route("calternal-private-route.invalid");
        let trusted = Link::build(
            &base,
            Some(&resolved),
            reqwest::Client::builder().add_root_certificate(root.clone()),
        )
        .unwrap();
        assert!(
            trusted
                .http
                .get(&base)
                .send()
                .await
                .unwrap()
                .status()
                .is_success()
        );
        let untrusted = Link::with_connect_address(&base, Some(&resolved)).unwrap();
        assert!(untrusted.http.get(&base).send().await.is_err());

        let (wrong_base, wrong_address) = route("calternal-wrong-name.invalid");
        let wrong = Link::build(
            &wrong_base,
            Some(&wrong_address),
            reqwest::Client::builder().add_root_certificate(root),
        )
        .unwrap();
        assert!(wrong.http.get(wrong_base).send().await.is_err());
    }
}
