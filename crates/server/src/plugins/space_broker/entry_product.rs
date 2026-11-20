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
    let (state, service, ids, machine, token, hosted) = tokio::task::block_in_place(|| {
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
        let (machine, token) = product_http::enroll(&service, Owner::Account { id: ids.owner });
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
        (state, service, ids, machine, token, hosted)
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
    let runner = Arc::new(Runner::new(backend, link, 2));
    let sync_service = service.clone();
    let sync = tokio::spawn(async move {
        let mut ack = 0;
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
            for envelope in sync_service.sync(machine, &token, request).await.unwrap() {
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
        let mut channel = service
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
    assert!(worker_revision.current().is_some());
    sync.abort();
    server.abort();
}
