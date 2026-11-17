//! Client ordering, refusal, cancellation and cache/bookmark invariants.
use calternal_client::{
    BookmarkStorage, CancellationToken, Client, ClientError, FileBookmarks, ItemCache,
    MemoryBookmarks, OfflineOutbox,
};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[tokio::test]
async fn outbox_drains_in_order_stops_on_refusal_and_retains_identity() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut ids = Vec::new();
        for status in [200, 403] {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let offset = loop {
                let mut bytes = [0; 1024];
                let n = socket.read(&mut bytes).await.unwrap();
                assert!(n > 0);
                buf.extend_from_slice(&bytes[..n]);
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..i]).to_lowercase();
                    let len = head
                        .lines()
                        .find_map(|s| s.strip_prefix("content-length: "))
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    if buf.len() >= i + 4 + len {
                        break i + 4;
                    }
                }
            };
            let request: Value = serde_json::from_slice(&buf[offset..]).unwrap();
            let id = request["request_id"].as_str().unwrap().to_owned();
            ids.push(id.clone());
            let body=if status==200 {json!({"ok":true,"request_id":id,"data":{"accepted":true}})} else {json!({"ok":false,"request_id":id,"error":{"code":"forbidden","message":"access removed"}})}.to_string();
            socket.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
        }
        ids
    });
    let client = Client::new(&format!("http://{addr}"), "fixture").unwrap();
    let mut outbox = OfflineOutbox::default();
    for id in ["one", "two", "three"] {
        outbox
            .enqueue(Client::pending("fixture.send", &json!({"text":id}), Some(id.into())).unwrap())
            .unwrap();
    }
    let error = outbox
        .drain(&client, &CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, ClientError::Api { status: 403, .. }));
    assert_eq!(server.await.unwrap(), ["one", "two"]);
    assert_eq!(
        outbox
            .pending()
            .map(|r| r.request_id.as_str())
            .collect::<Vec<_>>(),
        ["two", "three"]
    );
    assert!(
        outbox
            .enqueue(
                Client::pending(
                    "fixture.send",
                    &json!({"text":"changed"}),
                    Some("two".into())
                )
                .unwrap()
            )
            .is_err()
    );
}
#[tokio::test]
async fn cancellation_stops_reconnect_backoff_and_invalid_spaces_fail_locally() {
    let client = Client::new("http://127.0.0.1:1", "fixture").unwrap();
    let cancellation = CancellationToken::new();
    assert!(
        client
            .subscribe(
                vec!["name".into()],
                std::sync::Arc::new(MemoryBookmarks::default()),
                cancellation.clone()
            )
            .await
            .is_err()
    );
    let id = "00000000-0000-7000-8000-00000000abcd";
    assert!(
        client
            .subscribe(
                vec![id.into(), id.to_uppercase()],
                std::sync::Arc::new(MemoryBookmarks::default()),
                cancellation.clone()
            )
            .await
            .is_err()
    );
    let mut stream = client
        .subscribe(
            vec!["00000000-0000-7000-8000-000000000001".into()],
            std::sync::Arc::new(MemoryBookmarks::default()),
            cancellation.clone(),
        )
        .await
        .unwrap();
    let stop = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        cancellation.cancel();
    });
    assert!(matches!(
        tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
            .await
            .unwrap(),
        Err(ClientError::Cancelled)
    ));
    stop.await.unwrap();
}
#[tokio::test]
async fn file_bookmark_survives_new_client_storage_and_cache_is_bounded() {
    let root = std::env::temp_dir().join(format!("calternal-bookmarks-{}", uuid::Uuid::new_v4()));
    let id = "00000000-0000-7000-8000-000000000001";
    FileBookmarks::new(root.clone())
        .save(id, "opaque-signed-cursor")
        .await
        .unwrap();
    assert_eq!(
        FileBookmarks::new(root.clone())
            .load(id)
            .await
            .unwrap()
            .as_deref(),
        Some("opaque-signed-cursor")
    );
    assert!(
        FileBookmarks::new(root.clone())
            .save("../other", "token")
            .await
            .is_err()
    );
    let mut cache = ItemCache::new(1, None).unwrap();
    cache.put(id, Some(json!({"body":"one"}))).await.unwrap();
    let second = "00000000-0000-7000-8000-000000000002";
    cache
        .put(second, Some(json!({"body":"two"})))
        .await
        .unwrap();
    assert!(cache.get(id).await.unwrap().is_none());
    assert!(cache.get(second).await.unwrap().is_some());
    cache.clear();
    assert!(cache.get(second).await.unwrap().is_none());
    tokio::task::spawn_blocking(move || std::fs::remove_dir_all(root).unwrap())
        .await
        .unwrap();
}

#[tokio::test]
async fn reconnect_uses_only_the_accepted_bookmark() {
    use calternal_core::LiveFrame;
    use std::sync::Arc;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let space = "00000000-0000-7000-8000-000000000001";
    let task = tokio::spawn(async move {
        for attempt in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 1024];
                let n = socket.read(&mut chunk).await.unwrap();
                assert!(n > 0);
                request.extend_from_slice(&chunk[..n]);
                if request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let request = String::from_utf8(request).unwrap();
            assert_eq!(request.contains("accepted-bookmark"), attempt == 1);
            let frame = if attempt == 0 {
                LiveFrame::Ready {
                    space_id: space.into(),
                    bookmark: "accepted-bookmark".into(),
                }
            } else {
                LiveFrame::Entry {
                    space_id: space.into(),
                    entry: json!({"seq":2,"token":"second-bookmark","item":"00000000-0000-7000-8000-000000000002"}),
                }
            };
            let body = format!("data: {}\n\n", serde_json::to_string(&frame).unwrap());
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
        }
    });
    let client = Client::new(&format!("http://{addr}"), "fixture").unwrap();
    let storage = Arc::new(MemoryBookmarks::default());
    let mut live = client
        .subscribe(
            vec![space.into()],
            storage.clone(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let ready = live.next().await.unwrap();
    live.accept(&ready).await.unwrap();
    let entry = tokio::time::timeout(std::time::Duration::from_secs(3), live.next())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(entry, LiveFrame::Entry { .. }));
    assert_eq!(
        storage.load(space).await.unwrap().as_deref(),
        Some("accepted-bookmark")
    );
    live.accept(&entry).await.unwrap();
    assert_eq!(
        storage.load(space).await.unwrap().as_deref(),
        Some("second-bookmark")
    );
    task.await.unwrap();
}
