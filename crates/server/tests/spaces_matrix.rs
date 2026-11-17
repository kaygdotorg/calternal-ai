//! Real stored-directory operation matrix, live revocation and content isolation.
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use calternal_server::{
    api::{AppState, Shared, allowed_hosts, router},
    auth::{Directory, TokenTable},
    plugins::Registry,
    store::Store,
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::Arc,
};
use tower::ServiceExt;

struct Fixture {
    state: Shared,
    root: PathBuf,
    token: String,
    owner_token: String,
    actor: String,
    owner: String,
    shared: String,
    personal: String,
    target: String,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // `root` is appdata; its parent holds all three roots.
        let _ = std::fs::remove_dir_all(self.root.parent().unwrap());
    }
}
impl Fixture {
    async fn new(role: Option<&str>) -> Self {
        let role = role.map(str::to_owned);
        tokio::task::spawn_blocking(move || {
            let base =
                std::env::temp_dir().join(format!("calternal-spaces-{}", uuid::Uuid::new_v4()));
            let roots = calternal_roots::Roots { appdata: base.join("appdata"), userdata_ssd: base.join("userdata-ssd"), userdata_hdd: base.join("userdata-hdd") };
            let store = Store::open(&roots, true).unwrap();
            // Every later path is in appdata; `base` holds all three roots.
            let root = roots.appdata.clone();
            let tokens = TokenTable::load_or_create(&root).unwrap();
            let token = std::fs::read_to_string(root.join("dev-tokens/guest.token")).unwrap();
            let owner_token = std::fs::read_to_string(root.join("dev-tokens/owner.token")).unwrap();
            let actor = tokens
                .authenticate(token.as_bytes())
                .unwrap()
                .id
                .to_string();
            let owner_identity = tokens.authenticate(owner_token.as_bytes()).unwrap();
            let owner = owner_identity.id.to_string();
            let spaces = tokens
                .spaces
                .list(calternal_types::AccountId::parse(&owner).unwrap())
                .unwrap();
            let shared = spaces
                .iter()
                .find(|s| s.name == "shared")
                .unwrap()
                .id
                .to_string();
            let personal = spaces
                .iter()
                .find(|s| s.name == "personal")
                .unwrap()
                .id
                .to_string();
            let c = rusqlite::Connection::open(root.join("system/auth.db")).unwrap();
            match role.as_deref() {
                None => {
                    c.execute(
                        "DELETE FROM space_memberships WHERE account_id=?1",
                        [&actor],
                    )
                    .unwrap();
                }
                Some("owner") => {
                    c.execute(
                        "UPDATE spaces SET owner_account=?1 WHERE id=?2",
                        [&actor, &shared],
                    )
                    .unwrap();
                }
                Some(role) => {
                    c.execute(
                        "UPDATE space_memberships SET role=?1 WHERE account_id=?2",
                        [role, &actor],
                    )
                    .unwrap();
                }
            }
            let target = uuid::Uuid::now_v7().to_string();
            c.execute(
                "INSERT INTO users(id,name,username,created_at) VALUES(?1,'Target','target',0)",
                [&target],
            )
            .unwrap();
            c.execute(
                "INSERT INTO space_memberships VALUES(?1,?2,'reader')",
                [&shared, &target],
            )
            .unwrap();
            let state = Arc::new(AppState {
                store,
                tokens,
                registry: Registry::empty(),
                workers: calternal_server::plugins::WorkerPool::new(None),
                jobs: calternal_server::plugins::Jobs::disabled(),
                allowed_hosts: allowed_hosts(0),
            });
            Self {
                state,
                root,
                token,
                owner_token,
                actor,
                owner,
                shared,
                personal,
                target,
            }
        })
        .await
        .unwrap()
    }
    async fn call(
        &self,
        token: Option<&str>,
        rid: &str,
        op: &str,
        input: Value,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder()
            .method("POST")
            .uri("/api/v1/operations")
            .header("Host", "127.0.0.1:0")
            .header("Content-Type", "application/json");
        if let Some(token) = token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        let res = router(self.state.clone(), None)
            .oneshot(
                req.body(Body::from(
                    json!({"request_id":rid,"operation":op,"input":input}).to_string(),
                ))
                .unwrap(),
            )
            .await
            .unwrap();
        let status = res.status();
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }
    fn input(&self, op: &str, space: &str) -> Value {
        match op {
            "spaces.list" => json!({}),
            "spaces.create" => json!({"name":"created"}),
            "spaces.set_role" => json!({"space_id":space,"account_id":self.target,"role":"editor"}),
            "spaces.remove_member" | "spaces.transfer" => {
                json!({"space_id":space,"account_id":self.target})
            }
            _ => json!({"space_id":space}),
        }
    }
}
// None, Reader, Editor, Manager, Owner. Account scope is available without a space
// grant; leaving is permitted to nonowners, while explicit transfer is mandatory.
const MATRIX: &[(&str, [bool; 5])] = &[
    ("spaces.list", [true, true, true, true, true]),
    ("spaces.create", [true, true, true, true, true]),
    ("spaces.members", [false, true, true, true, true]),
    ("spaces.set_role", [false, false, false, true, true]),
    ("spaces.remove_member", [false, false, false, true, true]),
    ("spaces.leave", [false, true, true, true, false]),
    ("spaces.transfer", [false, false, false, false, true]),
    ("spaces.delete", [false, false, false, false, true]),
];
#[tokio::test]
async fn stored_operations_have_complete_role_space_and_authentication_matrix() {
    for (op, allowed) in MATRIX {
        for (index, role) in [
            None,
            Some("reader"),
            Some("editor"),
            Some("manager"),
            Some("owner"),
        ]
        .into_iter()
        .enumerate()
        {
            let f = Fixture::new(role).await;
            for token in [None, Some("bad-token")] {
                assert_eq!(
                    f.call(token, "auth", op, f.input(op, &f.shared)).await.0,
                    StatusCode::UNAUTHORIZED
                );
            }
            if !matches!(*op, "spaces.list" | "spaces.create") {
                let unknown = f
                    .call(Some(&f.token), "unknown", op, f.input(op, "absent"))
                    .await;
                let hidden = f
                    .call(Some(&f.token), "unknown", op, f.input(op, &f.personal))
                    .await;
                assert_eq!(unknown, hidden, "space existence leak {op}");
                assert_eq!(unknown.0, StatusCode::NOT_FOUND);
            }
            let response = f.call(Some(&f.token), op, op, f.input(op, &f.shared)).await;
            assert_eq!(
                response.0,
                if allowed[index] {
                    StatusCode::OK
                } else if role.is_none() && !matches!(*op, "spaces.list" | "spaces.create") {
                    StatusCode::NOT_FOUND
                } else {
                    StatusCode::FORBIDDEN
                },
                "{op}, {role:?}: {}",
                response.1
            );
        }
    }
}
#[tokio::test]
async fn sqlite_revocation_takes_effect_on_next_request_and_note_receipt_replay() {
    let f = Fixture::new(Some("editor")).await;
    let input = json!({"space_id":f.shared,"title":"private","markdown":"body"});
    let first = f
        .call(Some(&f.token), "receipt", "notes.create", input.clone())
        .await;
    assert_eq!(first.0, StatusCode::OK);
    assert_eq!(first.1["data"]["note"]["space_id"], f.shared);
    let removed = f
        .call(
            Some(&f.owner_token),
            "remove",
            "spaces.remove_member",
            json!({"space_id":f.shared,"account_id":f.actor}),
        )
        .await;
    assert_eq!(removed.0, StatusCode::OK);
    let replay = f
        .call(Some(&f.token), "receipt", "notes.create", input)
        .await;
    assert_eq!(replay.0, StatusCode::NOT_FOUND);
    assert_eq!(
        f.call(
            Some(&f.token),
            "next",
            "notes.list",
            json!({"space_id":f.shared})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}
#[tokio::test]
async fn server_admin_has_no_space_content_access() {
    let f = Fixture::new(None).await;
    let path = f.root.join("system/auth.db");
    let actor = f.actor.clone();
    tokio::task::spawn_blocking(move || {
        let c = rusqlite::Connection::open(path).unwrap();
        c.execute("UPDATE users SET role='admin' WHERE id=?1", [actor])
            .unwrap();
    })
    .await
    .unwrap();
    for op in ["notes.list", "plugins.list", "spaces.members"] {
        assert_eq!(
            f.call(Some(&f.token), "admin", op, json!({"space_id":f.personal}))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
}
#[tokio::test]
async fn discovery_and_alias_inputs_keep_stable_independent_uuid_identities() {
    let f = Fixture::new(Some("reader")).await;
    assert_ne!(f.actor, f.shared);
    assert_ne!(f.owner, f.personal);
    let result = f
        .call(Some(&f.owner_token), "list", "spaces.list", json!({}))
        .await;
    assert_eq!(result.0, StatusCode::OK);
    assert!(
        result.1["data"]["spaces"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| uuid::Uuid::parse_str(s["id"].as_str().unwrap())
                .unwrap()
                .get_version_num()
                == 7)
    );
    // A space's display name is not an address.
    let name = f
        .call(
            Some(&f.token),
            "same",
            "spaces.members",
            json!({"space_id":"shared"}),
        )
        .await;
    let uuid = f
        .call(
            Some(&f.token),
            "same",
            "spaces.members",
            json!({"space_id":f.shared}),
        )
        .await;
    assert_eq!(name.0, StatusCode::NOT_FOUND);
    assert_eq!(uuid.0, StatusCode::OK);
    let state = f.state.clone();
    let owner = f.owner.clone();
    let described = tokio::task::spawn_blocking(move || {
        state.tokens.describe(&calternal_server::auth::Identity {
            id: calternal_types::AccountId::parse(&owner).unwrap(),
        })
    })
    .await
    .unwrap();
    assert!(
        described
            .spaces
            .iter()
            .all(|s| s.role == calternal_core::SpaceRole::Owner)
    );
}

#[tokio::test]
async fn every_space_operation_reads_its_current_capability_rows() {
    for (op, capability) in [
        ("spaces.members", "space.read"),
        ("spaces.leave", "space.read"),
        ("spaces.set_role", "members.manage"),
        ("spaces.remove_member", "members.manage"),
        ("spaces.transfer", "space.transfer"),
        ("spaces.delete", "space.delete"),
    ] {
        let f = Fixture::new(Some(if op == "spaces.leave" {
            "reader"
        } else {
            "owner"
        }))
        .await;
        let before = f
            .call(Some(&f.token), "baseline", op, f.input(op, &f.shared))
            .await;
        assert_eq!(before.0, StatusCode::OK, "{op}: {}", before.1);
        // Use a fresh fixture since successful leave/delete/transfer changes scope.
        let f = Fixture::new(Some(if op == "spaces.leave" {
            "reader"
        } else {
            "owner"
        }))
        .await;
        let path = f.root.join("system/auth.db");
        let role = if op == "spaces.leave" {
            "reader"
        } else {
            "owner"
        };
        tokio::task::spawn_blocking(move || {
            let c = rusqlite::Connection::open(path).unwrap();
            c.execute(
                "DELETE FROM space_role_capabilities WHERE role=?1 AND capability=?2",
                [role, capability],
            )
            .unwrap();
        })
        .await
        .unwrap();
        assert_eq!(
            f.call(Some(&f.token), "denied", op, f.input(op, &f.shared))
                .await
                .0,
            StatusCode::FORBIDDEN,
            "{op}"
        );
    }
}

// Source engine/src/tests.rs:381: identical retry keys belonging to different
// accounts never replay each other's result; resource reads stay space-bound.
#[tokio::test]
async fn accounts_never_replay_each_others_results_or_read_another_space() {
    let f = Fixture::new(Some("editor")).await;
    let a = f
        .call(
            Some(&f.owner_token),
            "same",
            "notes.create",
            json!({"space_id":f.personal,"title":"owner secret","markdown":"secret"}),
        )
        .await;
    let b = f
        .call(
            Some(&f.token),
            "same",
            "notes.create",
            json!({"space_id":f.shared,"title":"guest note","markdown":"other"}),
        )
        .await;
    assert_eq!(a.0, StatusCode::OK);
    assert_eq!(b.0, StatusCode::OK);
    assert_ne!(a.1["data"]["note"]["id"], b.1["data"]["note"]["id"]);
    let list = f
        .call(
            Some(&f.token),
            "list",
            "notes.list",
            json!({"space_id":f.shared}),
        )
        .await;
    assert!(!list.1.to_string().contains("owner secret"));
    let cross = f
        .call(
            Some(&f.token),
            "cross",
            "notes.get",
            json!({"space_id":f.shared,"id":a.1["data"]["note"]["id"]}),
        )
        .await;
    let absent = f
        .call(
            Some(&f.token),
            "cross",
            "notes.get",
            json!({"space_id":f.shared,"id":uuid::Uuid::new_v4()}),
        )
        .await;
    assert_eq!(cross, absent);
    assert_eq!(cross.0, StatusCode::NOT_FOUND);
}

/// Request IDs are scoped to (account, space); directory changes (`spaces.*`) keep
/// their own receipts, so one ID used in both places gives two independent results.
#[tokio::test]
async fn directory_and_content_request_ids_are_independent() {
    let f = Fixture::new(Some("editor")).await;
    let note = json!({"space_id":f.shared,"title":"note","markdown":"body"});
    for (op, input) in [
        ("notes.create", note.clone()),
        ("spaces.create", json!({"name":"space"})),
    ] {
        let first = f.call(Some(&f.token), "same-id", op, input.clone()).await;
        assert_eq!(first.0, StatusCode::OK, "{op}");
        assert_eq!(f.call(Some(&f.token), "same-id", op, input).await, first, "{op} replays");
    }
}
