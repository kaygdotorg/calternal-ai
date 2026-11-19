use super::*;
use serde_json::json;

#[test]
fn item_head_scope_cannot_be_widened_by_callback_input() {
    let id = Uuid::new_v4();
    assert!(item_target_matches(
        "notes",
        id,
        &json!({"kind":"notes","item_id":id})
    ));
    assert!(!item_target_matches(
        "notes",
        id,
        &json!({"kind":"notes","item_id":Uuid::new_v4()})
    ));
    assert!(!item_target_matches(
        "notes",
        id,
        &json!({"kind":"notes","item_id":id,"space_id":SpaceId::new()})
    ));
}

use calternal_auth::{
    db::GrantRevision,
    token::{hash32, normalize_setup_token},
};
use calternal_content::{Content, FileFormat, Item, Placement, Request};
use calternal_plugin_host::space_channel::SpaceChannel;
use tokio::net::{TcpListener, TcpStream};

#[cfg(target_os = "linux")]
#[path = "product_fixture.rs"]
mod product_fixture;

const CHILD: &str = r#"
setInterval(() => {}, 1000);
const lines = require('node:readline').createInterface({input:process.stdin});
const call = require('node:crypto').randomUUID();
const send = (invocation, message) => process.stdout.write(JSON.stringify({
  protocol_version:2, invocation_id:invocation, message
}) + '\n');
lines.on('line', line => {
  const f = JSON.parse(line), m = f.message;
  if ('ticket' in f || 'epoch' in f || 'space_id' in (m.input || {})) process.exit(8);
  if (m.type === 'invoke') {
    setTimeout(() => send(f.invocation_id, {type:'host_call',call_id:call,method:'item_head',input:m.input}), 75);
  } else if (m.type === 'host_reply' && m.call_id === call && m.result.Ok) {
    send(f.invocation_id, {type:'complete',data:m.result.Ok});
  }
});
"#;

struct Fixture {
    content: Content,
    auth: Spaces,
    grants: GrantRevision,
    space: SpaceId,
    actor: AccountId,
    pin: PackagePin,
    _root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let path = root.path();
        let auth = Spaces::open(&path.join("auth.db")).unwrap();
        let ids = auth.seed_development().unwrap();
        let machines = auth.machines();
        let (code, _) = machines.issue_code(None, Duration::from_secs(30)).unwrap();
        let machine = machines
            .redeem_code(
                hash32(&normalize_setup_token(&code).unwrap()),
                vec![42; 32],
                "item head fixture".into(),
                "linux".into(),
                "x86_64".into(),
                false,
            )
            .unwrap()
            .unwrap();
        let space = ids.personal;
        let grants = auth.grant_revision(space, machine.id).unwrap();
        let content = Content::open(
            calternal_roots::Roots {
                appdata: path.join("appdata"),
                userdata_ssd: path.join("userdata-ssd"),
                userdata_hdd: path.join("userdata-hdd"),
            },
            path.join("secrets"),
            Duration::from_secs(30),
            calternal_content::DEFAULT_RECEIPT_RETENTION,
            |_| {},
        )
        .unwrap();
        content
            .register_kind(
                space,
                "notes",
                Placement::Visible {
                    folder: "Notes".into(),
                },
                FileFormat::Markdown,
            )
            .unwrap();
        Self {
            _root: root,
            content,
            auth,
            grants,
            space,
            actor: ids.owner,
            pin: PackagePin {
                package: "calternal-head-fixture".into(),
                digest: sha256_hex(CHILD.as_bytes()),
            },
        }
    }

    fn write(&self, id: Uuid) -> Item {
        self.content
            .create_assigned(
                self.space,
                id,
                "notes",
                json!({"title":"Fixture"}),
                b"# Retained body\n",
                Request {
                    account: self.actor,
                    via: None,
                    id: "head-fixture-item",
                    operation: "fixture.put",
                    fingerprint: "same checked payload",
                },
                |item| json!({"id": item.id, "revision": item.revision}),
            )
            .unwrap()
    }

    async fn carrier(&self) -> (Connected, SpaceChannel) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (connected, accepted) = tokio::join!(
            TcpStream::connect(listener.local_addr().unwrap()),
            listener.accept()
        );
        let assignment = Uuid::new_v4();
        let (host, runtime) = tokio::join!(
            SpaceChannel::host(
                accepted.unwrap().0,
                self.space,
                assignment,
                vec![self.pin.clone()]
            ),
            SpaceChannel::runtime(connected.unwrap(), self.space, assignment),
        );
        (
            Connected {
                space: self.space,
                channel: host.unwrap(),
                grants: self.grants.clone(),
            },
            runtime.unwrap(),
        )
    }

    fn scope(&self, channel: &SpaceChannel, item: Uuid) -> Scope {
        Scope {
            actor: self.actor,
            installation: None,
            package: self.pin.clone(),
            epoch: channel.epoch(),
            operation: "fixture.head".into(),
            kind: "notes".into(),
            item,
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejects_an_old_ticket_and_admits_a_fresh_read_after_reconnect() {
    let fixture = tokio::task::spawn_blocking(Fixture::new).await.unwrap();
    let (old, peer) = fixture.carrier().await;
    let target = Uuid::now_v7();
    let old_scope = fixture.scope(&old.channel, target);
    let (old_ticket, item) = tokio::task::block_in_place(|| {
        let admission = fixture.auth.invocation_reader(&old.grants).unwrap();
        let ticket = Ticket::mint(&admission, old_scope).unwrap();
        let rights = fixture
            .auth
            .capabilities(fixture.actor, fixture.space)
            .unwrap()
            .unwrap();
        assert!(rights.contains(Capability::ContentWrite));
        (ticket, fixture.write(target))
    });
    let old_epoch = old.channel.epoch();
    let prior_callback = calternal_plugin_host::space_channel::SpaceFrame {
        v: 1,
        epoch: old_epoch,
        seq: 1,
        package: Some(fixture.pin.package.clone()),
        digest: Some(fixture.pin.digest.clone()),
        ticket: Some(old_ticket.id()),
        message: SpaceMessage::Call {
            callback_id: Uuid::new_v4(),
            method: SpaceMethod::ItemHead,
            input: json!({"kind":"notes","item_id":item.id}),
        },
    };
    old_ticket.authorize_at(&prior_callback, Instant::now()).unwrap();
    drop((old, peer));

    let (fresh, mut runtime) = fixture.carrier().await;
    assert_ne!(fresh.channel.epoch(), old_epoch);
    assert!(old_ticket.authorize_at(
        &calternal_plugin_host::space_channel::SpaceFrame {
            epoch: fresh.channel.epoch(),
            ..prior_callback
        }, Instant::now()
    ).is_err());
    let scope = fixture.scope(&fresh.channel, item.id);
    let old_pin = fixture.pin.clone();
    let forged = tokio::spawn(async move {
        runtime.ready(&old_pin).unwrap();
        let admitted = runtime.receive().await.unwrap().frame().ticket.unwrap();
        assert_ne!(admitted, old_ticket.id());
        let callback = Uuid::new_v4();
        let input = json!({"kind":"notes","item_id":item.id});
        runtime
            .send(
                &old_pin,
                old_ticket.id(),
                SpaceMessage::Call {
                    callback_id: callback,
                    method: SpaceMethod::ItemHead,
                    input: input.clone(),
                },
            )
            .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), runtime.receive())
                .await
                .is_err()
        );
        runtime
            .send(
                &old_pin,
                admitted,
                SpaceMessage::Call {
                    callback_id: callback,
                    method: SpaceMethod::ItemHead,
                    input,
                },
            )
            .unwrap();
        let reply = runtime.receive().await.unwrap();
        let SpaceMessage::Reply { result, .. } = &reply.frame().message else {
            panic!("expected admitted reply")
        };
        assert_eq!(reply.frame().ticket, Some(admitted));
        runtime
            .send(
                &old_pin,
                admitted,
                SpaceMessage::Complete {
                    result: result.clone(),
                },
            )
            .unwrap();
        assert!(runtime.receive().await.is_none());
    });
    let reader = Arc::new(tokio::task::block_in_place(|| fixture.content.item_head_reader(fixture.space)).unwrap());
    exchange(fresh, fixture.auth.clone(), reader.clone(), scope)
        .await
        .unwrap();
    forged.await.unwrap();
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn enrolled_runner_multiplexes_and_stops_its_refenced_package_on_lease_loss() {
    use crate::{
        api::{AppState, allowed_hosts, dispatch, router},
        auth::StoredDirectory,
        plugins::{Jobs, Registry, WorkerPool},
        store::Store,
    };
    use calternal_machine::{
        identity::MachineKey,
        protocol::{
            Capacity, Down, EnrolRequest, Owner, PROTOCOL_VERSION, SessionFinish, SyncRequest,
        },
        runner::Runner,
    };
    use product_fixture::{FixtureBackend, TlsRelay, worker_pid};

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let root = tempfile::tempdir().unwrap();
    let relay = TlsRelay::start(root.path(), address).await;
    let (state, service, ids, items, machine, token) = tokio::task::block_in_place(|| {
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
        let directory = StoredDirectory::load_or_create(&roots.appdata).unwrap();
        let ids = directory.spaces.seed_development().unwrap();
        let content = store.shared_content();
        let items = [ids.personal, ids.shared].map(|space| {
            content.register_kind(
                space, "notes",
                Placement::Visible { folder: "Notes".into() },
                FileFormat::Markdown,
            ).unwrap();
            content.write(
                space, None, "notes", None, json!({"title":"Enrolled"}),
                b"# Fixture\n",
                Request {
                    account: ids.owner, via: None, id: "install-fixture",
                    operation: "fixture.put", fingerprint: "fixture",
                },
                |_| json!({}),
            ).unwrap()
        });
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
        let approvals = [ids.personal, ids.shared].map(|space| {
            ApprovedItemHead::new(
                space,
                PackagePin {
                    package: "calternal-head-fixture".into(),
                    digest: sha256_hex(CHILD.as_bytes()),
                },
                "fixture.head".into(),
                "notes".into(),
                CHILD.into(),
            ).unwrap()
        });
        let registry = Registry::empty()
            .with_space_item_heads(Vec::from(approvals)).unwrap();
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
        let (code, _) = service.issue_code(Owner::Pool).unwrap();
        let key = MachineKey::generate().unwrap();
        let machine = service
            .enrol(&EnrolRequest {
                code,
                public_key: key.public_key_b64(),
                name: "carrier".into(),
                os: "linux".into(),
                arch: "x86_64".into(),
                direct: false,
            })
            .unwrap()
            .machine_id;
        let challenge = service.session_start(machine).unwrap();
        let token = service
            .session_finish(&SessionFinish {
                machine_id: machine,
                signature: key.sign_session(machine, &challenge),
                challenge,
                boot_id: Uuid::new_v4(),
                protocol: PROTOCOL_VERSION,
                backend: "podman".into(),
                guest_memory_mib: None,
            })
            .unwrap()
            .token;
        (state, service, ids, items, machine, token)
    });
    let api = router(state.clone(), None).layer(axum::Extension(service.clone()));
    let server = tokio::spawn(async move { axum::serve(listener, api).await.unwrap() });
    let request = |ack| SyncRequest {
        ack,
        draining: false,
        discards: vec![],
        leases: vec![],
        capacity: Capacity {
            cpus: 1,
            available_memory_bytes: Some(64 << 30),
            max_sandboxes: 2,
            ..Default::default()
        },
    };
    service.sync(machine, &token, request(0)).await.unwrap();
    let link = Arc::new(relay.link);
    link.set_token(Some(Arc::from(token.as_str())));
    let placement_root = root.path().join("placements");
    let runner = Arc::new(Runner::new(
        Arc::new(FixtureBackend::new(&placement_root)),
        link,
        2,
    ));
    let sync_runner = runner.clone();
    let (assignments, mut assigned) = tokio::sync::mpsc::channel(2);
    let supervisor = tokio::spawn(async move {
        let mut ack = 0;
        let mut opened = 0;
        loop {
            for envelope in service.sync(machine, &token, request(ack)).await.unwrap() {
                ack = ack.max(envelope.seq);
                let message = envelope.message;
                if matches!(message, Down::Assign { .. }) {
                    assignments.try_send(message.clone()).unwrap();
                }
                if matches!(message, Down::OpenSpaceChannel { .. }) {
                    opened += 1;
                }
                sync_runner.handle(message).await;
                if opened == 2 {
                    return;
                }
            }
        }
    });
    let dispatch_read = |request, space, item| {
        dispatch(
            state.clone(),
            Identity { id: ids.owner },
            Session::Static,
            request,
            "fixture.head",
            serde_json::from_value(json!({"space_id":space,"item_id":item})).unwrap(),
        )
    };
    let (first, second) = tokio::join!(
        dispatch_read("fixture-one", ids.personal, items[0].id),
        dispatch_read("fixture-two", ids.personal, items[0].id),
    );
    let expected = |item: &Item| json!({"metadata":{"title":"Enrolled"}, "revision":item.revision});
    assert_eq!(first.unwrap(), expected(&items[0]));
    assert_eq!(second.unwrap(), expected(&items[0]));
    let mut first_assignment = assigned.recv().await.unwrap();
    let Down::Assign {
        ref mut fence,
        ref spec,
        ..
    } = first_assignment
    else {
        unreachable!();
    };
    let pid = worker_pid(
        &placement_root
            .join("workers")
            .join(spec.thread_id.to_string())
            .join("work"),
    )
    .await;
    *fence += 1;
    runner.handle(first_assignment).await;
    // Stop forwarding without sending EOF, cancellation or Stop to the runner.
    tokio::fs::write(&relay.stall, []).await.unwrap();
    let lost = Instant::now();
    // B shares the same API, scheduler, enrollment and runner while A's
    // actual enrolled TCP stream still has a live process and forwards nothing.
    let answer_b = tokio::time::timeout(Duration::from_secs(3),
        dispatch_read("fixture-three", ids.shared, items[1].id),
    ).await.unwrap().unwrap();
    assert_eq!(answer_b, expected(&items[1]));
    assert!(tokio::fs::try_exists(format!("/proc/{pid}")).await.unwrap());
    let second_assignment = assigned.recv().await.unwrap();
    assert!(matches!(second_assignment, Down::Assign { space_id, .. } if space_id == ids.shared));
    supervisor.await.unwrap();
    tokio::time::timeout(Duration::from_secs(5).saturating_sub(lost.elapsed()), async {
        while tokio::fs::try_exists(format!("/proc/{pid}")).await.unwrap() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("product supervisor must stop and reap the package");
    assert!(lost.elapsed() >= Duration::from_secs(3));
    state.registry.space_item_head(ids.shared, "fixture.head").unwrap()
        .wake.awake.lock().await.as_ref().unwrap().fence.close();
    let Down::Assign { spec, .. } = second_assignment else { unreachable!() };
    let work = placement_root.join("workers").join(spec.thread_id.to_string()).join("work");
    tokio::time::timeout(Duration::from_secs(5), async {
        while tokio::fs::try_exists(&work).await.unwrap() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    server.abort();
    let _ = server.await;
    drop(root);
}
