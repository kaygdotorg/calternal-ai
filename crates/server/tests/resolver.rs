//! Addresses resolve only through current stored space authority.
use calternal_server::{
    api::{AppState, Shared, dispatch},
    auth::{Directory, Identity, TokenTable},
    plugins::Registry,
    store::Store,
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

struct Fixture {
    _dir: tempfile::TempDir,
    state: Shared,
    owner: Identity,
    guest: Identity,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let tokens = TokenTable::load_or_create(dir.path()).unwrap();
        let identity = |name: &str| {
            let token = std::fs::read(dir.path().join(format!("dev-tokens/{name}.token"))).unwrap();
            tokens.authenticate(&token).unwrap()
        };
        let owner = identity("owner");
        let guest = identity("guest");
        let store = Store::open(dir.path()).unwrap();
        let state = Arc::new(AppState {
            store: Mutex::new(store),
            tokens,
            registry: Registry::load(std::path::Path::new("/nonexistent-test-plugins")),
            workers: calternal_server::plugins::WorkerPool::new(None),
            allowed_hosts: Vec::new(),
        });
        Self {
            _dir: dir,
            state,
            owner,
            guest,
        }
    }
    async fn run(
        &self,
        account: &Identity,
        key: &str,
        op: &str,
        input: Value,
    ) -> Result<Value, calternal_server::error::HostError> {
        dispatch(
            self.state.clone(),
            account.clone(),
            calternal_server::plugins::Session::Static,
            key,
            op,
            serde_json::from_value(input).unwrap(),
        )
        .await
    }
    async fn create(&self, space: &str) -> Value {
        self.run(
            &self.owner,
            &uuid::Uuid::now_v7().to_string(),
            "notes.create",
            json!({"space_id":space,"title":"fixture","markdown":"original"}),
        )
        .await
        .unwrap()["note"]
            .clone()
    }
}
#[tokio::test]
async fn no_access_wrong_kind_and_nonexistent_are_indistinguishable() {
    let f = Fixture::new();
    let note = f.create("personal").await;
    let mut errors = Vec::new();
    for (kind, id) in [
        ("notes", note["id"].as_str().unwrap().to_owned()),
        ("tasks", note["id"].as_str().unwrap().to_owned()),
        ("notes", uuid::Uuid::now_v7().to_string()),
    ] {
        let e = f
            .run(&f.guest, "r", "resolve", json!({"kind":kind,"id":id}))
            .await
            .unwrap_err();
        errors.push((e.code, e.message, e.details));
    }
    assert_eq!(errors[0], errors[1]);
    assert_eq!(errors[1], errors[2]);
    assert_eq!(errors[0].0, calternal_core::ErrorCode::NotFound);
    // These are open kinds, not schema-invalid enum values. Until a provider
    // exists, each follows the same hidden-existence outcome.
    for kind in ["tasks", "files", "collections", "future-kind"] {
        let e = f
            .run(
                &f.owner,
                "r",
                "resolve",
                json!({"kind":kind,"id":note["id"]}),
            )
            .await
            .unwrap_err();
        assert_eq!(e.code, calternal_core::ErrorCode::NotFound);
        assert_eq!(e.message, "item not found");
    }
}
#[tokio::test]
async fn one_account_resolves_two_spaces_without_a_space_hint() {
    let f = Fixture::new();
    let a = f.create("personal").await;
    let b = f.create("shared").await;
    for note in [&a, &b] {
        let resolved = f
            .run(
                &f.owner,
                "r",
                "resolve",
                json!({"kind":"notes","id":note["id"]}),
            )
            .await
            .unwrap();
        assert_eq!(resolved["space_id"], note["space_id"]);
        assert!(
            resolved["access"]
                .as_array()
                .unwrap()
                .contains(&json!("space.read"))
        );
        let read = f
            .run(&f.owner, "g", "notes.get", json!({"id":note["id"]}))
            .await
            .unwrap();
        assert_eq!(read["note"]["id"], note["id"]);
    }
    assert_ne!(a["space_id"], b["space_id"]);
}
#[tokio::test]
async fn revocation_hides_resolution_and_reads_on_the_next_request() {
    let f = Fixture::new();
    let note = f.create("shared").await;
    f.run(
        &f.guest,
        "r",
        "resolve",
        json!({"kind":"notes","id":note["id"]}),
    )
    .await
    .unwrap();
    f.run(
        &f.owner,
        "remove",
        "spaces.remove_member",
        json!({"space_id":note["space_id"],"account_id":f.guest.id.as_ref()}),
    )
    .await
    .unwrap();
    for (op, input) in [
        ("resolve", json!({"kind":"notes","id":note["id"]})),
        ("notes.get", json!({"id":note["id"]})),
    ] {
        let e = f.run(&f.guest, "r", op, input).await.unwrap_err();
        assert_eq!(e.code, calternal_core::ErrorCode::NotFound);
        assert_eq!(e.message, "item not found");
    }
}
#[tokio::test]
async fn notes_update_and_history_derive_space_and_reject_wrong_hints() {
    let f = Fixture::new();
    let note = f.create("personal").await;
    let updated = f
        .run(
            &f.owner,
            "update",
            "notes.update",
            json!({"id":note["id"],"base_revision":note["revision"],"markdown":"updated"}),
        )
        .await
        .unwrap();
    assert_eq!(updated["note"]["markdown"], "updated");
    let history = f
        .run(
            &f.owner,
            "history",
            "notes.history",
            json!({"id":note["id"]}),
        )
        .await
        .unwrap();
    assert_eq!(history["versions"].as_array().unwrap().len(), 2);
    let e = f
        .run(
            &f.owner,
            "wrong",
            "notes.get",
            json!({"id":note["id"],"space_id":"shared"}),
        )
        .await
        .unwrap_err();
    assert_eq!(e.code, calternal_core::ErrorCode::NotFound);
}
