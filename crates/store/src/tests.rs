use super::test_support::{TempDir, test_account, test_space};
use super::*;
use serde_json::json;
use std::time::Duration;

mod restore;

fn ev(kind: &str, item: &str) -> StampedEvent {
    StampedEvent {
        author: test_account(),
        kind: kind.into(),
        item: item.into(),
        data: json!({"n": item}),
        installation: None,
        request_id: None,
    }
}

#[tokio::test]
async fn commits_are_read_back_in_order() {
    let dir = TempDir::new();
    let s = SpaceStore::open_dir(dir.path()).await.unwrap();
    assert_eq!(s.head().seq, 0);
    let a = s
        .commit(Commit {
            events: vec![ev("t.a", "1"), ev("t.a", "2")],
            idempotency: None,
            durable: false,
        })
        .await
        .unwrap();
    assert_eq!(a.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(s.head().seq, 2);
    let all = s.since(0, 100).await.unwrap();
    assert_eq!(all, a);
    assert_eq!(s.since(1, 100).await.unwrap(), a[1..]);
    assert!(s.since(2, 100).await.unwrap().is_empty());
}

#[tokio::test]
async fn idempotency_is_saved_with_the_events_or_not_at_all() {
    let dir = TempDir::new();
    let s = SpaceStore::open_dir(dir.path()).await.unwrap();
    let idem = |result| IdemWrite {
        account: test_account(),
        key: "k1".into(),
        action: "t.do".into(),
        payload_hash: "h".into(),
        result,
    };
    s.commit(Commit {
        events: vec![ev("t.a", "1")],
        idempotency: Some(idem(json!({"ok": 1}))),
        durable: false,
    })
    .await
    .unwrap();
    let rec = s.idempotency(test_account(), "k1").await.unwrap().unwrap();
    assert_eq!(rec.result, json!({"ok": 1}));
    assert_eq!(rec.seq, 1);

    // Reusing the key rolls back the whole commit: no second event.
    let err = s
        .commit(Commit {
            events: vec![ev("t.a", "2")],
            idempotency: Some(idem(json!({"ok": 2}))),
            durable: false,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::IdempotencyConflict);
    assert_eq!(s.since(0, 100).await.unwrap().len(), 1);
    assert_eq!(
        s.idempotency(test_account(), "missing").await.unwrap(),
        None
    );
}

#[tokio::test]
async fn tokens_survive_a_restart_and_reject_strangers() {
    let dir = TempDir::new();
    let first = SpaceStore::open_dir(dir.path()).await.unwrap();
    first
        .commit(Commit {
            events: vec![ev("t.a", "1")],
            idempotency: None,
            durable: false,
        })
        .await
        .unwrap();
    let token = first.head().to_string();
    drop(first);

    let again = SpaceStore::open_dir(dir.path()).await.unwrap();
    assert_eq!(again.head().to_string(), token);
    assert_eq!(again.position(&token).await.unwrap(), 1);

    let other = TempDir::new();
    let stranger = SpaceStore::open_dir(other.path()).await.unwrap();
    assert_eq!(
        stranger.position(&token).await.unwrap_err().code,
        ErrorCode::SyncTokenExpired
    );
    let future = format!("{}-99", again.head().epoch);
    assert_eq!(
        again.position(&future).await.unwrap_err().code,
        ErrorCode::SyncTokenExpired
    );
}

#[tokio::test]
async fn reads_are_capped() {
    let dir = TempDir::new();
    let s = SpaceStore::open_dir(dir.path()).await.unwrap();
    let events = (0..read::MAX_PAGE + 50)
        .map(|i| ev("t.a", &i.to_string()))
        .collect();
    s.commit(Commit {
        events,
        idempotency: None,
        durable: false,
    })
    .await
    .unwrap();
    assert_eq!(s.since(0, usize::MAX).await.unwrap().len(), read::MAX_PAGE);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn live_entries_arrive_in_journal_order() {
    let dir = TempDir::new();
    let s = SpaceStore::open_dir(dir.path()).await.unwrap();
    let mut rx = s.subscribe();
    let mut tasks = Vec::new();
    for t in 0..16 {
        let s = s.clone();
        tasks.push(tokio::spawn(async move {
            for i in 0..20 {
                s.commit(Commit {
                    events: vec![ev("t.a", &format!("{t}-{i}"))],
                    idempotency: None,
                    durable: false,
                })
                .await
                .unwrap();
            }
        }));
    }
    for t in tasks {
        t.await.unwrap();
    }
    let mut last = 0;
    for _ in 0..320 {
        let e = rx.recv().await.unwrap();
        assert!(e.seq > last, "{} arrived after {last}", e.seq);
        last = e.seq;
    }
    assert_eq!(last, 320);
}

fn idem(key: &str) -> Commit {
    Commit {
        events: vec![ev("t.a", key)],
        idempotency: Some(IdemWrite {
            account: test_account(),
            key: key.into(),
            action: "t.do".into(),
            payload_hash: "h".into(),
            result: json!(key),
        }),
        durable: false,
    }
}

#[tokio::test]
async fn only_the_explicit_prune_removes_expired_unpinned_receipts() {
    let dir = TempDir::new();
    let store = SpaceStore::open_dir(dir.path()).await.unwrap();
    let key = "opaque:request";
    store.commit(idem(key)).await.unwrap();
    let other = AccountId::new();
    let mut same_text = idem(key);
    same_text.idempotency.as_mut().unwrap().account = other;
    store.commit(same_text).await.unwrap();
    drop(store);

    let db = rusqlite::Connection::open(dir.path().join("index.db")).unwrap();
    db.execute("UPDATE idempotency SET at_ms=0", []).unwrap();
    drop(db);
    let store = SpaceStore::open_dir(dir.path()).await.unwrap();
    store.commit(idem("fresh")).await.unwrap();
    assert!(store.idempotency(other, key).await.unwrap().is_some());

    store
        .prune_receipts(now_ms(), vec![(test_account(), key.into())])
        .await
        .unwrap();
    assert!(
        store
            .idempotency(test_account(), key)
            .await
            .unwrap()
            .is_some()
    );
    assert!(store.idempotency(other, key).await.unwrap().is_none());
    assert!(
        store
            .idempotency(test_account(), "fresh")
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(store.since(0, 100).await.unwrap().len(), 3);
}

#[tokio::test]
async fn a_store_with_a_listener_or_a_copy_is_in_use() {
    let dir = TempDir::new();
    let s = SpaceStore::open_dir(dir.path()).await.unwrap();
    assert!(s.is_unused());
    let rx = s.subscribe();
    assert!(!s.is_unused());
    drop(rx);
    let copy = s.clone();
    assert!(!s.is_unused());
    drop(copy);
    assert!(s.is_unused());
}

/// Members may reuse request IDs but can read only their own response.
#[tokio::test]
async fn receipts_are_partitioned_by_account_within_one_space() {
    let temp = TempDir::new();
    let space = SpaceId::new();
    let a = AccountId::new();
    let b = AccountId::new();
    assert_ne!(a.as_uuid(), space.as_uuid());
    let store = SpaceStore::open(
        temp.path().join("state"),
        space,
        &temp.path().join("secrets"),
    )
    .await
    .unwrap();
    for (account, value) in [(a, "private-a"), (b, "private-b")] {
        if account == a {
            assert!(store.idempotency(b, "request").await.unwrap().is_none());
        }
        let mut event = ev("t.member", value);
        event.author = account;
        store
            .commit(Commit {
                events: vec![event],
                idempotency: Some(IdemWrite {
                    account,
                    key: "request".into(),
                    action: "t.member".into(),
                    payload_hash: "h".into(),
                    result: json!(value),
                }),
                durable: false,
            })
            .await
            .unwrap();
        assert_eq!(
            store
                .idempotency(account, "request")
                .await
                .unwrap()
                .unwrap()
                .result,
            json!(value)
        );
        if account == a {
            assert!(store.idempotency(b, "request").await.unwrap().is_none());
        }
    }
    assert_eq!(
        store
            .idempotency(a, "request")
            .await
            .unwrap()
            .unwrap()
            .result,
        json!("private-a")
    );
    let entries = store.since(0, 100).await.unwrap();
    assert_eq!(
        entries.iter().map(|e| e.author).collect::<Vec<_>>(),
        vec![a, b]
    );
    assert!(
        temp.path()
            .join("state/spaces")
            .join(space.to_string())
            .join("index.db")
            .is_file()
    );
}

/// Equal epoch/head/seed bytes do not make a different space's signature valid.
#[tokio::test]
async fn cursors_and_copied_databases_cannot_cross_spaces() {
    let temp = TempDir::new();
    let state = temp.path().join("state");
    let secrets = temp.path().join("secrets");
    let a = SpaceId::new();
    let b = SpaceId::new();
    let first = SpaceStore::open(&state, a, &secrets).await.unwrap();
    let epoch = first.head().epoch;
    let token = first.token_at(0).await.unwrap();
    drop(first);
    drop(SpaceStore::open(&state, b, &secrets).await.unwrap());
    let a_secrets = secrets.join("spaces").join(a.to_string());
    let b_secrets = secrets.join("spaces").join(b.to_string());
    for name in ["token.key", "head"] {
        std::fs::copy(a_secrets.join(name), b_secrets.join(name)).unwrap();
    }
    let db = state.join("spaces").join(b.to_string()).join("index.db");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute("UPDATE meta SET value=?1 WHERE key='epoch'", [&epoch])
        .unwrap();
    drop(conn);
    let second = SpaceStore::open(&state, b, &secrets).await.unwrap();
    assert_eq!(second.head().epoch, epoch);
    assert_eq!(
        second.position(&token).await.unwrap_err().code,
        ErrorCode::SyncTokenExpired
    );
    assert_eq!(
        *second.restored().borrow(),
        0,
        "foreign cursors must not rotate the journal"
    );
    assert_eq!(
        second
            .position(&second.token_at(0).await.unwrap())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        second
            .position(&second.head().to_string())
            .await
            .unwrap_err()
            .code,
        ErrorCode::SyncTokenExpired
    );
    drop(second);
    // Refuse a database rebound under another directory, even when epochs match.
    let c = SpaceId::new();
    let destination = state.join("spaces").join(c.to_string());
    std::fs::create_dir_all(&destination).unwrap();
    std::fs::copy(&db, destination.join("index.db")).unwrap();
    assert!(SpaceStore::open(&state, c, &secrets).await.is_err());
}
