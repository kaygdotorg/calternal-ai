//! Replay and delivery authorization over the real HTTP live route.
use calternal_client::{CancellationToken, Client, MemoryBookmarks};
use calternal_core::AccountFrame;
use calternal_server::account_input::{Inputs, Limits};
use calternal_server::{
    api::{self, AppState, Shared},
    auth::{Directory, Identity, StoredDirectory},
    plugins::{Jobs, Registry, WorkerPool},
    store::Store,
};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

struct Fixture {
    _dir: tempfile::TempDir,
    state: Shared,
    url: String,
    token: String,
    guest_token: String,
    owner: Identity,
    spaces: Vec<String>,
    task: tokio::task::JoinHandle<()>,
    clock: Arc<AtomicI64>,
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
                let store = Store::open(
                    &roots,
                    uuid::Uuid::from_u128(1),
                    true,
                    calternal_content::DEFAULT_RECEIPT_RETENTION,
                )
                .unwrap();
                let tokens = StoredDirectory::load_or_create(&roots.appdata).unwrap();
                let token =
                    std::fs::read_to_string(dir.path().join("appdata/dev-tokens/owner.token"))
                        .unwrap();
                let guest_token =
                    std::fs::read_to_string(dir.path().join("appdata/dev-tokens/guest.token"))
                        .unwrap();
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
        let clock = Arc::new(AtomicI64::new(1_800_000_000_000));
        let injected = clock.clone();
        let state = Arc::new(AppState {
            inputs: Inputs::with_clock(
                dir.path(),
                Limits {
                    drafts: 2,
                    pending: 2,
                    bytes: 4096,
                    events: 3,
                    ..Default::default()
                },
                Arc::new(move || injected.load(Ordering::SeqCst)),
            )
            .with_content(store.shared_content()),
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
            clock,
        }
    }
    fn client(&self) -> Client {
        Client::new(&self.url, &self.token).unwrap()
    }
    async fn op(
        &self,
        guest: bool,
        name: &str,
        input: Value,
    ) -> Result<Value, calternal_client::ClientError> {
        let client = if guest {
            Client::new(&self.url, &self.guest_token).unwrap()
        } else {
            self.client()
        };
        let pending = Client::pending(name, &input, None)?;
        client
            .execute_cancellable(pending.request(), &CancellationToken::new())
            .await
    }
    async fn frame(live: &mut calternal_client::AccountConnection) -> AccountFrame {
        tokio::time::timeout(std::time::Duration::from_secs(5), live.next())
            .await
            .unwrap()
            .unwrap()
    }
    fn queue(&self, id: &str, age: i64) -> Value {
        json!({"space_id":self.spaces[0],"item_id":uuid::Uuid::now_v7().to_string(),"created_at_ms":self.clock.load(Ordering::SeqCst)-age,"request":{"request_id":id,"operation":"notes.create","input":{"space_id":self.spaces[0],"title":id,"markdown":"queued"}}})
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn drafts_sync_second_client_lww_caps_and_account_isolation() {
    let f = Fixture::new().await;
    let mut stream = f
        .client()
        .subscribe_account(
            Arc::new(MemoryBookmarks::default()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let ready = Fixture::frame(&mut stream).await;
    stream.accept(&ready).await.unwrap();
    let item = uuid::Uuid::now_v7().to_string();
    let key = json!({"space_id":f.spaces[0],"item_id":item});
    let mut save = key.clone();
    save["text"] = json!("first draft");
    let one = f.op(false, "drafts.save", save.clone()).await.unwrap();
    assert!(matches!(
        Fixture::frame(&mut stream).await,
        AccountFrame::Changed { .. }
    ));
    save["text"] = json!("second draft");
    let two = f.op(false, "drafts.save", save).await.unwrap();
    assert_ne!(one["record"]["revision"], two["record"]["revision"]);
    assert_eq!(
        f.op(false, "drafts.get", key.clone()).await.unwrap()["record"]["text"],
        "second draft"
    );
    assert!(f.op(true, "drafts.get", key.clone()).await.is_err());
    assert_eq!(
        f.op(true, "drafts.list", json!({})).await.unwrap()["records"],
        json!([])
    );
    f.op(
        false,
        "drafts.save",
        json!({"space_id":f.spaces[0],"item_id":uuid::Uuid::now_v7().to_string(),"text":"two"}),
    )
    .await
    .unwrap();
    assert!(f.op(false,"drafts.save",json!({"space_id":f.spaces[0],"item_id":uuid::Uuid::now_v7().to_string(),"text":"three"})).await.is_err());
    f.op(false, "drafts.delete", key).await.unwrap();
    assert!(f.op(false,"drafts.save",json!({"space_id":f.spaces[0],"item_id":uuid::Uuid::now_v7().to_string(),"text":"x".repeat(4096)})).await.is_err());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pending_stable_id_retry_edit_confirm_and_cancel() {
    let f = Fixture::new().await;
    let q = f.queue("send-once", 0);
    f.op(false, "pending.queue", q.clone()).await.unwrap();
    f.op(false, "pending.queue", q.clone()).await.unwrap();
    let mut edited = q.clone();
    edited["request"]["input"]["markdown"] = json!("edited");
    assert!(f.op(false, "pending.queue", edited.clone()).await.is_err());
    f.op(false, "pending.edit", edited.clone()).await.unwrap();
    assert!(f.op(true, "pending.edit", edited.clone()).await.is_err());
    assert!(
        f.op(true, "pending.submit", json!({"space_id":f.spaces[0],"id":"send-once"}))
            .await
            .is_err()
    );
    f.client()
        .drain_pending(&CancellationToken::new())
        .await
        .unwrap();
    // The terminal reply survives removal of the pending record, without a second result log.
    let retried = f
        .op(false, "pending.submit", json!({"space_id":f.spaces[0],"id":"send-once"}))
        .await
        .unwrap();
    assert!(retried["result"]["note"]["id"].is_string());
    // Simulate a lost acknowledgement: re-queue the exact stable envelope.
    f.op(false, "pending.queue", edited).await.unwrap();
    f.client()
        .drain_pending(&CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        f.op(false, "notes.list", json!({"space_id":f.spaces[0]}))
            .await
            .unwrap()["notes"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let old = f.queue("old", 8 * 24 * 60 * 60 * 1000);
    f.op(false, "pending.queue", old).await.unwrap();
    let held = f
        .op(false, "pending.submit", json!({"space_id":f.spaces[0],"id":"old"}))
        .await
        .unwrap();
    assert_eq!(held["record"]["state"], "confirmation_required");
    assert!(
        f.op(true, "pending.confirm", json!({"space_id":f.spaces[0],"id":"old"}))
            .await
            .is_err()
    );
    f.op(false, "pending.confirm", json!({"space_id":f.spaces[0],"id":"old"}))
        .await
        .unwrap();
    f.op(false, "pending.queue", f.queue("cancel", 0))
        .await
        .unwrap();
    assert!(
        f.op(true, "pending.cancel", json!({"space_id":f.spaces[0],"id":"cancel"}))
            .await
            .is_err()
    );
    f.op(false, "pending.cancel", json!({"space_id":f.spaces[0],"id":"cancel"}))
        .await
        .unwrap();
    assert_eq!(
        f.op(false, "pending.list", json!({})).await.unwrap()["records"],
        json!([])
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pending_cap_age_transition_and_revocation_cleanup() {
    let f = Fixture::new().await;
    for id in ["one", "two"] {
        f.op(false, "pending.queue", f.queue(id, 0)).await.unwrap();
    }
    assert!(
        f.op(false, "pending.queue", f.queue("three", 0))
            .await
            .is_err()
    );
    f.clock.fetch_add(7 * 24 * 60 * 60 * 1000, Ordering::SeqCst);
    let boundary = f.op(false, "pending.list", json!({})).await.unwrap();
    assert!(
        boundary["records"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["state"] == "queued")
    );
    f.clock.fetch_add(1, Ordering::SeqCst);
    let rows = f.op(false, "pending.list", json!({})).await.unwrap();
    assert!(
        rows["records"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["state"] == "confirmation_required")
    );
    let owner = f.owner.id.to_string();
    let space = f.spaces[0].clone();
    let st = f.state.clone();
    // Direct directory removal is also filtered on the next account operation.
    tokio::task::spawn_blocking(move || {
        st.directory
            .spaces
            .delete(
                calternal_types::AccountId::parse(&owner).unwrap(),
                calternal_types::SpaceId::parse(&space).unwrap(),
            )
            .unwrap()
    })
    .await
    .unwrap();
    assert!(
        f.op(false, "pending.submit", json!({"space_id":f.spaces[0],"id":"one"}))
            .await
            .is_err()
    );
    assert_eq!(
        f.op(false, "pending.list", json!({})).await.unwrap()["records"],
        json!([])
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn foreign_expired_and_restart_bookmarks_reset() {
    let f = Fixture::new().await;
    let storage = Arc::new(MemoryBookmarks::default());
    let mut own = f
        .client()
        .subscribe_account(storage.clone(), CancellationToken::new())
        .await
        .unwrap();
    let ready = Fixture::frame(&mut own).await;
    own.accept(&ready).await.unwrap();
    drop(own);
    let guest = Client::new(&f.url, &f.guest_token).unwrap();
    let mut foreign = guest
        .subscribe_account(storage.clone(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(Fixture::frame(&mut foreign).await, AccountFrame::Reset);
    for i in 0..5 {
        f.op(
            false,
            "drafts.save",
            json!({"space_id":f.spaces[0],"item_id":f.spaces[1],"text":format!("{i}")}),
        )
        .await
        .unwrap();
    }
    let mut expired = f
        .client()
        .subscribe_account(storage, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(Fixture::frame(&mut expired).await, AccountFrame::Reset);
    expired.resume_after_reload();
    assert!(matches!(
        Fixture::frame(&mut expired).await,
        AccountFrame::Ready { .. }
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restored_offline_input_keeps_age_and_transfers_to_confirmation_request() {
    let f = Fixture::new().await;
    let input: calternal_core::PendingQueueInput =
        serde_json::from_value(f.queue("offline-old", 8 * 24 * 60 * 60 * 1000)).unwrap();
    let mut outbox = calternal_client::OfflineOutbox::default();
    outbox.restore(input.clone()).unwrap();
    let restored = outbox.submissions().next().unwrap();
    assert_eq!(restored.created_at_ms, input.created_at_ms);
    assert_eq!(
        outbox
            .drain(&f.client(), &CancellationToken::new())
            .await
            .unwrap()[0]["record"]["state"],
        "confirmation_required"
    );
    assert_eq!(outbox.pending().count(), 0);
    assert_eq!(
        f.op(false, "pending.list", json!({})).await.unwrap()["records"][0]["request"]["request_id"],
        "offline-old"
    );
    assert_eq!(
        f.op(false, "notes.list", json!({"space_id":f.spaces[0]}))
            .await
            .unwrap()["notes"],
        json!([])
    );
}
