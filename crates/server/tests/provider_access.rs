//! Thread-token lifecycle with the real per-space jobs database; no provider account.
mod support;
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
    let directory = calternal_server::auth::StoredDirectory::load_or_create(dir.path()).unwrap();
    let dev = support::dev(&directory);
    let jobs = Jobs::open(dir.path())
        .unwrap()
        .with_security(directory.spaces.agents());
    let space = dev.personal.to_string();
    let account = dev.owner.id.to_string();
    let thread = Uuid::now_v7().to_string();
    let id = jobs
        .admit(Admission {
            space: space.clone(),
            plugin: "calternal-orchestrator".into(),
            account: calternal_types::AccountId::parse(&account).unwrap(),
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
        actor: calternal_types::Actor::agent(
            calternal_types::AccountId::parse(&account).unwrap(),
            Uuid::parse_str(&thread).unwrap(),
        ),
        space: space.clone(),
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
            "requests.approve",
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
    // A later person send applies its requested mode directly, with fresh grants.
    let next_id = jobs
        .admit(Admission {
            space: space.clone(),
            plugin: "calternal-orchestrator".into(),
            account: authority.actor.account,
            session: Session::Static,
            operation: "agent.turn".into(),
            input: json!({"request":{"thread_id":authority.thread().unwrap()}}),
            parent: None,
            generation: 0,
            request_key: "next turn".into(),
        })
        .unwrap();
    let next = jobs.claim(&space, &next_id).unwrap().unwrap();
    authority.job = next_id;
    authority.mode = AgentMode::FullAccess;
    let inherited = jobs.agent_prepare_scope(&authority, next.job.runs).unwrap();
    assert_eq!(
        inherited.mode,
        AgentMode::FullAccess,
        "the new turn ignored the person's requested wider mode"
    );
    assert!(inherited.grants.is_empty());
    let raw = std::fs::read(dir.path().join("spaces").join(space).join("jobs.db")).unwrap();
    assert!(
        !raw.windows(new.len()).any(|b| b == new.as_bytes()),
        "raw token was stored"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn thread_bearers_enforce_scope_before_handlers_and_cannot_approve() {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use calternal_server::{
        api::{self, AppState},
        auth::{Directory, StoredDirectory},
        machines::{MachineService, Settings},
        plugins::{Registry, WorkerPool},
        store::Store,
    };
    use std::sync::Arc;
    use tower::ServiceExt;
    let data = tempfile::tempdir().unwrap();
    let root = data.path().to_path_buf();
    let (tokens, who, principal) = tokio::task::spawn_blocking(move || {
        let tokens = StoredDirectory::load_or_create(&root).unwrap();
        let owner = std::fs::read(root.join("dev-tokens/owner.token")).unwrap();
        let who = tokens.authenticate(&owner).unwrap();
        let principal = tokens.describe(&who);
        (tokens, who, principal)
    })
    .await
    .unwrap();
    let space = principal.spaces[0].id.clone();
    let other = principal.spaces[1].id.clone();
    let roots = support::roots(data.path());
    let store = Store::open(
        &roots,
        Uuid::from_u128(1),
        true,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
    )
    .unwrap();
    let machines = tokio::task::block_in_place(|| {
        MachineService::open(data.path(), tokens.spaces.machines(), Settings::default())
    })
    .unwrap();
    let jobs = Jobs::open(data.path())
        .unwrap()
        .with_security(tokens.spaces.agents());
    let state = Arc::new(AppState {
        blocking: calternal_server::blocking::RequestWorkers::new().unwrap(),
        inputs: store.account_inputs(),
        store,
        directory: tokens,
        registry: Registry::load(
            &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins"),
        ),
        workers: WorkerPool::new(None),
        jobs,
        allowed_hosts: api::allowed_hosts(3210),
    });
    let created = api::dispatch(
        state.clone(),
        who.clone(),
        Session::Static,
        "create-own-thread",
        "threads.create",
        serde_json::from_value(json!({"space_id":space,"title":"Own thread"})).unwrap(),
    )
    .await
    .unwrap();
    let thread = created["thread"]["id"].as_str().unwrap().to_owned();
    let id = state
        .jobs
        .admit(Admission {
            space: space.clone(),
            plugin: "calternal-orchestrator".into(),
            account: who.id,
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
        actor: calternal_types::Actor::agent(who.id, Uuid::parse_str(&thread).unwrap()),
        space: space.clone(),
        job: id.clone(),
        mode: AgentMode::LeastAccess,
        source_app: String::new(),
        grants: vec![],
    };
    let old =
        tokio::task::block_in_place(|| state.jobs.agent_token(&scope, _run.job.runs)).unwrap();
    let app = api::router(state.clone(), None);
    let _machines = machines;
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
            "requests.approve",
            json!({"id":Uuid::now_v7(),"lifetime":"once"})
        )
        .await
        .0,
        403
    );
    scope.source_app = "notes".into();
    tokio::task::block_in_place(|| state.jobs.agent_scope_change(&scope)).unwrap();
    assert_eq!(
        call(old.clone(), "notes.list", json!({"space_id":space}))
            .await
            .0,
        401
    );
    let new =
        tokio::task::block_in_place(|| state.jobs.agent_token(&scope, _run.job.runs)).unwrap();
    assert_eq!(
        call(new.clone(), "notes.list", json!({"space_id":space}))
            .await
            .0,
        200
    );
    // Secret audit provenance reuses the existing Actor/Via type for the agent.
    scope.mode = AgentMode::FullAccess;
    tokio::task::block_in_place(|| state.jobs.agent_scope_change(&scope)).unwrap();
    let new =
        tokio::task::block_in_place(|| state.jobs.agent_token(&scope, _run.job.runs)).unwrap();
    let (status, reply) = call(
        new.clone(),
        "threads.send",
        json!({"id":thread,"message":"no self-widening","mode":"full_access"}),
    )
    .await;
    assert_eq!(status, 403, "{reply}");
    let (status,reply)=call(new.clone(),"secrets.request",json!({"space_id":space,"name":"provider-audit","label":"Fixture","reason":"Agent provenance"})).await;
    assert_eq!(status, 200, "{reply}");
    let path = data
        .path()
        .join("appdata/spaces")
        .join(&space)
        .join("secrets.db");
    let actor: String = tokio::task::spawn_blocking(move || {
        rusqlite::Connection::open(path)
            .unwrap()
            .query_row(
                "SELECT actor FROM audit ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap()
    })
    .await
    .unwrap();
    assert_eq!(
        actor,
        format!(
            "an agent for {}, in thread {}",
            who.id,
            scope.thread().unwrap()
        )
    );
    // Full access remains bound to one thread, including within the same space.
    for target_space in [&space, &other] {
        let thread = api::dispatch(
            state.clone(),
            who.clone(),
            Session::Static,
            &format!("create-{target_space}"),
            "threads.create",
            serde_json::from_value(json!({"space_id":target_space,"title":"Foreign thread"}))
                .unwrap(),
        )
        .await
        .unwrap();
        let target = &thread["thread"]["id"];
        let foreign = call(
            new.clone(),
            "threads.send",
            json!({"id":target,"message":"must not append"}),
        )
        .await;
        let missing = call(
            new.clone(),
            "threads.send",
            json!({"id":Uuid::now_v7(),"message":"must not append"}),
        )
        .await;
        assert_eq!(foreign.0, 404, "{}", foreign.1);
        assert_eq!(foreign.1["error"], missing.1["error"]);
        let saved = api::dispatch(
            state.clone(),
            who.clone(),
            Session::Static,
            "read-thread",
            "threads.get",
            serde_json::from_value(json!({"id":target})).unwrap(),
        )
        .await
        .unwrap();
        assert!(
            !saved["thread"]["markdown"]
                .as_str()
                .unwrap()
                .contains("must not append")
        );
    }
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
            .any(|o| o["id"] == "requests.approve")
    );
}
