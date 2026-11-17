//! Native manual-provider operations: foreign/missing equivalence and person-only controls.
use super::*;

/// Independently enumerated operations covered here; discovery additions require a case.
pub const OPERATIONS: &[&str] = &[
    "agent.check",
    "agent.run",
    "agent.settings",
    "agent.events",
    "agent.stop",
    "agent.resume",
    "agent.mode",
    "access.request",
    "access.approve",
    "machines.set",
    "machines.resume",
];

#[tokio::test]
async fn agent_operations_hide_foreign_jobs_spaces_and_machine_settings() {
    let f = Fixture::new().await;
    let thread = uuid::Uuid::now_v7().to_string();
    let start=f.op(1,"agent.run",json!({"space_id":f.spaces[0],"thread_id":thread,"prompt":"disposable conformance prompt"})).await;
    start.ok();
    let job = start.body["data"]["job"]["id"].as_str().unwrap();
    let missing = uuid::Uuid::now_v7().to_string();
    let code = f
        .req(
            "POST",
            "/api/v1/machines/codes",
            Some(&f.cookies[1]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    code.ok();
    let key = calternal_machine::identity::MachineKey::generate().unwrap();
    let machine=f.machine("/api/v1/machine/enroll",None,json!({"code":code.body["code"],"public_key":key.public_key_b64(),"name":"provider-conformance","os":"linux","arch":"x86_64","direct":false})).await;
    machine.ok();
    let mut rows = vec![];
    for who in [0, 2, 3] {
        for op in OPERATIONS {
            let (foreign, absent) = match *op {
                "agent.check" | "agent.run" => (
                    json!({"space_id":f.spaces[0],"thread_id":thread,"prompt":"test"}),
                    json!({"space_id":missing,"thread_id":thread,"prompt":"test"}),
                ),
                "machines.set" => (
                    json!({"machine_id":machine.body["machine_id"],"direct":true}),
                    json!({"machine_id":missing,"direct":true}),
                ),
                "machines.resume" => (
                    json!({"lease_id":missing}),
                    json!({"lease_id":uuid::Uuid::now_v7()}),
                ),
                "agent.settings" => (
                    json!({"job_id":job,"retry_settings":{"provider_errors":false,"subscription_limits":false}}),
                    json!({"job_id":missing,"retry_settings":{"provider_errors":false,"subscription_limits":false}}),
                ),
                "access.request" => (
                    json!({"job_id":job,"app":"notes"}),
                    json!({"job_id":missing,"app":"notes"}),
                ),
                "access.approve" => (
                    json!({"job_id":job,"approval_id":missing}),
                    json!({"job_id":missing,"approval_id":missing}),
                ),
                _ => (json!({"job_id":job}), json!({"job_id":missing})),
            };
            let a = f.op(who, op, foreign).await;
            let b = f.op(who, op, absent).await;
            assert_eq!(a.status, 404, "{who}/{op}: {}", a.body);
            assert_eq!(a.signature(), b.signature(), "{who}/{op}");
            rows.push(json!({"account":who,"operation":op,"status":a.status,"equal":true}));
        }
    }
    f.op(1, "agent.stop", json!({"job_id":job})).await.ok();
    evidence("provider-isolation", json!(rows));
}

#[tokio::test]
async fn access_approval_requires_the_person_and_rotates_into_explicit_resume() {
    let mut f = Fixture::new().await;
    let start=f.op(1,"agent.run",json!({"space_id":f.spaces[0],"thread_id":uuid::Uuid::now_v7(),"prompt":"fixture waiting for a machine","mode":"least_access"})).await;
    start.ok();
    let job = start.body["data"]["job"]["id"].as_str().unwrap().to_owned();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let state = f.op(1, "agent.events", json!({"job_id":job})).await;
        state.ok();
        if state.body["data"]["job"]["checkpoint"]["lease_id"].is_string() {
            break;
        }
        assert!(Instant::now() < deadline, "{}", state.body);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let request = f
        .op(
            1,
            "access.request",
            json!({"job_id":job,"app":"notes","write":false}),
        )
        .await;
    request.ok();
    let approval = request.body["data"]["approval_id"].clone();
    assert_eq!(
        f.op(
            2,
            "access.approve",
            json!({"job_id":job,"approval_id":approval})
        )
        .await
        .status,
        404
    );
    let login = f.login(1).await;
    login.ok();
    f.cookies[1] = login.cookie();
    let approved = f
        .op(
            1,
            "access.approve",
            json!({"job_id":job,"approval_id":approval}),
        )
        .await;
    approved.ok();
    assert_eq!(approved.body["data"]["job"]["state"], "interrupted");
    let resumed = f.op(1, "agent.resume", json!({"job_id":job})).await;
    resumed.ok();
    let mode = f
        .op(1, "agent.mode", json!({"job_id":job,"mode":"full_access"}))
        .await;
    mode.ok();
    assert_eq!(mode.body["data"]["job"]["state"], "interrupted");
    f.op(1, "agent.stop", json!({"job_id":job})).await.ok();
}
