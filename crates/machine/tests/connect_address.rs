//! The public origin must survive when a machine dials a private address.

use calternal_machine::link::{ConnectAddress, Link};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn private_address_delivers_the_unresolvable_origin_as_the_http_host() {
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let host = format!("calternal-resolve-test.invalid:{port}");
    let origin = format!("http://{host}");
    let direct = format!("{host}=127.0.0.1")
        .parse::<ConnectAddress>()
        .unwrap();
    let server = tokio::spawn(async move {
        let (mut connection, _) = socket.accept().await.unwrap();
        let mut input = Vec::new();
        while !input.ends_with(b"\r\n\r\n") {
            input.push(connection.read_u8().await.unwrap());
        }
        let request = String::from_utf8(input).unwrap().to_ascii_lowercase();
        assert!(request.starts_with("post /api/v1/machine/enroll http/1.1\r\n"));
        assert!(request.contains(&format!("host: {host}\r\n")), "{request}");
        connection
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}",
            )
            .await
            .unwrap();
    });

    let link = Link::with_connect_address(&origin, Some(&direct)).unwrap();
    assert_eq!(link.base(), origin);
    let result: serde_json::Value = link
        .post("/api/v1/machine/enroll", &(), false, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(result["ok"], true);
    server.await.unwrap();

    let wrong_port = format!(
        "calternal-resolve-test.invalid:{}=127.0.0.1",
        if port == 443 { 8443 } else { 443 }
    );
    let wrong_port = wrong_port.parse().unwrap();
    assert!(Link::with_connect_address(&origin, Some(&wrong_port)).is_err());
}
