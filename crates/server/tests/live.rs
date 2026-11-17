//! Replay and delivery authorization over the real HTTP live route.
use calternal_client::{BookmarkStorage, CancellationToken, Client, MemoryBookmarks};
use calternal_core::LiveFrame;
use calternal_server::{
    api::{self, AppState, Shared},
    auth::{Directory, Identity, StoredDirectory},
    plugins::{Jobs, Registry, Session, WorkerPool},
    store::Store,
};
use serde_json::{Value, json};
use std::sync::Arc;

struct Fixture {
    _dir: tempfile::TempDir,
    state: Shared,
    url: String,
    token: String,
    guest_token: String,
    owner: Identity,
    spaces: Vec<String>,
    task: tokio::task::JoinHandle<()>,
}
impl Fixture {
    async fn new() -> Self {
        let (dir, tokens, token, guest_token, owner, spaces, store) =
            tokio::task::spawn_blocking(|| {
                let dir = tempfile::tempdir().unwrap();
                let roots = calternal_roots::Roots {
                    appdata: dir.path().join("appdata"),
                    userdata_ssd: dir.path().join("userdata-ssd"),
                    userdata_hdd: dir.path().join("userdata-hdd"),
                };
                let store = Store::open(&roots, true).unwrap();
                let tokens = StoredDirectory::load_or_create(&roots.appdata).unwrap();
                let token =
                    std::fs::read_to_string(dir.path().join("appdata/dev-tokens/owner.token")).unwrap();
                let guest_token =
                    std::fs::read_to_string(dir.path().join("appdata/dev-tokens/guest.token")).unwrap();
                let owner = tokens.authenticate(token.as_bytes()).unwrap();
                let spaces = tokens
                    .describe(&owner)
                    .spaces
                    .into_iter()
                    .map(|s| s.id)
                    .collect();
                (dir, tokens, token, guest_token, owner, spaces, store)
            })
            .await
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(AppState {
            inputs: store.account_inputs(),
            store,
            directory: tokens,
            registry: Registry::load(std::path::Path::new("/missing")),
            workers: WorkerPool::new(None),
            jobs: Jobs::disabled(),
            allowed_hosts: api::allowed_hosts(port),
        });
        let app = api::router(state.clone(), None);
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            _dir: dir,
            state,
            url: format!("http://127.0.0.1:{port}"),
            token,
            guest_token,
            owner,
            spaces,
            task,
        }
    }
    fn client(&self) -> Client {
        Client::new(&self.url, &self.token).unwrap()
    }
    async fn create(&self, space: &str, i: usize) -> Value {
        api::dispatch(
            self.state.clone(),
            self.owner.clone(),
            Session::Static,
            &format!("create-{space}-{i}"),
            "notes.create",
            serde_json::from_value(
                json!({"space_id":space,"title":format!("{i}"),"markdown":"body"}),
            )
            .unwrap(),
        )
        .await
        .unwrap()
    }
    async fn next(live: &mut calternal_client::LiveConnection) -> LiveFrame {
        tokio::time::timeout(std::time::Duration::from_secs(10), live.next())
            .await
            .expect("event timeout")
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backlog_concurrent_writes_then_live_have_no_gap_or_duplicate() {
    let f = Fixture::new().await;
    let space = f.spaces[0].clone();
    let storage = Arc::new(MemoryBookmarks::default());
    let mut initial = f
        .client()
        .subscribe(
            vec![space.clone()],
            storage.clone(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let ready = Fixture::next(&mut initial).await;
    assert!(matches!(ready, LiveFrame::Ready { .. }));
    initial.accept(&ready).await.unwrap();
    drop(initial);
    for i in 0..10 {
        f.create(&space, i).await;
    }
    let mut live = f
        .client()
        .subscribe(vec![space.clone()], storage, CancellationToken::new())
        .await
        .unwrap();
    let writer_state = f.state.clone();
    let owner = f.owner.clone();
    let sp = space.clone();
    let writer = tokio::spawn(async move {
        for i in 10..80 {
            api::dispatch(
                writer_state.clone(),
                owner.clone(),
                Session::Static,
                &format!("create-{sp}-{i}"),
                "notes.create",
                serde_json::from_value(
                    json!({"space_id":sp,"title":format!("{i}"),"markdown":"body"}),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        }
    });
    let mut sequences = Vec::new();
    while sequences.len() < 80 {
        let frame = Fixture::next(&mut live).await;
        if let LiveFrame::Entry { entry, .. } = &frame {
            sequences.push(entry["seq"].as_i64().unwrap());
        }
        live.accept(&frame).await.unwrap();
    }
    writer.await.unwrap();
    assert_eq!(sequences, (1..=80).collect::<Vec<_>>());
}

#[tokio::test]
async fn multiplex_two_spaces_and_explicit_foreign_reset() {
    let f = Fixture::new().await;
    let storage = Arc::new(MemoryBookmarks::default());
    let mut live = f
        .client()
        .subscribe(f.spaces.clone(), storage.clone(), CancellationToken::new())
        .await
        .unwrap();
    for _ in 0..2 {
        let frame = Fixture::next(&mut live).await;
        live.accept(&frame).await.unwrap();
    }
    for space in &f.spaces {
        f.create(space, 0).await;
    }
    let mut seen = std::collections::HashSet::new();
    while seen.len() < 2 {
        let frame = Fixture::next(&mut live).await;
        if let LiveFrame::Entry { space_id, .. } = &frame {
            seen.insert(space_id.clone());
        }
        live.accept(&frame).await.unwrap();
    }
    drop(live);
    let foreign = storage.load(&f.spaces[0]).await.unwrap().unwrap();
    storage.save(&f.spaces[1], &foreign).await.unwrap();
    let mut live = f
        .client()
        .subscribe(vec![f.spaces[1].clone()], storage, CancellationToken::new())
        .await
        .unwrap();
    assert!(matches!(
        Fixture::next(&mut live).await,
        LiveFrame::Reset { .. }
    ));
}

#[tokio::test]
async fn nonmember_and_missing_space_have_identical_http_refusals() {
    let f = Fixture::new().await;
    let guest = Client::new(&f.url, &f.guest_token).unwrap();
    let grants = guest.describe().await.unwrap().principal.spaces;
    let hidden = f
        .spaces
        .iter()
        .find(|s| !grants.iter().any(|g| g.id == **s))
        .unwrap();
    let mut signatures = Vec::new();
    for space in [hidden.to_owned(), uuid::Uuid::now_v7().to_string()] {
        let response = reqwest::Client::new()
            .get(format!("{}/api/v1/live", f.url))
            .bearer_auth(&f.guest_token)
            .query(&[(
                "subscriptions",
                json!([{"space_id":space,"bookmark":null}]).to_string(),
            )])
            .send()
            .await
            .unwrap();
        let status = response.status();
        let body: Value = response.json().await.unwrap();
        signatures.push((status, body));
    }
    assert_eq!(signatures[0], signatures[1]);
    assert_eq!(signatures[0].0.as_u16(), 404);
}

#[tokio::test]
async fn idle_membership_revocation_closes_without_delivering_queued_entries() {
    let f = Fixture::new().await;
    let guest = Client::new(&f.url, &f.guest_token).unwrap();
    let shared = guest.describe().await.unwrap().principal.spaces[0]
        .id
        .clone();
    let account = guest.describe().await.unwrap().principal.id;
    let mut live = guest
        .subscribe(
            vec![shared.clone()],
            Arc::new(MemoryBookmarks::default()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(matches!(
        Fixture::next(&mut live).await,
        LiveFrame::Ready { .. }
    ));
    api::dispatch(
        f.state.clone(),
        f.owner.clone(),
        Session::Static,
        "remove",
        "spaces.remove_member",
        serde_json::from_value(json!({"space_id":shared,"account_id":account})).unwrap(),
    )
    .await
    .unwrap();
    f.create(&shared, 123).await;
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(5), live.next())
            .await
            .unwrap()
            .is_err()
    );
}

#[tokio::test]
async fn lagged_broadcast_recovers_from_database_without_skipping() {
    let f = Fixture::new().await;
    let space = f.spaces[0].clone();
    let note = f.create(&space, 0).await;
    let storage = Arc::new(MemoryBookmarks::default());
    let mut live = f
        .client()
        .subscribe(vec![space.clone()], storage, CancellationToken::new())
        .await
        .unwrap();
    let ready = Fixture::next(&mut live).await;
    live.accept(&ready).await.unwrap();
    let state = f.state.clone();
    let sp = space.clone();
    let (journal, mut witness) = tokio::task::spawn_blocking(move || {
        let content = state.store.shared_content();
        let space = sp.parse().unwrap();
        let journal = content.live_journal(space).unwrap();
        let receiver = journal.subscribe();
        (journal, receiver)
    })
    .await
    .unwrap();
    let author = f.owner.id;
    let count = calternal_store::LIVE_BUFFER + 20;
    // A single committed batch exceeds the broadcast ring before any receiver
    // can poll. The reader must use the database, not treat Lagged as a close.
    let events = (0..count)
        .map(|i| calternal_store::StampedEvent {
            author,
            kind: "notes.update".into(),
            item: note["note"]["id"].as_str().unwrap().into(),
            data: json!({"revision":format!("lag-{i}")}),
            installation: None,
            request_id: None,
        })
        .collect();
    journal
        .commit(calternal_store::Commit {
            events,
            idempotency: None,
        })
        .await
        .unwrap();
    assert!(matches!(
        witness.recv().await,
        Err(tokio::sync::broadcast::error::RecvError::Lagged(_))
    ));
    let mut seqs = Vec::new();
    while seqs.len() < count {
        let frame = Fixture::next(&mut live).await;
        if let LiveFrame::Entry { entry, .. } = &frame {
            seqs.push(entry["seq"].as_i64().unwrap());
        }
        live.accept(&frame).await.unwrap();
    }
    assert_eq!(seqs, (2..count as i64 + 2).collect::<Vec<_>>());
}

#[tokio::test]
async fn unread_connection_cancellation_releases_subscriber() {
    let f = Fixture::new().await;
    let cancellation = CancellationToken::new();
    let space = f.spaces[0].clone();
    let mut live = f
        .client()
        .subscribe(
            vec![space.clone()],
            Arc::new(MemoryBookmarks::default()),
            cancellation.clone(),
        )
        .await
        .unwrap();
    let _ = Fixture::next(&mut live).await;
    cancellation.cancel();
    assert!(matches!(
        live.next().await,
        Err(calternal_client::ClientError::Cancelled)
    ));
    drop(live);
}

#[tokio::test]
async fn offline_input_drains_after_authorized_connect() {
    let f = Fixture::new().await;
    let space = f.spaces[0].clone();
    let mut live = f
        .client()
        .subscribe(
            vec![space.clone()],
            Arc::new(MemoryBookmarks::default()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    live.outbox()
        .lock()
        .await
        .enqueue(
            Client::pending(
                "notes.create",
                &json!({"space_id":space,"title":"offline input","markdown":"queued"}),
                Some("stable-offline-id".into()),
            )
            .unwrap(),
        )
        .unwrap();
    loop {
        let frame = Fixture::next(&mut live).await;
        if let LiveFrame::Entry { entry, .. } = &frame {
            assert_eq!(entry["request_id"], "stable-offline-id");
            break;
        }
        live.accept(&frame).await.unwrap();
    }
    assert_eq!(live.outbox().lock().await.pending().count(), 0);
}

#[tokio::test]
async fn foreign_item_entries_are_filtered_without_advancing_over_unread_visible_items() {
    let f = Fixture::new().await;
    let guest = Client::new(&f.url, &f.guest_token).unwrap();
    let shared = guest.describe().await.unwrap().principal.spaces[0]
        .id
        .clone();
    let private = f.spaces.iter().find(|s| **s != shared).unwrap();
    let private_note = f.create(private, 0).await;
    let mut live = guest
        .subscribe(
            vec![shared.clone()],
            Arc::new(MemoryBookmarks::default()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let ready = Fixture::next(&mut live).await;
    live.accept(&ready).await.unwrap();
    let state = f.state.clone();
    let sp = shared.clone();
    let journal = tokio::task::spawn_blocking(move || {
        let content = state.store.shared_content();
        let space = sp.parse().unwrap();
        content.live_journal(space).unwrap()
    })
    .await
    .unwrap();
    journal
        .commit(calternal_store::Commit {
            events: vec![calternal_store::StampedEvent {
                author: f.owner.id,
                kind: "notes.update".into(),
                item: private_note["note"]["id"].as_str().unwrap().into(),
                data: json!({"secret":"must not be delivered"}),
                installation: None,
                request_id: None,
            }],
            idempotency: None,
        })
        .await
        .unwrap();
    let frame = Fixture::next(&mut live).await;
    assert!(matches!(frame, LiveFrame::Ready { .. }));
    live.accept(&frame).await.unwrap();
    let visible = f.create(&shared, 1).await;
    let frame = loop {
        let frame = Fixture::next(&mut live).await;
        if matches!(frame, LiveFrame::Entry { .. }) {
            break frame;
        }
        live.accept(&frame).await.unwrap();
    };
    if let LiveFrame::Entry { entry, .. } = frame {
        assert_eq!(entry["item"], visible["note"]["id"]);
        assert_eq!(entry["seq"], 2);
    }
}
