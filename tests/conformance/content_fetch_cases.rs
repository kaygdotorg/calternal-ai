//! Production router and broker conformance with the debug-only media upstream.
use super::*;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const WHO: usize = 1;
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDAT\x08\xd7c\xf8\xcf\xc0\x00\x00\x03\x01\x01\x00\x18\xdd\x8d\xb0\x00\x00\x00\x00IEND\xaeB\x60\x82";
const SYNTHETIC: &str = "https://content-fetch.test";

struct ProxyFixture {
    server: Fixture,
    requests: Arc<Mutex<Vec<String>>>,
}

impl ProxyFixture {
    async fn new() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let hits = Arc::clone(&requests);
        tokio::spawn(async move {
            while let Ok((connection, _)) = listener.accept().await {
                tokio::spawn(upstream(connection, hits.clone()));
            }
        });
        let mut server = Fixture::new().await;
        server.stop();
        std::fs::write(server.dir.path().join("content-fetch-origin"), origin).unwrap();
        server.start().await;
        server
            .op(
                WHO,
                "plugins.enable",
                json!({
                    "space_id":server.spaces[0], "plugin_id":"content-fetch-demo"
                }),
            )
            .await
            .ok();
        Self { server, requests }
    }

    async fn write(&self, urls: &[&str]) -> (String, Vec<String>) {
        let text = urls
            .iter()
            .map(|url| format!(r#"<img src="{url}">"#))
            .collect::<String>();
        let frame = self
            .server
            .op(
                WHO,
                "content-fetch-demo.write",
                json!({
                    "space_id":self.server.spaces[0], "text":text
                }),
            )
            .await;
        frame.ok();
        let frame = self
            .server
            .op(
                WHO,
                "plugins.network.content.get",
                json!({
                    "space_id":self.server.spaces[0], "frame_id":frame.body["data"]["frame_id"]
                }),
            )
            .await;
        frame.ok();
        let url = frame.body["data"]["frame_url"].as_str().unwrap().to_owned();
        let response = self
            .server
            .client
            .get(format!("{}{}", self.server.base, url))
            .header("Host", "localhost:18080")
            .header("Cookie", &self.server.cookies[WHO])
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let document = response.text().await.unwrap();
        let assets = document
            .split("src=\"")
            .skip(1)
            .map(|part| part.split('"').next().unwrap().to_owned())
            .collect();
        assert!(!document.contains("content-fetch.test"));
        (url, assets)
    }

    async fn asset(&self, path: &str) -> reqwest::Response {
        self.server
            .client
            .get(format!("{}{}", self.server.base, path))
            .header("Host", "localhost:18080")
            .header("Origin", "null")
            .header("Sec-Fetch-Site", "cross-site")
            .send()
            .await
            .unwrap()
    }

    async fn preference(&self, preference: &str) {
        self.server
            .op(
                WHO,
                "plugins.network.content.preference.set",
                json!({
                    "space_id":self.server.spaces[0], "preference":preference
                }),
            )
            .await
            .ok();
    }

    async fn explicit_load(&self, frame: &str) {
        let response = self
            .server
            .client
            .post(format!("{}{}", self.server.base, frame))
            .header("Host", "localhost:18080")
            .header("Cookie", &self.server.cookies[WHO])
            .header("Origin", ORIGIN)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 202);
    }

    fn hits(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

async fn upstream(mut stream: tokio::net::TcpStream, hits: Arc<Mutex<Vec<String>>>) {
    let mut incoming = Vec::new();
    while !incoming.ends_with(b"\r\n\r\n") && incoming.len() < 4096 {
        let Ok(byte) = stream.read_u8().await else {
            return;
        };
        incoming.push(byte);
    }
    let request = String::from_utf8(incoming).unwrap();
    let path = request
        .lines()
        .next()
        .unwrap()
        .split_ascii_whitespace()
        .nth(1)
        .unwrap();
    hits.lock().unwrap().push(request.to_owned());
    let (status, extra, body) = match path.split('?').next().unwrap_or(path) {
        "/good" | "/ask" | "/never" => ("200 OK", "Content-Type: image/png\r\n", PNG),
        "/redirect-ok" => ("302 Found", "Location: /good\r\n", &b""[..]),
        "/redirect-private" => (
            "302 Found",
            "Location: https://private.content-fetch.test/good\r\n",
            &b""[..],
        ),
        "/redirect-credentials" => (
            "302 Found",
            "Location: https://user:pass@content-fetch.test/good\r\n",
            &b""[..],
        ),
        "/redirect-http" => (
            "302 Found",
            "Location: http://content-fetch.test/good\r\n",
            &b""[..],
        ),
        "/redirect-port" => (
            "302 Found",
            "Location: https://content-fetch.test:8443/good\r\n",
            &b""[..],
        ),
        "/redirect-loop" => ("302 Found", "Location: /redirect-loop\r\n", &b""[..]),
        "/svg" => ("200 OK", "Content-Type: image/svg+xml\r\n", &b"<svg/>"[..]),
        "/bad-image" => ("200 OK", "Content-Type: image/png\r\n", &b"<svg/>"[..]),
        "/ambiguous" => (
            "200 OK",
            "Content-Type: image/png\r\nContent-Type: text/html\r\n",
            PNG,
        ),
        "/oversized" => {
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 10000001\r\nConnection: close\r\n\r\n").await;
            return;
        }
        "/slow" => {
            tokio::time::sleep(Duration::from_secs(12)).await;
            return;
        }
        _ => ("404 Not Found", "", &b""[..]),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    if stream.write_all(head.as_bytes()).await.is_ok() {
        let _ = stream.write_all(body).await;
    }
}

#[tokio::test]
async fn media_is_ingested_anonymously_and_ask_requires_explicit_load() {
    let f = ProxyFixture::new().await;
    let (_, assets) = f.write(&[&format!("{SYNTHETIC}/redirect-ok")]).await;
    assert_eq!(assets.len(), 1);
    let first = f.asset(&assets[0]).await;
    assert_eq!(first.status(), 200);
    assert_eq!(first.headers()["Content-Type"], "image/png");
    assert_eq!(first.headers()["X-Content-Type-Options"], "nosniff");
    assert_eq!(first.headers()["Cache-Control"], "no-store");
    assert_eq!(first.bytes().await.unwrap().as_ref(), PNG);
    assert_eq!(f.asset(&assets[0]).await.status(), 200);
    let diagnostics = f
        .server
        .op(
            WHO,
            "plugins.diagnostics",
            json!({
                "space_id": f.server.spaces[0], "plugin_id": "content-fetch-demo"
            }),
        )
        .await;
    diagnostics.ok();
    assert_eq!(
        diagnostics.body["data"]["fetch_hosts"],
        json!(["content-fetch.test"])
    );
    let hits = f.hits();
    assert_eq!(hits.len(), 2);
    for request in hits {
        let request = request.to_ascii_lowercase();
        assert!(request.starts_with("get "));
        assert!(request.contains("\r\nuser-agent: calternal\r\n"));
        for header in ["cookie", "referer", "authorization"] {
            assert!(!request.contains(&format!("\r\n{header}:")));
        }
    }
    let (prefix, signature) = assets[0].rsplit_once('/').unwrap();
    let replacement = if signature.starts_with('a') { 'b' } else { 'a' };
    let tampered = format!("{prefix}/{replacement}{}", &signature[1..]);
    assert_eq!(f.asset(&tampered).await.status(), 404);

    f.preference("ask").await;
    let (frame, assets) = f.write(&[&format!("{SYNTHETIC}/ask")]).await;
    assert_eq!(f.asset(&assets[0]).await.status(), 404);
    assert_eq!(f.hits().len(), 2);
    f.explicit_load(&frame).await;
    assert_eq!(f.asset(&assets[0]).await.status(), 200);
    f.preference("never").await;
    assert_eq!(f.asset(&assets[0]).await.status(), 404);
    let (frame, assets) = f.write(&[&format!("{SYNTHETIC}/never")]).await;
    f.explicit_load(&frame).await;
    assert_eq!(f.asset(&assets[0]).await.status(), 404);
    assert_eq!(f.hits().len(), 3);
}

#[tokio::test]
async fn nonpublic_dns_answers_are_refused() {
    let f = ProxyFixture::new().await;
    for host in [
        "private",
        "loopback",
        "link-local",
        "local-v6",
        "mixed",
        "server-range",
    ] {
        let (_, assets) = f
            .write(&[&format!("https://{host}.content-fetch.test/good")])
            .await;
        assert_eq!(assets.len(), 1, "{host}");
        assert_eq!(f.asset(&assets[0]).await.status(), 404, "{host}");
    }
    assert!(f.hits().is_empty());
}

#[tokio::test]
async fn redirects_and_invalid_urls_are_checked_before_each_hop() {
    let f = ProxyFixture::new().await;
    for path in [
        "redirect-private",
        "redirect-credentials",
        "redirect-http",
        "redirect-port",
        "redirect-loop",
    ] {
        let (_, assets) = f.write(&[&format!("{SYNTHETIC}/{path}")]).await;
        assert_eq!(f.asset(&assets[0]).await.status(), 404, "{path}");
    }
    let hits = f.hits();
    assert_eq!(
        hits.iter()
            .filter(|hit| hit.starts_with("GET /redirect-loop "))
            .count(),
        4
    );
    assert!(hits.iter().all(|hit| hit.starts_with("GET /redirect-")));
    let (_, assets) = f
        .write(&[
            "http://content-fetch.test/good",
            "https://127.0.0.1/good",
            "https://user:pass@content-fetch.test/good",
            "https://content-fetch.test:8443/good",
        ])
        .await;
    assert!(assets.is_empty());
}

#[tokio::test]
async fn media_type_size_and_time_refusals_never_enter_the_cache() {
    let f = ProxyFixture::new().await;
    for path in ["svg", "bad-image", "ambiguous", "oversized", "slow"] {
        let (_, assets) = f.write(&[&format!("{SYNTHETIC}/{path}")]).await;
        assert_eq!(f.asset(&assets[0]).await.status(), 404, "{path}");
    }
    assert_eq!(f.hits().len(), 5);
}

#[tokio::test]
async fn queue_capacity_has_a_distinct_asset_failure() {
    let f = ProxyFixture::new().await;
    let urls: Vec<_> = (0..9)
        .map(|n| format!("{SYNTHETIC}/slow?slot={n}"))
        .collect();
    let (_, assets) = f
        .write(&urls.iter().map(String::as_str).collect::<Vec<_>>())
        .await;
    assert_eq!(assets.len(), 9);
    let dropped = f.asset(&assets[8]).await;
    assert_eq!(dropped.status(), 429);
    assert_eq!(dropped.headers()["Retry-After"], "60");
    assert_eq!(
        dropped.text().await.unwrap(),
        "Remote media is busy. Try again in a minute."
    );
}
