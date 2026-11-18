use calternal_client::{Client, ClientError};
use calternal_core::*;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
async fn server(status: u16, body: Value) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let bytes = serde_json::to_vec(&body).unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut input = Vec::new();
        loop {
            let mut chunk = [0; 1024];
            let n = socket.read(&mut chunk).await.unwrap();
            if n == 0 {
                break;
            }
            input.extend_from_slice(&chunk[..n]);
            if let Some(offset) = input.windows(4).position(|w| w == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&input[..offset]).to_lowercase();
                let length = header
                    .lines()
                    .find_map(|s| {
                        s.strip_prefix("content-length: ")
                            .and_then(|s| s.parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if input.len() >= offset + 4 + length {
                    break;
                }
            }
        }
        let response = format!(
            "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        socket.write_all(&bytes).await.unwrap();
        String::from_utf8(input).unwrap()
    });
    (format!("http://{addr}"), task)
}
#[tokio::test]
async fn typed_notes_and_sensitive_header() {
    let (url, task) = server(
        200,
        json!({"request_id":"retry-1","ok":true,"data":{"notes":[]}}),
    )
    .await;
    let client = Client::new(&url, "fixture-token").unwrap();
    let pending = Client::pending(
        "notes.list",
        &NotesListInput {
            cursor: None,
            tag: None,
            space_id: "personal".into(),
        },
        Some("retry-1".into()),
    )
    .unwrap();
    let result: NotesListResult = client.execute(pending.request()).await.unwrap();
    assert!(result.notes.is_empty());
    let request = task.await.unwrap();
    assert!(request.starts_with("POST /api/v1/operations "));
    assert!(
        request
            .to_lowercase()
            .contains("authorization: bearer fixture-token")
    );
    assert!(request.contains("retry-1"));
}
#[tokio::test]
async fn discovery_is_bare_and_versions_checked() {
    let (url,task)=server(200,json!({"protocol_version":1,"genui_version":2,"principal":{"id":"owner","spaces":[]},"operations":[]})).await;
    assert_eq!(
        Client::new(&url, "token")
            .unwrap()
            .describe()
            .await
            .unwrap()
            .principal
            .id,
        "owner"
    );
    assert!(task.await.unwrap().starts_with("GET /api/v1/describe "));
}
#[tokio::test]
async fn conflict_is_typed_not_retried() {
    let (url, task) = server(
        409,
        json!({"request_id":"r","ok":false,"error":{"code":"conflict","message":"stale"}}),
    )
    .await;
    let pending = Client::pending(
        "notes.list",
        &NotesListInput {
            cursor: None,
            tag: None,
            space_id: "personal".into(),
        },
        Some("r".into()),
    )
    .unwrap();
    let result = Client::new(&url, "token")
        .unwrap()
        .invoke(pending.request())
        .await;
    assert!(matches!(
        result,
        Err(ClientError::Api {
            status: 409,
            error: ApiError {
                code: ErrorCode::Conflict,
                ..
            },
            ..
        })
    ));
    task.await.unwrap();
    assert_eq!(pending.request().request_id, "r");
}
#[tokio::test]
async fn mismatched_request_id_rejected() {
    let (url, task) = server(200, json!({"request_id":"wrong","ok":true,"data":{}})).await;
    let pending = Client::pending(
        "notes.list",
        &NotesListInput {
            cursor: None,
            tag: None,
            space_id: "personal".into(),
        },
        Some("r".into()),
    )
    .unwrap();
    assert!(matches!(
        Client::new(&url, "token")
            .unwrap()
            .invoke(pending.request())
            .await,
        Err(ClientError::Protocol(_))
    ));
    task.await.unwrap();
}
#[test]
fn allow_https_and_reject_insecure_remote_or_credential_origins() {
    for url in [
        "http://user:secret@localhost:3210",
        "http://example.org",
        "http://localhost/?token=secret",
        "http://localhost/#secret",
        "http://localhost/path",
    ] {
        assert!(Client::new(url, "token").is_err(), "{url}")
    }
    assert!(Client::new("http://localhost:3210", "token\nheader").is_err());
    assert!(Client::new("http://[::1]:3210", "token").is_ok());
    assert!(Client::new("https://example.org", "token").is_ok());
    assert!(Client::new("https://user:secret@example.org", "token").is_err());
}
#[tokio::test]
async fn predispatch_auth_failure_can_have_null_identity() {
    let (url,task)=server(401,json!({"request_id":null,"ok":false,"error":{"code":"unauthorized","message":"bad bearer"}})).await;
    let pending = Client::pending(
        "notes.list",
        &NotesListInput {
            cursor: None,
            tag: None,
            space_id: "personal".into(),
        },
        Some("r".into()),
    )
    .unwrap();
    assert!(matches!(
        Client::new(&url, "token")
            .unwrap()
            .invoke(pending.request())
            .await,
        Err(ClientError::ReauthenticationRequired)
    ));
    task.await.unwrap();
    assert_eq!(pending.request().request_id, "r");
}

#[tokio::test]
async fn dropped_mutation_response_retries_same_retained_identity_and_input() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let mut bodies = Vec::new();
        for attempt in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0; 2048];
                let n = socket.read(&mut chunk).await.unwrap();
                assert_ne!(n, 0);
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(offset) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..offset]).to_lowercase();
                    let length = header
                        .lines()
                        .find_map(|s| {
                            s.strip_prefix("content-length: ")
                                .and_then(|s| s.parse::<usize>().ok())
                        })
                        .unwrap();
                    if bytes.len() >= offset + 4 + length {
                        bodies.push(bytes[offset + 4..offset + 4 + length].to_vec());
                        break;
                    }
                }
            }
            if attempt == 1 {
                let note: Value =
                    serde_json::from_str(include_str!("../../../contract/fixtures/note.json"))
                        .unwrap();
                let result =
                    json!({"request_id":"retained-mutation","ok":true,"data":{"note":note}})
                        .to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{result}",
                    result.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
            // First connection closes after full receipt, before any response.
        }
        bodies
    });
    let client = Client::new(&url, "fixture-token").unwrap();
    let input = NotesCreateInput {
        space_id: "personal".into(),
        title: "Fixture".into(),
        markdown: "Hello world\n".into(),
    };
    let request_id = "retained-mutation";
    client.notes_create(&input, request_id).await.unwrap();
    let bodies = task.await.unwrap();
    assert_eq!(bodies.len(), 2);
    assert_eq!(bodies[0], bodies[1]);
    let request: Value = serde_json::from_slice(&bodies[0]).unwrap();
    assert_eq!(request["request_id"], request_id);
}

#[tokio::test]
async fn cancellation_interrupts_inflight_transport_and_is_sticky() {
    use calternal_client::{CancellationToken, PendingCall, operations};
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = Client::new(
        &format!("http://{}", listener.local_addr().unwrap()),
        "token",
    )
    .unwrap();
    let pending = PendingCall::<operations::SpacesList>::new(&SpacesListInput {}).unwrap();
    let cancellation = CancellationToken::new();
    let cancel = cancellation.clone();
    let peer = tokio::spawn(async move {
        let (_socket, _) = listener.accept().await.unwrap();
        cancel.cancel();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    });
    assert!(matches!(
        client.call(&pending, &cancellation).await,
        Err(ClientError::Cancelled)
    ));
    assert!(matches!(
        client.call(&pending, &cancellation).await,
        Err(ClientError::Cancelled)
    ));
    peer.await.unwrap();
}

#[tokio::test]
async fn transport_retry_is_bounded_and_preserves_exact_envelope() {
    use calternal_client::CancellationToken;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = Client::new(
        &format!("http://{}", listener.local_addr().unwrap()),
        "token",
    )
    .unwrap();
    let pending =
        Client::pending("spaces.list", &SpacesListInput {}, Some("bounded".into())).unwrap();
    let peer = tokio::spawn(async move {
        let mut bodies = Vec::new();
        for _ in 0..3 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0; 2048];
                let n = socket.read(&mut chunk).await.unwrap();
                assert_ne!(n, 0);
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(offset) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..offset]).to_lowercase();
                    let length = header
                        .lines()
                        .find_map(|s| {
                            s.strip_prefix("content-length: ")
                                .and_then(|s| s.parse::<usize>().ok())
                        })
                        .unwrap();
                    if bytes.len() >= offset + 4 + length {
                        bodies.push(bytes[offset + 4..offset + 4 + length].to_vec());
                        break;
                    }
                }
            }
        }
        bodies
    });
    assert!(matches!(
        client
            .execute_cancellable::<serde_json::Value>(pending.request(), &CancellationToken::new())
            .await,
        Err(ClientError::Transport)
    ));
    let bodies = peer.await.unwrap();
    assert_eq!(bodies.len(), 3);
    assert!(bodies.windows(2).all(|pair| pair[0] == pair[1]));
}
