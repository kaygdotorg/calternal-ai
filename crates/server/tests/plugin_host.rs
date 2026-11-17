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
    std::env::temp_dir().join(format!(
        "calternal-plugin-host-{tag}-{}",
        uuid::Uuid::new_v4()
    ))
}

/// Insights plugin (enabled by default) whose worker runs `on_invoke`/`on_reply`.
fn insights(dir: &Path, on_invoke: &str, on_reply: &str) -> Plugin {
    let worker = support::worker(dir, on_invoke, on_reply);
    support::plugin(support::insights_manifest(), Some(&worker), true)
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
    support::plugin(manifest, Some(&worker), false)
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
        jobs: calternal_server::plugins::Jobs::open(&data).unwrap(),
        allowed_hosts: allowed_hosts(0),
    });
    Fixture { state, root }
}

impl Fixture {
    async fn op(&self, who: &str, rid: &str, op: &str, input: Value) -> Result<Value, HostError> {
        dispatch(
            self.state.clone(),
            principal(who),
            calternal_server::plugins::Session::Static,
            rid,
            op,
            obj(input),
        )
        .await
    }
    async fn note(&self, space: &str, text: &str) -> String {
        let rid = uuid::Uuid::new_v4().to_string();
        let r = self
            .op(
                "owner",
                &rid,
                "notes.create",
                json!({"space_id": space, "title": "T", "markdown": text}),
            )
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
    std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| {
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
    assert!(
        authz::POLICIES
            .iter()
            .all(|p| !p.operation.contains("insights"))
    );

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
    );
    let account = support::plugin(
        support::manifest(
            json!({"id": "account", "operations": op("account.make", "spaces.create")}),
        ),
        None,
        false,
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
    let e = f
        .op("owner", "t0", "tagger.tag", input.clone())
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);
    f.op(
        "owner",
        "e",
        "plugins.enable",
        json!({"space_id": "shared", "plugin_id": "tagger"}),
    )
    .await
    .unwrap();
    // Enable registered the hidden kind and created the standard collection file.
    let ssd = f.root.join("data/userdata-ssd");
    assert!(
        find(&ssd, "All tags.json"),
        "standard collection not provisioned"
    );

    // The guest is a Reader in `shared`: the manifest requires content.write.
    let e = f
        .op("guest", "g", "tagger.tag", input.clone())
        .await
        .unwrap_err();
    assert_eq!(
        (e.code, e.message.as_str()),
        (ErrorCode::Forbidden, "insufficient role for this space")
    );
    // The owner's call writes through the broker into the registered kind.
    let r = f
        .op("owner", "t1", "tagger.tag", input.clone())
        .await
        .unwrap();
    let id = r["id"].as_str().unwrap().to_owned();
    let item = f
        .state
        .store
        .lock()
        .unwrap()
        .read_item("shared", "tags", &id)
        .unwrap();
    assert_eq!(item.bytes, b"urgent");
    assert!(find(&ssd, "tags"), "hidden kind folder missing");
    // A missing space hides existence exactly like a host operation.
    let e = f
        .op(
            "owner",
            "t2",
            "tagger.tag",
            json!({"space_id": uuid::Uuid::now_v7().to_string(), "label": "x"}),
        )
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
    assert_eq!(
        (e.code, e.message.as_str()),
        (ErrorCode::Unavailable, "extension request was refused")
    );
    let list = f
        .op("owner", "l", "notes.list", json!({"space_id": "personal"}))
        .await
        .unwrap();
    assert_eq!(
        list["notes"].as_array().unwrap().len(),
        1,
        "a refused call wrote"
    );
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
        "globalThis.n=(globalThis.n||0)+1;call(f,'read_item',{kind:'notes',item_id:m.input.id})",
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
    assert_eq!(
        r["view"]["root"]["children"][0]["value"],
        "LANG|ERR_ACCESS_DENIED|false"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deadline_overflow_and_malformed_output_stop_the_worker() {
    let cases = [
        (
            "hang",
            "",
            "extension timed out",
            WORKER_TIMEOUT + Duration::from_secs(2),
        ),
        (
            "flood",
            "process.stdout.write('x'.repeat(300*1024))",
            "extension output exceeded limit",
            Duration::from_secs(2),
        ),
        (
            "garbage",
            "process.stdout.write('not json\\n')",
            "extension returned invalid output",
            Duration::from_secs(2),
        ),
        (
            "schema",
            "done(f,{stats:{words:-1}})",
            "extension returned invalid output",
            Duration::from_secs(2),
        ),
    ];
    for (tag, on_invoke, message, bound) in cases {
        let dir = tmp(tag);
        let f = fixture(tag, vec![insights(&dir, on_invoke, "")], None).await;
        let id = f.note("personal", "x\n").await;
        let t0 = Instant::now();
        let e = f.summarize(&id).await.unwrap_err();
        assert_eq!(
            (e.code, e.message.as_str()),
            (ErrorCode::Unavailable, message),
            "{tag}"
        );
        assert!(t0.elapsed() < bound, "{tag} took {:?}", t0.elapsed());
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pool_capacity_is_bounded_and_released() {
    let dir = tmp("cap-worker");
    let f = fixture(
        "cap",
        vec![insights(
            &dir,
            support::READ_NOTE,
            support::COMPLETE_AFTER_GO,
        )],
        Some(1),
    )
    .await;
    let personal = f.note("personal", "a\n").await;
    let shared = f.note("shared", "b\n").await;
    let state = f.state.clone();
    let p2 = personal.clone();
    let first = tokio::spawn(async move {
        dispatch(
            state,
            principal("owner"),
            calternal_server::plugins::Session::Static,
            "c1",
            "notes-insights.summarize",
            obj(json!({"id": p2})),
        )
        .await
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
    assert_eq!(
        (e.code, e.message.as_str()),
        (ErrorCode::Unavailable, "plugin bundle failed verification")
    );
    assert_eq!(f.state.workers.live(), 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manifest_target_names_the_item_field_and_hides_foreign_items() {
    // The manifest, not the trusted entry, says `note` holds the target item ID; the
    // plugin receives its own input unchanged (minus the resolved space).
    let dir = tmp("target-worker");
    let worker = support::worker(
        &dir,
        "call(f,'read_item',{kind:'notes',item_id:m.input.note})",
        "const r=stats(m.ok?m.data.text:'');r.view.root.children[0].value=JSON.stringify(inv.get(f.invocation_id).input);done(f,r)",
    );
    let mut manifest = support::insights_manifest();
    manifest.id = "targeted".into();
    let op = &mut manifest.operations[0];
    op.id = "targeted.count".into();
    op.input_schema = json!({"type": "object", "required": ["note", "mode"], "additionalProperties": false,
        "properties": {"note": {"type": "string"}, "mode": {"type": "string"}}});
    op.target = Some(calternal_core::ManifestTarget {
        kind: "notes".into(),
        field: "note".into(),
    });
    let plugin = support::plugin(manifest, Some(&worker), true);
    let f = fixture("target", vec![plugin], None).await;
    assert_eq!(
        f.state.registry.policy("targeted.count").unwrap().scope,
        Scope::Note
    );
    let own = f.note("personal", "one two three\n").await;
    let r = f
        .op(
            "owner",
            "t1",
            "targeted.count",
            json!({"note": own, "mode": "fast"}),
        )
        .await
        .unwrap();
    assert_eq!(r["stats"]["words"], 3);
    // Compare as JSON, not text: key order depends on serde_json's `preserve_order`
    // feature, which other workspace crates may switch on.
    let echoed: Value =
        serde_json::from_str(r["view"]["root"]["children"][0]["value"].as_str().unwrap()).unwrap();
    assert_eq!(echoed, json!({"note": own, "mode": "fast"}));
    // A note in a space the caller cannot read answers exactly like a missing one.
    let missing = uuid::Uuid::now_v7().to_string();
    let absent = f
        .op(
            "guest",
            "t2",
            "targeted.count",
            json!({"note": missing, "mode": "fast"}),
        )
        .await
        .unwrap_err();
    let foreign = f
        .op(
            "guest",
            "t3",
            "targeted.count",
            json!({"note": own, "mode": "fast"}),
        )
        .await
        .unwrap_err();
    assert_eq!(absent.code, ErrorCode::NotFound);
    assert_eq!(
        (foreign.code, foreign.message),
        (absent.code, absent.message)
    );
    // The ID must be in the declared field; `id` is not a fallback.
    let wrong = f
        .op(
            "owner",
            "t4",
            "targeted.count",
            json!({"id": own, "mode": "fast"}),
        )
        .await
        .unwrap_err();
    assert_eq!(wrong.code, ErrorCode::InvalidInput);
    let _ = std::fs::remove_dir_all(dir);
}

/// Whether any file under `root` has extension `ext`.
fn has_extension(root: &Path, ext: &str) -> bool {
    std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| {
            let p = e.path();
            p.extension().is_some_and(|x| x == ext) || (p.is_dir() && has_extension(&p, ext))
        })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_text_kind_is_enabled_and_stored_under_its_extension() {
    let dir = tmp("blob-worker");
    let worker = support::worker(
        &dir,
        "call(f,'write_item',{kind:'shapes',text:m.input.body})",
        "done(f,m.ok?{id:m.data.id}:{})",
    );
    let manifest = support::manifest(json!({
        "id": "shapes",
        "kinds": [{"id": "shapes", "storage": {"hidden": true}, "claimed_text": false,
                   "extension": "geojson", "schema": {"type": "object"}}],
        "operations": [{
            "id": "shapes.save",
            "input_schema": {"type": "object", "properties": {"body": {"type": "string"}}, "required": ["body"]},
            "output_schema": {"type": "object"},
            "required_capability": "content.write", "mutates": true, "idempotency": "request_id"
        }],
        "host_calls": ["write_item"]
    }));
    let f = fixture(
        "shapes",
        vec![support::plugin(manifest, Some(&worker), false)],
        None,
    )
    .await;
    f.op(
        "owner",
        "e",
        "plugins.enable",
        json!({"space_id": "shared", "plugin_id": "shapes"}),
    )
    .await
    .unwrap();
    let body = r#"{"type":"Point","coordinates":[0,0]}"#;
    let r = f
        .op(
            "owner",
            "s1",
            "shapes.save",
            json!({"space_id": "shared", "body": body}),
        )
        .await
        .unwrap();
    let id = r["id"].as_str().unwrap().to_owned();
    let item = f
        .state
        .store
        .lock()
        .unwrap()
        .read_item("shared", "shapes", &id)
        .unwrap();
    assert_eq!(
        item.bytes,
        body.as_bytes(),
        "opaque bytes are stored unchanged"
    );
    assert!(
        has_extension(&f.root.join("data"), "geojson"),
        "file not stored under its extension"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_calls_recheck_the_callers_sign_in() {
    // An installation session that is no longer live (here: unknown to the directory)
    // is admitted by the policy gate but refused at the worker's first host call, so
    // the write never lands.
    let f = fixture("session", vec![tagger(&tmp("session-worker"))], None).await;
    f.op(
        "owner",
        "e",
        "plugins.enable",
        json!({"space_id": "shared", "plugin_id": "tagger"}),
    )
    .await
    .unwrap();
    let gone = calternal_server::plugins::Session::Installation {
        account: uuid::Uuid::now_v7().to_string(),
        installation: uuid::Uuid::now_v7().to_string(),
    };
    let e = calternal_server::api::dispatch(
        f.state.clone(),
        principal("owner"),
        gone,
        "s1",
        "tagger.tag",
        obj(json!({"space_id": "shared", "label": "late"})),
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::Unauthorized);
    assert!(
        !find(&f.root.join("data/userdata-ssd"), "late.md"),
        "write landed without a live sign-in"
    );
    // The same call with a live (static) credential writes.
    f.op(
        "owner",
        "s2",
        "tagger.tag",
        json!({"space_id": "shared", "label": "now"}),
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mutating_operation_retry_replays_its_receipt() {
    let f = fixture("receipt", vec![tagger(&tmp("receipt-worker"))], None).await;
    f.op(
        "owner",
        "e",
        "plugins.enable",
        json!({"space_id": "shared", "plugin_id": "tagger"}),
    )
    .await
    .unwrap();
    let input = json!({"space_id": "shared", "label": "once"});
    let first = f
        .op("owner", "r1", "tagger.tag", input.clone())
        .await
        .unwrap();
    // Same request ID and input: the original result, and the worker is not run again
    // (a second run would allocate a second item ID).
    let again = f.op("owner", "r1", "tagger.tag", input).await.unwrap();
    assert_eq!(first, again);
    // Same request ID, different input: a conflict, nothing written.
    let e = f
        .op(
            "owner",
            "r1",
            "tagger.tag",
            json!({"space_id": "shared", "label": "other"}),
        )
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Conflict);
    assert!(!find(&f.root.join("data/userdata-ssd"), "other.md"));
}

/// The test-only jobs plugin from `tests/plugins/jobs-demo`, digest pinned.
fn jobs_demo() -> Plugin {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/plugins/jobs-demo");
    let manifest =
        calternal_plugin_host::parse_manifest(&std::fs::read(dir.join("manifest.json")).unwrap())
            .unwrap();
    support::plugin(manifest, Some(&dir.join("worker.js")), false)
}

async fn jobs_fixture(tag: &str) -> Fixture {
    let f = fixture(tag, vec![jobs_demo()], None).await;
    for space in ["shared", "personal"] {
        f.op(
            "owner",
            &format!("enable-{space}"),
            "plugins.enable",
            json!({"space_id": space, "plugin_id": "jobs-demo"}),
        )
        .await
        .unwrap();
    }
    f
}

impl Fixture {
    async fn start_job(&self, space: &str, input: Value) -> String {
        let mut input = input;
        input["space_id"] = json!(space);
        let rid = uuid::Uuid::new_v4().to_string();
        let r = self
            .op("owner", &rid, "jobs-demo.start", input)
            .await
            .unwrap();
        r["job_id"].as_str().unwrap().to_owned()
    }
    async fn job(&self, who: &str, id: &str) -> Result<Value, HostError> {
        self.op(
            who,
            &uuid::Uuid::new_v4().to_string(),
            "jobs.get",
            json!({"job_id": id}),
        )
        .await
        .map(|r| r["job"].clone())
    }
    async fn until(&self, id: &str, state: &str) -> Value {
        let end = Instant::now() + Duration::from_secs(20);
        loop {
            let job = self.job("owner", id).await.unwrap();
            if job["state"] == state {
                return job;
            }
            assert!(Instant::now() < end, "job never reached {state}: {job}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn job_is_durable_streams_in_order_checkpoints_and_finishes() {
    let f = jobs_fixture("job-run").await;
    let id = f
        .start_job("shared", json!({"steps": 6, "delay_ms": 30, "every": 2}))
        .await;
    // Durable before the ID was returned: the record exists in the space's job file.
    let space = f.state.jobs.route(&id).expect("job routed to its space");
    assert!(
        f.root
            .join("data/spaces")
            .join(&space)
            .join("jobs.db")
            .is_file()
    );
    let sub = f.state.jobs.subscribe(&id).unwrap();
    let done = f.until(&id, "done").await;
    assert_eq!(
        done["result"],
        json!({"steps": 6, "resumed_from": 0, "value": 6})
    );
    assert_eq!(done["checkpoint"], json!({"step": 6}));
    assert_eq!(done["checkpoint_seq"], 6);
    // Chunks carry consecutive host sequence numbers in emission order.
    let batch = sub.live.read(0);
    let seqs: Vec<u64> = batch.chunks.iter().map(|c| c.seq).collect();
    assert_eq!(seqs, (0..6).collect::<Vec<_>>());
    for c in &batch.chunks {
        assert_eq!(
            serde_json::from_str::<Value>(&c.data).unwrap(),
            json!({"step": c.seq})
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_discards_late_output_and_needs_the_starter_or_a_manager() {
    let f = jobs_fixture("job-cancel").await;
    let id = f
        .start_job("shared", json!({"steps": 500, "delay_ms": 20, "every": 5}))
        .await;
    f.until(&id, "running").await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    // The guest reads `shared` but lacks plugins.use: it may watch, not cancel.
    assert_eq!(f.job("guest", &id).await.unwrap()["state"], "running");
    let e = f
        .op("guest", "gc", "jobs.cancel", json!({"job_id": id}))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);
    let r = f
        .op("owner", "oc", "jobs.cancel", json!({"job_id": id}))
        .await
        .unwrap();
    assert_eq!(r["job"]["state"], "cancelled");
    let at_cancel = f.job("owner", &id).await.unwrap();
    let seq = f.state.jobs.live(&id).read(0).chunks.last().map(|c| c.seq);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let later = f.job("owner", &id).await.unwrap();
    assert_eq!(later, at_cancel, "a cancelled job's record changed");
    assert!(
        f.state.jobs.live(&id).read(0).chunks.last().map(|c| c.seq) <= seq,
        "late chunks were accepted after cancel"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn child_jobs_report_exactly_once_to_their_parent() {
    let f = jobs_fixture("job-children").await;
    let id = f
        .start_job(
            "shared",
            json!({"steps": 0, "delay_ms": 0, "every": 1, "children": 3}),
        )
        .await;
    let done = f.until(&id, "done").await;
    assert_eq!(done["result"], json!({"sum": 6, "reports": 3}));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_job_outside_the_callers_spaces_is_not_found() {
    let f = jobs_fixture("job-hidden").await;
    // `personal` belongs to the owner alone; the guest is a member of `shared` only.
    let id = f
        .start_job("personal", json!({"steps": 1, "delay_ms": 0, "every": 1}))
        .await;
    let hidden = f.job("guest", &id).await.unwrap_err();
    let missing = f
        .job("guest", &uuid::Uuid::now_v7().to_string())
        .await
        .unwrap_err();
    assert_eq!(hidden.code, ErrorCode::NotFound);
    assert_eq!(
        (hidden.code, hidden.message),
        (missing.code, missing.message)
    );
    let cancel = f
        .op("guest", "hc", "jobs.cancel", json!({"job_id": id}))
        .await
        .unwrap_err();
    assert_eq!(cancel.code, ErrorCode::NotFound);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn job_start_retry_returns_the_same_job() {
    let f = jobs_fixture("job-retry").await;
    let input = json!({"space_id": "shared", "steps": 1, "delay_ms": 0, "every": 1});
    let a = f
        .op("owner", "same", "jobs-demo.start", input.clone())
        .await
        .unwrap();
    let b = f
        .op("owner", "same", "jobs-demo.start", input)
        .await
        .unwrap();
    assert_eq!(a, b);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn secret_handle_requests_are_owned_by_the_space_uuid() {
    use calternal_secrets::{Owner, SecretStore};
    let dir = tmp("secret-worker");
    let worker = support::worker(
        &dir,
        "call(f,'request_secret_handle',{name:'api-key'})",
        "done(f,{ok:m.ok})",
    );
    let manifest = support::manifest(json!({
        "id": "keyed",
        "secrets": ["api-key"],
        "operations": [{
            "id": "keyed.use",
            "input_schema": {"type": "object"},
            "output_schema": {"type": "object"},
            "required_capability": "plugins.use", "mutates": false, "idempotency": "read_only"
        }],
        "host_calls": ["request_secret_handle"]
    }));
    let f = fixture("secret", vec![support::plugin(manifest, Some(&worker), true)], None).await;
    // The secret does not exist yet: the call is refused and one request is recorded
    // for the space (by UUID), not for the content-space key or the account.
    let r = f.op("owner", "k1", "keyed.use", json!({"space_id": "shared"})).await.unwrap();
    assert_eq!(r, json!({"ok": false}));
    f.op("owner", "k2", "keyed.use", json!({"space_id": "shared"})).await.unwrap();
    let (_, space) = f.state.store.lock().unwrap().worker_ids("owner", "shared").unwrap();
    let owner = Owner::Space(calternal_auth::SpaceId::parse(&space).unwrap());
    let secrets = f.state.store.lock().unwrap().secrets();
    let pending = secrets.pending(&owner).unwrap();
    assert_eq!(pending.len(), 1, "a repeated request must not queue a second prompt");
    assert_eq!(pending[0].name, "plugin:keyed:api-key");
    let _ = std::fs::remove_dir_all(dir);
}
