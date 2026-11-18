//! A signup session must reverify before approval, then the browser can retry.

#[path = "account_common/mod.rs"]
mod common;
mod support;

use calternal_core::{AgentMode, OperationRisk};
use calternal_server::{
    plugins::{Session, jobs::Admission},
    turns::access::Authority,
};
use common::*;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn approval_after_signup_succeeds_with_reverified_cookie() {
    let app = app_with_jobs();
    let (cookie, mut device) = first_owner_setup_session(&app, "First owner").await;
    let profile = get(&app, "/api/v1/auth/me", Some(&cookie)).await;
    let account =
        calternal_types::AccountId::parse(profile.body["user"]["id"].as_str().unwrap()).unwrap();
    let spaces = post(
        &app,
        "/api/v1/operations",
        Some(&cookie),
        json!({"operation":"spaces.list","input":{},"request_id":"approval-spaces"}),
    )
    .await;
    let space = spaces.body["data"]["spaces"].as_array().unwrap()[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let state = app.state.clone();
    let request = tokio::task::spawn_blocking(move || {
        let thread = Uuid::now_v7();
        let job = state
            .jobs
            .admit(Admission {
                space: space.clone(),
                plugin: "calternal-orchestrator".into(),
                account,
                session: Session::Static,
                operation: "agent.turn".into(),
                input: json!({"request":{"thread_id":thread}}),
                parent: None,
                generation: 0,
                request_key: "approval-fixture".into(),
            })
            .unwrap();
        let run = state.jobs.claim(&space, &job).unwrap().unwrap().job.runs;
        let authority = Authority {
            actor: calternal_types::Actor::agent(account, thread),
            space: space.clone(),
            job: job.clone(),
            mode: AgentMode::LeastAccess,
            source_app: "notes".into(),
            grants: vec![],
        };
        state.jobs.agent_prepare_scope(&authority, run).unwrap();
        state.jobs.agent_token(&authority, run).unwrap();
        let input = serde_json::from_value(
            json!({"space_id":space,"title":"Approval fixture","markdown":"Example"}),
        )
        .unwrap();
        let id = state
            .jobs
            .agent_action_request(
                &authority,
                "notes.create",
                &input,
                "write-fixture",
                OperationRisk::Reversible,
                true,
            )
            .unwrap();
        support::park_request(&state.jobs, &space, &job, &id);
        id
    })
    .await
    .unwrap();
    let approval = json!({"operation":"requests.approve","input":{"id":request,"lifetime":"once"},"request_id":"approve-once"});
    let stale = post(&app, "/api/v1/operations", Some(&cookie), approval.clone()).await;
    assert_eq!(stale.status, 403, "{}", stale.body);
    assert_eq!(stale.code(), "passkey_reverification_required");

    let start = post(
        &app,
        "/api/v1/auth/reverify/start",
        Some(&cookie),
        json!({}),
    )
    .await;
    assert_eq!(start.status, 200, "{}", start.body);
    let assertion = device.sign(&start);
    let renewed = post(
        &app,
        "/api/v1/auth/reverify/finish",
        Some(&cookie),
        json!({"ceremony":start.body["ceremony"],"credential":assertion,"device":"Browser"}),
    )
    .await;
    assert_eq!(renewed.status, 200, "{}", renewed.body);
    let renewed_cookie = renewed.session();
    assert_ne!(renewed_cookie, cookie);
    let accepted = post(&app, "/api/v1/operations", Some(&renewed_cookie), approval).await;
    assert_eq!(accepted.status, 200, "{}", accepted.body);
    assert_eq!(accepted.body["data"]["status"], "approved");
}
