//! Native manual-provider operations: foreign/missing equivalence and person-only controls.
use super::*;

/// Independently enumerated operations covered here; discovery additions require a case.
pub const OPERATIONS: &[&str] = &[
    "agent.readiness",
    "agent.check",
    "agent.run",
    "agent.settings",
    "agent.events",
    "agent.stop",
    "agent.discard",
    "agent.resume",
    "agent.mode",
    "access.request",
    "machines.set",
    "machines.resume",
];

#[tokio::test]
async fn agent_operations_hide_foreign_jobs_spaces_and_machine_settings() {
    let f = Fixture::new().await;
    let thread = uuid::Uuid::now_v7().to_string();
    let start=f.op(1,"agent.run",json!({"space_id":f.spaces[0],"thread_id":thread,"prompt":"disposable conformance prompt"})).await;
    start.ok();
    assert_eq!(
        start.body["data"]["job"]["space_id"], f.spaces[0],
        "UUID casing created a different job root"
    );
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
                "agent.readiness" => (json!({"space_id":f.spaces[0]}), json!({"space_id":missing})),
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
    f.op(1, "agent.discard", json!({"job_id":job})).await.ok();
    evidence("provider-isolation", json!(rows));
}

#[tokio::test]
async fn manual_turns_share_request_admission_with_content_and_directory_writes() {
    let f = Fixture::new().await;
    let rid = "turn-shared-admission";
    let input =
        json!({"space_id":f.spaces[0],"thread_id":uuid::Uuid::now_v7(),"prompt":"receipt probe"});
    let first = f.op_id(1, "agent.run", input.clone(), rid).await;
    first.ok();
    let replay = f.op_id(1, "agent.run", input, rid).await;
    replay.ok();
    assert_eq!(
        first.body["data"]["job"]["id"],
        replay.body["data"]["job"]["id"]
    );
    let content = f
        .op_id(
            1,
            "notes.create",
            json!({"space_id":f.spaces[0],"title":"receipt probe","markdown":"body"}),
            rid,
        )
        .await;
    assert_eq!(
        content.status, 409,
        "turn receipt did not reserve the shared identity"
    );
    let directory = f
        .op_id(
            1,
            "spaces.set_role",
            json!({"space_id":f.spaces[0],"account_id":f.ids[2],"role":"reader"}),
            rid,
        )
        .await;
    assert_eq!(directory.status, 409);
    let other = "content-before-turn";
    f.op_id(
        1,
        "notes.create",
        json!({"space_id":f.spaces[0],"title":"receipt probe","markdown":"body"}),
        other,
    )
    .await
    .ok();
    let turn=f.op_id(1,"agent.run",json!({"space_id":f.spaces[0],"thread_id":uuid::Uuid::now_v7(),"prompt":"receipt probe"}),other).await;
    assert_eq!(turn.status, 409);
}

#[tokio::test]
async fn source_app_requires_an_enabled_plugin_or_a_native_operation_group() {
    let f = Fixture::new().await;
    for (source, status) in [
        ("invented-app", 400),
        ("notes-insights", 403),
        ("notes", 200),
        ("spaces", 200),
    ] {
        let answer=f.op(1,"agent.run",json!({"space_id":f.spaces[0],"thread_id":uuid::Uuid::now_v7(),"source_app":source,"prompt":"source scope probe"})).await;
        assert_eq!(answer.status, status, "{source}: {}", answer.body);
    }
}

#[tokio::test]
async fn readiness_is_private_even_among_members_of_the_same_space() {
    let f = Fixture::new().await;
    let target = json!({"space_id":f.spaces[3]});
    let open = |who: usize| {
        f.client
            .get(format!("{}/api/v1/account/live?readiness=true", f.base))
            .header("host", "localhost:18080")
            .header("origin", ORIGIN)
            .header("cookie", &f.cookies[who])
            .send()
    };
    let mut owner = live_cases::Live {
        response: open(1).await.unwrap(),
        buffer: vec![],
    };
    let mut member = live_cases::Live {
        response: open(2).await.unwrap(),
        buffer: vec![],
    };
    assert_eq!(owner.next().await.unwrap()["type"], "ready");
    assert_eq!(member.next().await.unwrap()["type"], "ready");

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
    let enrolled = f
        .machine(
            "/api/v1/machine/enroll",
            None,
            json!({
                "code":code.body["code"],"public_key":key.public_key_b64(),"name":"owner-host",
                "os":"linux","arch":"x86_64","direct":false
            }),
        )
        .await;
    enrolled.ok();

    assert_eq!(
        owner.next().await.unwrap(),
        json!({"type":"readiness_changed"})
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(600), member.next())
            .await
            .is_err()
    );
    let visible = f.op(1, "agent.readiness", target.clone()).await;
    visible.ok();
    assert_eq!(
        visible.body["data"]["machines"][0]["machine_id"],
        enrolled.body["machine_id"]
    );
    assert_eq!(visible.body["data"]["machines"][0]["connected"], false);
    let hidden = f.op(2, "agent.readiness", target.clone()).await;
    hidden.ok();
    assert_eq!(hidden.body["data"]["machines"], json!([]));
    assert_eq!(hidden.body["data"]["pool"], json!([]));

    let foreign = f
        .op(
            2,
            "agent.check",
            json!({
                "space_id":f.spaces[3], "machine_id":enrolled.body["machine_id"]
            }),
        )
        .await;
    let absent = f
        .op(
            2,
            "agent.check",
            json!({
                "space_id":f.spaces[3], "machine_id":uuid::Uuid::new_v4()
            }),
        )
        .await;
    assert_eq!(foreign.status, 404);
    assert_eq!(foreign.signature(), absent.signature());
    let probe = f
        .op(
            1,
            "agent.check",
            json!({
                "space_id":f.spaces[3],"machine_id":enrolled.body["machine_id"]
            }),
        )
        .await;
    probe.ok();
    let opaque = f
        .op(
            2,
            "agent.events",
            json!({"job_id":probe.body["data"]["job"]["id"]}),
        )
        .await;
    let absent = f
        .op(2, "agent.events", json!({"job_id":uuid::Uuid::new_v4()}))
        .await;
    assert_eq!(opaque.status, 404);
    assert_eq!(opaque.signature(), absent.signature());
    f.op(
        1,
        "agent.stop",
        json!({"job_id":probe.body["data"]["job"]["id"]}),
    )
    .await
    .ok();

    let saved = f
        .op(
            2,
            "threads.create",
            json!({"space_id":f.spaces[1],"title":"Other space"}),
        )
        .await;
    saved.ok();
    let foreign_thread = f
        .op(
            1,
            "agent.readiness",
            json!({
                "space_id":f.spaces[3],"thread_id":saved.body["data"]["thread"]["id"]
            }),
        )
        .await;
    let absent_thread = f
        .op(
            1,
            "agent.readiness",
            json!({
                "space_id":f.spaces[3],"thread_id":uuid::Uuid::new_v4()
            }),
        )
        .await;
    assert_eq!(foreign_thread.status, 404);
    assert_eq!(foreign_thread.signature(), absent_thread.signature());
}

#[tokio::test]
async fn pool_readiness_requires_entitlement_and_omits_host_identity() {
    let f = Fixture::new().await;
    let admin = |args: &[&str]| {
        let output = Command::new(std::env::var_os("CALTERNAL_SERVER_BIN").unwrap())
            .args(["--json", "machines"])
            .args(args)
            .arg("--appdata")
            .arg(f.appdata())
            .arg("--userdata-ssd")
            .arg(f.dir.path().join("userdata-ssd"))
            .arg("--userdata-hdd")
            .arg(f.dir.path().join("userdata-hdd"))
            .args(["--installation-id", "00000000-0000-0000-0000-000000000001"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
    };
    let enrollment = admin(&["code", "--pool"]);
    let key = calternal_machine::identity::MachineKey::generate().unwrap();
    let pool = f
        .machine(
            "/api/v1/machine/enroll",
            None,
            json!({
                "code":enrollment["code"],"public_key":key.public_key_b64(),"name":"pool-host",
                "os":"linux","arch":"x86_64","direct":false
            }),
        )
        .await;
    pool.ok();
    admin(&["pool-grant", &f.ids[1]]);
    let target = json!({"space_id":f.spaces[3]});
    let entitled = f.op(1, "agent.readiness", target.clone()).await;
    entitled.ok();
    let row = &entitled.body["data"]["pool"][0];
    assert_eq!(row["connected"], false);
    assert_eq!(row["execution"], "sandboxed");
    for key in ["last_seen_ms", "machine_id", "name"] {
        assert!(row.get(key).is_none());
    }
    let member = f.op(2, "agent.readiness", target.clone()).await;
    member.ok();
    assert_eq!(member.body["data"]["pool"], json!([]));
    let hidden = f
        .op(
            1,
            "agent.check",
            json!({"space_id":f.spaces[3],"machine_id":pool.body["machine_id"]}),
        )
        .await;
    let absent = f
        .op(
            1,
            "agent.check",
            json!({"space_id":f.spaces[3],"machine_id":uuid::Uuid::new_v4()}),
        )
        .await;
    assert_eq!(hidden.status, 404);
    assert_eq!(hidden.signature(), absent.signature());
    admin(&["pool-grant", &f.ids[1], "--remove"]);
    let revoked = f.op(1, "agent.readiness", target).await;
    revoked.ok();
    assert_eq!(revoked.body["data"]["pool"], json!([]));
}
