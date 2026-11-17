//! Thread-token lifecycle with the real per-space jobs database; no provider account.
use calternal_core::AgentMode;
use calternal_server::{
    plugins::{
        Jobs, Session,
        jobs::{Admission, RunEnd},
    },
    turns::access::Authority,
};
use serde_json::json;
use uuid::Uuid;

#[test]
fn tokens_rotate_expire_at_end_and_never_cross_space_or_thread() {
    let dir = tempfile::tempdir().unwrap();
    let jobs = Jobs::open(dir.path()).unwrap();
    let space = Uuid::now_v7().to_string();
    let account = Uuid::now_v7().to_string();
    let thread = Uuid::now_v7().to_string();
    let id = jobs
        .admit(Admission {
            space: space.clone(),
            plugin: "calternal-orchestrator".into(),
            account: account.clone(),
            session: Session::Static,
            operation: "agent.turn".into(),
            input: json!({"request":{"thread_id":thread}}),
            parent: None,
            generation: 0,
            request_key: "test".into(),
        })
        .unwrap();
    let run = jobs.claim(&space, &id).unwrap().unwrap();
    let mut authority = Authority {
        account,
        space: space.clone(),
        thread,
        job: id.clone(),
        mode: AgentMode::LeastAccess,
        source_app: "notes".into(),
        grants: vec![],
    };
    let old = jobs.agent_token(&authority, run.job.runs).unwrap();
    let a = jobs.agent_authenticate(&old).unwrap().unwrap();
    assert!(
        a.check(
            "notes.list",
            &serde_json::from_value(json!({"space_id":space})).unwrap()
        )
        .is_ok()
    );
    assert!(
        a.check(
            "notes.list",
            &serde_json::from_value(json!({"space_id":Uuid::now_v7()})).unwrap()
        )
        .is_err()
    );
    assert!(
        a.check(
            "agent.stop",
            &serde_json::from_value(json!({"job_id":Uuid::now_v7()})).unwrap()
        )
        .is_err()
    );
    assert!(
        a.check(
            "access.approve",
            &serde_json::from_value(json!({"job_id":id})).unwrap()
        )
        .is_err()
    );
    authority.mode = AgentMode::ApproveForMe;
    jobs.agent_scope_change(&authority).unwrap();
    assert!(jobs.agent_authenticate(&old).unwrap().is_none());
    let new = jobs.agent_token(&authority, run.job.runs).unwrap();
    assert_ne!(new, old);
    assert!(jobs.agent_authenticate(&new).unwrap().is_some());
    jobs.agent_interrupt(&space, &id).unwrap();
    jobs.agent_resume(&space, &id).unwrap();
    let next = jobs.claim(&space, &id).unwrap().unwrap();
    let current = jobs.agent_token(&authority, next.job.runs).unwrap();
    assert!(jobs.agent_authenticate(&new).unwrap().is_none());
    assert!(jobs.agent_token(&authority, run.job.runs).is_err());
    jobs.agent_revoke_run(&space, &id, run.job.runs).unwrap();
    jobs.finish(
        &space,
        &id,
        run.job.runs,
        RunEnd::Done(json!({"late":true})),
    )
    .unwrap();
    assert!(jobs.agent_authenticate(&current).unwrap().is_some());
    assert!(
        next.live.is_current(next.job.runs),
        "late finish stopped the successor stream"
    );
    jobs.finish(&space, &id, next.job.runs, RunEnd::Done(json!({})))
        .unwrap();
    assert!(jobs.agent_authenticate(&current).unwrap().is_none());
    let raw = std::fs::read(dir.path().join("spaces").join(space).join("jobs.db")).unwrap();
    assert!(
        !raw.windows(new.len()).any(|b| b == new.as_bytes()),
        "raw token was stored"
    );
}

#[tokio::test]
async fn thread_bearers_enforce_scope_before_handlers_and_cannot_approve() {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use calternal_server::{
        api::{self, AppState},
        auth::{Directory, TokenTable},
        machines::{MachineService, Settings},
        plugins::{Registry, WorkerPool},
        store::Store,
    };
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;
    let data = tempfile::tempdir().unwrap();
    let root = data.path().to_path_buf();
    let (tokens, who, principal) = tokio::task::spawn_blocking(move || {
        let tokens = TokenTable::load_or_create(&root).unwrap();
        let owner = std::fs::read(root.join("dev-tokens/owner.token")).unwrap();
        let who = tokens.authenticate(&owner).unwrap();
        let principal = tokens.describe(&who);
        (tokens, who, principal)
    })
    .await
    .unwrap();
    let space = principal.spaces[0].id.clone();
    let other = principal.spaces[1].id.clone();
    let state = Arc::new(AppState {
        store: Mutex::new(Store::open(data.path()).unwrap()),
        tokens,
        registry: Registry::load(
            &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins"),
        ),
        workers: WorkerPool::new(None),
        jobs: Jobs::open(data.path()).unwrap(),
        allowed_hosts: api::allowed_hosts(3210),
    });
    let thread = Uuid::now_v7().to_string();
    let id = state
        .jobs
        .admit(Admission {
            space: space.clone(),
            plugin: "calternal-orchestrator".into(),
            account: who.id.to_string(),
            session: Session::Static,
            operation: "agent.turn".into(),
            input: json!({"request":{"thread_id":thread}}),
            parent: None,
            generation: 1,
            request_key: "guard".into(),
        })
        .unwrap();
    let _run = state.jobs.claim(&space, &id).unwrap().unwrap();
    let mut scope = Authority {
        account: who.id.to_string(),
        space: space.clone(),
        thread,
        job: id.clone(),
        mode: AgentMode::LeastAccess,
        source_app: String::new(),
        grants: vec![],
    };
    let old = state.jobs.agent_token(&scope, _run.job.runs).unwrap();
    let app = api::router(state.clone(), None).layer(axum::Extension(
        MachineService::open(data.path(), Settings::default()).unwrap(),
    ));
    let call = |token: String, op: &str, input: serde_json::Value| {
        let app = app.clone();
        let op = op.to_owned();
        async move {
            let request = Request::builder()
                .uri("/api/v1/operations")
                .method("POST")
                .header("host", "localhost:3210")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    json!({"operation":op,"request_id":"scope-test","input":input}).to_string(),
                ))
                .unwrap();
            let response = app.oneshot(request).await.unwrap();
            let status = response.status().as_u16();
            let body: serde_json::Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
                    .unwrap();
            (status, body)
        }
    };
    let (status, denied) = call(old.clone(), "notes.list", json!({"space_id":space})).await;
    assert_eq!(status, 403, "{denied}");
    assert!(denied["error"]["details"]["approval_id"].is_string());
    let foreign = call(old.clone(), "notes.list", json!({"space_id":other})).await;
    let missing = call(
        old.clone(),
        "notes.list",
        json!({"space_id":Uuid::now_v7()}),
    )
    .await;
    assert_eq!(foreign.0, 404);
    assert_eq!(foreign.1["error"], missing.1["error"]);
    assert_eq!(
        call(
            old.clone(),
            "access.approve",
            json!({"space_id":space,"job_id":id,"approval_id":Uuid::now_v7()})
        )
        .await
        .0,
        403
    );
    scope.source_app = "notes".into();
    state.jobs.agent_scope_change(&scope).unwrap();
    assert_eq!(
        call(old.clone(), "notes.list", json!({"space_id":space}))
            .await
            .0,
        401
    );
    let new = state.jobs.agent_token(&scope, _run.job.runs).unwrap();
    assert_eq!(
        call(new.clone(), "notes.list", json!({"space_id":space}))
            .await
            .0,
        200
    );
    let read = Request::builder()
        .uri("/api/v1/describe")
        .header("host", "localhost:3210")
        .header("authorization", format!("Bearer {new}"))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(read).await.unwrap();
    let discovery: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    assert_eq!(
        discovery["principal"]["spaces"].as_array().unwrap().len(),
        1
    );
    assert!(
        !discovery["operations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["id"] == "access.approve")
    );
}
