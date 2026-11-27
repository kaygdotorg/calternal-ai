use calternal_machine::egress::{ConnectProxy, EgressDialer, EgressPolicy, EgressResolver};
use calternal_machine::sandbox::BoxFuture;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, UnixStream},
};

struct FixedResolver {
    addresses: Vec<IpAddr>,
    calls: Arc<AtomicUsize>,
}

impl EgressResolver for FixedResolver {
    fn resolve<'a>(&'a self, host: &'a str) -> BoxFuture<'a, std::io::Result<Vec<IpAddr>>> {
        Box::pin(async move {
            assert_eq!(host, "api.anthropic.com.");
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.addresses.clone())
        })
    }
}

struct RedirectDialer {
    address: SocketAddr,
    calls: Arc<AtomicUsize>,
}

impl EgressDialer for RedirectDialer {
    fn connect<'a>(&'a self, _: SocketAddr) -> BoxFuture<'a, std::io::Result<TcpStream>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            TcpStream::connect(self.address).await
        })
    }
}

async fn proxy_for(
    socket: &std::path::Path,
    resolved: Vec<IpAddr>,
    dial_address: SocketAddr,
) -> (ConnectProxy, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let resolver_calls = Arc::new(AtomicUsize::new(0));
    let dialer_calls = Arc::new(AtomicUsize::new(0));
    let policy = EgressPolicy::new(["api.anthropic.com"]).unwrap();
    let proxy = ConnectProxy::start(
        socket,
        policy,
        Arc::new(FixedResolver {
            addresses: resolved,
            calls: resolver_calls.clone(),
        }),
        Arc::new(RedirectDialer {
            address: dial_address,
            calls: dialer_calls.clone(),
        }),
    )
    .await
    .unwrap();
    (proxy, resolver_calls, dialer_calls)
}

async fn request(socket: &std::path::Path, host: &str) -> (UnixStream, Vec<u8>) {
    let mut client = UnixStream::connect(socket).await.unwrap();
    client
        .write_all(format!("CONNECT {host}:443 HTTP/1.1\r\nHost: {host}:443\r\n\r\n").as_bytes())
        .await
        .unwrap();
    let mut response = Vec::new();
    let mut byte = [0; 1];
    while !response.ends_with(b"\r\n\r\n") {
        client.read_exact(&mut byte).await.unwrap();
        response.push(byte[0]);
    }
    (client, response)
}

#[tokio::test]
async fn declared_hostname_connects_to_the_resolved_test_server() {
    let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_address = server.local_addr().unwrap();
    let resolved = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));
    let root = tempfile::tempdir().unwrap();
    let socket = root.path().join("proxy.sock");
    let (proxy, resolver_calls, dialer_calls) =
        proxy_for(&socket, vec![resolved], server_address).await;
    let server_task = tokio::spawn(async move {
        let (mut upstream, _) = server.accept().await.unwrap();
        let mut request = [0; 5];
        upstream.read_exact(&mut request).await.unwrap();
        assert_eq!(&request, b"hello");
        upstream.write_all(b"world").await.unwrap();
    });

    let (mut client, response) = request(&socket, "api.anthropic.com").await;
    assert!(response.starts_with(b"HTTP/1.1 200"));
    client.write_all(b"hello").await.unwrap();
    let mut reply = [0; 5];
    client.read_exact(&mut reply).await.unwrap();
    assert_eq!(&reply, b"world");
    client.shutdown().await.unwrap();
    server_task.await.unwrap();
    assert_eq!(resolver_calls.load(Ordering::SeqCst), 1);
    assert_eq!(dialer_calls.load(Ordering::SeqCst), 1);
    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn undeclared_hostname_is_refused_before_dns_or_dial() {
    let root = tempfile::tempdir().unwrap();
    let socket = root.path().join("proxy.sock");
    let (proxy, resolver_calls, dialer_calls) = proxy_for(
        &socket,
        vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))],
        "127.0.0.1:9".parse().unwrap(),
    )
    .await;
    let (_, response) = request(&socket, "unlisted.example").await;
    assert!(response.starts_with(b"HTTP/1.1 403"));
    assert_eq!(resolver_calls.load(Ordering::SeqCst), 0);
    assert_eq!(dialer_calls.load(Ordering::SeqCst), 0);
    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn private_dns_answer_is_refused_before_connect() {
    let root = tempfile::tempdir().unwrap();
    let socket = root.path().join("proxy.sock");
    let (proxy, resolver_calls, dialer_calls) = proxy_for(
        &socket,
        vec![
            IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
        ],
        "127.0.0.1:9".parse().unwrap(),
    )
    .await;
    let (_, response) = request(&socket, "api.anthropic.com").await;
    assert!(response.starts_with(b"HTTP/1.1 403"));
    assert_eq!(resolver_calls.load(Ordering::SeqCst), 1);
    assert_eq!(dialer_calls.load(Ordering::SeqCst), 0);
    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn stalled_handshakes_have_a_ceiling_and_release_their_slots() {
    use std::{future::pending, time::Duration};
    use tokio::time::{sleep, timeout};

    struct StalledResolver {
        calls: Arc<AtomicUsize>,
    }

    impl EgressResolver for StalledResolver {
        fn resolve<'a>(&'a self, _: &'a str) -> BoxFuture<'a, std::io::Result<Vec<IpAddr>>> {
            Box::pin(async {
                self.calls.fetch_add(1, Ordering::SeqCst);
                pending().await
            })
        }
    }

    let root = tempfile::tempdir().unwrap();
    let socket = root.path().join("proxy.sock");
    let calls = Arc::new(AtomicUsize::new(0));
    let proxy = ConnectProxy::start(
        &socket,
        EgressPolicy::new(["api.anthropic.com"]).unwrap(),
        Arc::new(StalledResolver {
            calls: Arc::clone(&calls),
        }),
        Arc::new(calternal_machine::egress::TcpDialer),
    )
    .await
    .unwrap();
    let open = || async {
        let mut client = UnixStream::connect(&socket).await.unwrap();
        let _ = client
            .write_all(
                b"CONNECT api.anthropic.com:443 HTTP/1.1\r\nHost: api.anthropic.com:443\r\n\r\n",
            )
            .await;
        client
    };
    let mut clients = Vec::new();
    for _ in 0..4 {
        clients.push(open().await);
    }
    timeout(Duration::from_secs(1), async {
        while calls.load(Ordering::SeqCst) != 4 {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let mut rejected = open().await;
    let mut byte = [0; 1];
    let result = timeout(Duration::from_millis(500), rejected.read(&mut byte))
        .await
        .expect("the fifth handshake must be rejected immediately");
    assert!(!matches!(result, Ok(1)));
    assert_eq!(calls.load(Ordering::SeqCst), 4);

    for mut client in clients {
        let result = timeout(Duration::from_secs(6), client.read(&mut byte))
            .await
            .expect("stalled DNS outlived the handshake deadline");
        assert!(!matches!(result, Ok(1)));
    }
    let _next = open().await;
    timeout(Duration::from_secs(1), async {
        while calls.load(Ordering::SeqCst) != 5 {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("expired handshakes did not release admission");
    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn malformed_connect_is_rejected_before_resolution() {
    let root = tempfile::tempdir().unwrap();
    let socket = root.path().join("proxy.sock");
    let (proxy, resolver_calls, _) = proxy_for(
        &socket,
        vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))],
        "127.0.0.1:9".parse().unwrap(),
    )
    .await;
    let start = "CONNECT api.anthropic.com:443 HTTP/1.1\r\nHost: api.anthropic.com:443\r\n";
    for (message, status) in [
        (format!("{start}X bad: value\r\n\r\n"), "400"),
        (format!("{start}X: bad\tvalue\u{7f}\r\n\r\n"), "400"),
        (format!("{start}Transfer-Encoding: chunked\r\n\r\n"), "400"),
        (format!("{start}Content-Length: 0\r\n\r\n"), "400"),
        (
            format!("{start}\r\n").replace("CONNECT ", "CONNECT\t"),
            "400",
        ),
        (format!("{start}X: {}\r\n\r\n", "x".repeat(16_380)), "431"),
    ] {
        let mut client = UnixStream::connect(&socket).await.unwrap();
        if status == "431" {
            let split = 16_380;
            client
                .write_all(&message.as_bytes()[..split])
                .await
                .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            client
                .write_all(&message.as_bytes()[split..])
                .await
                .unwrap();
        } else {
            client.write_all(message.as_bytes()).await.unwrap();
        }
        let mut response = Vec::new();
        let mut byte = [0; 1];
        while !response.ends_with(b"\r\n\r\n") {
            client.read_exact(&mut byte).await.unwrap();
            response.push(byte[0]);
        }
        let response = String::from_utf8(response).unwrap();
        assert!(
            response.starts_with(&format!("HTTP/1.1 {status} ")),
            "{response}"
        );
    }
    assert_eq!(resolver_calls.load(Ordering::SeqCst), 0);
    proxy.shutdown().await.unwrap();
}
