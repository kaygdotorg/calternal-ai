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
    entry::EntryProgram,
    space_channel::{PackagePin, SpaceMessage},
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;
use uuid::Uuid;

const SCRIPT: &[u8] = br#"#!/usr/local/bin/node
const fs = require('node:fs');
const {call,input} = JSON.parse(fs.readFileSync(0, 'utf8'));
if (call !== 'fixture-call' || fs.existsSync('/work')) process.exit(1);
process.stdout.write('{"seq":1,"kind":"output","data":');
setTimeout(() => {
  process.stdout.write(JSON.stringify({text:input.text}) + '}\n');
  if (input.wait_for_stop) {
    setTimeout(() => process.stdout.write('{"seq":2,"kind":"output","data":{"text":"still live"}}\n'), 40);
    setInterval(() => {}, 1000);
  } else if (input.disconnect) {
    setTimeout(() => {
      process.stdout.write('{"seq":2,"kind":"output","data":{"text":"offline"}}\n{"seq":3,"kind":"terminal","state":"ok"}\n');
    }, 6200);
  } else {
    process.stdout.write('{"seq":2,"kind":"terminal","state":"ok"}\n');
  }
}, 20);
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
                        program: EntryProgram {
                            image_source: source.into(),
                            image_digest: digest.into(),
                            entry_sha256: sha256_hex(SCRIPT),
                            entry_base64: STANDARD.encode(SCRIPT),
                            invocation: json!({"call":"fixture-call","input":{"text":"fixture","wait_for_stop":stop}}),
                        },
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
    tokio::time::timeout(Duration::from_secs(30), async {
        let ticket = Uuid::new_v4();
        let (origin, mut channel) = service.open_entry_channel(
            ids.owner, ids.personal, machine, ticket, pin.clone(),
        ).await.unwrap();
        assert!(matches!(channel.receive().await.unwrap().frame().message, SpaceMessage::Ready {}));
        channel.sender().send_checked_wait(
            &pin,
            ticket,
            SpaceMessage::StartEntry { program: EntryProgram {
                image_source: source.into(),
                image_digest: digest.into(),
                entry_sha256: sha256_hex(SCRIPT),
                entry_base64: STANDARD.encode(SCRIPT),
                invocation: json!({"call":"fixture-call","input":{"text":"fixture","disconnect":true}}),
            }},
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
            assert!(matches!(&recovered.receive().await.unwrap().frame().message,
                SpaceMessage::EntryAttached { origin: proof, buffered_through: 3 } if *proof == origin));
            let output = recovered.receive().await.unwrap();
            assert!(matches!(&output.frame().message, SpaceMessage::Chunk { data }
                if data == &json!({"seq":2,"kind":"output","data":{"text":"offline"}})));
            let terminal = recovered.receive().await.unwrap();
            assert!(matches!(&terminal.frame().message, SpaceMessage::Complete { result: Ok(data) }
                if data == &json!({"seq":3,"kind":"terminal","state":"ok"})));
            recovered.sender().send(&pin, ticket, SpaceMessage::EntryAck { through: 3 }).unwrap();
            assert!(recovered.receive().await.is_none());
        }
    }).await.unwrap();
    assert!(worker_revision.current().is_some());
    sync.abort();
    server.abort();
}
