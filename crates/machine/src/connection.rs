//! The machine's outbound connection: open a session, then long-poll `sync`
//! until shut down, reconnecting with capped exponential backoff.
//!
//! Each `sync` acknowledges messages, reports capacity and every held lease, and
//! is held by the server for up to `poll_hold_ms` when there is nothing to send.
//! An idle machine therefore makes about three requests a minute.
//!
//! Local lease expiry removes containers after a lease TTL without a successful
//! sync. Reconnect reports expired leases for reconciliation. Mounted workspaces
//! and account homes survive, and fenced work requires explicit resumption.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::identity::Credentials;
use crate::link::{ConnectAddress, Link};
use crate::protocol::{
    PROTOCOL_VERSION, SessionChallenge, SessionFinish, SessionGranted, SessionStart, SyncRequest,
    SyncResponse,
};
use crate::ready::Ready;
use crate::runner::Runner;
use crate::sandbox::Backend;
use crate::{Error, Result};

/// Settings of a running machine role.
pub struct Config {
    /// Directory holding the credentials file.
    pub dir: std::path::PathBuf,
    /// Sandbox backend.
    pub backend: Arc<dyn Backend>,
    /// Most sandboxes at once.
    pub max_sandboxes: u32,
}

/// Run the machine role until `shutdown` completes (orderly exit, `Ok`) or the
/// machine is revoked ([`Error::Revoked`], after stopping every sandbox).
pub async fn run(config: Config, shutdown: impl std::future::Future<Output = ()>) -> Result<()> {
    run_at(config, None, &Ready::default(), shutdown).await
}

pub(crate) async fn run_at(
    config: Config,
    address: Option<&ConnectAddress>,
    ready: &Ready,
    shutdown: impl std::future::Future<Output = ()>,
) -> Result<()> {
    let _ownership = crate::ownership::lock(&config.dir).await?;
    let path = config.dir.clone();
    let creds = tokio::task::spawn_blocking(move || Credentials::load(&path))
        .await
        .map_err(|_| Error::Protocol("cannot load machine identity".into()))??;
    config
        .backend
        .initialize()
        .await
        .map_err(|e| Error::Protocol(e.0))?;
    let link = Arc::new(Link::with_connect_address(&creds.server, address)?);
    let runner = Arc::new(Runner::new(
        config.backend.clone(),
        link.clone(),
        config.max_sandboxes,
    ));
    let boot_id = Uuid::new_v4();
    tracing::info!(machine = %creds.machine_id, server = %creds.server, "machine role starting");
    let result = tokio::select! {
        r = session_loop(&creds, &link, &runner, boot_id, ready) => r,
        () = shutdown => Ok(()),
    };
    let closed = runner.release_all().await;
    let backend_closed = config.backend.shutdown().await;
    result.and_then(|()| {
        if closed {
            backend_closed.map_err(|e| Error::Protocol(e.0))
        } else {
            Err(Error::Protocol(
                "sandbox teardown incomplete; restart the machine role to sweep its sandboxes"
                    .into(),
            ))
        }
    })
}

async fn session_loop(
    creds: &Credentials,
    link: &Link,
    runner: &Arc<Runner>,
    boot_id: Uuid,
    ready: &Ready,
) -> Result<()> {
    let mut backoff = Backoff::default();
    let mut last_ok = Instant::now();
    let mut lease_ttl = Duration::from_secs(90);
    loop {
        let granted = match open_session(creds, link, boot_id, runner).await {
            Ok(g) => g,
            Err(Error::Revoked) => return Err(Error::Revoked),
            Err(e) if matches!(e.status(), Some(403 | 404)) => return Err(Error::Revoked),
            Err(e) => {
                tracing::warn!(error = %e, "cannot open session; retrying");
                expire_locally(runner, last_ok, lease_ttl).await;
                tokio::time::sleep(backoff.next()).await;
                continue;
            }
        };
        runner.configure_backend(&granted.backend)?;
        lease_ttl = Duration::from_millis(granted.lease_ttl_ms);
        let hold = Duration::from_millis(granted.poll_hold_ms);
        link.set_token(Some(granted.token.into()));
        tracing::info!("session open");
        if let Err(error) = ready.report("Machine session opened") {
            if ready.pending() {
                return Err(error.into());
            }
            tracing::warn!(%error, "could not update the machine service status");
        }
        let mut ack = 0u64;
        loop {
            let machine = runner.clone();
            let capacity = tokio::task::spawn_blocking(move || machine.capacity())
                .await
                .map_err(|error| Error::Protocol(error.to_string()))?;
            let req = SyncRequest {
                ack,
                capacity,
                draining: false,
                leases: runner.reports(),
                discards: runner.discard_reports(),
            };
            match link
                .post::<_, SyncResponse>(
                    "/api/v1/machine/sync",
                    &req,
                    true,
                    hold + Duration::from_secs(15),
                )
                .await
            {
                Ok(resp) => {
                    runner.ack_discards(&req.discards);
                    backoff = Backoff::default();
                    last_ok = Instant::now();
                    for env in resp.messages {
                        if env.seq > ack {
                            ack = env.seq;
                            runner.handle(env.message).await;
                        }
                    }
                    let runner = runner.clone();
                    tokio::spawn(async move { runner.park_idle(lease_ttl).await });
                }
                Err(Error::Revoked) => return Err(Error::Revoked),
                Err(e) if e.status() == Some(401) => {
                    tracing::info!("session ended by the server; opening a new one");
                    link.set_token(None);
                    break;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "sync failed; retrying");
                    expire_locally(runner, last_ok, lease_ttl).await;
                    tokio::time::sleep(backoff.next()).await;
                }
            }
        }
    }
}

async fn expire_locally(runner: &Runner, last_ok: Instant, ttl: Duration) {
    if last_ok.elapsed() >= ttl {
        runner.release_all().await;
    }
}

/// Query durable owner erasures for this identity without opening a session.
pub async fn erasure_status(creds: &Credentials) -> Result<crate::protocol::ErasureStatus> {
    erasure_status_at(creds, None).await
}

pub(crate) async fn erasure_status_at(
    creds: &Credentials,
    address: Option<&ConnectAddress>,
) -> Result<crate::protocol::ErasureStatus> {
    let link = Link::with_connect_address(&creds.server, address)?;
    let timeout = Duration::from_secs(3);
    let SessionChallenge { challenge } = link
        .post(
            "/api/v1/machine/session/start",
            &SessionStart {
                machine_id: creds.machine_id,
            },
            false,
            timeout,
        )
        .await?;
    let request = crate::protocol::ErasureStatusRequest {
        machine_id: creds.machine_id,
        signature: creds.key.sign_erasure_status(creds.machine_id, &challenge),
        challenge,
    };
    link.post("/api/v1/machine/status", &request, false, timeout)
        .await
}

async fn open_session(
    creds: &Credentials,
    link: &Link,
    boot_id: Uuid,
    runner: &Runner,
) -> Result<SessionGranted> {
    let t = Duration::from_secs(20);
    let SessionChallenge { challenge } = link
        .post(
            "/api/v1/machine/session/start",
            &SessionStart {
                machine_id: creds.machine_id,
            },
            false,
            t,
        )
        .await?;
    let finish = SessionFinish {
        machine_id: creds.machine_id,
        signature: creds.key.sign_session(creds.machine_id, &challenge),
        challenge,
        boot_id,
        protocol: PROTOCOL_VERSION,
        backend: runner.backend_name().into(),
        guest_memory_mib: runner.guest_memory_mib(),
    };
    link.post("/api/v1/machine/session/finish", &finish, false, t)
        .await
}

/// Exponential backoff from 0.5 s to 30 s with up to 25 % random jitter.
#[derive(Debug)]
struct Backoff(Duration);

impl Default for Backoff {
    fn default() -> Self {
        Self(Duration::from_millis(500))
    }
}

impl Backoff {
    fn next(&mut self) -> Duration {
        let base = self.0;
        self.0 = (self.0 * 2).min(Duration::from_secs(30));
        let jitter = (Uuid::new_v4().as_u128() % 250) as u32;
        base + base * jitter / 1000
    }
}

/// Default machine directory: `$CALTERNAL_MACHINE_DIR`, else
/// `$XDG_DATA_HOME/calternal/machine`, else `~/.local/share/calternal/machine`.
pub fn default_dir() -> std::path::PathBuf {
    if let Some(d) = std::env::var_os("CALTERNAL_MACHINE_DIR") {
        return d.into();
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(&std::env::var_os("HOME").unwrap_or_default()).join(".local/share")
        });
    base.join("calternal/machine")
}
