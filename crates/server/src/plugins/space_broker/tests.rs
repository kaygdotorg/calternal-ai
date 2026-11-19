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
use calternal_machine::{
    protocol::Stream,
    runner::space_channel::relay_package,
    sandbox::{Exec, OutputEvent},
};
use calternal_plugin_host::space_channel::SpaceChannel;
use std::process::Stdio;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
};

const CHILD: &str = r#"
const lines = require('node:readline').createInterface({input:process.stdin});
const call = require('node:crypto').randomUUID();
let invocation;
const send = message => process.stdout.write(JSON.stringify({
  protocol_version:2, invocation_id:invocation, message
}) + '\n');
lines.on('line', line => {
  const f = JSON.parse(line), m = f.message;
  if ('ticket' in f || 'epoch' in f || 'space_id' in (m.input || {})) process.exit(8);
  if (m.type === 'invoke') {
    invocation = f.invocation_id;
    send({type:'host_call',call_id:call,method:'item_head',input:m.input});
  } else if (m.type === 'host_reply' && m.call_id === call && m.result.Ok) {
    send({type:'complete',data:m.result.Ok});
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
                    id: "head-fixture-lost-reply",
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

async fn real_package(
    mut runtime: SpaceChannel,
    pin: PackagePin,
) -> Result<(), calternal_plugin_host::Error> {
    let mut child = tokio::task::block_in_place(|| {
        tokio::process::Command::new("node")
            .args(["--permission", "-e", CHILD])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap()
    });
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (input, mut writes) = mpsc::channel::<Vec<u8>>(4);
    let (send, events) = mpsc::channel(4);
    let write = tokio::spawn(async move {
        while let Some(data) = writes.recv().await {
            if stdin.write_all(&data).await.is_err() {
                break;
            }
        }
    });
    let read = tokio::spawn(async move {
        let mut buffer = [0; 4096];
        while let Ok(n) = stdout.read(&mut buffer).await {
            if n == 0
                || send
                    .send(OutputEvent::Data(Stream::Stdout, buffer[..n].to_vec()))
                    .await
                    .is_err()
            {
                break;
            }
        }
    });
    let result = relay_package(
        &mut runtime,
        &pin,
        Exec {
            input: Some(input),
            events,
        },
    )
    .await;
    let _ = child.kill().await;
    write.abort();
    read.abort();
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approved_item_head_reaches_a_real_package_after_reconnect() {
    let fixture = tokio::task::spawn_blocking(Fixture::new).await.unwrap();
    let (old, peer) = fixture.carrier().await;
    let target = Uuid::now_v7();
    let old_scope = fixture.scope(&old.channel, target);
    let (old_ticket, item) = tokio::task::block_in_place(|| {
        let ticket = Ticket::mint(&fixture.auth, &old.grants, old_scope).unwrap();
        let rights = fixture
            .auth
            .capabilities(fixture.actor, fixture.space)
            .unwrap()
            .unwrap();
        assert!(rights.contains(Capability::ContentWrite));
        (ticket, fixture.write(target))
    });
    let old_epoch = old.channel.epoch();
    // The fixture writer committed, but this connection is lost before delivery.
    drop((old, peer));
    let (replay, replay_peer) = fixture.carrier().await;
    let replay_scope = fixture.scope(&replay.channel, item.id);
    let (receipt, count) = tokio::task::block_in_place(|| {
        let admitted = Ticket::mint(&fixture.auth, &replay.grants, replay_scope).unwrap();
        assert_ne!(admitted.id(), old_ticket.id());
        admitted.require_live(Instant::now()).unwrap();
        let receipt = fixture
            .content
            .receipt(fixture.space, fixture.actor, "head-fixture-lost-reply")
            .unwrap()
            .unwrap();
        let count = fixture
            .content
            .list(fixture.space, "notes", 2)
            .unwrap()
            .len();
        admitted.require_live(Instant::now()).unwrap();
        (receipt, count)
    });
    drop((replay, replay_peer));
    assert_eq!(receipt.operation, "fixture.put");
    assert_eq!(receipt.fingerprint, "same checked payload");
    assert_eq!(receipt.item, Some(item.id));
    assert_eq!(
        receipt.result,
        json!({"id": item.id, "revision":item.revision})
    );
    assert_eq!(count, 1);

    let (fresh, mut runtime) = fixture.carrier().await;
    assert_ne!(fresh.channel.epoch(), old_epoch);
    let scope = fixture.scope(&fresh.channel, item.id);
    let old_pin = fixture.pin.clone();
    let forged = tokio::spawn(async move {
        runtime.ready(&old_pin).unwrap();
        assert_ne!(
            runtime.receive().await.unwrap().frame().ticket,
            Some(old_ticket.id())
        );
        runtime
            .send(
                &old_pin,
                old_ticket.id(),
                SpaceMessage::Call {
                    callback_id: Uuid::new_v4(),
                    method: SpaceMethod::ItemHead,
                    input: json!({"kind":"notes","item_id":item.id}),
                },
            )
            .unwrap();
        assert!(runtime.receive().await.is_none());
    });
    let reader = Arc::new(fixture.content.item_head_reader(fixture.space));
    let error = exchange(fresh, fixture.auth.clone(), reader.clone(), scope)
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Forbidden);
    forged.await.unwrap();

    let (fresh, runtime) = fixture.carrier().await;
    let scope = fixture.scope(&fresh.channel, item.id);
    let child = tokio::spawn(real_package(runtime, fixture.pin.clone()));
    let answer = exchange(fresh, fixture.auth.clone(), reader, scope)
        .await
        .unwrap();
    assert_eq!(
        answer,
        json!({"metadata":{"title":"Fixture"}, "revision":item.revision})
    );
    child.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn approved_operation_dispatches_over_an_enrolled_http_upgrade() {
    use crate::{
        api::{AppState, allowed_hosts, dispatch, router},
        auth::StoredDirectory,
        plugins::{Jobs, Registry, WorkerPool},
        store::Store,
    };
    use calternal_machine::{
        identity::MachineKey,
        protocol::{
            Capacity, Down, EnrolRequest, Owner, PROTOCOL_VERSION, SPACE_CHANNEL_UPGRADE,
            SessionFinish, SyncRequest,
        },
    };

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (root, state, service, ids, item, machine, token) = tokio::task::block_in_place(|| {
        let root = tempfile::tempdir().unwrap();
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
        content
            .register_kind(
                ids.personal,
                "notes",
                Placement::Visible {
                    folder: "Notes".into(),
                },
                FileFormat::Markdown,
            )
            .unwrap();
        let item = content
            .write(
                ids.personal,
                None,
                "notes",
                None,
                json!({"title":"Enrolled"}),
                b"# Fixture\n",
                Request {
                    account: ids.owner,
                    via: None,
                    id: "install-fixture",
                    operation: "fixture.put",
                    fingerprint: "fixture",
                },
                |_| json!({}),
            )
            .unwrap();
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
        let approval = ApprovedItemHead::new(
            ids.personal,
            PackagePin {
                package: "calternal-head-fixture".into(),
                digest: sha256_hex(CHILD.as_bytes()),
            },
            "fixture.head".into(),
            "notes".into(),
            CHILD.into(),
        )
        .unwrap();
        let registry = Registry::empty()
            .with_space_item_heads(vec![approval])
            .unwrap();
        let state = Arc::new(AppState {
            inputs: store.account_inputs(),
            store,
            directory,
            registry,
            blocking: crate::blocking::RequestWorkers::new().unwrap(),
            workers: WorkerPool::with_local_root(None, 1, root.path().join("workers")),
            jobs: Jobs::disabled(),
            allowed_hosts: allowed_hosts(address.port()),
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
        (root, state, service, ids, item, machine, token)
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
    let supervisor = tokio::spawn(async move {
        let binding = tokio::time::timeout(Duration::from_secs(5), async {
            let mut ack = 0;
            loop {
                for envelope in service.sync(machine, &token, request(ack)).await.unwrap() {
                    ack = ack.max(envelope.seq);
                    if let Down::OpenSpaceChannel { channel } = envelope.message {
                        return channel;
                    }
                }
            }
        })
        .await
        .unwrap();
        // This loopback connection represents the server side of TLS termination.
        let response = reqwest::Client::new()
            .get(format!(
                "http://{address}/api/v1/machine/spaces/{}/channels/{}",
                binding.space, binding.assignment,
            ))
            .bearer_auth(&token)
            .header("connection", "Upgrade")
            .header("upgrade", SPACE_CHANNEL_UPGRADE)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::SWITCHING_PROTOCOLS);
        let runtime = SpaceChannel::runtime(
            response.upgrade().await.unwrap(),
            binding.space,
            binding.assignment,
        )
        .await
        .unwrap();
        real_package(runtime, binding.package).await.unwrap();
    });
    let answer = dispatch(
        state,
        Identity { id: ids.owner },
        Session::Static,
        "fixture-invoke",
        "fixture.head",
        serde_json::from_value(json!({"space_id":ids.personal,"item_id":item.id})).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        answer,
        json!({"metadata":{"title":"Enrolled"}, "revision":item.revision})
    );
    supervisor.await.unwrap();
    server.abort();
    let _ = server.await;
    drop(root);
}
