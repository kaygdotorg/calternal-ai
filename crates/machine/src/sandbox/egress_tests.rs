use super::egress::{EgressProxy, Resolver, SOCKET, public_resolver};
use calternal_plugin_host::entry::EntryNetwork;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UnixStream};

async fn connect(dir: &std::path::Path, request: &str) -> (UnixStream, String) {
    let mut stream = UnixStream::connect(dir.join(SOCKET)).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        head.push(stream.read_u8().await.unwrap());
    }
    (stream, String::from_utf8(head).unwrap())
}

#[tokio::test]
async fn proxy_relays_only_approved_https_hosts() {
    let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = echo.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = echo.accept().await {
            tokio::spawn(async move {
                let (mut read, mut write) = stream.split();
                let _ = tokio::io::copy(&mut read, &mut write).await;
            });
        }
    });
    let lookups = Arc::new(AtomicUsize::new(0));
    let counted = lookups.clone();
    let resolve: Resolver = Arc::new(move |host| {
        counted.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            assert_eq!(host, "allowed.test");
            Ok(target)
        })
    });
    let dir = tempfile::tempdir().unwrap();
    let network = EntryNetwork::Sites {
        hosts: vec!["allowed.test".into()],
    };
    let _proxy = EgressProxy::start(dir.path(), &network, resolve)
        .unwrap()
        .unwrap();

    let (mut tunnel, head) = connect(
        dir.path(),
        "CONNECT Allowed.Test:443 HTTP/1.1\r\nHost: allowed.test:443\r\n\r\n",
    )
    .await;
    assert!(head.starts_with("HTTP/1.1 200 "), "{head}");
    tunnel.write_all(b"ping").await.unwrap();
    let mut echoed = [0; 4];
    tunnel.read_exact(&mut echoed).await.unwrap();
    assert_eq!(&echoed, b"ping");

    for refused in [
        "CONNECT denied.test:443 HTTP/1.1\r\n\r\n",
        "CONNECT allowed.test:80 HTTP/1.1\r\n\r\n",
        "CONNECT 127.0.0.1:443 HTTP/1.1\r\n\r\n",
        "GET http://allowed.test/ HTTP/1.1\r\n\r\n",
    ] {
        let (_, head) = connect(dir.path(), refused).await;
        assert!(head.starts_with("HTTP/1.1 403 "), "{refused}: {head}");
    }
    assert_eq!(lookups.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn no_network_binds_nothing_and_the_resolver_refuses_local_answers() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        EgressProxy::start(dir.path(), &EntryNetwork::None, public_resolver())
            .unwrap()
            .is_none()
    );
    assert!(!dir.path().join(SOCKET).exists());
    assert!(public_resolver()("localhost".into()).await.is_err());
}
