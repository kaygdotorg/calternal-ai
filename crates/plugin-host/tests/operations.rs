#![cfg(feature = "operations")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use calternal_plugin_host::operations::{
    Call, Code, Delivery, Event, EventKind, Method, Operations, Person,
};
use calternal_types::{AccountId, SpaceId};
use serde_json::{Value, json};
use tokio::process::Command;
use uuid::Uuid;

const FIXTURE: &str = "packages/plugin-sdk/test/fixtures/operations.ts";
const WORKER: &str = "packages/plugin-sdk/dist/operations-worker.js";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn endpoint(
    space: SpaceId,
    entry: PathBuf,
    declared: impl Fn(EventKind, &str) -> bool + Send + Sync + 'static,
    idle: Duration,
) -> Operations {
    let repo = repo();
    let worker = repo.join(WORKER);
    assert!(
        worker.is_file(),
        "Build @calternal/plugin-sdk before running this integration test"
    );
    let path = std::env::var_os("PATH").expect("The dev shell supplies a PATH");
    let node = std::env::split_paths(&path)
        .map(|directory| directory.join("node"))
        .find(|candidate| candidate.is_file())
        .expect("Use nix develop to supply Node 24")
        .canonicalize()
        .unwrap();
    Operations::new(
        space,
        declared,
        move || {
            let mut command = Command::new(&node);
            command
                .arg("--conditions=calternal-operations")
                .arg("--permission")
                .arg(format!("--allow-fs-read={}", repo.display()))
                .arg(&worker)
                .arg(&entry);
            command
        },
        idle,
        Duration::from_secs(6),
    )
}

fn delivery(
    space: SpaceId,
    actor: AccountId,
    kind: EventKind,
    selector: &str,
    key: &str,
    item: Uuid,
    input: Value,
) -> Delivery {
    Delivery {
        selector: selector.into(),
        input,
        me: Person {
            id: actor,
            role: "owner".into(),
        },
        space,
        event: Event {
            kind,
            key: key.into(),
            item: Some(item),
            before: None,
            request: None,
            selected_operation: None,
        },
    }
}

async fn probe(worker: &Operations, space: SpaceId, actor: AccountId, item: Uuid) -> Value {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&calls);
    let result = worker.invoke(delivery(space, actor, EventKind::Call, "probe", "probe-event", item, json!({ "item": item })),
        move |call| {
            observed.lock().unwrap().push((call.method, call.input.clone()));
            async move {
                Ok(match call.method {
                    Method::Query => json!({"rows":[],"totals":{},"cursor":null}),
                    Method::Read => json!({"id":item,"kind":"test.items","fields":{},"computed":{},"body":"","records":[],"version":"v1"}),
                    Method::State => json!({"value": 7}),
                    Method::Call => json!({"inspected":true}),
                    Method::Run => json!({"id":"test-run", "status":"running"}),
                    Method::RunSend | Method::RunStop => Value::Null,
                    Method::Write => panic!("probe does not write"),
                })
            }
        }).await.unwrap();
    assert_eq!(
        *calls.lock().unwrap(),
        [
            (Method::Query, json!({"from":"test.items"})),
            (Method::Read, json!({"item":item})),
            (
                Method::State,
                json!({"op":"get","item":item,"key":"cursor"})
            ),
            (
                Method::State,
                json!({"op":"set","item":item,"key":"cursor","value":7})
            ),
            (
                Method::Call,
                json!({"operation":"test-other.inspect","input":{"item":item}})
            ),
            (
                Method::Run,
                json!({"program":"test-harness","scope":"session","targets":[item]})
            ),
            (Method::RunSend, json!({"id":"test-run","input":"hello"})),
            (Method::RunStop, json!({"id":"test-run"})),
        ]
    );
    assert_eq!(result["me"], json!({"id":actor,"role":"owner"}));
    assert_eq!(result["space"], json!({"id":space}));
    assert_eq!(
        result["event"],
        json!({"kind":"call","key":"probe-event","item":item})
    );
    assert_eq!(result["status"], "running");
    result["pid"].clone()
}

#[tokio::test]
async fn lazy_reuse_space_isolation_idle_restart_and_crash() {
    let space = SpaceId::new();
    let other_space = SpaceId::new();
    let actor = AccountId::new();
    let item = Uuid::new_v4();
    let idle = Duration::from_secs(1);
    let fixture = repo().join(FIXTURE);
    let active = endpoint(
        space,
        fixture.clone(),
        |kind, selector| kind == EventKind::Call && matches!(selector, "probe" | "crash"),
        idle,
    );
    let isolated = endpoint(other_space, fixture, |_, name| name == "probe", idle);

    let refused = active
        .invoke(
            delivery(
                space,
                actor,
                EventKind::Call,
                "hidden",
                "hidden-event",
                item,
                Value::Null,
            ),
            |_| async { panic!("undeclared operation reached the route") },
        )
        .await
        .unwrap_err();
    assert_eq!(Code::from_error(&refused), Code::NotAllowed);
    let first = probe(&active, space, actor, item).await;
    assert_eq!(probe(&active, space, actor, item).await, first);
    assert_ne!(probe(&isolated, other_space, actor, item).await, first);

    let entered = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed = Arc::clone(&entered);
    let never_committed = active
        .invoke(
            delivery(
                space,
                actor,
                EventKind::Call,
                "crash",
                "crash-event",
                item,
                json!({ "item": item }),
            ),
            move |call| {
                observed.store(true, std::sync::atomic::Ordering::SeqCst);
                assert_eq!(call.input["changes"].as_array().unwrap().len(), 2);
                assert_eq!(call.idempotency_key.as_deref(), Some("crash-event:write:0"));
                // The mock commit waits for the complete transaction. Exiting closes
                // the invocation before any route side effect takes place.
                std::future::pending()
            },
        )
        .await
        .unwrap_err();
    assert_eq!(Code::from_error(&never_committed), Code::Unreachable);
    assert!(entered.load(std::sync::atomic::Ordering::SeqCst));

    let recovered = probe(&active, space, actor, item).await;
    assert_ne!(recovered, first);
    tokio::time::sleep(idle + Duration::from_millis(200)).await;
    assert_ne!(probe(&active, space, actor, item).await, recovered);
}

#[tokio::test]
async fn change_replay_uses_host_owned_write_ordinals() {
    let space = SpaceId::new();
    let actor = AccountId::new();
    let item = Uuid::new_v4();
    let worker = endpoint(
        space,
        repo().join(FIXTURE),
        |kind, name| kind == EventKind::Change && name == "test.items",
        Duration::from_secs(1),
    );
    let keys = Arc::new(Mutex::new(Vec::new()));
    for _ in 0..2 {
        let received = Arc::clone(&keys);
        let value = worker
            .invoke(
                delivery(
                    space,
                    actor,
                    EventKind::Change,
                    "test.items",
                    "committed-change",
                    item,
                    json!(item),
                ),
                move |call| {
                    received.lock().unwrap().push(call.idempotency_key);
                    async { Ok(Value::Null) }
                },
            )
            .await
            .unwrap();
        assert_eq!(value, Value::Null);
    }
    assert_eq!(
        *keys.lock().unwrap(),
        [
            Some("committed-change:write:0".into()),
            None,
            Some("committed-change:write:1".into()),
            Some("committed-change:write:0".into()),
            None,
            Some("committed-change:write:1".into()),
        ]
    );
}

#[derive(Default)]
struct Mock {
    receipts: std::collections::HashMap<String, (Value, Value)>,
    records: Vec<Value>,
    title: String,
}

impl Mock {
    fn route(&mut self, item: Uuid, call: Call) -> Result<Value, calternal_types::Error> {
        assert_eq!(call.method, Method::Write);
        let key = call.idempotency_key.expect("host-issued write key");
        if let Some((input, output)) = self.receipts.get(&key) {
            if *input != call.input {
                return Err(Code::Conflict.error(
                    "The retry differs from the original.",
                    "Use a new event key.",
                ));
            }
            return Ok(output.clone());
        }
        let changes = call.input["changes"].as_array().unwrap();
        assert_eq!(changes.len(), 1);
        let change = &changes[0];
        assert_eq!(change["item"], json!(item));
        let mut receipt = json!({"id":item,"version":format!("v{}", self.receipts.len() + 1)});
        match change["op"].as_str().unwrap() {
            "append record" => {
                self.records.push(change["record"].clone());
                receipt["anchor"] = json!("first-record");
                receipt["link"] = json!("[Message](<Chats/Conversation/chat.md#^first-record>)");
            }
            "change" => self.title = change["fields"]["title"].as_str().unwrap().to_owned(),
            op => panic!("unexpected operation: {op}"),
        }
        let output = json!({"changes":[receipt]});
        self.receipts.insert(key, (call.input, output.clone()));
        Ok(output)
    }
}

#[tokio::test]
async fn upstream_chat_send_replay_rename_and_error() {
    let repo = repo();
    let path = repo.join("plugins-v2/calternal-chat/operations.ts");
    assert!(
        path.is_file(),
        "Provision the upstream Chat vectors in this disposable test checkout"
    );
    let space = SpaceId::new();
    let actor = AccountId::new();
    let item = Uuid::new_v4();
    let worker = endpoint(
        space,
        path,
        |kind, name| kind == EventKind::Call && matches!(name, "chats.send" | "chats.rename"),
        Duration::from_secs(1),
    );
    let mock = Arc::new(Mutex::new(Mock::default()));
    let invoke = |selector, key, input: Value| {
        let state = Arc::clone(&mock);
        worker.invoke(
            delivery(space, actor, EventKind::Call, selector, key, item, input),
            move |call| {
                let output = state.lock().unwrap().route(item, call);
                async move { output }
            },
        )
    };
    let send = json!({"item":item,"text":"First message"});
    let first = invoke("chats.send", "send-event", send.clone())
        .await
        .unwrap();
    assert_eq!(first["changes"][0]["anchor"], "first-record");
    assert_eq!(
        invoke("chats.send", "send-event", send).await.unwrap(),
        first
    );
    let conflict = invoke(
        "chats.send",
        "send-event",
        json!({"item":item,"text":"Different"}),
    )
    .await
    .unwrap_err();
    assert_eq!(Code::from_error(&conflict), Code::Conflict);
    invoke(
        "chats.rename",
        "rename-event",
        json!({"item":item,"title":"New title"}),
    )
    .await
    .unwrap();
    let invalid = invoke("chats.send", "empty-event", json!({"item":item,"text":" "}))
        .await
        .unwrap_err();
    assert_eq!(Code::from_error(&invalid), Code::Invalid);
    assert_eq!(invalid.message, "Write a message or attach a file.");
    let mock = mock.lock().unwrap();
    assert_eq!(
        mock.records,
        [json!({"fields": {}, "body": "First message"})]
    );
    assert_eq!(mock.title, "New title");
    assert_eq!(mock.receipts.len(), 2);
}

#[tokio::test]
async fn send_and_start_replay_reuses_write_and_run_receipts() {
    let repo = repo();
    let space = SpaceId::new();
    let actor = AccountId::new();
    let item = Uuid::new_v4();
    let idle = Duration::from_secs(1);
    let harness = Arc::new(endpoint(
        space,
        repo.join("plugins-v2/calternal-chat-claude-code/operations.ts"),
        |kind, name| kind == EventKind::Call && name == "runs.start",
        idle,
    ));
    let chat = endpoint(
        space,
        repo.join(FIXTURE),
        |_, name| name == "send-with-harness",
        idle,
    );
    let mock = Arc::new(Mutex::new(Mock::default()));
    let runs = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let inputs = Arc::new(Mutex::new(std::collections::HashMap::new()));
    for _ in 0..2 {
        let (harness, mock, runs, inputs) =
            (harness.clone(), mock.clone(), runs.clone(), inputs.clone());
        let response = chat.invoke(delivery(space, actor, EventKind::Call, "send-with-harness", "gesture", item, json!({
            "chat":item,"draft":"Start here","harness":"calternal-chat-claude-code.runs.start"
        })), move |call| {
            let (harness, mock, runs, inputs) = (harness.clone(), mock.clone(), runs.clone(), inputs.clone());
            async move {
                match call.method {
                    Method::Write => mock.lock().unwrap().route(item, call),
                    Method::Call => {
                        assert_eq!(call.input["operation"], "calternal-chat-claude-code.runs.start");
                        assert_eq!(call.input["input"], json!({
                            "chat":item,"prompt":"[Message](<Chats/Conversation/chat.md#^first-record>)"
                        }));
                        let key = call.idempotency_key.unwrap();
                        assert_eq!(key, "gesture:call:0");
                        let mut event = delivery(space, actor, EventKind::Call, "runs.start", &key, item, call.input["input"].clone());
                        event.event.request = Some("gesture".into());
                        harness.invoke(event, move |run| {
                            let (runs, inputs) = (runs.clone(), inputs.clone());
                            async move {
                                let key = run.idempotency_key.unwrap();
                                match run.method {
                                    Method::Run => {
                                        assert_eq!(run.input, json!({
                                            "program":"calternal-provider-claude-code","scope":"session","targets":[item]
                                        }));
                                        assert_eq!(key, "gesture:call:0:run:0");
                                        let mut runs = runs.lock().unwrap();
                                        let receipt = runs.entry(key).or_insert_with(|| json!({"id":"test-run","status":"running"}));
                                        Ok(receipt.clone())
                                    }
                                    Method::RunSend => {
                                        let seq = run.input["input"]["seq"].as_u64().unwrap();
                                        assert!((1..=2).contains(&seq));
                                        assert_eq!(key, format!("gesture:call:0:run-send:{}", seq - 1));
                                        assert_eq!(run.input["id"], "test-run");
                                        if seq == 2 {
                                            assert_eq!(run.input["input"]["command"]["prompt"],
                                                "[Message](<Chats/Conversation/chat.md#^first-record>)");
                                        }
                                        let mut inputs = inputs.lock().unwrap();
                                        assert_eq!(inputs.entry(key).or_insert_with(|| run.input.clone()), &run.input);
                                        Ok(Value::Null)
                                    }
                                    _ => panic!("unexpected run callback"),
                                }
                            }
                        }).await
                    }
                    _ => panic!("unexpected chat callback"),
                }
            }
        }).await.unwrap();
        assert_eq!(response, json!({"id":"test-run","status":"running"}));
    }
    assert_eq!(mock.lock().unwrap().records.len(), 1);
    assert_eq!(runs.lock().unwrap().len(), 1);
    assert_eq!(inputs.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn refuses_undeclared_calls_and_reaction_runs() {
    let space = SpaceId::new();
    let actor = AccountId::new();
    let item = Uuid::new_v4();
    let worker = endpoint(
        space,
        repo().join(FIXTURE),
        |kind, name| match kind {
            EventKind::Call => name == "try-undeclared",
            EventKind::Change => name == "test.restricted",
        },
        Duration::from_secs(1),
    );
    let refused = worker
        .invoke(
            delivery(
                space,
                actor,
                EventKind::Call,
                "try-undeclared",
                "undeclared-call",
                item,
                Value::Null,
            ),
            |call| async move {
                assert_eq!(call.method, Method::Call);
                assert_eq!(call.input["operation"], "test-other.hidden");
                Err(Code::NotAllowed.error(
                    "This operation is absent from calls.",
                    "Declare the operation.",
                ))
            },
        )
        .await
        .unwrap_err();
    assert_eq!(Code::from_error(&refused), Code::NotAllowed);
    assert_eq!(refused.message, "This operation is absent from calls.");
    let refused = worker
        .invoke(
            delivery(
                space,
                actor,
                EventKind::Change,
                "test.restricted",
                "change-run",
                item,
                json!(item),
            ),
            |_| async { panic!("a reaction attempted to start a run") },
        )
        .await
        .unwrap_err();
    assert_eq!(Code::from_error(&refused), Code::NotAllowed);
}
