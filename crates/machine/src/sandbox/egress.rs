//! Approved execution egress for one entry, as an HTTPS CONNECT proxy on a
//! private Unix socket. The sandbox itself has no network, so this socket is
//! its only way out and every destination is checked here, never in the image.
use std::{io, net::SocketAddr, path::Path, sync::Arc, time::Duration};

use calternal_plugin_host::entry::{EntryNetwork, hostname};
use calternal_types::public_address;
use futures_util::future::BoxFuture;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UnixListener, UnixStream},
    sync::Semaphore,
    task::JoinSet,
};

use super::{SandboxError, SandboxResult};

/// Socket name inside the private run directory, which the sandbox sees at
/// `/run/calternal/egress.sock`.
pub(crate) const SOCKET: &str = "egress.sock";
/// Path the entry is told about through `CALTERNAL_EGRESS_SOCKET`.
pub(crate) const ENDPOINT: &str = "/run/calternal/egress.sock";

const HEAD_LIMIT: usize = 4096;
const CONNECTIONS: usize = 16;
const SETUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Resolve an approved hostname to one address the proxy may dial, or fail.
pub(crate) type Resolver =
    Arc<dyn Fn(String) -> BoxFuture<'static, io::Result<SocketAddr>> + Send + Sync>;

/// Running proxy. Dropping it closes the listener and every relayed
/// connection, so it must live exactly as long as the run's authority.
pub struct EgressProxy {
    task: tokio::task::JoinHandle<()>,
}

impl Drop for EgressProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl EgressProxy {
    /// Bind `dir/egress.sock` and serve the policy. Returns `None` for
    /// `EntryNetwork::None`, which gets no socket at all. Costs one bind; each
    /// connection reads one bounded request head, resolves once and dials once.
    pub(crate) fn start(
        dir: &Path,
        network: &EntryNetwork,
        resolve: Resolver,
    ) -> SandboxResult<Option<Self>> {
        let allowed: Option<Arc<[String]>> = match network {
            EntryNetwork::None => return Ok(None),
            EntryNetwork::AnyPublic => None,
            EntryNetwork::Sites { hosts } => Some(hosts.clone().into()),
        };
        let listener = UnixListener::bind(dir.join(SOCKET)).map_err(SandboxError::new)?;
        let task = tokio::spawn(serve(listener, allowed, resolve));
        Ok(Some(Self { task }))
    }
}

/// The machine resolver: DNS once, then refuse the whole answer if any
/// address is non-public (the same rule as the server's netguard).
pub(crate) fn public_resolver() -> Resolver {
    Arc::new(|host| {
        Box::pin(async move {
            let addresses: Vec<SocketAddr> = tokio::net::lookup_host((format!("{host}."), 443))
                .await?
                .collect();
            if addresses.is_empty() || addresses.iter().any(|a| public_address::is_denied(a.ip())) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "non-public address",
                ));
            }
            Ok(addresses[0])
        })
    })
}

async fn serve(listener: UnixListener, allowed: Option<Arc<[String]>>, resolve: Resolver) {
    let slots = Arc::new(Semaphore::new(CONNECTIONS));
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            Some(_) = connections.join_next() => {}
            accepted = listener.accept() => {
                let Ok((stream, _)) = accepted else { return };
                let Ok(slot) = slots.clone().try_acquire_owned() else { continue };
                let (allowed, resolve) = (allowed.clone(), resolve.clone());
                connections.spawn(async move {
                    let _slot = slot;
                    let _ = relay(stream, allowed.as_deref(), &resolve).await;
                });
            }
        }
    }
}

async fn relay(
    mut client: UnixStream,
    allowed: Option<&[String]>,
    resolve: &Resolver,
) -> io::Result<()> {
    let setup = async {
        let host = match read_connect(&mut client).await? {
            Some(host) if allowed.is_none_or(|hosts| hosts.contains(&host)) => host,
            _ => return Ok(None),
        };
        let Ok(address) = resolve(host).await else {
            return Ok(None);
        };
        Ok::<_, io::Error>(TcpStream::connect(address).await.ok())
    };
    let upstream = tokio::time::timeout(SETUP_TIMEOUT, setup)
        .await
        .map_err(|_| io::ErrorKind::TimedOut)??;
    let Some(mut upstream) = upstream else {
        client
            .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
            .await?;
        return Ok(());
    };
    client
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;
    tokio::io::copy_bidirectional(&mut client, &mut upstream).await?;
    Ok(())
}

// Read `CONNECT host:443 HTTP/1.1` and its headers byte by byte so no tunnel
// byte is consumed. Returns the lowercase host, or None for any other request.
async fn read_connect(client: &mut UnixStream) -> io::Result<Option<String>> {
    let mut head = Vec::with_capacity(256);
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() == HEAD_LIMIT {
            return Ok(None);
        }
        head.push(client.read_u8().await?);
    }
    let Some(line) = head.split(|&b| b == b'\n').next() else {
        return Ok(None);
    };
    let Ok(line) = std::str::from_utf8(line) else {
        return Ok(None);
    };
    let mut parts = line.trim_end_matches('\r').split(' ');
    let (Some("CONNECT"), Some(authority), Some("HTTP/1.1" | "HTTP/1.0"), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Ok(None);
    };
    let Some(host) = authority.strip_suffix(":443").map(str::to_ascii_lowercase) else {
        return Ok(None);
    };
    Ok(hostname(&host).then_some(host))
}
