//! Deterministic disable-during-invocation test: the worker blocks until a
//! `go` file appears, the plugin is disabled while it is blocked, and the
//! host must discard the result (generation changed) instead of delivering it.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use calternal_core::{False, PluginManifest, PluginOperation};
use calternal_server::api::{AppState, allowed_hosts, dispatch};
use calternal_server::auth::{TokenTable, dev_principals};
use calternal_server::plugins::{
    LoadedPlugin, MAX_CONCURRENT_WORKERS, Registry, RegistryEntry, file_sha256, resolve_node,
};
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
    std::fs::create_dir_all(&wdir).unwrap();
    let worker = wdir.join("worker.js");
    std::fs::write(
        &worker,
        r#"const fs=require('fs');let s='';process.stdin.on('data',d=>s+=d);process.stdin.on('end',()=>{const req=JSON.parse(s);const t=setInterval(()=>{if(fs.existsSync(__dirname+'/go')){clearInterval(t);process.stdout.write(JSON.stringify({protocol_version:1,invocation_id:req.invocation_id,ok:true,data:{stats:{words:1,characters:1,lines:1},view:{genui_version:1,root:{type:'text',id:'t',text:'done'}}}}))}},20)});"#,
    )
    .unwrap();
    let worker = std::fs::canonicalize(&worker).unwrap();
    let manifest = PluginManifest {
        id: "notes-insights".into(),
        version: "0.1.0".into(),
        api_version: 1,
        name: "Insights".into(),
        description: "race test".into(),
        dependencies: vec![],
        capabilities: vec!["notes.read".into()],
        operations: vec![PluginOperation {
            id: "notes-insights.summarize".into(),
            description: "stats".into(),
            mutates: False,
        }],
        views: vec![],
    };
    let plugin = LoadedPlugin {
        entry: RegistryEntry {
            id: "notes-insights".into(),
            version: "0.1.0".into(),
            manifest: "x".into(),
            worker: Some("x".into()),
            worker_sha256: Some(file_sha256(&worker).unwrap()),
            default_enabled: false,
        },
        manifest,
        worker: Some((worker.clone(), worker.parent().unwrap().to_path_buf())),
        unavailable: None,
    };
    let data = root.0.join("data");
    let node = resolve_node(None).await.expect("node >= 22.13");
    let permits = node.permits.clone();
    let state = Arc::new(AppState {
        store: Mutex::new(Store::open(&data).unwrap()),
        tokens: TokenTable::load_or_create(&data).unwrap(),
        registry: Registry {
            plugins: vec![plugin],
        },
        node: Some(node),
        allowed_hosts: allowed_hosts(0),
    });
    let owner = dev_principals()
        .into_iter()
        .find(|p| p.id == "owner")
        .unwrap();

    let note = dispatch(
        state.clone(),
        owner.clone(),
        "c",
        "notes.create",
        obj(json!({"space_id": "personal", "title": "T", "markdown": "x"})),
    )
    .await
    .unwrap();
    let id = note["note"]["id"].as_str().unwrap().to_string();
    let toggle = |rid: &'static str, op: &'static str| {
        dispatch(
            state.clone(),
            owner.clone(),
            rid,
            op,
            obj(json!({"space_id": "personal", "plugin_id": "notes-insights"})),
        )
    };
    toggle("e1", "plugins.enable").await.unwrap();

    let (s2, o2, id2) = (state.clone(), owner.clone(), id.clone());
    let call = tokio::spawn(async move {
        dispatch(
            s2,
            o2,
            "s",
            "notes-insights.summarize",
            obj(json!({"space_id": "personal", "id": id2})),
        )
        .await
    });
    // The worker permit is taken only after the authorized read and
    // generation capture, so this proves the invocation is in flight.
    let t0 = std::time::Instant::now();
    while permits.available_permits() == MAX_CONCURRENT_WORKERS {
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

    // Disabled now: new invocations are denied before any worker spawns.
    let err = dispatch(
        state.clone(),
        owner.clone(),
        "s2",
        "notes-insights.summarize",
        obj(json!({"space_id": "personal", "id": id})),
    )
    .await
    .expect_err("disabled");
    assert_eq!(err.code, calternal_core::ErrorCode::Forbidden);
    assert_eq!(permits.available_permits(), MAX_CONCURRENT_WORKERS);
}
