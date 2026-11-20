//! Scripted entry qualification through the enrolled HTTPS upgrade and Podman.
use super::{TlsRelay, product_http};
use crate::{
    api::{AppState, allowed_hosts, router},
    auth::StoredDirectory,
    machines::{MachineService, Settings},
    plugins::{Jobs, Registry, WorkerPool},
    store::{Store, sha256_hex},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_machine::{
    protocol::{Capacity, Owner, SyncRequest},
    runner::Runner,
    sandbox::{Backend, podman::PodmanBackend},
};
use calternal_plugin_host::{
    entry::{EntryLogin, EntryNetwork, EntryProgram},
    space_channel::{PackagePin, SpaceMessage, SpaceMethod},
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;
use uuid::Uuid;

const OFFLINE_FRAMES: u64 = 320;

fn program(source: &str, digest: &str, input: serde_json::Value) -> EntryProgram {
    EntryProgram {
        image_source: source.into(),
        image_digest: digest.into(),
        entry_sha256: sha256_hex(SCRIPT),
        entry_base64: STANDARD.encode(SCRIPT),
        invocation: json!({"call":"fixture-call","input":input}),
        methods: vec![],
        login: None,
        network: EntryNetwork::None,
        run_limit_secs: 60,
    }
}
const OFFLINE_BYTES: usize = 12_000;

const SCRIPT: &[u8] = br#"#!/usr/local/bin/node
const fs = require('node:fs');
const {once} = require('node:events');
const {call,input} = JSON.parse(fs.readFileSync(0, 'utf8'));
if (call !== 'fixture-call' || fs.existsSync('/work')) process.exit(1);
process.stdout.write('{"seq":1,"kind":"output","data":');
setTimeout(() => {
  process.stdout.write(JSON.stringify({text:input.text}) + '}\n');
  if (input.wait_for_stop) {
    setTimeout(() => process.stdout.write('{"seq":2,"kind":"output","data":{"text":"still live"}}\n'), 40);
    setInterval(() => {}, 1000);
  } else if (input.disconnect) {
    setTimeout(async () => {
      for (let seq = 2; seq <= input.offline_frames + 1; ++seq) {
        const frame = JSON.stringify({seq,kind:'output',data:{seq,text:'x'.repeat(input.offline_bytes)}}) + '\n';
        if (!process.stdout.write(frame)) await once(process.stdout, 'drain');
      }
      process.stdout.write(JSON.stringify({seq:input.offline_frames + 2,kind:'terminal',state:'ok'}) + '\n');
    }, 6200);
  } else if (input.provider) {
    provider();
  } else {
    process.stdout.write('{"seq":2,"kind":"terminal","state":"ok"}\n');
  }
}, 20);
function provider() {
  const dir = require('node:path').join(process.env.HOME, '.fixture-login');
  const file = dir + '/runs';
  const runs = (fs.existsSync(file) ? Number(fs.readFileSync(file, 'utf8')) : 0) + 1;
  fs.writeFileSync(file, String(runs));
  const sock = require('node:net').connect(process.env.CALTERNAL_BROKER_SOCKET);
  const replies = [];
  let buffer = '';
  sock.on('data', data => {
    buffer += data;
    for (let end; (end = buffer.indexOf('\n')) >= 0; buffer = buffer.slice(end + 1)) {
      replies.push(JSON.parse(buffer.slice(0, end)));
      if (replies.length === 2) sock.write('{"type":"call","callback_id":"c3","method":"item_head","input":{},"ticket":"forged"}\n');
    }
  });
  sock.on('close', () => {
    const egress = process.env.CALTERNAL_EGRESS_SOCKET ?? null;
    process.stdout.write(JSON.stringify({seq:2,kind:'output',data:{runs,replies,egress}}) + '\n');
    process.stdout.write('{"seq":3,"kind":"terminal","state":"ok"}\n');
  });
  sock.write('{"type":"call","callback_id":"c1","method":"item_purge","input":{}}\n');
  sock.write('{"type":"call","callback_id":"c2","method":"item_head","input":{"probe":true}}\n');
}
"#;

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn enrolled_entry_delivers_only_the_exact_tickets_ordered_frames() {
    let Ok(image) = std::env::var("CALTERNAL_TEST_ENTRY_IMAGE") else {
        eprintln!("SKIP enrolled Podman entry: CALTERNAL_TEST_ENTRY_IMAGE is unset");
        return;
    };
    let (source, digest) = image.split_once('@').unwrap();
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay = TlsRelay::start(root.path(), listener.local_addr().unwrap()).await;
    let boot = Uuid::new_v4();
    let (state, service, ids, machine, key, token, hosted) = tokio::task::block_in_place(|| {
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
        let service = MachineService::open(
            &roots.appdata,
            directory.spaces.machines(),
            Settings {
                poll_hold: Duration::from_millis(20),
                ..Settings::default()
            },
        )
        .unwrap();
        store.bind_machines(&service);
        let (machine, key) = product_http::enroll_key(&service, Owner::Account { id: ids.owner });
        let token = product_http::session(&service, machine, &key, boot);
        let (hosted, _) = product_http::enroll(&service, Owner::Pool);
        let state = Arc::new(AppState {
            inputs: store.account_inputs(),
            store,
            directory,
            registry: Registry::empty(),
            blocking: crate::blocking::RequestWorkers::new().unwrap(),
            workers: WorkerPool::with_local_root(None, 1, root.path().join("workers")),
            jobs: Jobs::disabled(),
            allowed_hosts: allowed_hosts(relay.port),
        });
        (state, service, ids, machine, key, token, hosted)
    });
    let worker_revision = tokio::task::block_in_place(|| {
        state
            .directory
            .spaces
            .grant_revision(ids.personal, hosted)
            .unwrap()
    });
    let router = router(state, None).layer(axum::Extension(service.clone()));
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let link = Arc::new(relay.link);
    link.set_token(Some(Arc::from(token.as_str())));
    let backend = Arc::new(PodmanBackend::new(
        "podman",
        root.path().join("placements"),
        "unused",
    ));
    backend.initialize().await.unwrap();
    let runner = Arc::new(Runner::new(backend, link.clone(), 2));
    let sync_service = service.clone();
    let (session, mut sessions) = tokio::sync::watch::channel(token);
    let sync = tokio::spawn(async move {
        let mut ack = 0;
        let mut token = sessions.borrow_and_update().clone();
        loop {
            let request = SyncRequest {
                ack,
                draining: false,
                discards: vec![],
                leases: vec![],
                capacity: Capacity {
                    max_sandboxes: 2,
                    ..Capacity::default()
                },
            };
            let result = tokio::select! {
                biased;
                changed = sessions.changed() => {
                    changed.unwrap();
                    token = sessions.borrow_and_update().clone();
                    ack = 0;
                    continue;
                }
                result = sync_service.sync(machine, &token, request) => result,
            };
            let Ok(envelopes) = result else {
                sessions.changed().await.unwrap();
                token = sessions.borrow_and_update().clone();
                ack = 0;
                continue;
            };
            for envelope in envelopes {
                ack = ack.max(envelope.seq);
                runner.handle(envelope.message).await;
            }
        }
    });
    let pin = PackagePin {
        package: "fixture-entry".into(),
        digest: "a".repeat(64),
    };
    for (actor, placement) in [(ids.guest, machine), (ids.owner, hosted)] {
        assert!(
            service
                .open_entry_channel(actor, ids.personal, placement, Uuid::new_v4(), pin.clone())
                .await
                .is_err()
        );
    }
    for (valid_ticket, stop) in [(false, false), (true, false), (true, true)] {
        let ticket = Uuid::new_v4();
        let (_, mut channel) = service
            .open_entry_channel(ids.owner, ids.personal, machine, ticket, pin.clone())
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(15), async {
            let ready = channel.receive().await.unwrap();
            assert!(matches!(ready.frame().message, SpaceMessage::Ready {}));
            channel
                .sender()
                .send_checked_wait(
                    &pin,
                    if valid_ticket { ticket } else { Uuid::new_v4() },
                    SpaceMessage::StartEntry {
                        program: program(source, digest, json!({"text":"fixture","wait_for_stop":stop})),
                    },
                    || true,
                )
                .await
                .unwrap();
            if !valid_ticket {
                assert!(channel.receive().await.is_none());
                return;
            }
            let output = channel.receive().await.unwrap();
            assert_eq!(output.frame().ticket, Some(ticket));
            assert!(
                matches!(&output.frame().message, SpaceMessage::Chunk { data }
                if data == &json!({"seq": 1, "kind": "output", "data": {"text":"fixture"}}))
            );
            if stop {
                channel.sender().send(&pin, Uuid::new_v4(), SpaceMessage::Cancel {}).unwrap();
                let still_live = channel.receive().await.unwrap();
                assert_eq!(still_live.frame().ticket, Some(ticket));
                assert!(matches!(&still_live.frame().message, SpaceMessage::Chunk { data } if data["seq"] == 2));
                channel.sender().send(&pin, ticket, SpaceMessage::Cancel {}).unwrap();
                assert!(channel.receive().await.is_none());
                return;
            }
            let terminal = channel.receive().await.unwrap();
            assert_eq!(terminal.frame().ticket, Some(ticket));
            assert!(
                matches!(&terminal.frame().message, SpaceMessage::Complete { result: Ok(result) }
                if result == &json!({"seq": 2, "kind": "terminal", "state": "ok"}))
            );
            channel.sender().send(&pin, ticket, SpaceMessage::EntryAck { through: 2 }).unwrap();
            assert!(channel.receive().await.is_none());
        })
        .await
        .unwrap();
        let staged = root.path().join("placements/entries");
        if valid_ticket {
            // Cancellation fences before verified removal. Wait for the
            // scoped backend operation without making a latency claim.
            tokio::time::timeout(Duration::from_secs(15), async {
                loop {
                    if tokio::fs::read_dir(&staged)
                        .await
                        .unwrap()
                        .next_entry()
                        .await
                        .unwrap()
                        .is_none()
                    {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .unwrap();
        } else {
            assert!(!tokio::fs::try_exists(staged).await.unwrap());
        }
    }
    tokio::time::timeout(Duration::from_secs(45), async {
        let ticket = Uuid::new_v4();
        let (origin, mut channel) = service.open_entry_channel(
            ids.owner, ids.personal, machine, ticket, pin.clone(),
        ).await.unwrap();
        assert!(matches!(channel.receive().await.unwrap().frame().message, SpaceMessage::Ready {}));
        channel.sender().send_checked_wait(
            &pin,
            ticket,
            SpaceMessage::StartEntry { program: program(source, digest, json!({"text":"fixture","disconnect":true,"offline_frames":OFFLINE_FRAMES,"offline_bytes":OFFLINE_BYTES})) },
            || true,
        ).await.unwrap();
        assert!(matches!(&channel.receive().await.unwrap().frame().message,
            SpaceMessage::Chunk { data } if data["seq"] == 1));
        // Simulate a lost machine session. The receiver committed frame one
        // but the old link disappears before it sends the commit ACK.
        link.set_token(None);
        assert!(channel.receive().await.is_none());
        drop(channel);
        let staged = root.path().join("placements/entries");
        loop {
            if tokio::fs::read_dir(&staged).await.unwrap().next_entry().await.unwrap().is_none() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let token = tokio::task::block_in_place(|| product_http::session(&service, machine, &key, boot));
        link.set_token(Some(Arc::from(token.as_str())));
        session.send_replace(token);
        let logs = root.path().join("placements/entry-logs");
        let mut entries = tokio::fs::read_dir(&logs).await.unwrap();
        let saved = entries.next_entry().await.unwrap().expect("offline working log");
        assert!(saved.metadata().await.unwrap().len() >= OFFLINE_FRAMES * OFFLINE_BYTES as u64);
        assert!(entries.next_entry().await.unwrap().is_none());
        for exact in [false, true] {
            let (_, mut recovered) = service.open_entry_channel(
                ids.owner, ids.personal, machine, ticket, pin.clone(),
            ).await.unwrap();
            assert!(matches!(recovered.receive().await.unwrap().frame().message, SpaceMessage::Ready {}));
            recovered.sender().send_checked_wait(
                &pin,
                ticket,
                SpaceMessage::ResumeEntry { origin: if exact { origin } else { Uuid::new_v4() }, after: 1 },
                || true,
            ).await.unwrap();
            if !exact {
                assert!(recovered.receive().await.is_none());
                continue;
            }
            let terminal_seq = OFFLINE_FRAMES + 2;
            assert!(matches!(&recovered.receive().await.unwrap().frame().message,
                SpaceMessage::EntryAttached { origin: proof, buffered_through } if *proof == origin && *buffered_through == terminal_seq));
            let expected_text = "x".repeat(OFFLINE_BYTES);
            for seq in 2..terminal_seq {
                let output = recovered.receive().await.unwrap();
                assert_eq!(output.frame().ticket, Some(ticket));
                assert!(matches!(&output.frame().message, SpaceMessage::Chunk { data }
                    if data["seq"] == seq && data["data"]["seq"] == seq && data["data"]["text"] == expected_text));
                recovered.sender().send(&pin, ticket, SpaceMessage::EntryAck { through: seq }).unwrap();
            }
            let terminal = recovered.receive().await.unwrap();
            assert!(matches!(&terminal.frame().message, SpaceMessage::Complete { result: Ok(data) }
                if data == &json!({"seq":terminal_seq,"kind":"terminal","state":"ok"})));
            recovered.sender().send(&pin, ticket, SpaceMessage::EntryAck { through: terminal_seq }).unwrap();
            assert!(recovered.receive().await.is_none());
        }
        loop {
            if tokio::fs::read_dir(&logs).await.unwrap().next_entry().await.unwrap().is_none() {
                break;
            }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    // Provider path: one declared callback answered by the host, an
    // undeclared one refused on the machine, a forged field closing the
    // stream, and login backing private to each harness contribution.
    for (contribution, runs) in [("first", 1), ("first", 2), ("second", 1)] {
        let ticket = Uuid::new_v4();
        let (_, mut channel) = service
            .open_entry_channel(ids.owner, ids.personal, machine, ticket, pin.clone())
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(30), async {
            assert!(matches!(channel.receive().await.unwrap().frame().message, SpaceMessage::Ready {}));
            let mut started = program(source, digest, json!({"text":"fixture","provider":true}));
            started.methods = vec![SpaceMethod::new("item_head").unwrap()];
            started.login = Some(EntryLogin {
                account: ids.owner,
                package: "fixture-entry".into(),
                contribution: contribution.into(),
                directories: vec![".fixture-login".into()],
            });
            channel.sender().send_checked_wait(&pin, ticket, SpaceMessage::StartEntry { program: started }, || true)
                .await.unwrap();
            let mut called = false;
            let mut seq = 0;
            let result = loop {
                let delivery = channel.receive().await.unwrap();
                assert_eq!(delivery.frame().ticket, Some(ticket));
                match &delivery.frame().message {
                    SpaceMessage::Call { callback_id, method, input } => {
                        assert!(!called && method.as_str() == "item_head" && input == &json!({"probe":true}));
                        called = true;
                        channel.sender().send(&pin, ticket, SpaceMessage::Reply {
                            callback_id: *callback_id, result: Ok(json!({"head":"fixture"})),
                        }).unwrap();
                    }
                    SpaceMessage::Chunk { data } => {
                        seq += 1;
                        assert_eq!(data["seq"], seq);
                        channel.sender().send(&pin, ticket, SpaceMessage::EntryAck { through: seq }).unwrap();
                        if seq == 2 { break data["data"].clone(); }
                    }
                    other => panic!("unexpected {other:?}"),
                }
            };
            assert!(called);
            assert_eq!(result, json!({"runs": runs, "egress": null, "replies": [
                {"type":"reply","callback_id":"c1","ok":false,"error":{"status":403,"code":"forbidden"}},
                {"type":"reply","callback_id":"c2","ok":true,"data":{"head":"fixture"}},
            ]}));
            let terminal = channel.receive().await.unwrap();
            assert!(matches!(&terminal.frame().message, SpaceMessage::Complete { result: Ok(data) } if data["seq"] == 3));
            channel.sender().send(&pin, ticket, SpaceMessage::EntryAck { through: 3 }).unwrap();
            assert!(channel.receive().await.is_none());
        })
        .await
        .unwrap();
    }
    assert!(worker_revision.current().is_some());
    sync.abort();
    server.abort();
}
