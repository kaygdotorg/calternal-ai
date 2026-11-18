use super::*;
use axum::{Router, extract::Request, routing::get};
use std::sync::Mutex as SyncMutex;

type Calls = Arc<SyncMutex<Vec<(String, bool)>>>;

async fn stand_in(calls: Calls) -> (upstream::Upstream, tokio::task::JoinHandle<()>) {
    let app = Router::new().fallback(get(move |req: Request| {
        let calls = calls.clone();
        async move {
            let uri = req.uri().to_string();
            let authorized = req.headers().get(header::AUTHORIZATION)
                .is_some_and(|value| value == "Client-ID fixture-only");
            calls.lock().unwrap().push((uri.clone(), authorized));
            let id = catalog::DEFAULT;
            if uri == format!("/photos/{id}") && authorized {
                return axum::Json(serde_json::json!({
                    "id": id,
                    "urls": {"raw": "https://images.unsplash.com/photo-fixture?ixid=view&auto=format"},
                    "links": {"download_location": format!("https://api.unsplash.com/photos/{id}/download?ixid=pick")}
                })).into_response();
            }
            if uri == format!("/photos/{id}/download?ixid=pick") && authorized {
                return "{}".into_response();
            }
            if uri.starts_with("/photo-fixture?") && !req.headers().contains_key(header::AUTHORIZATION) {
                return ([(header::CONTENT_TYPE, "image/jpeg")], b"\xff\xd8\xffstub\xff\xd9".to_vec()).into_response();
            }
            StatusCode::BAD_REQUEST.into_response()
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut upstream = upstream::Upstream::new("fixture-only").unwrap();
    upstream.stand_in = Some(
        format!("http://{}", listener.local_addr().unwrap())
            .parse()
            .unwrap(),
    );
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (upstream, task)
}

#[tokio::test]
async fn guarded_fetch_tracks_first_use_and_keeps_published_bytes_across_restart() {
    let calls = Calls::default();
    let (upstream, server) = stand_in(calls.clone()).await;
    let root = tempfile::tempdir().unwrap();
    let service = Wallpapers::open(root.path(), upstream).await.unwrap();
    let id = catalog::DEFAULT;
    let (slot, thumb) = catalog::find(id, "thumb").unwrap();
    let (_, light) = catalog::find(id, "blur-light-3x").unwrap();
    service.rendition(slot, thumb).await.unwrap();
    assert_eq!(calls.lock().unwrap().len(), 2);
    let (a, b) = tokio::join!(
        service.rendition(slot, light),
        service.rendition(slot, light)
    );
    let a = a.unwrap();
    assert_eq!(a.etag, b.unwrap().etag);
    {
        let requests = calls.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert_eq!(
            requests[2],
            (format!("/photos/{id}/download?ixid=pick"), true)
        );
        let (path, authorized) = &requests[3];
        assert!(!authorized);
        for pair in [
            "w=288",
            "blur=120",
            "bri=35",
            "con=-25",
            "sat=-8",
            "ixid=view",
            "fm=jpg",
        ] {
            assert!(path.contains(pair), "{path}");
        }
        assert!(!path.contains("auto=format"));
    }
    drop(service);
    let dir = root.path().join("wallpapers");
    std::fs::write(dir.join("obsolete.full"), b"obsolete").unwrap();
    std::fs::write(dir.join(".calternal-tmp-crash"), b"partial").unwrap();
    let (upstream, next_server) = stand_in(calls.clone()).await;
    let restarted = Wallpapers::open(root.path(), upstream).await.unwrap();
    assert!(!dir.join("obsolete.full").exists());
    assert!(!dir.join(".calternal-tmp-crash").exists());
    assert_eq!(restarted.rendition(slot, light).await.unwrap().etag, a.etag);
    assert_eq!(calls.lock().unwrap().len(), 4);
    let (_, full) = catalog::find(id, "full").unwrap();
    restarted.rendition(slot, full).await.unwrap();
    assert_eq!(
        calls.lock().unwrap().len(),
        6,
        "restart repeats metadata for a cold slot, with the tracking marker reused"
    );
    server.abort();
    next_server.abort();
}
