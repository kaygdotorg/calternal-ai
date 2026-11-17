//! Server wiring of the plugin host: manifest-derived policy, provisioning on
//! enable, brokered host calls, and long-lived bounded workers.

mod support;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use calternal_core::{ErrorCode, JsonObject};
use calternal_server::api::{AppState, Shared, allowed_hosts, dispatch};
use calternal_server::auth::{DevPrincipal, TokenTable, dev_principals};
use calternal_server::authz::{self, Capability, Constraint, Scope};
use calternal_server::error::HostError;
use calternal_server::plugins::{Plugin, Registry, WORKER_TIMEOUT, WorkerPool, resolve_node};
use calternal_server::store::Store;
use serde_json::{Value, json};

struct Fixture {
    state: Shared<TokenTable>,
    root: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn obj(v: Value) -> JsonObject {
    serde_json::from_value(v).unwrap()
}

fn principal(id: &str) -> DevPrincipal {
    dev_principals().into_iter().find(|p| p.id == id).unwrap()
}

fn tmp(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("calternal-plugin-host-{tag}-{}", uuid::Uuid::new_v4()))
}

const SUMMARIZE: &[(&str, &str, &str)] = &[("notes-insights.summarize", "notes", "note_id")];

/// Insights plugin (enabled by default) whose worker runs `on_invoke`/`on_reply`.
fn insights(dir: &Path, on_invoke: &str, on_reply: &str) -> Plugin {
    let worker = support::worker(dir, on_invoke, on_reply);
    support::plugin(support::insights_manifest(), Some(&worker), true, SUMMARIZE)
}

/// A plugin with its own hidden kind, a standard collection and a mutating
/// operation that writes one item through the broker.
fn tagger(dir: &Path) -> Plugin {
    let worker = support::worker(
        dir,
        "call(f,'write_item',{kind:'tags',text:m.input.label,metadata:{label:m.input.label}})",
        "done(f,m.ok?{id:m.data.id}:{})",
    );
    let manifest = support::manifest(json!({
        "id": "tagger",
        "kinds": [{"id": "tags", "storage": {"hidden": true}, "claimed_text": true, "schema": {"type": "object"}}],
        "collections": [{"id": "all-tags", "kind": "tags", "name": "All tags"}],
        "operations": [{
            "id": "tagger.tag",
            "input_schema": {"type": "object", "properties": {"label": {"type": "string"}}, "required": ["label"], "additionalProperties": false},
            "output_schema": {"type": "object"},
            "required_capability": "content.write",
            "mutates": true,
            "idempotency": "request_id"
        }],
        "host_calls": ["write_item"]
    }));
    support::plugin(manifest, Some(&worker), false, &[])
}

async fn fixture(tag: &str, plugins: Vec<Plugin>, pool: Option<usize>) -> Fixture {
    let root = tmp(tag);
    let data = root.join("data");
    std::fs::create_dir_all(&data).unwrap();
    let node = resolve_node(None).await.expect("node >= 22.13 on PATH");
    let workers = match pool {
        Some(cap) => WorkerPool::with_capacity(Some(node), cap),
        None => WorkerPool::new(Some(node)),
    };
    let state = Arc::new(AppState {
        store: Mutex::new(Store::open(&data).unwrap()),
        tokens: TokenTable::load_or_create(&data).unwrap(),
        registry: Registry::from_plugins(plugins),
        workers,
        allowed_hosts: allowed_hosts(0),
    });
    Fixture { state, root }
}

impl Fixture {
    async fn op(&self, who: &str, rid: &str, op: &str, input: Value) -> Result<Value, HostError> {
        dispatch(self.state.clone(), principal(who), rid, op, obj(input)).await
    }
    async fn note(&self, space: &str, text: &str) -> String {
        let rid = uuid::Uuid::new_v4().to_string();
        let r = self
            .op("owner", &rid, "notes.create", json!({"space_id": space, "title": "T", "markdown": text}))
            .await
            .unwrap();
        r["note"]["id"].as_str().unwrap().to_owned()
    }
    async fn summarize(&self, id: &str) -> Result<Value, HostError> {
        let rid = uuid::Uuid::new_v4().to_string();
        self.op("owner", &rid, "notes-insights.summarize", json!({"id": id}))
            .await
    }
}

fn find(root: &Path, name: &str) -> bool {
    std::fs::read_dir(root).into_iter().flatten().flatten().any(|e| {
        let p = e.path();
        p.file_name().is_some_and(|n| n == name) || (p.is_dir() && find(&p, name))
    })
}

#[test]
fn policy_rows_are_derived_from_manifests() {
    let dir = tmp("policy");
    let registry = Registry::from_plugins(vec![insights(&dir, "", ""), tagger(&dir.join("t"))]);
    // No plugin row is written into the host table.
    assert!(authz::policy("notes-insights.summarize").is_none());
    assert!(authz::POLICIES.iter().all(|p| !p.operation.contains("insights")));

    let summarize = registry.policy("notes-insights.summarize").unwrap();
    assert_eq!(summarize.required, Capability::SpaceRead);
    assert_eq!(summarize.scope, Scope::Note);
    assert_eq!(summarize.constraint, Constraint::PluginInvocation);
    let tag = registry.policy("tagger.tag").unwrap();
    assert_eq!(tag.required, Capability::ContentWrite);
    assert_eq!(tag.scope, Scope::Space);
    assert_eq!(registry.plugin_policies().count(), 2);
    // Host rows still resolve through the same lookup.
    assert_eq!(
        registry.policy("notes.list").unwrap().required,
        Capability::SpaceRead
    );

    // A plugin may not shadow a host operation or demand an account-level capability.
    let op = |id: &str, cap: &str| {
        json!([{"id": id, "input_schema": {}, "output_schema": {}, "required_capability": cap,
                "mutates": false, "idempotency": "read_only"}])
    };
    let shadow = support::plugin(
        support::manifest(json!({"id": "shadow", "operations": op("notes.list", "space.read")})),
        None,
        false,
        &[],
    );
    let account = support::plugin(
        support::manifest(json!({"id": "account", "operations": op("account.make", "spaces.create")})),
        None,
        false,
        &[],
    );
    let registry = Registry::from_plugins(vec![shadow, account]);
    assert!(registry.plugins().is_empty());
    assert!(registry.owner_of_operation("notes.list").is_none());
    assert!(registry.policy("account.make").is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manifest_policy_gates_dispatch_and_enable_provisions_kinds_and_collections() {
    let f = fixture("tagger", vec![tagger(&tmp("tagger-worker"))], None).await;
    let input = json!({"space_id": "shared", "label": "urgent"});
    // Disabled by default: admitted by policy, then refused as disabled.
    let e = f.op("owner", "t0", "tagger.tag", input.clone()).await.unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);
    f.op("owner", "e", "plugins.enable", json!({"space_id": "shared", "plugin_id": "tagger"}))
        .await
        .unwrap();
    // Enable registered the hidden kind and created the standard collection file.
    let ssd = f.root.join("data/userdata-ssd");
    assert!(find(&ssd, "All tags.json"), "standard collection not provisioned");

    // The guest is a Reader in `shared`: the manifest requires content.write.
    let e = f.op("guest", "g", "tagger.tag", input.clone()).await.unwrap_err();
    assert_eq!((e.code, e.message.as_str()), (ErrorCode::Forbidden, "insufficient role for this space"));
    // The owner's call writes through the broker into the registered kind.
    let r = f.op("owner", "t1", "tagger.tag", input.clone()).await.unwrap();
    let id = r["id"].as_str().unwrap().to_owned();
    let item = f.state.store.lock().unwrap().read_item("shared", "tags", &id).unwrap();
    assert_eq!(item.bytes, b"urgent");
    assert!(find(&ssd, "tags"), "hidden kind folder missing");
    // A missing space hides existence exactly like a host operation.
    let e = f
        .op("owner", "t2", "tagger.tag", json!({"space_id": uuid::Uuid::now_v7().to_string(), "label": "x"}))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undeclared_host_call_is_refused_before_any_effect() {
    // notes-insights declares only read_item; this worker tries to write a note.
    let dir = tmp("undeclared-worker");
    let plugin = insights(
        &dir,
        "call(f,'write_item',{kind:'notes',text:'injected',metadata:{title:'x'}})",
        "done(f,stats('should not be delivered'))",
    );
    let f = fixture("undeclared", vec![plugin], None).await;
    let id = f.note("personal", "hello world\n").await;
    let e = f.summarize(&id).await.unwrap_err();
    assert_eq!((e.code, e.message.as_str()), (ErrorCode::Unavailable, "extension request was refused"));
    let list = f
        .op("owner", "l", "notes.list", json!({"space_id": "personal"}))
        .await
        .unwrap();
    assert_eq!(list["notes"].as_array().unwrap().len(), 1, "a refused call wrote");
    // The same worker keeps serving well-behaved invocations of other kinds of call.
    assert_eq!(f.state.workers.live(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_long_lived_worker_per_plugin_and_space() {
    // The worker counts invocations in memory: the second call sees 2.
    let dir = tmp("reuse-worker");
    let plugin = insights(
        &dir,
        "globalThis.n=(globalThis.n||0)+1;call(f,'read_item',{kind:'notes',item_id:m.input.note_id})",
        "const r=stats(m.data.text);r.stats.words=globalThis.n;done(f,r)",
    );
    let f = fixture("reuse", vec![plugin], None).await;
    let personal = f.note("personal", "a b c\n").await;
    let shared = f.note("shared", "a b c\n").await;
    assert_eq!(f.summarize(&personal).await.unwrap()["stats"]["words"], 1);
    assert_eq!(f.summarize(&personal).await.unwrap()["stats"]["words"], 2);
    assert_eq!(f.state.workers.live(), 1);
    // Another space gets its own process and state.
    assert_eq!(f.summarize(&shared).await.unwrap()["stats"]["words"], 1);
    assert_eq!(f.state.workers.live(), 2);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worker_reads_only_its_own_space_and_has_no_ambient_access() {
    // The worker ignores its input and probes: a foreign note ID planted in its
    // bundle directory, its environment, and a file outside that directory.
    let dir = tmp("probe-worker");
    let plugin = insights(
        &dir,
        "call(f,'read_item',{kind:'notes',item_id:fs.readFileSync(__dirname+'/foreign','utf8')})",
        "const env=Object.keys(process.env).sort().join(',');let fsr;try{fs.readFileSync('/etc/hostname');fsr='read'}catch(e){fsr=e.code}const r=stats(m.ok?m.data.text:'');r.view.root.children[0].value=env+'|'+fsr+'|'+m.ok;done(f,r)",
    );
    let f = fixture("probe", vec![plugin], None).await;
    let foreign = f.note("shared", "secret words here\n").await;
    let own = f.note("personal", "mine\n").await;
    std::fs::write(dir.join("foreign"), &foreign).unwrap();
    let r = f.summarize(&own).await.unwrap();
    assert_eq!(r["stats"]["words"], 0, "foreign text reached the worker");
    assert_eq!(r["view"]["root"]["children"][0]["value"], "LANG|ERR_ACCESS_DENIED|false");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deadline_overflow_and_malformed_output_stop_the_worker() {
    let cases = [
        ("hang", "", "extension timed out", WORKER_TIMEOUT + Duration::from_secs(2)),
        ("flood", "process.stdout.write('x'.repeat(300*1024))", "extension output exceeded limit", Duration::from_secs(2)),
        ("garbage", "process.stdout.write('not json\\n')", "extension returned invalid output", Duration::from_secs(2)),
        ("schema", "done(f,{stats:{words:-1}})", "extension returned invalid output", Duration::from_secs(2)),
    ];
    for (tag, on_invoke, message, bound) in cases {
        let dir = tmp(tag);
        let f = fixture(tag, vec![insights(&dir, on_invoke, "")], None).await;
        let id = f.note("personal", "x\n").await;
        let t0 = Instant::now();
        let e = f.summarize(&id).await.unwrap_err();
        assert_eq!((e.code, e.message.as_str()), (ErrorCode::Unavailable, message), "{tag}");
        assert!(t0.elapsed() < bound, "{tag} took {:?}", t0.elapsed());
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pool_capacity_is_bounded_and_released() {
    let dir = tmp("cap-worker");
    let f = fixture("cap", vec![insights(&dir, support::READ_NOTE, support::COMPLETE_AFTER_GO)], Some(1)).await;
    let personal = f.note("personal", "a\n").await;
    let shared = f.note("shared", "b\n").await;
    let state = f.state.clone();
    let p2 = personal.clone();
    let first = tokio::spawn(async move {
        dispatch(state, principal("owner"), "c1", "notes-insights.summarize", obj(json!({"id": p2}))).await
    });
    let t0 = Instant::now();
    while f.state.workers.in_flight() == 0 {
        assert!(t0.elapsed() < Duration::from_secs(5));
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let busy = f.summarize(&shared).await.unwrap_err();
    assert_eq!(busy.details.unwrap()["reason"], "worker_capacity");
    std::fs::write(dir.join("go"), "").unwrap();
    first.await.unwrap().unwrap();
    // The idle worker is evicted to make room for the other space.
    f.summarize(&shared).await.unwrap();
    assert_eq!(f.state.workers.live(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tampered_bundle_is_refused_before_start() {
    let dir = tmp("digest-worker");
    let plugin = insights(&dir, support::READ_NOTE, support::COMPLETE);
    std::fs::write(dir.join("worker.js"), "process.exit(0)").unwrap();
    let f = fixture("digest", vec![plugin], None).await;
    let id = f.note("personal", "x\n").await;
    let e = f.summarize(&id).await.unwrap_err();
    assert_eq!((e.code, e.message.as_str()), (ErrorCode::Unavailable, "plugin bundle failed verification"));
    assert_eq!(f.state.workers.live(), 0);
    let _ = std::fs::remove_dir_all(dir);
}
