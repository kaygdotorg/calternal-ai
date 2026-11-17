//! Machine readiness and saved reasons through the official adapter fixture.
use super::*;

struct Feed {
    response: reqwest::Response,
    pending: Vec<u8>,
}

impl Feed {
    async fn open(server: &Server, route: &str) -> Self {
        Self::open_as(server, route, &server.owner).await
    }

    async fn open_as(server: &Server, route: &str, token: &str) -> Self {
        let response = reqwest::Client::new()
            .get(format!("{}{route}", server.url()))
            .bearer_auth(token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        Self {
            response,
            pending: Vec::new(),
        }
    }

    async fn next(&mut self, predicate: impl Fn(&Value) -> bool) -> Value {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(end) = self.pending.windows(2).position(|bytes| bytes == b"\n\n") {
                    let raw: Vec<_> = self.pending.drain(..end + 2).collect();
                    if let Some(data) = raw.strip_prefix(b"data: ") {
                        let frame = serde_json::from_slice(data).unwrap();
                        if predicate(&frame) {
                            return frame;
                        }
                    }
                } else {
                    let chunk = self
                        .response
                        .chunk()
                        .await
                        .unwrap()
                        .expect("live feed ended");
                    self.pending.extend_from_slice(&chunk);
                }
            }
        })
        .await
        .expect("missing live frame")
    }
}

async fn reason(server: &Server, space: &str, thread: &Value, code: &str) -> Value {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        let got = op(server, "threads.get", json!({"id":thread})).await;
        let reason = &got["thread"]["turn_reason"];
        if reason["code"] == code {
            let rows = op(server, "threads.list", json!({"space_id":space})).await;
            let listed = rows["threads"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["id"] == *thread)
                .unwrap();
            assert_eq!(listed["turn_reason"], *reason);
            return reason.clone();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "expected {code}, got {reason}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn projects_saved_reasons_and_refreshes_provider_status() {
    require_adapter_build();
    let data = TempDir::new("readiness-data");
    let plugins = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let server = Server::start_with_plugins(&data.0, free_port(), &plugins);
    let bin = TempDir::new("readiness-bin");
    let fixture = std::fs::read_to_string(plugins.join("calternal-claude-code/test/fake-claude.mjs")).unwrap()
        .replace("const emit=m=>", r#"if(prompt.includes('park-for-person')) {
  const form = {genui_version:2,source:'$release=""\nForm([Input($release,"Which release?")], "Release selection")'};
  const answer = await fetch(new URL("/api/v1/operations", process.env.CALTERNAL_URL), {
    method:'POST',headers:{Authorization:'Bearer '+process.env.CALTERNAL_AGENT_TOKEN,'Content-Type':'application/json'},
    body:JSON.stringify({operation:'requests.ask',request_id:'fixture-readiness-question',input:{form}})
  });
  if(!answer.ok) process.exit(72);
  setInterval(()=>{}, 1000);
  await new Promise(()=>{});
}
const emit=m=>"#);
    std::fs::write(bin.join("claude"), fixture).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(bin.join("claude"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mdir = TempDir::new("readiness-machine");
    let (_, spaces) = server.principal(&server.guest).await;
    let space = spaces
        .into_iter()
        .find(|(_, role)| role == "reader")
        .unwrap()
        .0;
    let make = |title: &str| json!({"space_id":space,"title":title});
    let thread =
        op(&server, "threads.create", make("Waiting thread")).await["thread"]["id"].clone();
    assert_eq!(
        reason(&server, &space, &thread, "idle").await["code"],
        "idle"
    );
    let queued = op(
        &server,
        "threads.send",
        json!({"id":thread,"message":"hang"}),
    )
    .await;
    reason(&server, &space, &thread, "no_machine").await;

    let code = server.code(&server.owner).await;
    assert_eq!(enrol(&server.url(), &code, &mdir.0), 0);
    reason(&server, &space, &thread, "machine_offline").await;
    let readiness = op(
        &server,
        "agent.readiness",
        json!({"space_id":space,"thread_id":thread}),
    )
    .await;
    let machine_id = readiness["machines"][0]["machine_id"].clone();
    assert_eq!(readiness["machines"][0]["connected"], false);
    assert!(readiness["machines"][0]["providers"]["calternal-claude-code"]["check"].is_null());

    let mut machine = machine_with_adapter(&mdir.0, &bin.0);
    wait_online(&server, &server.owner, 1).await;
    let queued_id = queued["job"]["id"].as_str().unwrap();
    until(&server, queued_id, |j| {
        j["checkpoint"]["machine_id"].is_string()
    })
    .await;
    machine.kill();
    let stopped = reason(&server, &space, &thread, "interrupted").await;
    assert_eq!(stopped["resumable"], true);
    assert_eq!(stopped["why"], "machine_offline");
    assert!(stopped["since_ms"].is_number());
    op(&server, "agent.stop", json!({"job_id":queued_id})).await;
    reason(&server, &space, &thread, "cancelled").await;

    let _machine = machine_with_adapter(&mdir.0, &bin.0);
    wait_online(&server, &server.owner, 1).await;
    let provider = "calternal-claude-code";
    let (account, _) = server.principal(&server.owner).await;
    let home = mdir.join(&format!("sandboxes/accounts/{account}/home"));
    let mut account_feed = Feed::open(&server, "/api/v1/account/live?readiness=true").await;
    account_feed.next(|f| f["type"] == "ready").await;
    let check = || json!({"space_id":space,"machine_id":machine_id});
    let cached = || json!({"space_id":space});
    for signed_in in [false, true, false] {
        if signed_in {
            std::fs::write(home.join("signed-in"), "fixture only").unwrap();
        } else if home.join("signed-in").exists() {
            std::fs::remove_file(home.join("signed-in")).unwrap();
        }
        let probe = op(&server, "agent.check", check()).await;
        let (_, events) = until(&server, probe["job"]["id"].as_str().unwrap(), |j| {
            j["state"] == "done"
        })
        .await;
        assert!(
            events
                .iter()
                .any(|e| e["event"]["type"] == "check" && e["event"]["signed_in"] == signed_in)
        );
        account_feed
            .next(|f| f["type"] == "readiness_changed")
            .await;
        let snapshot = op(&server, "agent.readiness", cached()).await;
        let status = &snapshot["machines"][0]["providers"][provider];
        assert_eq!(status["check"]["signed_in"], signed_in);
        assert_eq!(status["check"]["sign_in_command"], "claude auth login");
        assert_eq!(status["ready"], signed_in);
    }

    let spare = TempDir::new("readiness-spare-machine");
    let code = server.code(&server.owner).await;
    assert_eq!(enrol(&server.url(), &code, &spare.0), 0);
    let unpinned = op(&server, "agent.readiness", json!({"space_id":space})).await;
    assert_eq!(unpinned["machines"].as_array().unwrap().len(), 2);
    let subscriptions = json!([{"space_id":space}]).to_string();
    let url = reqwest::Url::parse_with_params(
        &format!("{}/api/v1/live", server.url()),
        [("subscriptions", subscriptions)],
    )
    .unwrap();
    let route = format!("/api/v1/live?{}", url.query().unwrap());
    let mut journal = Feed::open(&server, &route).await;
    let mut shared = Feed::open_as(&server, &route, &server.guest).await;
    journal.next(|f| f["type"] == "ready").await;
    shared.next(|f| f["type"] == "ready").await;
    for (message, expected) in [
        ("auth-failure", "provider_signed_out"),
        ("transient-always", "retrying"),
        ("fail", "failed"),
        ("crash", "interrupted"),
        ("park-for-person", "waiting_on_person"),
    ] {
        let title = message.replace('-', " ");
        let thread = op(&server, "threads.create", make(&title)).await["thread"]["id"].clone();
        let turn = op(
            &server,
            "threads.send",
            json!({"id":thread,"message":message}),
        )
        .await;
        let got = reason(&server, &space, &thread, expected).await;
        let frame = journal
            .next(|f| {
                f["entry"]["item"] == thread
                    && f["entry"]["data"]["turn_reason"]["code"] == expected
            })
            .await;
        assert_eq!(frame["entry"]["data"]["turn_reason"], got);
        match expected {
            "provider_signed_out" => {
                assert_eq!(got["provider"], provider);
                assert_eq!(got["sign_in_command"], "claude auth login");
                let entry = shared
                    .next(|f| {
                        f["entry"]["item"] == thread && f["entry"]["data"]["state"] == "failed"
                    })
                    .await;
                let visible = &entry["entry"]["data"]["turn_reason"];
                assert_eq!(*visible, json!({"code":"failed","class":"permanent"}));
                let view = op_as(&server, &server.guest, "threads.get", json!({"id":thread})).await;
                assert_eq!(view["thread"]["turn_reason"], *visible);
                assert!(!view.to_string().contains(machine_id.as_str().unwrap()));
            }
            "retrying" => {
                assert_eq!(got["attempt"], 1);
                assert_eq!(got["rule"], "provider_backoff");
                assert!(got["next_attempt_ms"].is_number());
                op(&server, "agent.stop", json!({"job_id":turn["job"]["id"]})).await;
            }
            "failed" => assert_eq!(got["class"], "permanent"),
            "interrupted" => assert_eq!(got["resumable"], false),
            "waiting_on_person" => {
                assert!(got["request_id"].is_string());
                let pinned = op(
                    &server,
                    "agent.readiness",
                    json!({"space_id":space,"thread_id":thread}),
                )
                .await;
                assert_eq!(pinned["machines"].as_array().unwrap().len(), 1);
                assert_eq!(pinned["machines"][0]["machine_id"], machine_id);
                let entry = shared
                    .next(|f| {
                        f["entry"]["item"] == thread && f["entry"]["data"]["state"] == "waiting"
                    })
                    .await;
                assert_eq!(
                    entry["entry"]["data"]["turn_reason"],
                    json!({"code":"waiting_on_person"})
                );
                op(&server, "agent.stop", json!({"job_id":turn["job"]["id"]})).await;
            }
            _ => unreachable!(),
        }
    }
    op(
        &server,
        "agent.settings",
        json!({
            "retry_settings":{"provider_errors":false,"subscription_limits":false}
        }),
    )
    .await;
    for (prompt, class) in [
        ("transient-always", "transient"),
        ("usage-limit", "usage_limit"),
    ] {
        let thread = op(&server, "threads.create", make(class)).await["thread"]["id"].clone();
        op(
            &server,
            "threads.send",
            json!({"id":thread,"message":prompt}),
        )
        .await;
        let failed = reason(&server, &space, &thread, "failed").await;
        assert_eq!(failed["class"], class);
        assert_eq!(failed["reset_at_ms"].is_number(), class == "usage_limit");
    }
}
