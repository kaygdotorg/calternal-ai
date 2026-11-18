//! Deterministic disable-during-invocation test: the worker blocks until a
//! `go` file appears, the plugin is disabled while it is blocked, and the
//! host must discard the result (generation changed) instead of delivering it.

mod support;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use calternal_server::api::{AppState, allowed_hosts, dispatch};
use calternal_server::auth::StoredDirectory;
use calternal_server::plugins::{Registry, WorkerPool, resolve_node};
use calternal_server::store::Store;
use serde_json::{Value, json};

struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn obj(v: Value) -> calternal_core::JsonObject {
    serde_json::from_value(v).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disable_during_invocation_discards_result() {
    let mut b = [0u8; 6];
    getrandom::fill(&mut b).unwrap();
    let root = Tmp(std::env::temp_dir().join(format!("calternal-race-{}", hex::encode(b))));
    let wdir = root.0.join("worker");
    // The worker reads the note through the broker, then blocks until `go` exists.
    let worker = support::worker(&wdir, support::READ_NOTE, support::COMPLETE_AFTER_GO);
    let plugin = support::plugin(support::insights_manifest(), Some(&worker), false);
    let roots = support::roots(&root.0);
    let store = Store::open(
        &roots,
        uuid::Uuid::from_u128(1),
        true,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
    )
    .unwrap();
    let data = roots.appdata;
    let node = resolve_node(None).await.expect("node >= 22.13");
    let state = Arc::new(AppState {
        blocking: calternal_server::blocking::RequestWorkers::new().unwrap(),
        inputs: store.account_inputs(),
        store,
        directory: StoredDirectory::load_or_create(&data).unwrap(),
        registry: Registry::from_plugins(vec![plugin]),
        workers: WorkerPool::new(Some(node)),
        jobs: calternal_server::plugins::Jobs::disabled(),
        allowed_hosts: allowed_hosts(0),
    });
    let dev = support::dev(&state.directory);
    let owner = dev.owner;

    let note = dispatch(
        state.clone(),
        owner.clone(),
        calternal_server::plugins::Session::Static,
        "c",
        "notes.create",
        obj(json!({"space_id": dev.personal, "title": "T", "markdown": "x"})),
    )
    .await
    .unwrap();
    let id = note["note"]["id"].as_str().unwrap().to_string();
    let toggle = |rid: &'static str, op: &'static str| {
        dispatch(
            state.clone(),
            owner.clone(),
            calternal_server::plugins::Session::Static,
            rid,
            op,
            obj(json!({"space_id": dev.personal, "plugin_id": "calternal-notes-insights"})),
        )
    };
    toggle("e1", "plugins.enable").await.unwrap();

    let (s2, o2, id2) = (state.clone(), owner.clone(), id.clone());
    let call = tokio::spawn(async move {
        dispatch(
            s2,
            o2,
            calternal_server::plugins::Session::Static,
            "s",
            "notes-insights.summarize",
            obj(json!({"space_id": dev.personal, "id": id2})),
        )
        .await
    });
    // An invocation is admitted to a worker only after the authorized
    // generation capture, so this proves it is in flight.
    let t0 = std::time::Instant::now();
    while state.workers.in_flight() == 0 {
        assert!(
            t0.elapsed() < Duration::from_secs(5),
            "worker never started"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    toggle("d1", "plugins.disable").await.unwrap();
    std::fs::write(wdir.join("go"), "").unwrap();
    let err = call.await.unwrap().expect_err("result must be discarded");
    assert_eq!(err.code, calternal_core::ErrorCode::Conflict);
    assert_eq!(err.details.unwrap()["reason"], "plugin_generation_changed");

    // Disabled now: new invocations are denied before reaching a worker.
    let err = dispatch(
        state.clone(),
        owner.clone(),
        calternal_server::plugins::Session::Static,
        "s2",
        "notes-insights.summarize",
        obj(json!({"space_id": dev.personal, "id": id})),
    )
    .await
    .expect_err("disabled");
    assert_eq!(err.code, calternal_core::ErrorCode::Forbidden);
    assert_eq!(state.workers.in_flight(), 0);
}
