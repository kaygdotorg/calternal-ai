//! Host-side CONNECT egress for harness containers.
//!
//! The container has no network interface. Its loopback bridge forwards CONNECT
//! bytes over a mounted Unix socket; this module checks each declared hostname,
//! resolves it on the host for every connection, refuses unsafe answers, and
//! dials the already-checked address without resolving again.
mod tunnel;
use crate::sandbox::BoxFuture;
use std::{
    collections::HashSet,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UnixListener, UnixStream},
    sync::{Semaphore, oneshot},
    task::{JoinHandle, JoinSet},
};
use tunnel::Limits;

const MAX_CONNECT_HEADERS: usize = 16 * 1024;
const MAX_SYSTEM_DNS_JOBS: usize = 16;

/// Resolves a declared hostname on the host for each CONNECT request.
pub trait EgressResolver: Send + Sync + 'static {
    /// Return all addresses currently associated with host. The proxy rejects
    /// the entire answer if any address is unsafe. Cost is one system lookup.
    fn resolve<'a>(&'a self, host: &'a str) -> BoxFuture<'a, io::Result<Vec<IpAddr>>>;
}

/// Resolves names with the host operating system's configured resolver.
pub struct SystemResolver {
    jobs: Arc<Semaphore>,
}

impl Default for SystemResolver {
    fn default() -> Self {
        Self {
            jobs: Arc::new(Semaphore::new(MAX_SYSTEM_DNS_JOBS)),
        }
    }
}

impl EgressResolver for SystemResolver {
    fn resolve<'a>(&'a self, host: &'a str) -> BoxFuture<'a, io::Result<Vec<IpAddr>>> {
        Box::pin(async move {
            let permit = self
                .jobs
                .clone()
                .try_acquire_owned()
                .map_err(|_| io::Error::other("system DNS lookup capacity exhausted"))?;
            let host = host.to_owned();
            // libc lookup may outlive the async handshake deadline. The job
            // itself owns the permit until libc returns.
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                (host.as_str(), 443)
                    .to_socket_addrs()
                    .map(|addresses| addresses.map(|address| address.ip()).collect())
            })
            .await
            .map_err(io::Error::other)?
        })
    }
}

/// Connects to the numeric address that the host resolver already checked.
pub trait EgressDialer: Send + Sync + 'static {
    /// Open a TCP stream to address without doing another name lookup.
    fn connect<'a>(&'a self, address: SocketAddr) -> BoxFuture<'a, io::Result<TcpStream>>;
}

/// Dials the exact numeric endpoint supplied by the proxy resolver.
pub struct TcpDialer;

impl EgressDialer for TcpDialer {
    fn connect<'a>(&'a self, address: SocketAddr) -> BoxFuture<'a, io::Result<TcpStream>> {
        Box::pin(async move { TcpStream::connect(address).await })
    }
}

/// Exact host allowlist taken from the installed plugin outbound declaration.
#[derive(Clone, Debug)]
pub struct EgressPolicy {
    hosts: HashSet<String>,
}

impl EgressPolicy {
    /// Build an exact DNS-name allowlist. IP literals, wildcards, local names,
    /// malformed labels and names with a trailing dot are refused.
    pub fn new<I, S>(hosts: I) -> Result<Self, EgressError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut allowed = HashSet::new();
        for host in hosts {
            allowed.insert(normalize_hostname(host.as_ref())?);
        }
        Ok(Self { hosts: allowed })
    }

    fn allows(&self, host: &str) -> bool {
        self.hosts.contains(host)
    }
}

/// A host-side proxy start or policy failure.
#[derive(Debug, thiserror::Error)]
pub enum EgressError {
    /// A declared host is not a canonical public DNS name.
    #[error("invalid outbound hostname")]
    InvalidHost,
    /// The host-side Unix socket could not be created or managed.
    #[error("cannot start egress proxy: {0}")]
    Io(#[from] io::Error),
    /// The proxy or run-control task ended before it could be stopped cleanly.
    #[error("egress control task failed")]
    Task,
    /// The wrapper never connected to its host control socket.
    #[error("run wrapper did not connect to the host control socket")]
    ControlNotConnected,
    /// The wrapper connected but did not send the expected readiness frame.
    #[error("run wrapper did not complete the control handshake")]
    ControlHandshake,
    /// The wrapper did not connect to its host control socket in time.
    #[error("run control socket did not become ready")]
    ControlTimeout,
}

/// A CONNECT proxy bound to one private Unix socket for one harness run.
pub struct ConnectProxy {
    socket: PathBuf,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl ConnectProxy {
    /// Bind a host-side proxy to socket. The parent directory must be private
    /// to the run and mounted read-only in a network-isolated container.
    pub async fn start(
        socket: &Path,
        policy: EgressPolicy,
        resolver: Arc<dyn EgressResolver>,
        dialer: Arc<dyn EgressDialer>,
    ) -> Result<Self, EgressError> {
        Self::start_limited(socket, policy, resolver, dialer, Limits::default()).await
    }

    async fn start_limited(
        socket: &Path,
        policy: EgressPolicy,
        resolver: Arc<dyn EgressResolver>,
        dialer: Arc<dyn EgressDialer>,
        limits: Limits,
    ) -> Result<Self, EgressError> {
        let listener = UnixListener::bind(socket)?;
        let (shutdown, mut stop) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            let budget = Arc::new(AtomicU64::new(limits.bytes));
            let policy = Arc::new(policy);
            loop {
                tokio::select! {
                    biased;
                    _ = &mut stop => break,
                    _ = connections.join_next(), if !connections.is_empty() => {}
                    accepted = listener.accept() => match accepted {
                        Ok((client, _)) => {
                            if connections.len() >= limits.concurrent || budget.load(Ordering::Relaxed) == 0 {
                                continue;
                            }
                            let policy = Arc::clone(&policy);
                            let resolver = Arc::clone(&resolver);
                            let dialer = Arc::clone(&dialer);
                            let budget = Arc::clone(&budget);
                            connections.spawn(async move {
                                serve_connection(client, &policy, resolver.as_ref(), dialer.as_ref(), &budget, limits).await;
                            });
                        }
                        Err(_) => break,
                    },
                }
            }
            connections.abort_all();
            while connections.join_next().await.is_some() {}
        });
        Ok(Self {
            socket: socket.to_owned(),
            shutdown: Some(shutdown),
            task: Some(task),
        })
    }

    /// Stop accepting tunnels and remove the socket path. Cost is one task join
    /// and one filesystem unlink; active connections are cancelled.
    pub async fn shutdown(mut self) -> Result<(), EgressError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            task.await.map_err(|_| EgressError::Task)?;
        }
        match tokio::fs::remove_file(&self.socket).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(EgressError::Io(error)),
        }
    }
}

impl Drop for ConnectProxy {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn serve_connection(
    mut client: UnixStream,
    policy: &EgressPolicy,
    resolver: &dyn EgressResolver,
    dialer: &dyn EgressDialer,
    budget: &AtomicU64,
    limits: Limits,
) {
    let connect = handshake(&mut client, policy, resolver, dialer);
    if let Ok(Some((host, upstream, request, header_end))) =
        tokio::time::timeout(limits.handshake, connect).await
    {
        tunnel::relay(
            client,
            upstream,
            &request[header_end..],
            &host,
            budget,
            limits,
        )
        .await;
    }
}

async fn handshake(
    client: &mut UnixStream,
    policy: &EgressPolicy,
    resolver: &dyn EgressResolver,
    dialer: &dyn EgressDialer,
) -> Option<(String, TcpStream, Vec<u8>, usize)> {
    let mut request = Vec::with_capacity(1024);
    let mut buffer = [0_u8; 2048];
    let header_end = loop {
        if let Some(position) = find_header_end(&request) {
            break position;
        }
        if request.len() >= MAX_CONNECT_HEADERS {
            refuse(client, "431 Request Header Fields Too Large").await;
            return None;
        }
        let count = client.read(&mut buffer).await.ok()?;
        if count == 0 {
            return None;
        }
        request.extend_from_slice(&buffer[..count]);
    };
    let Some(host) = parse_connect(&request[..header_end]) else {
        refuse(client, "400 Bad Request").await;
        return None;
    };
    if !policy.allows(&host) {
        eprintln!("harness egress refused undeclared host {host}");
        refuse(client, "403 Forbidden").await;
        return None;
    }
    let addresses = match resolver.resolve(&host).await {
        Ok(addresses) if !addresses.is_empty() => addresses,
        _ => {
            refuse(client, "502 Bad Gateway").await;
            return None;
        }
    };
    if addresses.iter().any(|address| is_denied_address(*address)) {
        eprintln!("harness egress refused unsafe DNS answer for {host}");
        refuse(client, "403 Forbidden").await;
        return None;
    }
    let upstream = match dialer.connect(SocketAddr::new(addresses[0], 443)).await {
        Ok(stream) => stream,
        Err(_) => {
            refuse(client, "502 Bad Gateway").await;
            return None;
        }
    };
    client
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await
        .ok()?;
    Some((host, upstream, request, header_end))
}

async fn refuse(client: &mut UnixStream, status: &str) {
    let response = format!("HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n");
    let _ = client.write_all(response.as_bytes()).await;
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|end| end + 4)
}

fn parse_connect(headers: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(headers).ok()?;
    let mut lines = text.strip_suffix("\r\n\r\n")?.split("\r\n");
    let mut request = lines.next()?.split_ascii_whitespace();
    if request.next()? != "CONNECT" {
        return None;
    }
    let authority = request.next()?;
    if request.next()? != "HTTP/1.1" || request.next().is_some() {
        return None;
    }
    let (host, port) = authority.rsplit_once(':')?;
    if host.contains(':') || port != "443" {
        return None;
    }
    let host = normalize_hostname(host).ok()?;
    let mut found_host = false;
    for line in lines {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("host") {
            if found_host {
                return None;
            }
            found_host = true;
            let (header_host, header_port) = value.trim().rsplit_once(':')?;
            if header_port != "443"
                || header_host.contains(':')
                || normalize_hostname(header_host).ok()? != host
            {
                return None;
            }
        }
    }
    found_host.then_some(host)
}

fn normalize_hostname(host: &str) -> Result<String, EgressError> {
    if host.is_empty() || host.len() > 253 || host.ends_with('.') || host.parse::<IpAddr>().is_ok()
    {
        return Err(EgressError::InvalidHost);
    }
    let normalized = host.to_ascii_lowercase();
    if normalized == "localhost"
        || normalized.ends_with(".localhost")
        || normalized.ends_with(".local")
        || normalized.ends_with(".internal")
        || normalized.ends_with(".test")
    {
        return Err(EgressError::InvalidHost);
    }
    if !normalized.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.as_bytes()[0].is_ascii_alphanumeric()
            && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    }) {
        return Err(EgressError::InvalidHost);
    }
    Ok(normalized)
}

fn is_denied_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_denied_v4(address),
        IpAddr::V6(address) => is_denied_v6(address),
    }
}

fn is_denied_v4(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    let value = u32::from(address);
    address.is_private()
        || address.is_loopback()
        || address.is_link_local()
        || address.is_unspecified()
        || address.is_multicast()
        || octets == [255, 255, 255, 255]
        || octets[0] == 0
        || in_v4_range(value, 0x6440_0000, 10)
        || in_v4_range(value, 0xc000_0000, 24)
        || in_v4_range(value, 0xc000_0200, 24)
        || in_v4_range(value, 0xc612_0000, 15)
        || in_v4_range(value, 0xc633_6400, 24)
        || in_v4_range(value, 0xcb00_7100, 24)
        || octets[0] >= 240
}

fn in_v4_range(address: u32, network: u32, prefix: u32) -> bool {
    let mask = u32::MAX << (32 - prefix);
    address & mask == network & mask
}

fn is_denied_v6(address: Ipv6Addr) -> bool {
    let segments = address.segments();
    if let Some(mapped) = address.to_ipv4() {
        return is_denied_v4(mapped);
    }
    address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || segments[0] & 0xfe00 == 0xfc00
        || segments[0] & 0xffc0 == 0xfe80
        || segments[0] == 0x2001 && segments[1] == 0x0
        || segments[0] == 0x2001 && segments[1] == 0x0db8
        || segments[0] == 0x2002
        || segments[0] == 0x0064 && segments[1] == 0xff9b
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    #[test]
    fn refuses_special_use_addresses_and_accepts_a_public_test_address() {
        for address in [
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)),
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            IpAddr::V6(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1)),
            IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1)),
        ] {
            assert!(is_denied_address(address), "{address}");
        }
        assert!(!is_denied_address(IpAddr::V4(Ipv4Addr::new(
            93, 184, 216, 34
        ))));
    }

    #[test]
    fn hostname_policy_is_exact_and_ascii_canonical() {
        assert_eq!(
            normalize_hostname("API.Anthropic.com").unwrap(),
            "api.anthropic.com"
        );
        for host in [
            "127.0.0.1",
            "api.anthropic.com.",
            "*.anthropic.com",
            "localhost",
            "service.internal",
            "bad host",
        ] {
            assert!(normalize_hostname(host).is_err(), "{host}");
        }
    }
}

/// Host-lifetime socket for one harness wrapper in a network-isolated container.
pub struct RunControl {
    ready: Option<oneshot::Receiver<Result<(), EgressError>>>,
    task: Option<JoinHandle<()>>,
}

impl RunControl {
    /// Bind the run-control socket beside the egress proxy before starting the
    /// wrapper. The container connects once and announces readiness.
    pub async fn start(socket: &Path) -> Result<Self, EgressError> {
        let listener = UnixListener::bind(socket)?;
        let (ready_tx, ready) = oneshot::channel();
        let task = tokio::spawn(async move {
            let accepted = tokio::time::timeout(Duration::from_secs(5), listener.accept()).await;
            let (mut stream, _) = match accepted {
                Ok(Ok(connection)) => connection,
                Ok(Err(_)) => {
                    let _ = ready_tx.send(Err(EgressError::Task));
                    return;
                }
                Err(_) => {
                    let _ = ready_tx.send(Err(EgressError::ControlNotConnected));
                    return;
                }
            };
            let mut hello = [0_u8; 6];
            if stream.read_exact(&mut hello).await.is_err() || &hello != b"READY\n" {
                let _ = ready_tx.send(Err(EgressError::ControlHandshake));
                return;
            }
            let _ = ready_tx.send(Ok(()));
            // Hold the connection until the wrapper exits or the host dies.
            let mut buffer = [0_u8; 128];
            while stream.read(&mut buffer).await.is_ok_and(|count| count > 0) {}
        });
        Ok(Self {
            ready: Some(ready),
            task: Some(task),
        })
    }

    /// Wait until the wrapper has connected and sent its readiness frame.
    pub async fn wait_ready(&mut self) -> Result<(), EgressError> {
        let ready = self.ready.take().ok_or(EgressError::Task)?;
        tokio::time::timeout(Duration::from_secs(6), ready)
            .await
            .map_err(|_| EgressError::ControlTimeout)?
            .map_err(|_| EgressError::Task)??;
        Ok(())
    }
}

impl Drop for RunControl {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
