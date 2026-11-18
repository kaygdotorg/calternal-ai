//! Addresses resolve only through current stored space authority.
mod support;
use calternal_server::{
    api::{AppState, Shared, dispatch},
    auth::{Identity, StoredDirectory},
    plugins::Registry,
    store::Store,
};
use serde_json::{Value, json};
use std::sync::Arc;

struct Fixture {
    _dir: tempfile::TempDir,
    state: Shared,
    owner: Identity,
    guest: Identity,
    dev: support::Dev,
}
impl Fixture {
    fn new() -> Self {
        Self::with_http(false)
    }
    fn with_http(http: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let roots = support::roots(dir.path());
        let store = Store::open(
            &roots,
            uuid::Uuid::from_u128(1),
            true,
            calternal_content::DEFAULT_RECEIPT_RETENTION,
        )
        .unwrap();
        let tokens = if http {
            StoredDirectory::open(
                &roots.appdata,
                calternal_auth::AuthConfig::new("http://localhost:8080").unwrap(),
                true,
                None,
            )
            .unwrap()
        } else {
            StoredDirectory::load_or_create(&roots.appdata).unwrap()
        };
        let dev = support::dev(&tokens);
        let (owner, guest) = (dev.owner, dev.guest);
        let state = Arc::new(AppState {
            blocking: calternal_server::blocking::RequestWorkers::new().unwrap(),
            inputs: store.account_inputs(),
            store,
            directory: tokens,
            registry: Registry::load(std::path::Path::new("/nonexistent-test-plugins")),
            workers: calternal_server::plugins::WorkerPool::new(None),
            jobs: calternal_server::plugins::Jobs::disabled(),
            allowed_hosts: calternal_server::api::allowed_hosts(8080),
        });
        Self {
            _dir: dir,
            state,
            owner,
            guest,
            dev,
        }
    }
    async fn run(
        &self,
        account: &Identity,
        key: &str,
        op: &str,
        input: Value,
    ) -> Result<Value, calternal_types::Error> {
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
    async fn create(&self, space: calternal_types::SpaceId) -> Value {
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
    let note = f.create(f.dev.personal).await;
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
    let a = f.create(f.dev.personal).await;
    let b = f.create(f.dev.shared).await;
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
    let note = f.create(f.dev.shared).await;
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
        json!({"space_id":note["space_id"],"account_id":f.guest.id}),
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
    let note = f.create(f.dev.personal).await;
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
            json!({"id":note["id"],"space_id":f.dev.shared}),
        )
        .await
        .unwrap_err();
    assert_eq!(e.code, calternal_core::ErrorCode::NotFound);
}

#[tokio::test]
async fn item_share_does_not_open_the_space_or_sibling_notes() {
    let f = Fixture::new();
    let note = f.create(f.dev.personal).await;
    let sibling = f.create(f.dev.personal).await;
    let item = json!({"kind":"notes","id":note["id"]});
    let grant = f
        .run(
            &f.owner,
            "grant",
            "sharing.grant",
            json!({
                "item": item, "audience": {"account_id": f.guest.id}, "permission":"view"
            }),
        )
        .await
        .unwrap();
    let result = f
        .run(&f.guest, "get", "notes.get", json!({"id":note["id"]}))
        .await
        .unwrap();
    assert_eq!(result["note"]["markdown"], "original");
    let error = f
        .run(
            &f.guest,
            "get-sibling",
            "notes.get",
            json!({"id":sibling["id"]}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, calternal_core::ErrorCode::NotFound);
    let error = f
        .run(
            &f.guest,
            "write",
            "notes.update",
            json!({
                "id":note["id"],"base_revision":note["revision"],"markdown":"denied"
            }),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, calternal_core::ErrorCode::Forbidden);
    f.run(
        &f.owner,
        "revoke",
        "sharing.revoke",
        json!({
            "item":item,"grant_id":grant["grant_id"]
        }),
    )
    .await
    .unwrap();
    let error = f
        .run(
            &f.guest,
            "get-after-revoke",
            "notes.get",
            json!({"id":note["id"]}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, calternal_core::ErrorCode::NotFound);
}

#[tokio::test]
async fn group_edits_follow_live_membership_and_preserve_a_direct_view_grant() {
    let f = Fixture::with_http(true);
    let note = f.create(f.dev.personal).await;
    let item = json!({"kind":"notes","id":note["id"]});
    let group = f
        .run(
            &f.owner,
            "group",
            "groups.create",
            json!({"name":"Share testers"}),
        )
        .await
        .unwrap();
    let group_id = group["group_id"]
        .as_str()
        .or_else(|| group["id"].as_str())
        .unwrap();
    f.run(
        &f.owner,
        "add",
        "groups.add_member",
        json!({"group_id":group_id,"account_id":f.guest.id}),
    )
    .await
    .unwrap();
    f.run(
        &f.owner,
        "grant-view",
        "sharing.grant",
        json!({"item":item,"audience":{"account_id":f.guest.id},"permission":"view"}),
    )
    .await
    .unwrap();
    f.run(
        &f.owner,
        "grant-edit",
        "sharing.grant",
        json!({"item":item,"audience":{"group_id":group_id},"permission":"edit"}),
    )
    .await
    .unwrap();
    let list = f
        .run(&f.owner, "list", "sharing.list", json!({"item":item}))
        .await
        .unwrap();
    assert_eq!(list["grants"].as_array().unwrap().len(), 2);
    let result = f
        .run(
            &f.guest,
            "edit",
            "notes.update",
            json!({"id":note["id"],"base_revision":note["revision"],"markdown":"group edit"}),
        )
        .await
        .unwrap();
    f.run(
        &f.owner,
        "remove",
        "groups.remove_member",
        json!({"group_id":group_id,"account_id":f.guest.id}),
    )
    .await
    .unwrap();
    let read = f
        .run(&f.guest, "read", "notes.get", json!({"id":note["id"]}))
        .await
        .unwrap();
    assert_eq!(read["note"]["markdown"], "group edit");
    let error = f
        .run(
            &f.guest,
            "edit-again",
            "notes.update",
            json!({"id":note["id"],"base_revision":result["note"]["revision"],"markdown":"denied"}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, calternal_core::ErrorCode::Forbidden);
}

#[tokio::test]
async fn unauthenticated_link_survives_restore_and_each_link_can_be_revoked_alone() {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;
    let f = Fixture::with_http(true);
    let note = f.create(f.dev.personal).await;
    let item = json!({"kind":"notes","id":note["id"]});
    let link = f
        .run(
            &f.owner,
            "link1",
            "sharing.link_create",
            json!({"item":item,"label":"First"}),
        )
        .await
        .unwrap();
    let spare = f
        .run(
            &f.owner,
            "link2",
            "sharing.link_create",
            json!({"item":item,"label":"Second"}),
        )
        .await
        .unwrap();
    let copy = |grant: Value| {
        let f = &f;
        let item = &item;
        async move {
            let result = f
                .run(
                    &f.owner,
                    "copy",
                    "sharing.link_copy",
                    json!({"item":item,"grant_id":grant}),
                )
                .await
                .unwrap();
            result["url"]
                .as_str()
                .unwrap()
                .trim_start_matches("http://localhost:8080")
                .to_owned()
        }
    };
    let path = copy(link["grant_id"].clone()).await;
    let spare_path = copy(spare["grant_id"].clone()).await;
    assert_ne!(path, spare_path);
    let app = calternal_server::api::router(f.state.clone(), None);
    let open = |path: &str| {
        app.clone().oneshot(
            Request::builder()
                .uri(path)
                .header("host", "localhost:8080")
                .header("sec-fetch-site", "cross-site")
                .header("accept", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
    };
    let response = open(&path).await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["referrer-policy"], "no-referrer");
    assert_eq!(response.headers()["x-robots-tag"], "noindex, nofollow");
    let content: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(content, json!({"title":"fixture","markdown":"original"}));
    f.run(&f.owner, "trash", "notes.delete", json!({"id":note["id"]}))
        .await
        .unwrap();
    assert_eq!(open(&path).await.unwrap().status(), 404);
    f.run(
        &f.owner,
        "restore",
        "notes.restore",
        json!({"id":note["id"]}),
    )
    .await
    .unwrap();
    assert_eq!(open(&path).await.unwrap().status(), 200);
    f.run(
        &f.owner,
        "revoke",
        "sharing.revoke",
        json!({"item":item,"grant_id":link["grant_id"]}),
    )
    .await
    .unwrap();
    assert_eq!(open(&path).await.unwrap().status(), 404);
    assert_eq!(open(&spare_path).await.unwrap().status(), 200);
}

#[tokio::test]
async fn saved_collection_shares_its_explicit_members_and_follows_owner_edits() {
    let f = Fixture::new();
    let included = f.create(f.dev.personal).await;
    let sibling = f.create(f.dev.personal).await;
    let created = f
        .run(
            &f.owner,
            "new-list",
            "collections.create",
            json!({
                "space_id":f.dev.personal,"title":"A small set","members":[included["id"]]
            }),
        )
        .await
        .unwrap();
    let id = created["id"].clone();
    f.run(&f.owner,"share-list","sharing.grant",json!({
        "item":{"kind":"collections","id":id},"audience":{"account_id":f.guest.id},"permission":"edit"
    })).await.unwrap();
    let collection = f
        .run(&f.guest, "view-list", "collections.get", json!({"id":id}))
        .await
        .unwrap();
    assert_eq!(collection["members"][0]["id"], included["id"]);
    f.run(
        &f.guest,
        "read-member",
        "notes.get",
        json!({"id":included["id"]}),
    )
    .await
    .unwrap();
    let error = f
        .run(
            &f.guest,
            "read-sibling",
            "notes.get",
            json!({"id":sibling["id"]}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, calternal_core::ErrorCode::NotFound);
    let error = f
        .run(
            &f.guest,
            "widen-list",
            "collections.set_members",
            json!({
                "id":id,"base_revision":created["revision"],"members":[sibling["id"]]
            }),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error.code,
        calternal_core::ErrorCode::Forbidden | calternal_core::ErrorCode::NotFound
    ));
    f.run(
        &f.owner,
        "shrink-list",
        "collections.set_members",
        json!({
            "id":id,"base_revision":created["revision"],"members":[]
        }),
    )
    .await
    .unwrap();
    let error = f
        .run(
            &f.guest,
            "read-removed",
            "notes.get",
            json!({"id":included["id"]}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, calternal_core::ErrorCode::NotFound);
}
