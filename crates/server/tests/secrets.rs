//! Secret operations: stored-directory role matrix, account isolation, the
//! fresh-session provide endpoint and leak scans of responses and data files.
mod account_common;

use account_common::{App, app, first_owner, first_owner_setup_session, post, send};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use calternal_server::{
    api::{AppState, Shared, allowed_hosts, router},
    auth::{Directory, StoredDirectory},
    plugins::Registry,
    store::Store,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tower::ServiceExt;

const CANARY: &str = "sk-canary-server-5d2e81-never-stored-in-clear";

struct Fixture {
    state: Shared,
    root: PathBuf,
    guest: String,
    owner: String,
    shared: String,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // `root` is appdata; its parent holds all three roots.
        let _ = std::fs::remove_dir_all(self.root.parent().unwrap());
    }
}
impl Fixture {
    /// Development directory; the guest's role in `shared` is set to `role`
    /// (`None` removes the membership).
    async fn new(role: Option<&'static str>) -> Self {
        tokio::task::spawn_blocking(move || {
            let base =
                std::env::temp_dir().join(format!("calternal-secrets-{}", uuid::Uuid::new_v4()));
            let roots = calternal_roots::Roots {
                appdata: base.join("appdata"),
                userdata_ssd: base.join("userdata-ssd"),
                userdata_hdd: base.join("userdata-hdd"),
            };
            let store = Store::open(
                &roots,
                uuid::Uuid::from_u128(1),
                true,
                calternal_content::DEFAULT_RECEIPT_RETENTION,
            )
            .unwrap();
            // Every later path is in appdata; `base` holds all three roots.
            let root = roots.appdata.clone();
            let tokens = StoredDirectory::load_or_create(&root).unwrap();
            let guest = std::fs::read_to_string(root.join("dev-tokens/guest.token")).unwrap();
            let owner = std::fs::read_to_string(root.join("dev-tokens/owner.token")).unwrap();
            let guest_id = tokens
                .authenticate(guest.as_bytes())
                .unwrap()
                .id
                .to_string();
            let owner_id = tokens
                .authenticate(owner.as_bytes())
                .unwrap()
                .id
                .to_string();
            let shared = tokens
                .spaces
                .list(calternal_types::AccountId::parse(&owner_id).unwrap())
                .unwrap()
                .into_iter()
                .find(|s| s.name == "shared")
                .unwrap()
                .id
                .to_string();
            let c = rusqlite::Connection::open(root.join("system/auth.db")).unwrap();
            match role {
                None => c.execute(
                    "DELETE FROM space_memberships WHERE account_id=?1 AND space_id=?2",
                    [&guest_id, &shared],
                ),
                Some("owner") => c.execute(
                    "UPDATE spaces SET owner_account=?1 WHERE id=?2",
                    [&guest_id, &shared],
                ),
                Some(role) => c.execute(
                    "UPDATE space_memberships SET role=?1 WHERE account_id=?2 AND space_id=?3",
                    [role, &guest_id, &shared],
                ),
            }
            .unwrap();
            let state = Arc::new(AppState {
                blocking: calternal_server::blocking::RequestWorkers::new().unwrap(),
                inputs: store.account_inputs(),
                store,
                directory: tokens,
                registry: Registry::empty(),
                workers: calternal_server::plugins::WorkerPool::new(None),
                jobs: calternal_server::plugins::Jobs::disabled(),
                allowed_hosts: allowed_hosts(0),
            });
            Self {
                state,
                root,
                guest,
                owner,
                shared,
            }
        })
        .await
        .unwrap()
    }
    async fn call(&self, token: &str, op: &str, input: Value) -> (StatusCode, Value) {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/operations")
            .header("Host", "127.0.0.1:0")
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::from(
                json!({"request_id": uuid::Uuid::now_v7().to_string(), "operation": op, "input": input}).to_string(),
            ))
            .unwrap();
        let res = router(self.state.clone(), None).oneshot(req).await.unwrap();
        let status = res.status();
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }
    async fn request(&self, token: &str, space: &str, name: &str) -> String {
        let (status, body) = self
            .call(
                token,
                "secrets.request",
                json!({"space_id": space, "name": name, "label": "L", "reason": "R"}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["data"]["request_id"].as_str().unwrap().to_owned()
    }
}

// Guest role in `shared`: none, reader, editor, manager, owner.
const ROLES: [Option<&str>; 5] = [
    None,
    Some("reader"),
    Some("editor"),
    Some("manager"),
    Some("owner"),
];
// request needs plugins.use; management requires secrets.manage.
const MATRIX: &[(&str, [bool; 5])] = &[
    ("secrets.request", [false, false, true, true, true]),
    ("secrets.list", [false, false, false, true, true]),
    ("secrets.revoke", [false, false, false, true, true]),
];

#[tokio::test]
async fn space_secrets_follow_the_capability_matrix() {
    for (index, role) in ROLES.into_iter().enumerate() {
        let f = Fixture::new(role).await;
        for (op, allowed) in MATRIX {
            let pending = f.request(&f.owner, &f.shared, "target").await;
            let input = match *op {
                "secrets.request" => {
                    json!({"space_id": f.shared, "name": "x", "label": "L", "reason": "R"})
                }
                "secrets.list" => json!({"space_id": f.shared}),
                _ => json!({"space_id": f.shared, "request_id": pending}),
            };
            let (status, body) = f.call(&f.guest, op, input).await;
            if allowed[index] {
                assert_eq!(status, StatusCode::OK, "{role:?} {op}: {body}");
            } else if role.is_none() {
                assert_eq!(
                    (status, body["error"]["message"].as_str()),
                    (StatusCode::NOT_FOUND, Some("space not found"))
                );
            } else {
                assert_eq!(status, StatusCode::FORBIDDEN, "{role:?} {op}: {body}");
            }
            // Same answer for a space that does not exist.
            let (status, _) = f
                .call(
                    &f.guest,
                    op,
                    json!({"space_id": uuid::Uuid::now_v7().to_string()}),
                )
                .await;
            assert!(
                matches!(status, StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST),
                "{op}"
            );
        }
    }
}

#[tokio::test]
async fn personal_secrets_are_isolated_and_provide_is_not_an_operation() {
    let f = Fixture::new(Some("reader")).await;
    let id = f.request(&f.guest, "", "github").await;
    let (_, mine) = f.call(&f.guest, "secrets.list", json!({})).await;
    assert_eq!(mine["data"]["requests"][0]["id"], json!(id));
    let (_, theirs) = f.call(&f.owner, "secrets.list", json!({})).await;
    assert_eq!(theirs["data"], json!({"secrets": [], "requests": []}));
    // The owner cannot withdraw the guest's personal request.
    let (status, _) = f
        .call(&f.owner, "secrets.revoke", json!({"request_id": id}))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = f
        .call(&f.guest, "secrets.revoke", json!({"request_id": id}))
        .await;
    assert_eq!(status, StatusCode::OK);
    // No operation can carry a value in.
    let (status, body) = f
        .call(
            &f.guest,
            "secrets.provide",
            json!({"request_id": id, "value": CANARY}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, _) = f
        .call(
            &f.guest,
            "secrets.revoke",
            json!({"name": "a", "request_id": id}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn development_bearer_tokens_cannot_provide() {
    let f = Fixture::new(Some("owner")).await;
    let id = f.request(&f.guest, "", "k").await;
    let req = Request::builder()
        .method("POST")
        .uri(calternal_server::secrets::PROVIDE_PATH)
        .header("Host", "127.0.0.1:0")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", f.guest))
        .body(Body::from(
            json!({"request_id": id, "value": CANARY}).to_string(),
        ))
        .unwrap();
    let res = router(f.state.clone(), None).oneshot(req).await.unwrap();
    // Without account sessions the route is not mounted at all.
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(scan(f.root.parent().unwrap()).is_empty());
}

async fn op(app: &App, cookie: &str, operation: &str, input: Value) -> Value {
    let r = post(
        app,
        "/api/v1/operations",
        Some(cookie),
        json!({"request_id": uuid::Uuid::now_v7().to_string(), "operation": operation, "input": input}),
    )
    .await;
    assert_eq!(r.status, 200, "{operation}: {}", r.body);
    r.body["data"].clone()
}

fn scan(dir: &Path) -> Vec<PathBuf> {
    let mut hits = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            hits.extend(scan(&path));
        } else if std::fs::read(&path)
            .unwrap()
            .windows(CANARY.len())
            .any(|w| w == CANARY.as_bytes())
        {
            hits.push(path);
        }
    }
    hits
}

#[tokio::test]
async fn fresh_session_provides_and_value_never_leaks() {
    let app = app();
    let (cookie, _device) = first_owner(&app, "Owner").await;
    let id = op(
        &app,
        &cookie,
        "secrets.request",
        json!({"name": "openai", "label": "OpenAI", "reason": "agent run"}),
    )
    .await["request_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let space = op(&app, &cookie, "spaces.create", json!({"name": "Team"})).await["space"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let space_request = op(
        &app,
        &cookie,
        "secrets.request",
        json!({"space_id": space, "name": "smtp", "label": "SMTP", "reason": "mail"}),
    )
    .await["request_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut responses = String::new();
    for body in [
        json!({"request_id": id, "value": CANARY}),
        json!({"space_id": space, "request_id": space_request, "value": CANARY}),
    ] {
        let r = post(
            &app,
            calternal_server::secrets::PROVIDE_PATH,
            Some(&cookie),
            body,
        )
        .await;
        assert_eq!(r.status, 200, "{}", r.body);
        assert_eq!(r.body["secret"]["version"], 1);
        responses.push_str(&r.body.to_string());
    }
    // Answered once; a second answer finds nothing.
    let again = post(
        &app,
        calternal_server::secrets::PROVIDE_PATH,
        Some(&cookie),
        json!({"request_id": id, "value": CANARY}),
    )
    .await;
    assert_eq!(again.status, 404);
    responses.push_str(&again.body.to_string());
    let personal = op(&app, &cookie, "secrets.list", json!({})).await;
    assert_eq!(personal["secrets"][0]["name"], "openai");
    assert_eq!(personal["requests"], json!([]));
    let team = op(&app, &cookie, "secrets.list", json!({"space_id": space})).await;
    assert_eq!(team["secrets"][0]["name"], "smtp");
    responses.push_str(&personal.to_string());
    responses.push_str(&team.to_string());
    assert!(!responses.contains(CANARY));
    // Databases, WAL files, journals, receipts and recovery records.
    assert_eq!(scan(app.dir.path()), Vec::<PathBuf>::new());
    op(&app, &cookie, "secrets.revoke", json!({"name": "openai"})).await;
    assert_eq!(
        op(&app, &cookie, "secrets.list", json!({})).await["secrets"],
        json!([])
    );
}

#[tokio::test]
async fn provide_needs_a_fresh_passkey_session() {
    let app = app();
    // A setup session has registered a passkey but never asserted one.
    let (cookie, _device) = first_owner_setup_session(&app, "Owner").await;
    let id = op(
        &app,
        &cookie,
        "secrets.request",
        json!({"name": "k", "label": "K", "reason": "R"}),
    )
    .await["request_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let body = json!({"request_id": id, "value": CANARY});
    let stale = post(
        &app,
        calternal_server::secrets::PROVIDE_PATH,
        Some(&cookie),
        body.clone(),
    )
    .await;
    assert_eq!(stale.status, 403, "{}", stale.body);
    let anonymous = post(
        &app,
        calternal_server::secrets::PROVIDE_PATH,
        None,
        body.clone(),
    )
    .await;
    assert_eq!(anonymous.status, 401);
    let forged = send(
        &app,
        "POST",
        calternal_server::secrets::PROVIDE_PATH,
        &[("authorization", "Bearer not-a-session")],
        Body::from(body.to_string()),
    )
    .await;
    assert_eq!(forged.status, 401);
    let foreign = send(
        &app,
        "POST",
        calternal_server::secrets::PROVIDE_PATH,
        &[
            ("cookie", cookie.as_str()),
            ("origin", "https://evil.example"),
        ],
        Body::from(body.to_string()),
    )
    .await;
    assert!(
        foreign.status == 401 || foreign.status == 403,
        "{}",
        foreign.status
    );
    // Still pending, nothing stored.
    let listed = op(&app, &cookie, "secrets.list", json!({})).await;
    assert_eq!(
        (
            listed["secrets"].as_array().unwrap().len(),
            listed["requests"].as_array().unwrap().len()
        ),
        (0, 1)
    );
    assert!(scan(app.dir.path()).is_empty());
}

#[tokio::test]
async fn protected_key_records_refuse_missing_keys_and_finish_erasure() {
    let f = Fixture::new(Some("manager")).await;
    let state = f.state.clone();
    let root = f.root.clone();
    let space = calternal_types::SpaceId::parse(&f.shared).unwrap();
    tokio::task::spawn_blocking(move || {
        use calternal_secrets::{LocalStore, Owner, SecretName, SecretStore, SecretValue};
        let keys = state.store.secrets();
        let owner = Owner::Space(space);
        let actor = calternal_types::Actor::account(calternal_types::AccountId::new());
        calternal_server::secrets::configure_keys(&keys, state.directory.spaces.clone(), &root)
            .unwrap();
        let h = keys
            .put(
                &owner,
                &SecretName::new("key").unwrap(),
                "Key",
                SecretValue::new(b"secret".to_vec()).unwrap(),
                &actor,
            )
            .unwrap();
        let path = root
            .join("spaces")
            .join(space.to_string())
            .join("secret.key");
        let original = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        let reopened = LocalStore::open(&root).unwrap();
        assert!(
            calternal_server::secrets::configure_keys(
                &reopened,
                state.directory.spaces.clone(),
                &root
            )
            .is_err()
        );
        assert!(!path.exists());
        std::fs::write(&path, original).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        // Simulate a crash after the erasing record is acknowledged but before unlink.
        let fingerprint = calternal_secrets::key_fingerprint(&path).unwrap();
        state
            .directory
            .spaces
            .record_secret_key("space", space.as_uuid(), fingerprint, true)
            .unwrap();
        let reopened = LocalStore::open(&root).unwrap();
        calternal_server::secrets::configure_keys(&reopened, state.directory.spaces.clone(), &root)
            .unwrap();
        assert!(!path.exists());
        assert!(reopened.handle(&owner, &h.name).is_err());
        assert!(
            reopened
                .put(
                    &owner,
                    &h.name,
                    "Key",
                    SecretValue::new(b"new".to_vec()).unwrap(),
                    &actor
                )
                .is_err()
        );
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn stale_space_writer_cannot_reopen_after_http_delete() {
    let fixture = Fixture::new(Some("reader")).await;
    let space = calternal_types::SpaceId::parse(&fixture.shared).unwrap();
    let id = fixture
        .state
        .directory
        .authenticate(fixture.owner.as_bytes())
        .unwrap()
        .id;
    let state = fixture.state.clone();
    let stale = tokio::task::spawn_blocking(move || state.store.content().live_journal(space))
        .await
        .unwrap()
        .unwrap();
    let (status, response) = fixture
        .call(&fixture.owner, "spaces.delete", json!({"space_id":space}))
        .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    let state = fixture.state.clone();
    let error = tokio::task::spawn_blocking(move || {
        state.store.create(
            id,
            "stale-owner-write",
            "notes.create",
            "stale",
            space,
            "Stale",
            "body",
        )
    })
    .await
    .unwrap()
    .unwrap_err();
    assert_eq!(error.code, calternal_types::ErrorCode::NotFound);
    assert!(
        stale
            .commit(calternal_store::Commit {
                events: vec![],
                idempotency: None,
                durable: true
            })
            .await
            .is_err()
    );
    assert!(!fixture.root.join("spaces").join(space.to_string()).exists());
}
