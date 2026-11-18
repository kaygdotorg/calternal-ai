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
    let missing_space = f
        .op(
            1,
            "calternal-plugin-guide.explain",
            json!({"topic":"reference"}),
        )
        .await;
    assert_eq!(missing_space.status, 400);
    assert_eq!(missing_space.body["error"]["details"]["path"], "/space_id");
    let wrong_name = f
        .op(
            1,
            "calternal-plugin-guide.scaffold",
            json!({"space_id":space,"name":42}),
        )
        .await;
    assert_eq!(wrong_name.status, 400);
    assert_eq!(wrong_name.body["error"]["details"]["path"], "/name");
    assert_eq!(
        wrong_name.body["error"]["details"]["expected"],
        "expected string"
    );
    assert!(
        !wrong_name.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("42")
    );
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
async fn installed_package_is_live_and_survives_restart() {
    let mut f = Fixture::new().await;
    let space = f.spaces[0].clone();
    let account = f.ids[1].clone();
    let other_account = f.ids[2].clone();
    let source = f.dir.path().join("fixture.calplugin");
    let id = "conformance-package";
    let op = "conformance-package.read";
    let note = f.note(1, &space).await;
    let note_id = note["id"].as_str().unwrap();
    let archive = |version, body| {
        json!({
            "manifest": {
                "id": id, "name":"Conformance package", "icon":"puzzle", "version":version,
                "api_version":1, "kinds":[],
                "operations":[{
                    "id":op, "input_schema":{"type":"object","properties":{"hold":{"type":"boolean"}},"additionalProperties":false},
                    "output_schema":{"type":"object"},"required_capability":"space.read",
                    "mutates":false,"idempotency":"read_only","risk":"read"
                }],
                "collections":[],"views":[],"attention":[],"settings":[],"schedules":[],
                "webhooks":[],"secrets":[],"hooks":[],"themes":[],"host_calls": match version {
                    "0.3.0" | "0.5.0" => vec!["read_item", "write_item"],
                    "0.4.0" => vec!["write_item"],
                    _ => vec!["read_item"],
                }
            },
            "worker": format!(
                "require('node:readline').createInterface({{input:process.stdin}}).on('line', line => {{
                    const request = JSON.parse(line);
                    if (request.type === 'ready') {{
                        console.log(JSON.stringify(request)); return;
                    }}
                    const {{invocation_id, message}} = request;
                    const send = message => console.log(JSON.stringify({{protocol_version:2, invocation_id, message}}));
                    if (message.type === 'host_reply') {{
                        if (message.ok) send({{type:'complete',data:{{value:{body}}}}});
                        else send({{type:'failed'}});
                    }} else if (message.input.hold) {{
                        setTimeout(() => send({{type:'host_call',call_id:'late',method:'read_item',
                            input:{{kind:'notes',item_id:'{note_id}'}},authority:message.authority}}), 500);
                    }} else send({{type:'complete',data:{{value:{body}}}}});
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
        let args = |approval, target_account| {
            run(&[
                "plugin",
                action,
                source.to_str().unwrap(),
                "--appdata",
                appdata.to_str().unwrap(),
                "--approve",
                approval,
                "--space",
                &space,
                "--account",
                target_account,
            ])
        };
        assert!(!args("wrong-checksum", &account).status.success());
        let checksum = review["data"]["checksum"].as_str().unwrap();
        if action == "install" {
            let denied = args(checksum, &other_account);
            assert!(!denied.status.success(), "{denied:?}");
            let reply: Value = serde_json::from_slice(&denied.stdout).unwrap();
            assert!(
                reply["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("cannot manage plugins")
            );
        }
        let result = args(checksum, &account);
        assert!(result.status.success(), "{:?}", result);
    };

    std::fs::write(&source, archive("0.1.0", 1).to_string()).unwrap();
    stage("install", &f.appdata());
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
    let second = f.op(1, op, scoped()).await;
    second.ok();
    assert_eq!(second.body["data"]["value"], 2);

    let hold = || json!({"space_id":space,"hold":true});
    let widening = async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        std::fs::write(&source, archive("0.3.0", 3).to_string()).unwrap();
        stage("update", &f.appdata());
        assert_eq!(f.op(1, op, scoped()).await.status, 403);
    };
    let (old, ()) = tokio::join!(f.op(1, op, hold()), widening);
    old.ok();
    assert_eq!(old.body["data"]["value"], 2);
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
    f.stop();
    f.start().await;
    f.op(1, op, scoped()).await.ok();

    let narrowing = async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        std::fs::write(&source, archive("0.4.0", 4).to_string()).unwrap();
        stage("update", &f.appdata());
    };
    let (revoked, ()) = tokio::join!(f.op(1, op, hold()), narrowing);
    assert_ne!(
        revoked.status, 200,
        "a narrowed revision delivered a result"
    );
    assert_eq!(f.op(1, op, scoped()).await.body["data"]["value"], 4);

    std::fs::write(&source, archive("0.5.0", 5).to_string()).unwrap();
    stage("update", &f.appdata());
    f.op(
        1,
        "plugins.enable",
        json!({"plugin_id":id,"space_id":space}),
    )
    .await
    .ok();
    let removal = async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let removed = run(&[
            "plugin",
            "remove",
            id,
            "--appdata",
            f.appdata().to_str().unwrap(),
        ]);
        assert!(removed.status.success(), "{removed:?}");
    };
    let (revoked, ()) = tokio::join!(f.op(1, op, hold()), removal);
    assert_ne!(revoked.status, 200, "a removed revision delivered a result");
    assert_eq!(f.op(1, op, scoped()).await.status, 404);
}

#[tokio::test]
async fn drafts_require_the_development_build_and_stay_in_one_space() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let mut f = Fixture::new().await;
    let binary = std::env::var_os("CALTERNAL_SERVER_BIN").unwrap();
    let help = Command::new(&binary)
        .args(["plugin", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    let development_build = String::from_utf8(help.stdout)
        .unwrap()
        .lines()
        .any(|line| line.trim_start().starts_with("dev "));
    let id = "conformance-draft";
    let op = "conformance-draft.result";
    let space = f.spaces[0].clone();
    let account = f.ids[1].clone();
    let send = |root: &Path, worker: &str, account: &str| {
        let package = json!({
            "action": "draft", "space": space, "account": account,
            "package": {
                "manifest": {
                    "id": id, "name":"Conformance draft", "icon":"puzzle", "version":"0.1.0",
                    "api_version":1,
                    "operations":[{
                        "id":op, "input_schema":{"type":"object","properties":{"sentence":{"type":"string"}},"additionalProperties":false},
                        "output_schema":{"type":"object","properties":{"value":{"type":"number"}},"required":["value"]},"required_capability":"space.read",
                        "mutates":false,"idempotency":"read_only","risk":"read"
                    }],
                    "collections":[],"views":[],"attention":[],"settings":[],"schedules":[],
                    "webhooks":[],"secrets":[],"hooks":[],"themes":[],"host_calls":[]
                },
                "worker":worker
            }
        });
        let body = serde_json::to_vec(&package).unwrap();
        let mut sock = UnixStream::connect(root.join("system/admin.plg")).unwrap();
        sock.write_all(&(body.len() as u32).to_be_bytes()).unwrap();
        sock.write_all(&body).unwrap();
        let mut reply = Vec::new();
        sock.read_to_end(&mut reply).unwrap();
        serde_json::from_slice::<Result<(), String>>(&reply).unwrap()
    };
    let worker = |result: Value| {
        format!(
        "require('node:readline').createInterface({{input:process.stdin}}).on('line', line => {{
            const request = JSON.parse(line);
            if (request.type === 'ready') {{ console.log(line); return; }}
            console.log(JSON.stringify({{protocol_version:2,invocation_id:request.invocation_id,
                message:{{type:'complete',data:{{value:{result}}}}}}}));
        }});"
    )
    };
    let scope = || json!({"space_id":space});
    let first = send(&f.appdata(), &worker(json!(1)), &account);
    if !development_build {
        assert!(first.unwrap_err().contains("unknown variant `draft`"));
        f.stop();
        f.development = true;
        f.start().await;
        assert!(
            send(&f.appdata(), &worker(json!(1)), &account)
                .unwrap_err()
                .contains("unknown variant `draft`")
        );
        assert_eq!(f.op(1, op, scope()).await.status, 404);
        return;
    }
    assert_eq!(first.unwrap_err(), "drafts require a development server");
    f.stop();
    f.development = true;
    f.start().await;
    assert!(
        send(&f.appdata(), &worker(json!(1)), &f.ids[2])
            .unwrap_err()
            .contains("cannot manage plugins")
    );
    assert_eq!(f.op(1, op, scope()).await.status, 404);
    send(&f.appdata(), &worker(json!(1)), &account).unwrap();
    let result = f.op(1, op, scope()).await;
    result.ok();
    assert_eq!(result.body["data"]["value"], 1);
    let other = f
        .op(2, "plugins.list", json!({"space_id":f.spaces[1]}))
        .await;
    other.ok();
    assert!(
        !other.body["data"]["plugins"]
            .as_array()
            .unwrap()
            .iter()
            .any(|plugin| plugin["manifest"]["id"] == id)
    );
    assert_eq!(
        f.op(2, op, json!({"space_id":f.spaces[1]})).await.status,
        404
    );
    let diagnostics = |space: &str, account: &str, op: &str| {
        Command::new(&binary)
            .args(["plugin", "diagnostics", id, op, "--json", "--appdata"])
            .arg(f.appdata())
            .args(["--space", space, "--account", account])
            .output()
            .unwrap()
    };
    let show = || {
        let result = diagnostics(&space, &account, op);
        assert!(result.status.success(), "{result:?}");
        serde_json::from_slice::<Value>(&result.stdout).unwrap()["data"].clone()
    };
    let first_ready = show();
    assert_eq!(first_ready["phase"], "ready");
    assert!(first_ready["timing"]["handshake_ms"].is_number());
    assert!(!diagnostics(&space, &f.ids[2], op).status.success());
    assert!(!diagnostics(&f.spaces[1], &f.ids[2], op).status.success());
    assert!(
        !diagnostics(&space, &account, "conformance-draft.missing")
            .status
            .success()
    );
    assert!(send(&f.appdata(), "process.exit(1)", &account).is_err());
    assert_eq!(f.op(1, op, scope()).await.body["data"]["value"], 1);
    assert_eq!(show()["ready_revision"], first_ready["ready_revision"]);
    let failed = show()["last_failed_candidate"].clone();
    assert_eq!(failed["phase"], "handshake", "{failed:?}");
    assert!(
        failed["message"]
            .as_str()
            .is_some_and(|m| !m.is_empty() && m.len() <= 300)
    );
    send(
        &f.appdata(),
        &worker(json!("private-worker-value")),
        &account,
    )
    .unwrap();
    assert!(show()["last_failed_candidate"].is_null());
    let err = f.op(1, op, scope()).await;
    assert_eq!(err.status, 503);
    assert_eq!(
        err.body["error"]["details"]["path"],
        "/properties/value/type"
    );
    assert!(!err.body.to_string().contains("private-worker-value"));
    for _ in 0..6 {
        let input = json!({"space_id":space,"sentence":1});
        assert_eq!(f.op(1, op, input).await.status, 400);
    }
    let recent = show();
    let errors = recent["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 5);
    assert!(
        errors.iter().all(|e| e["path"] == "/sentence"),
        "{errors:?}"
    );
    f.op(1, op, scope()).await;
    assert_eq!(show()["errors"][0]["path"], "/properties/value/type");
    send(&f.appdata(), &worker(json!(2)), &account).unwrap();
    assert_eq!(show()["errors"], json!([]));
    assert_eq!(f.op(1, op, scope()).await.body["data"]["value"], 2);
    f.stop();
    f.start().await;
    assert_eq!(f.op(1, op, scope()).await.status, 404);
}
