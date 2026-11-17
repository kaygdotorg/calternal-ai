//! Authorization conformance over the real HTTP router, mutable memberships,
//! retry receipts, cross-space IDs, and asynchronous worker delivery.

mod support;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use calternal_core::{Principal, SpaceGrant, SpaceRole, operation_catalog};
use calternal_server::api::{AppState, Shared, allowed_hosts, dispatch, router};
use calternal_server::auth::{Directory, Identity};
use calternal_server::authz::POLICIES;
use calternal_server::plugins::{Registry, WorkerPool, resolve_node};
use calternal_server::store::Store;
use calternal_types::{AccountId, Capability, CapabilitySet, SpaceId};
use serde_json::{Value, json};
use tower::ServiceExt;

// Independent operation expectations, not derived from the host policy table.
const MATRIX: &[(&str, Capability, [bool; 4])] = &[
    (
        "notes.list",
        Capability::SpaceRead,
        [true, true, true, true],
    ),
    ("notes.get", Capability::SpaceRead, [true, true, true, true]),
    (
        "notes.history",
        Capability::SpaceRead,
        [true, true, true, true],
    ),
    (
        "notes.create",
        Capability::ContentWrite,
        [false, true, true, true],
    ),
    (
        "notes.update",
        Capability::ContentWrite,
        [false, true, true, true],
    ),
    (
        "plugins.list",
        Capability::SpaceRead,
        [true, true, true, true],
    ),
    (
        "plugins.enable",
        Capability::PluginsManage,
        [false, false, true, true],
    ),
    (
        "plugins.disable",
        Capability::PluginsManage,
        [false, false, true, true],
    ),
    (
        "plugins.preview",
        Capability::SpaceRead,
        [true, true, true, true],
    ),
    (
        "notes-insights.summarize",
        Capability::SpaceRead,
        [true, true, true, true],
    ),
];
const TOKEN: &str = "matrix-token";

/// Reader, Editor, Manager and Owner, as auth.db seeds them (schema.sql); written
/// out here as independent expectations.
fn roles() -> [CapabilitySet; 4] {
    use Capability::*;
    let sets: [&[Capability]; 4] = [
        &[SpaceRead],
        &[SpaceRead, ContentWrite, PluginsUse],
        &[
            SpaceRead,
            ContentWrite,
            PluginsUse,
            PluginsManage,
            MembersInvite,
            MembersManage,
            SpaceSettings,
            SecretsManage,
        ],
        &[
            SpaceRead,
            ContentWrite,
            PluginsUse,
            PluginsManage,
            MembersInvite,
            MembersManage,
            SpaceSettings,
            SecretsManage,
            ManagersManage,
            SpaceTransfer,
            SpaceDelete,
        ],
    ];
    sets.map(CapabilitySet::new)
}

struct MutableDirectory {
    identity: Identity,
    space: SpaceId,
    grants: Mutex<Option<CapabilitySet>>,
    session_valid: AtomicBool,
}

impl MutableDirectory {
    fn new(role: u8) -> Self {
        Self {
            identity: Identity {
                id: AccountId::new(),
            },
            space: SpaceId::new(),
            grants: Mutex::new(Self::role_set(role)),
            session_valid: AtomicBool::new(true),
        }
    }
    fn role_set(role: u8) -> Option<CapabilitySet> {
        role.checked_sub(1)
            .and_then(|index| roles().get(index as usize).copied())
    }

    fn set_role(&self, role: u8) {
        *self.grants.lock().unwrap() = Self::role_set(role);
    }

    fn set_capabilities(&self, grants: CapabilitySet) {
        *self.grants.lock().unwrap() = Some(grants);
    }
}

impl Directory for MutableDirectory {
    fn authenticate(&self, token: &[u8]) -> Option<Identity> {
        (token == TOKEN.as_bytes() && self.session_valid.load(Ordering::SeqCst))
            .then_some(self.identity)
    }

    fn capabilities(&self, account: AccountId, space: SpaceId) -> Option<CapabilitySet> {
        if account != self.identity.id || space != self.space {
            return None;
        }
        *self.grants.lock().unwrap()
    }

    fn describe(&self, principal: &Identity) -> Principal {
        Principal {
            id: principal.id.to_string(),
            spaces: self
                .capabilities(principal.id, self.space)
                .into_iter()
                .map(|_| SpaceGrant {
                    id: self.space.to_string(),
                    role: SpaceRole::Reader,
                })
                .collect(),
        }
    }
}

struct Fixture {
    state: Shared<MutableDirectory>,
    note: Value,
    root: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl Fixture {
    async fn new(role: u8, block: bool) -> Self {
        let root = std::env::temp_dir().join(format!("calternal-authz-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        // Optional deterministic barrier is reached only after the brokered read.
        let worker = support::worker(
            &root,
            support::READ_NOTE,
            if block {
                support::COMPLETE_AFTER_GO
            } else {
                support::COMPLETE
            },
        );
        let plugin = support::plugin(support::insights_manifest(), Some(&worker), true);
        let store = Store::open(
            &support::roots(&root.join("data")),
            uuid::Uuid::from_u128(1),
            true,
            calternal_content::DEFAULT_RECEIPT_RETENTION,
        )
        .unwrap();
        let tokens = MutableDirectory::new(role);
        let note = serde_json::to_value(
            store
                .create(
                    tokens.identity.id,
                    "seed",
                    "notes.create",
                    "seed",
                    tokens.space,
                    "Fixture",
                    "fixture\n",
                )
                .unwrap(),
        )
        .unwrap();
        let state = Arc::new(AppState {
            inputs: store.account_inputs(),
            store,
            directory: tokens,
            registry: Registry::from_plugins(vec![plugin]),
            workers: WorkerPool::new(Some(
                resolve_node(None)
                    .await
                    .expect("Node >= 22.13 for worker tests"),
            )),
            jobs: calternal_server::plugins::Jobs::disabled(),
            allowed_hosts: allowed_hosts(0),
        });
        Self { state, note, root }
    }

    /// The one space the mutable directory grants, as wire text.
    fn space(&self) -> String {
        self.state.directory.space.to_string()
    }

    fn input(&self, op: &str, space: &str) -> Value {
        match op {
            "notes.list" | "plugins.list" => json!({"space_id":space}),
            "notes.create" => json!({"space_id":space,"title":"Created","markdown":"new\n"}),
            "notes.update" => {
                json!({"space_id":space,"id":self.note["id"],"base_revision":self.note["revision"],"markdown":"updated\n"})
            }
            "notes.get" | "notes.history" | "notes-insights.summarize" => {
                json!({"space_id":space,"id":self.note["id"]})
            }
            _ => json!({"space_id":space,"plugin_id":"notes-insights"}),
        }
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
        let response = router(self.state.clone(), None)
            .oneshot(
                req.body(Body::from(
                    json!({"request_id":rid,"operation":op,"input":input}).to_string(),
                ))
                .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }
}

fn item_operation(op: &str) -> bool {
    matches!(
        op,
        "notes.get" | "notes.history" | "notes.update" | "notes-insights.summarize"
    )
}

fn denial(response: (StatusCode, Value), status: StatusCode, message: &str) {
    assert_eq!(response.0, status, "{}", response.1);
    assert_eq!(response.1["ok"], false);
    assert_eq!(
        response.1["error"],
        json!({
            "code": if status == StatusCode::UNAUTHORIZED { "unauthorized" } else if status == StatusCode::NOT_FOUND {"not_found"} else { "forbidden" },
            "message": message,
        })
    );
}

#[tokio::test]
async fn every_registered_operation_has_an_independent_matrix_row() {
    let fixture = Fixture::new(3, false).await;
    let rows: BTreeSet<_> = MATRIX.iter().map(|(op, _, _)| *op).collect();
    let mut registered: BTreeSet<_> = operation_catalog().into_iter().map(|op| op.id).collect();
    for plugin in fixture.state.registry.plugins() {
        registered.extend(plugin.manifest.operations.iter().map(|op| op.id.clone()));
    }
    assert_eq!(rows.len(), MATRIX.len(), "duplicate matrix row");
    // The real stored-directory matrix lives in spaces_matrix.rs, where every
    // directory handler and its target invariants are exercised against SQLite.
    let stored_rows: BTreeSet<_> = [
        // Private account authority and target isolation: account_input.rs.
        "drafts.save",
        "drafts.list",
        "drafts.get",
        "drafts.delete",
        "pending.queue",
        "pending.edit",
        "pending.list",
        "pending.cancel",
        "pending.confirm",
        "pending.submit",
        "resolve", // dedicated address matrix in resolver.rs
        "spaces.list",
        "spaces.create",
        "spaces.members",
        "spaces.set_role",
        "spaces.remove_member",
        "spaces.leave",
        "spaces.transfer",
        "spaces.delete",
        // Job rows need a durable job; their role matrix is in plugin_host.rs.
        "jobs.get",
        "jobs.cancel",
        // Account-or-space owned; dedicated stored-directory matrix in secrets.rs.
        "secrets.request",
        "secrets.list",
        "secrets.revoke",
    ]
    .into_iter()
    .collect();
    let rows: BTreeSet<_> = rows.union(&stored_rows).copied().collect();
    assert_eq!(
        rows.iter()
            .copied()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        registered
    );
    // Host rows plus manifest-derived plugin rows cover every operation exactly once.
    let plugin_rows: Vec<&str> = fixture
        .state
        .registry
        .plugin_policies()
        .map(|p| p.operation.as_ref())
        .collect();
    assert!(
        POLICIES
            .iter()
            .all(|p| !plugin_rows.contains(&p.operation.as_ref())),
        "a plugin row is hardcoded in the host table"
    );
    let all: BTreeSet<String> = POLICIES
        .iter()
        .map(|p| p.operation.to_string())
        .chain(calternal_core::account_input_operations().map(str::to_owned))
        .chain(plugin_rows.iter().map(|p| (*p).to_owned()))
        .collect();
    assert_eq!(
        rows.iter()
            .map(|s| (*s).to_owned())
            .collect::<BTreeSet<_>>(),
        all
    );
    assert_eq!(
        POLICIES.len() + calternal_core::account_input_operations().count() + plugin_rows.len(),
        MATRIX.len() + stored_rows.len(),
        "duplicate policy"
    );
    // Include the repository registry even though execution uses a controlled worker.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    for plugin in Registry::load(&root).plugins() {
        for op in &plugin.manifest.operations {
            assert!(
                rows.contains(op.id.as_str()),
                "missing matrix row: {}",
                op.id
            );
        }
    }
}

#[tokio::test]
async fn every_operation_role_unknown_space_and_token_matrix() {
    for role in 0..=4 {
        let fixture = Fixture::new(role, false).await;
        // Run disable last so the summarize and preview success paths see an enabled plugin.
        let mut operations = MATRIX.to_vec();
        operations.sort_by_key(|(op, _, _)| *op == "plugins.disable");
        for (op, _, allowed) in operations {
            let response = fixture
                .call(Some(TOKEN), op, op, fixture.input(op, &fixture.space()))
                .await;
            if role != 0 && allowed[(role - 1) as usize] {
                assert_eq!(
                    response.0,
                    StatusCode::OK,
                    "role {role}, {op}: {}",
                    response.1
                );
                assert_eq!(response.1["ok"], true);
            } else {
                denial(
                    response,
                    if role == 0 {
                        StatusCode::NOT_FOUND
                    } else {
                        StatusCode::FORBIDDEN
                    },
                    if role == 0 && item_operation(op) {
                        "item not found"
                    } else if role == 0 {
                        "space not found"
                    } else {
                        "insufficient role for this space"
                    },
                );
            }
            denial(
                fixture
                    .call(Some(TOKEN), "unknown", op, fixture.input(op, "unknown"))
                    .await,
                StatusCode::NOT_FOUND,
                if item_operation(op) {
                    "item not found"
                } else {
                    "space not found"
                },
            );
            for token in [None, Some("invalid-token")] {
                denial(
                    fixture
                        .call(token, "token", op, fixture.input(op, &fixture.space()))
                        .await,
                    StatusCode::UNAUTHORIZED,
                    "missing or invalid session",
                );
            }
        }
    }
}

#[tokio::test]
async fn no_grant_hides_existing_and_absent_resources() {
    let fixture = Fixture::new(0, false).await;
    for (op, _, _) in MATRIX {
        let existing = fixture
            .call(Some(TOKEN), "same", op, fixture.input(op, &fixture.space()))
            .await;
        let mut absent = fixture.input(op, &fixture.space());
        if absent.get("id").is_some() {
            absent["id"] = json!("00000000-0000-4000-8000-000000000000");
        }
        if absent.get("plugin_id").is_some() {
            absent["plugin_id"] = json!("absent-plugin");
        }
        let missing = fixture.call(Some(TOKEN), "same", op, absent).await;
        let unknown = fixture
            .call(Some(TOKEN), "same", op, fixture.input(op, "unknown"))
            .await;
        assert_eq!(existing, missing, "resource existence leak: {op}");
        assert_eq!(existing, unknown, "space existence leak: {op}");
        denial(
            existing,
            StatusCode::NOT_FOUND,
            if item_operation(op) {
                "item not found"
            } else {
                "space not found"
            },
        );
    }
}

#[tokio::test]
async fn role_changes_and_revocation_apply_to_next_request_and_receipt_replay() {
    let fixture = Fixture::new(3, false).await;
    let input = fixture.input("notes.create", &fixture.space());
    let first = fixture
        .call(Some(TOKEN), "retry", "notes.create", input.clone())
        .await;
    assert_eq!(first.0, StatusCode::OK);
    fixture.state.directory.set_role(1);
    denial(
        fixture
            .call(Some(TOKEN), "retry", "notes.create", input.clone())
            .await,
        StatusCode::FORBIDDEN,
        "insufficient role for this space",
    );
    fixture.state.directory.set_role(2);
    assert_eq!(
        first,
        fixture
            .call(Some(TOKEN), "retry", "notes.create", input)
            .await
    );
    fixture.state.directory.set_role(0);
    denial(
        fixture
            .call(
                Some(TOKEN),
                "read",
                "notes.get",
                fixture.input("notes.get", &fixture.space()),
            )
            .await,
        StatusCode::NOT_FOUND,
        "item not found",
    );
    fixture.state.directory.set_role(3);
    fixture
        .state
        .directory
        .session_valid
        .store(false, Ordering::SeqCst);
    denial(
        fixture
            .call(
                Some(TOKEN),
                "read",
                "notes.get",
                fixture.input("notes.get", &fixture.space()),
            )
            .await,
        StatusCode::UNAUTHORIZED,
        "missing or invalid session",
    );
}

#[tokio::test]
async fn cross_space_note_ids_are_bound_to_the_authorized_space() {
    let fixture = Fixture::new(3, false).await;
    let other = fixture
        .state
        .store
        .create(
            AccountId::new(),
            "other",
            "notes.create",
            "other",
            SpaceId::new(),
            "Other",
            "secret\n",
        )
        .unwrap();
    for op in [
        "notes.get",
        "notes.history",
        "notes.update",
        "notes-insights.summarize",
    ] {
        let mut input = fixture.input(op, &fixture.space());
        input["id"] = json!(other.id);
        let cross = fixture.call(Some(TOKEN), "cross", op, input.clone()).await;
        input["id"] = json!("00000000-0000-4000-8000-000000000000");
        let absent = fixture.call(Some(TOKEN), "cross", op, input).await;
        assert_eq!(cross, absent, "cross-space leak: {op}");
        assert_eq!(cross.0, StatusCode::NOT_FOUND, "{}", cross.1);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn membership_revocation_during_worker_discards_delivery() {
    let fixture = Fixture::new(1, true).await;
    let state = fixture.state.clone();
    let identity = state.directory.authenticate(TOKEN.as_bytes()).unwrap();
    let input = serde_json::from_value(fixture.input("notes-insights.summarize", &fixture.space()))
        .unwrap();
    let call = tokio::spawn(dispatch(
        state,
        identity,
        calternal_server::plugins::Session::Static,
        "in-flight",
        "notes-insights.summarize",
        input,
    ));
    // Admission to a worker happens only after authorization and generation
    // capture. The worker then waits on a read-only file barrier, matching
    // runtime filesystem limits.
    let start = std::time::Instant::now();
    while fixture.state.workers.in_flight() == 0 {
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "worker did not admit the invocation"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    fixture.state.directory.set_role(0);
    std::fs::write(fixture.root.join("go"), "").unwrap();
    let error = call
        .await
        .unwrap()
        .expect_err("revoked member must not receive worker data");
    assert_eq!(error.code, calternal_core::ErrorCode::NotFound);
    assert_eq!(error.message, "item not found");
}

#[tokio::test]
async fn directory_role_decides_and_plugin_retries_recheck_role() {
    let fixture = Fixture::new(1, false).await;
    let input = serde_json::from_value(fixture.input("plugins.disable", &fixture.space())).unwrap();
    let error = dispatch(
        fixture.state.clone(),
        fixture.state.directory.identity,
        calternal_server::plugins::Session::Static,
        "forged",
        "plugins.disable",
        input,
    )
    .await
    .expect_err("a reader cannot disable plugins");
    assert_eq!(error.code, calternal_core::ErrorCode::Forbidden);
    assert_eq!(error.message, "insufficient role for this space");

    fixture.state.directory.set_role(3);
    let input = fixture.input("plugins.disable", &fixture.space());
    let first = fixture
        .call(
            Some(TOKEN),
            "plugin-retry",
            "plugins.disable",
            input.clone(),
        )
        .await;
    assert_eq!(first.0, StatusCode::OK);
    fixture.state.directory.set_role(2);
    denial(
        fixture
            .call(
                Some(TOKEN),
                "plugin-retry",
                "plugins.disable",
                input.clone(),
            )
            .await,
        StatusCode::FORBIDDEN,
        "insufficient role for this space",
    );
    fixture.state.directory.set_role(3);
    assert_eq!(
        first,
        fixture
            .call(Some(TOKEN), "plugin-retry", "plugins.disable", input)
            .await
    );
}

#[tokio::test]
async fn no_grant_hides_summarize_binding_registration() {
    let mut fixture = Fixture::new(0, false).await;
    let input = fixture.input("notes-insights.summarize", &fixture.space());
    let registered = fixture
        .call(
            Some(TOKEN),
            "same",
            "notes-insights.summarize",
            input.clone(),
        )
        .await;
    Arc::get_mut(&mut fixture.state).unwrap().registry = Registry::empty();
    let absent = fixture
        .call(
            Some(TOKEN),
            "same",
            "notes-insights.summarize",
            input.clone(),
        )
        .await;
    assert_eq!(registered, absent);
    denial(absent, StatusCode::NOT_FOUND, "item not found");
    fixture.state.directory.set_role(1);
    assert_eq!(
        fixture
            .call(Some(TOKEN), "authorized", "notes-insights.summarize", input)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

/// Each operation accepts its capability alone and rejects every other capability.
#[tokio::test]
async fn every_operation_checks_only_its_required_capability() {
    for &(op, required, _) in MATRIX {
        for allowed in [true, false] {
            let fixture = Fixture::new(0, false).await;
            let grants = if allowed {
                CapabilitySet::new(&[required])
            } else {
                CapabilitySet::new(&Capability::ALL).without(required)
            };
            fixture.state.directory.set_capabilities(grants);
            let response = fixture
                .call(
                    Some(TOKEN),
                    "capability",
                    op,
                    fixture.input(op, &fixture.space()),
                )
                .await;
            if allowed {
                assert_eq!(response.0, StatusCode::OK, "{op}: {}", response.1);
            } else {
                denial(
                    response,
                    if item_operation(op) && required == Capability::SpaceRead {
                        StatusCode::NOT_FOUND
                    } else {
                        StatusCode::FORBIDDEN
                    },
                    if item_operation(op) && required == Capability::SpaceRead {
                        "item not found"
                    } else {
                        "insufficient role for this space"
                    },
                );
            }
        }
    }
}

#[test]
fn built_in_roles_cover_exact_documented_capabilities() {
    let names: BTreeSet<_> = Capability::ALL
        .iter()
        .map(|capability| capability.name())
        .collect();
    assert_eq!(names.len(), Capability::ALL.len());
    for capability in Capability::ALL {
        assert!(
            roles().iter().any(|role| role.contains(capability))
                || matches!(
                    capability,
                    Capability::SpacesList | Capability::SpacesCreate
                ),
            "unheld capability: {}",
            capability.name()
        );
        assert_eq!(Capability::from_name(capability.name()), Some(capability));
    }
    assert_eq!(Capability::from_name("unknown.permission"), None);
    assert_eq!(CapabilitySet::default(), CapabilitySet::empty());
    // Plugin rows come from the shipped manifests through the same lookup.
    let registry = Registry::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins"));
    for (op, capability, _) in MATRIX {
        assert_eq!(registry.policy(op).unwrap().required, *capability);
    }
}
