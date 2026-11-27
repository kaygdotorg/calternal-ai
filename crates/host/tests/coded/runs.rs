//! Test the real harness with the fake provider image.
use super::*;

static PODMAN: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn selected_harness_replies_once_and_enforces_actor_scope_and_egress() {
    use calternal_machine::sandbox::podman::PodmanBackend;
    use std::time::Duration;

    let Ok(image) = std::env::var("CALTERNAL_TEST_PROVIDER_IMAGE") else {
        eprintln!("SKIPPED: set CALTERNAL_TEST_PROVIDER_IMAGE to run the fake provider");
        return;
    };
    let _serial = PODMAN.lock().await;
    let f = Fixture::new().await;
    let root = f._root.path().join("runs");
    let _containers = podman_guard::PodmanGuard::new(&root);
    let backend = Arc::new(PodmanBackend::new("podman", &root, &image));
    f.host.clone().with_podman_runs(backend, image);
    for plugin in ["calternal-chat", "calternal-chat-claude-code"] {
        f.install(checkout().join("plugins-v2").join(plugin))
            .await
            .unwrap();
    }
    let item = f.chat().await;
    let other_item = f.chat().await;
    let send = |item: Value, text: &str, key: &str| {
        f.host.clone().trusted_call(
            f.owner.into(),
            f.space,
            Call {
                call: "calternal-chat.chats.send".into(),
                input: json!({"item":item,"text":text}),
                idempotency_key: Some(key.to_owned()),
                slot_selection: Some(calternal_host::SlotSelection {
                    slot: "harness".into(),
                    filler: "calternal-chat-claude-code".into(),
                }),
            },
        )
    };
    let hello = send(item.clone(), "hello from host", "once").await.unwrap();
    let retry = send(item.clone(), "hello from host", "once").await.unwrap();
    assert_eq!(hello, retry);
    let run = hello["run"]["id"].clone();
    assert!(run.is_string());
    let caller = Actor::account(f.owner);
    let stranger = Actor::account(f.auth.create("Other person").await.unwrap().id);
    let send_input = |actor, seq, prompt: Value| {
        f.call(actor, "s.run.send",
        json!({"id":run,"input":{"version":1,"seq":seq,"command":{"type":"send","prompt":prompt,"context":[]}}}), None)
    };
    assert_eq!(
        send_input(stranger, 3, json!("forbidden"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::Forbidden
    );
    let outside = f
        .call(
            caller.clone(),
            "calternal-chat.chats.send",
            json!({"item":other_item,"text":"outside"}),
            Some("outside"),
        )
        .await
        .unwrap();
    assert_eq!(
        send_input(caller.clone(), 3, outside["changes"][0]["link"].clone())
            .await
            .unwrap_err()
            .code,
        ErrorCode::Forbidden
    );

    let wait_for = |count| {
        let item = item.clone();
        let caller = caller.clone();
        let f = &f;
        async move {
            tokio::time::timeout(Duration::from_secs(15), async {
                loop {
                    let read = f
                        .call(caller.clone(), "s.read", json!({"item":item}), None)
                        .await
                        .unwrap();
                    let records = read["records"].as_array().unwrap();
                    if records.len() >= count {
                        break records.clone();
                    }
                    tokio::time::sleep(Duration::from_millis(30)).await;
                }
            })
            .await
            .unwrap_or_else(|_| panic!("fake reply did not reach record {count}"))
        }
    };
    let records = wait_for(2).await;
    assert_eq!(records.len(), 2);
    assert_eq!(records[1]["body"], "Echo: hello from host");
    assert_eq!(records[1]["run"], run);

    let queued = json!({"id":run,"input":{"version":1,"seq":3,
        "command":{"type":"send","prompt":"egress-denied","context":[]}}});
    for _ in 0..2 {
        let receipt = f
            .call(
                caller.clone(),
                "s.run.send",
                queued.clone(),
                Some("queued-input"),
            )
            .await
            .unwrap();
        assert_eq!(receipt, json!({"status":"queued"}));
    }
    let mut changed = queued;
    changed["input"]["command"]["prompt"] = json!("different input");
    assert_eq!(
        f.call(caller.clone(), "s.run.send", changed, Some("queued-input"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::Conflict,
    );
    let records = wait_for(3).await;
    assert_eq!(records[2]["body"], "Egress: refused");
    assert_eq!(records[2]["run"], run);
    let container =
        PodmanBackend::container_name(f.owner.as_uuid(), run.as_str().unwrap().parse().unwrap());
    send_input(caller.clone(), 4, json!("hold: ignore signals"))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            let processes = tokio::process::Command::new("podman")
                .args(["top", &container, "args"])
                .output()
                .await
                .unwrap();
            if String::from_utf8_lossy(&processes.stdout)
                .lines()
                .any(|line| line.trim() == "sleep 60")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await
    .expect("fake provider did not start the process family");
    let started = std::time::Instant::now();
    f.call(caller.clone(), "s.run.stop", json!({"id":run}), None)
        .await
        .unwrap();
    assert!(started.elapsed() < Duration::from_secs(2));
    let processes = tokio::process::Command::new("podman")
        .args(["top", &container, "args"])
        .output()
        .await
        .unwrap();
    assert!(!String::from_utf8_lossy(&processes.stdout).contains("sleep 60"));
    let retried = send(item.clone(), "hello from host", "once").await.unwrap();
    assert_eq!(retried["run"]["id"], run);
    assert_eq!(retried["run"]["status"], "stopped");
}

#[path = "../../../../tests/support/podman_guard.rs"]
mod podman_guard;

#[tokio::test]
async fn sandbox_callbacks_keep_scope_and_target_removal_stops_the_run() {
    use calternal_machine::sandbox::podman::PodmanBackend;
    use std::time::Duration;

    let Ok(image) = std::env::var("CALTERNAL_TEST_PROVIDER_IMAGE") else {
        eprintln!("SKIPPED: set CALTERNAL_TEST_PROVIDER_IMAGE to exercise sandbox callbacks");
        return;
    };
    let _serial = PODMAN.lock().await;
    let f = Fixture::new().await;
    let root = f._root.path().join("runs");
    let _containers = podman_guard::PodmanGuard::new(&root);
    f.host
        .clone()
        .with_podman_runs(Arc::new(PodmanBackend::new("podman", &root, &image)), image);
    f.install(checkout().join("plugins-v2/calternal-chat"))
        .await
        .unwrap();
    f.install(checkout().join("plugins-v2/calternal-chat-claude-code"))
        .await
        .unwrap();
    let harness = tempfile::tempdir().unwrap();
    let path = harness.path();
    for directory in ["kinds", "programs"] {
        std::fs::create_dir(path.join(directory)).unwrap();
    }
    std::fs::write(
        path.join("plugin.md"),
        r#"---
plugin: scope-check
name: Scope check
version: 1.0.0
format: 1
programs:
  check: executable programs/check
reads: calternal-chat.chats
calls: calternal-chat.chats.send
replies: calternal-chat.chats.send
fills:
  calternal-chat:harness: operation start, as "Scope check"
---
Exercise the scope checks through sandbox output.
"#,
    )
    .unwrap();
    std::fs::write(
        path.join("kinds/runs.md"),
        r#"---
kind: runs
name: Run
format: 1
hidden: keep 7d
fields:
  chat: link(calternal-chat.chats)
operations:
  start: code
---
Run the scope probe.
"#,
    )
    .unwrap();
    std::fs::write(
        path.join("operations.ts"),
        r#"import { on } from "@calternal/plugin-sdk";
export const start = on.call("runs.start", async ({ chat }, s) => {
  const run = await s.run("check", { scope: "session", targets: [chat] });
  return { id: run.id };
});
"#,
    )
    .unwrap();
    std::fs::write(
        path.join("programs/check"),
        r#"#!/usr/bin/env node
import { createConnection } from "node:net";
import { createInterface } from "node:readline";
import { once } from "node:events";

const control = createConnection(process.env.CALTERNAL_RUN_CONTROL_SOCKET);
await once(control, "connect");
control.write("READY\n");
const input = createInterface({ input: process.stdin })[Symbol.asyncIterator]();
const { item, other } = JSON.parse((await input.next()).value);
const emit = (frame) => console.log(JSON.stringify(frame));
async function call(id, call, inputValue) {
  emit({ host_call: { id, call, input: inputValue } });
  return JSON.parse((await input.next()).value).host_reply;
}
const allowed = await call(1, "s.read", { item });
const denied = await call(2, "s.read", { item: other });
const write = await call(3, "s.write", {
  changes: [{ op: "append record", item: other, record: { body: "Outside" } }],
});
const text = [allowed.result?.id === item && !("state" in allowed.result), denied.error?.code, write.error?.code].join("|");
emit({ event: { type: "text_delta", text } });
emit({ event: { type: "done", outcome: "done" } });
control.end();
await input.next();
"#,
    )
    .unwrap();
    f.install(path.to_owned()).await.unwrap();
    let item = f.chat().await;
    let other = f.chat().await;
    let actor = Actor::account(f.owner);
    let response = f
        .host
        .clone()
        .trusted_call(
            actor.clone(),
            f.space,
            Call {
                call: "calternal-chat.chats.send".into(),
                input: json!({"item":item,"text":"check scope"}),
                idempotency_key: Some("scope-check".into()),
                slot_selection: Some(calternal_host::SlotSelection {
                    slot: "harness".into(),
                    filler: "scope-check".into(),
                }),
            },
        )
        .await
        .unwrap();
    let changed_filler = f
        .host
        .clone()
        .trusted_call(
            actor.clone(),
            f.space,
            Call {
                call: "calternal-chat.chats.send".into(),
                input: json!({"item":item,"text":"check scope"}),
                idempotency_key: Some("scope-check".into()),
                slot_selection: Some(calternal_host::SlotSelection {
                    slot: "harness".into(),
                    filler: "calternal-chat-claude-code".into(),
                }),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(changed_filler.code, ErrorCode::Conflict);
    let id = response["run"]["id"].clone();
    f.call(
        actor.clone(),
        "s.run.send",
        json!({"id":id,"input":{"item":item,"other":other}}),
        None,
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let read = f
                .call(actor.clone(), "s.read", json!({"item":item}), None)
                .await
                .unwrap();
            if !read["records"][1].is_null() {
                assert_eq!(read["records"][1]["body"], "true|not-allowed|not-allowed");
                break;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await
    .expect("the scope probe did not complete");
    let read = f
        .call(actor.clone(), "s.read", json!({"item":other}), None)
        .await
        .unwrap();
    assert!(read["records"].as_array().unwrap().is_empty());
    let run = id.as_str().unwrap().parse().unwrap();
    let container = PodmanBackend::container_name(f.owner.as_uuid(), run);
    f.call(
        actor.clone(),
        "s.write",
        json!({"changes":[{"op":"remove","item":item}]}),
        Some("remove-target"),
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let status = tokio::process::Command::new("podman")
                .args(["top", &container])
                .output()
                .await
                .unwrap();
            if !status.status.success() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await
    .expect("removing the target did not terminate the sandbox");
    f.call(actor, "s.run.stop", json!({"id":id}), None)
        .await
        .unwrap();
}
