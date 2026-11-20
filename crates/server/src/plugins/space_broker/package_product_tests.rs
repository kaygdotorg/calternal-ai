//! Live enablement of v2 item callbacks: a package installed through the
//! real archive path, activated in a space, invoked through the router's
//! dispatch and run by an enrolled machine runner over the TLS carrier.
use super::*;
use crate::{
    api::{AppState, allowed_hosts, dispatch, router},
    auth::StoredDirectory,
    plugins::{Jobs, WorkerPool, package_store},
    store::Store,
};
use calternal_machine::{
    protocol::{Capacity, Owner, SyncRequest},
    runner::Runner,
    sandbox::Backend,
};
use calternal_types::ItemRef;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;

const MANIFEST: &[u8] = include_bytes!(
    "../../../../../contract/fixtures/description-v2/manifests/reading-list-installer-v2-app.json"
);
const TEMPLATE: &[u8] = include_bytes!(
    "../../../../../contract/fixtures/description-v2/reading-list-installer-v2-app.json"
);

/// Child-pipe worker. It never sees the ticket, epoch or space, and builds
/// its subeffect request IDs from the host-stamped author context.
const WORKER: &str = r#"
setInterval(() => {}, 1000);
const crypto = require('node:crypto');
const lines = require('node:readline').createInterface({input: process.stdin});
const pending = new Map();
const send = (invocation, message) => process.stdout.write(JSON.stringify({
  protocol_version: 2, invocation_id: invocation, message
}) + '\n');
const call = (invocation, method, input) => new Promise(resolve => {
  const id = crypto.randomUUID();
  pending.set(id, resolve);
  send(invocation, {type: 'host_call', call_id: id, method, input});
});
const ok = reply => { if (!reply.Ok) throw new Error(JSON.stringify(reply)); return reply.Ok; };
const b64 = text => Buffer.from(text).toString('base64');
async function run(invocation, m) {
  if ('ticket' in m || 'space_id' in m.input || !m.author_context.request_id) throw new Error('scope');
  const id = m.author_context.request_id, kind = 'reading-item';
  if (m.input.title.startsWith('Captured')) {
    const made = ok(await call(invocation, 'item_create', {
      request_id: id + '/create', kind, name: 'Captured.md', metadata: {title: m.input.title},
      attempt_discovery: 'creator_only',
      attested: {source: {kind: 'input'}, prefix: '# ', suffix: '\n', entry_anchor: 'm-1'}
    }));
    const spans = ok(await call(invocation, 'item_spans', {item: made.item, revision: made.revision}));
    if (spans.body_provenance !== 'attested' || spans.spans.length !== 1
        || spans.spans[0].span !== made.span || spans.spans[0].attribution.actor !== m.author_context.actor)
      throw new Error('spans');
    return {created: made.item};
  }
  const refused = await call(invocation, 'item_create', {
    request_id: id + '/attested', kind, name: 'Refused.md', metadata: {title: m.input.title},
    attested: {source: {kind: 'input'}}
  });
  if (refused.Err !== 'forbidden') throw new Error('uncaptured source was accepted');
  const made = ok(await call(invocation, 'item_create', {
    request_id: id + '/create', kind, name: 'Plain.md', metadata: {title: m.input.title}, body: b64('Hello')
  }));
  const added = ok(await call(invocation, 'item_append', {
    request_id: id + '/append', item: made.item, expected_revision: made.revision, bytes: b64(' world')
  }));
  const head = ok(await call(invocation, 'item_head', {item: made.item}));
  if (head.bytes !== 11 || head.revision !== added.revision || added.end_offset !== 11) throw new Error('head');
  const page = ok(await call(invocation, 'item_range', {
    item: made.item, revision: made.revision, offset: 0, length: 64
  }));
  if (page.bytes !== b64('Hello')) throw new Error('range');
  return {created: made.item};
}
lines.on('line', line => {
  const frame = JSON.parse(line), m = frame.message;
  if (m.type === 'invoke') {
    run(frame.invocation_id, m).then(
      data => send(frame.invocation_id, {type: 'complete', data}),
      error => { process.stderr.write(String(error) + '\n'); send(frame.invocation_id, {type: 'failed'}); });
  } else if (m.type === 'host_reply' && pending.has(m.call_id)) {
    const resolve = pending.get(m.call_id);
    pending.delete(m.call_id);
    resolve(m.result);
  }
});
"#;

/// Build and install the archive through the operator path into `appdata`.
fn install(appdata: &std::path::Path, source: &std::path::Path) {
    let mut manifest: Value = serde_json::from_slice(MANIFEST).unwrap();
    let digest = sha256_hex(WORKER.as_bytes());
    manifest["worker"] = json!({"path":"worker.js", "sha256": digest});
    let kind = manifest["kinds"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|kind| kind["id"] == "reading-item")
        .unwrap();
    kind["append_file"] = json!(true);
    kind["attempt_discovery"] = json!("item_policy");
    let add = manifest["operations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|operation| operation["id"] == "reading.add")
        .unwrap();
    add["methods"] = json!([
        "item_create",
        "item_head",
        "item_range",
        "item_spans",
        "item_append"
    ]);
    add["write_origins"] = json!({"host_input":"title"});
    let template = manifest["apps"][0]["description"]["template"]["path"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::write(source.join(template), TEMPLATE).unwrap();
    std::fs::write(source.join("worker.js"), WORKER).unwrap();
    std::fs::write(
        source.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let package_store::InstallPackage::V2(package, _) =
        package_store::InstallPackage::read(source).unwrap()
    else {
        panic!("expected a v2 package");
    };
    let archive = package.archive().unwrap();
    let checked = package_store::validate_v2_archive(&archive).unwrap();
    package_store::commit_v2(
        appdata,
        checked.id(),
        &archive,
        &package.checksum().unwrap(),
    )
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn enrolled_package_writes_and_reads_items_through_live_callbacks() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let root = tempfile::tempdir().unwrap();
    let relay = TlsRelay::start(root.path(), address).await;
    let (state, service, ids, machine, token) = tokio::task::block_in_place(|| {
        let roots = calternal_roots::Roots {
            appdata: root.path().join("appdata"),
            userdata_ssd: root.path().join("ssd"),
            userdata_hdd: root.path().join("hdd"),
        };
        let store = Store::open(
            &roots,
            Uuid::new_v4(),
            true,
            calternal_content::DEFAULT_RECEIPT_RETENTION,
        )
        .unwrap();
        let source = root.path().join("package");
        std::fs::create_dir(&source).unwrap();
        install(&roots.appdata, &source);
        let distribution = root.path().join("distribution");
        std::fs::create_dir(&distribution).unwrap();
        std::fs::write(
            distribution.join("trusted-plugins.json"),
            r#"{"plugins":[]}"#,
        )
        .unwrap();
        let registry =
            package_store::load(&roots.appdata, Some(&distribution), None, false).unwrap();
        let directory = StoredDirectory::load_or_create(&roots.appdata).unwrap();
        let ids = directory.spaces.seed_development().unwrap();
        let service = MachineService::open(
            &roots.appdata,
            directory.spaces.machines(),
            crate::machines::Settings {
                poll_hold: Duration::from_millis(20),
                ..Default::default()
            },
        )
        .unwrap();
        store.bind_machines(&service);
        service.set_pool_grant(ids.owner, true).unwrap();
        let state = Arc::new(AppState {
            inputs: store.account_inputs(),
            store,
            directory,
            registry,
            blocking: crate::blocking::RequestWorkers::new().unwrap(),
            workers: WorkerPool::with_local_root(None, 1, root.path().join("workers")),
            jobs: Jobs::disabled(),
            allowed_hosts: allowed_hosts(relay.port),
        });
        let (machine, token) = product_http::enroll(&service, Owner::Pool);
        (state, service, ids, machine, token)
    });
    let api = router(state.clone(), None).layer(axum::Extension(service.clone()));
    let server = tokio::spawn(async move { axum::serve(listener, api).await.unwrap() });
    let link = Arc::new(relay.link);
    link.set_token(Some(Arc::from(token.as_str())));
    let backend: Arc<dyn Backend> = Arc::new(FixtureBackend::new(
        &root.path().join("placements"),
        relay.node,
    ));
    let runner = Arc::new(Runner::new(backend, link, 2));
    let sync_runner = runner.clone();
    let sync_service = service.clone();
    let supervisor = tokio::spawn(async move {
        let mut ack = 0;
        loop {
            let report = SyncRequest {
                ack,
                draining: false,
                discards: vec![],
                leases: sync_runner.reports(),
                capacity: Capacity {
                    cpus: 1,
                    small_plugin_vm: true,
                    available_memory_bytes: Some(64 << 30),
                    max_sandboxes: 2,
                    ..Default::default()
                },
            };
            for envelope in sync_service.sync(machine, &token, report).await.unwrap() {
                ack = ack.max(envelope.seq);
                sync_runner.handle(envelope.message).await;
            }
        }
    });
    let call = |request: &'static str, operation: &'static str, input: Value| {
        dispatch(
            state.clone(),
            Identity { id: ids.owner },
            Session::Static,
            request,
            operation,
            serde_json::from_value(input).unwrap(),
        )
    };
    let add = json!({"space_id":ids.personal,"title":"Plain","url":"https://example.com/a"});
    assert_eq!(
        call("before", "reading.add", add.clone())
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound,
        "an installed package does nothing until the space enables it"
    );
    let enabled = call(
        "enable",
        "plugins.enable",
        json!({"space_id":ids.personal,"plugin_id":"calternal-reading-list"}),
    )
    .await
    .unwrap();
    assert_eq!(enabled["plugin"]["enabled"], true);

    let first = tokio::time::timeout(
        Duration::from_secs(30),
        call("first", "reading.add", add.clone()),
    )
    .await
    .unwrap()
    .unwrap();
    let created = ItemRef(first["created"].as_str().unwrap().to_owned());
    let content = state.store.shared_content();
    let local = content
        .resolve_item_ref(ids.personal, "reading-item", &created)
        .unwrap();
    let item = content.get(ids.personal, local).unwrap();
    assert_eq!(item.bytes, b"Hello world");
    // Raw plugin bytes are opaque under 14.36: protected and unproven.
    assert!(
        !content
            .creator_discoverable(ids.personal, local, "reading-item", ids.owner)
            .unwrap()
    );
    assert!(
        !content
            .item_spans(
                ids.personal,
                "reading-item",
                local,
                &item.revision,
                None,
                128
            )
            .unwrap()
            .attested
    );
    // A lost reply retried with the same request replays every subeffect.
    assert_eq!(
        call("first", "reading.add", add.clone()).await.unwrap(),
        first
    );
    assert_eq!(
        content.get(ids.personal, local).unwrap().revision,
        item.revision
    );

    // The checked composer route supplies the capture; the worker never
    // sends the middle bytes.
    let captured =
        json!({"space_id":ids.personal,"title":"Captured title","url":"https://example.com/b"});
    let second = super::package::invoke(
        state.clone(),
        Identity { id: ids.owner },
        Session::Static,
        "second",
        "reading.add",
        serde_json::from_value(captured).unwrap(),
        None,
        Some("Captured title".into()),
    )
    .await
    .unwrap();
    let attested = content
        .resolve_item_ref(
            ids.personal,
            "reading-item",
            &ItemRef(second["created"].as_str().unwrap().to_owned()),
        )
        .unwrap();
    let body = content.get(ids.personal, attested).unwrap();
    assert_eq!(body.bytes, b"# Captured title\n");
    let page = content
        .item_spans(
            ids.personal,
            "reading-item",
            attested,
            &body.revision,
            None,
            128,
        )
        .unwrap();
    assert!(page.attested);
    assert_eq!(
        (page.spans[0].middle_offset, page.spans[0].middle_length),
        (2, 14)
    );
    assert_eq!(page.spans[0].anchor.as_deref(), Some("m-1"));
    assert!(
        content
            .creator_discoverable(ids.personal, attested, "reading-item", ids.owner)
            .unwrap()
    );
    // A capture that does not match the operation input is refused.
    assert!(
        super::package::invoke(
            state.clone(),
            Identity { id: ids.owner },
            Session::Static,
            "third",
            "reading.add",
            serde_json::from_value(
                json!({"space_id":ids.personal,"title":"Captured other","url":"https://example.com/c"})
            )
            .unwrap(),
            None,
            Some("Captured title".into()),
        )
        .await
        .is_err()
    );

    let disabled = call(
        "disable",
        "plugins.disable",
        json!({"space_id":ids.personal,"plugin_id":"calternal-reading-list"}),
    )
    .await
    .unwrap();
    assert_eq!(disabled["plugin"]["enabled"], false);
    assert_eq!(
        call("after", "reading.add", add).await.unwrap_err().code,
        ErrorCode::NotFound
    );
    supervisor.abort();
    server.abort();
}
