//! Space activation and membership checks for client-facing plugin contributions.
use super::*;

#[tokio::test]
async fn contributions_resolve_only_in_the_authorized_enabled_space() {
    let f = Fixture::new().await;
    let list = |account, space: &str| {
        let input = json!({"space_id": space});
        f.op(account, "plugins.list", input)
    };
    let space = &f.spaces[0];
    let disabled = list(1, space).await;
    disabled.ok();
    assert!(disabled.body["data"]["slots"].is_null());
    f.op(
        1,
        "plugins.enable",
        json!({"space_id":space,"plugin_id":"calternal-notes-insights"}),
    )
    .await
    .ok();
    let active = list(1, space).await;
    active.ok();
    let slots = active.body["data"]["slots"].as_array().unwrap();
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0]["plugin_id"], "calternal-notes-insights");
    assert_eq!(slots[0]["contribution"]["id"], "note-statistics");

    let foreign = list(2, space).await;
    assert_eq!(foreign.status, 404);
    assert!(foreign.body.get("data").is_none());

    let other = list(2, &f.spaces[1]).await;
    other.ok();
    assert!(other.body["data"]["slots"].is_null());

    f.op(
        1,
        "plugins.disable",
        json!({"space_id":space,"plugin_id":"calternal-notes-insights"}),
    )
    .await
    .ok();
    let disabled = list(1, space).await;
    disabled.ok();
    assert!(disabled.body["data"]["slots"].is_null());
}

#[tokio::test]
async fn guide_operations_observe_space_isolation_and_activation() {
    let f = Fixture::new().await;
    let space = &f.spaces[0];
    let cases = [
        ("explain", json!({"topic":"reference"})),
        ("scaffold", json!({"name":"example-plugin"})),
        ("check", json!({"manifest":{}})),
        ("publish", json!({})),
    ];
    for (name, mut input) in cases {
        input["space_id"] = json!(space);
        let operation = format!("calternal-plugin-guide.{name}");
        let denied = f.op(2, &operation, input.clone()).await;
        assert_eq!(denied.status, 404);
        let disabled = f.op(1, &operation, input).await;
        assert_eq!(disabled.status, 403);
    }
    f.op(
        1,
        "plugins.enable",
        json!({
            "space_id": space, "plugin_id": "calternal-plugin-guide"
        }),
    )
    .await
    .ok();
    let published = f
        .op(
            1,
            "calternal-plugin-guide.publish",
            json!({"space_id": space}),
        )
        .await;
    published.ok();
    let id = &published.body["data"]["item"]["id"];
    let reference = f
        .op(1, "resolve", json!({"kind":"plugin-guides","id":id}))
        .await;
    reference.ok();
    assert_eq!(reference.body["data"]["space_id"], *space);
    assert_eq!(
        f.op(2, "resolve", json!({"kind":"plugin-guides","id":id}))
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn installed_package_has_the_same_worker_and_space_boundary_after_restart() {
    let mut f = Fixture::new().await;
    let source = f.dir.path().join("fixture.calplugin");
    let id = "conformance-package";
    let op = "conformance-package.read";
    let archive = |version, body| {
        json!({
            "manifest": {
                "id": id, "name":"Conformance package", "icon":"puzzle", "version":version,
                "api_version":1, "kinds":[],
                "operations":[{
                    "id":op, "input_schema":{"type":"object","additionalProperties":false},
                    "output_schema":{"type":"object"},"required_capability":"space.read",
                    "mutates":false,"idempotency":"read_only","risk":"read"
                }],
                "collections":[],"views":[],"attention":[],"settings":[],"schedules":[],
                "webhooks":[],"secrets":[],"hooks":[],"themes":[],"host_calls": if version == "0.3.0" { vec!["read_item"] } else { vec![] }
            },
            "worker": format!(
                "require('node:readline').createInterface({{input:process.stdin}}).on('line', line => {{
                    const {{invocation_id}} = JSON.parse(line);
                    console.log(JSON.stringify({{protocol_version:2, invocation_id,
                      message:{{type:'complete',data:{{ value:{body} }}}}}}));
                }});",
            )
        })
    };
    let run = |args: &[&str]| {
        Command::new(std::env::var_os("CALTERNAL_SERVER_BIN").unwrap())
            .args(args)
            .arg("--json")
            .output()
            .unwrap()
    };
    let stage = |action, appdata: &Path| {
        let reviewed = run(&["plugin", "review", source.to_str().unwrap()]);
        assert!(reviewed.status.success(), "{:?}", reviewed);
        let review: Value = serde_json::from_slice(&reviewed.stdout).unwrap();
        let args = |approval| {
            run(&[
                "plugin",
                action,
                source.to_str().unwrap(),
                "--appdata",
                appdata.to_str().unwrap(),
                "--approve",
                approval,
            ])
        };
        assert!(!args("wrong-checksum").status.success());
        let result = args(review["data"]["checksum"].as_str().unwrap());
        assert!(result.status.success(), "{:?}", result);
    };

    let space = f.spaces[0].clone();
    std::fs::write(&source, archive("0.1.0", 1).to_string()).unwrap();
    stage("install", &f.appdata());
    f.stop();
    f.start().await;
    let scoped = || json!({"space_id":space});
    assert_eq!(f.op(1, op, scoped()).await.status, 403);
    f.op(
        1,
        "plugins.enable",
        json!({"plugin_id":id,"space_id":space}),
    )
    .await
    .ok();
    let first = f.op(1, op, scoped()).await;
    first.ok();
    assert_eq!(first.body["data"]["value"], 1);
    assert_eq!(f.op(2, op, scoped()).await.status, 404);
    assert_eq!(
        f.op(2, op, json!({"space_id":f.spaces[1]})).await.status,
        403
    );

    std::fs::write(&source, archive("0.2.0", 2).to_string()).unwrap();
    stage("update", &f.appdata());
    f.stop();
    f.start().await;
    let second = f.op(1, op, scoped()).await;
    second.ok();
    assert_eq!(second.body["data"]["value"], 2);

    std::fs::write(&source, archive("0.3.0", 3).to_string()).unwrap();
    stage("update", &f.appdata());
    f.stop();
    f.start().await;
    assert_eq!(f.op(1, op, scoped()).await.status, 403);
    f.op(
        1,
        "plugins.enable",
        json!({"plugin_id":id,"space_id":space}),
    )
    .await
    .ok();
    let third = f.op(1, op, scoped()).await;
    third.ok();
    assert_eq!(third.body["data"]["value"], 3);

    let removed = run(&[
        "plugin",
        "remove",
        id,
        "--appdata",
        f.appdata().to_str().unwrap(),
    ]);
    assert!(removed.status.success(), "{:?}", removed);
    f.stop();
    f.start().await;
    assert_eq!(f.op(1, op, scoped()).await.status, 404);
}
