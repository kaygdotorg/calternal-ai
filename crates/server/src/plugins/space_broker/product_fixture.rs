use super::*;
use calternal_machine::{
    link::Link,
    protocol::SandboxSpec,
    sandbox::{
        Backend, BoxFuture, Exec, ExecRequest, Sandbox, SandboxResult, direct::DirectBackend,
    },
};
use calternal_plugin_host::space_channel::MAX_TICKET_RESULT_FRAME;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
};
use uuid::Uuid;

// Keep production's direct-placement restriction intact. Only this fixture
// delegates an isolated test assignment to the direct backend.
struct FixtureBackend {
    direct: DirectBackend,
    node: String,
}

impl FixtureBackend {
    fn new(root: &Path, node: String) -> Self {
        Self {
            direct: DirectBackend::new(root),
            node,
        }
    }
}

impl Backend for FixtureBackend {
    fn name(&self) -> &'static str {
        "fixture"
    }

    fn discard(&self, account: Uuid, thread: Uuid) -> BoxFuture<'_, SandboxResult<()>> {
        self.direct.discard(account, thread)
    }

    fn erase(&self, account: Uuid, thread: Option<Uuid>) -> BoxFuture<'_, SandboxResult<()>> {
        self.direct.erase(account, thread)
    }

    fn start<'a>(
        &'a self,
        spec: &'a SandboxSpec,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        Box::pin(async move {
            Ok(Arc::new(FixtureSandbox {
                direct: self.direct.start(spec).await?,
                node: self.node.clone(),
            }) as Arc<dyn Sandbox>)
        })
    }
}

// The worker clears PATH. Supply the shell's pinned absolute Node program,
// then let DirectSandbox start, stream and reap the process normally.
struct FixtureSandbox {
    direct: Arc<dyn Sandbox>,
    node: String,
}

impl Sandbox for FixtureSandbox {
    fn exec(&self, mut request: ExecRequest) -> BoxFuture<'_, SandboxResult<Exec>> {
        assert_eq!(request.argv[0], "node");
        request.argv[0] = self.node.clone();
        self.direct.exec(request)
    }

    fn freeze(&self) -> BoxFuture<'_, SandboxResult<()>> {
        self.direct.freeze()
    }
    fn resume(&self) -> BoxFuture<'_, SandboxResult<()>> {
        self.direct.resume()
    }
    fn stop(&self) -> BoxFuture<'_, SandboxResult<()>> {
        self.direct.stop()
    }
    fn release(&self) -> BoxFuture<'_, SandboxResult<()>> {
        self.direct.release()
    }
}

async fn worker_pid(work: &Path) -> String {
    let mut entries = tokio::fs::read_dir("/proc").await.unwrap();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        if tokio::fs::read_link(entry.path().join("cwd"))
            .await
            .as_deref()
            .ok()
            == Some(work)
        {
            return entry.file_name().into_string().unwrap();
        }
    }
    panic!("package must be running before removing its lease renewals");
}

// TLS termination stays outside the product router, as in deployment. Dropping
// bytes in both directions still leaves both real TCP sockets open.
const RELAY: &str = r#"
const fs = require('node:fs');
const net = require('node:net');
const tls = require('node:tls');
const port = Number(process.argv[1]);
let next = 0;
const server = tls.createServer({
  key: fs.readFileSync('server.key'), cert: fs.readFileSync('server.pem')
}, downstream => {
  const upstream = net.connect(port, '127.0.0.1');
  const first = next++ === 0;
  const blocked = () => first && fs.existsSync('stall');
  for (const [source, target] of [[upstream, downstream], [downstream, upstream]]) {
    source.on('data', bytes => { if (!blocked()) target.write(bytes); });
    source.on('end', () => { if (!blocked()) target.end(); });
    source.on('error', () => { if (!blocked()) target.destroy(); });
  }
});
server.listen(0, '127.0.0.1', () => console.log(server.address().port + ' ' + process.execPath));
"#;

struct TlsRelay {
    link: Link,
    port: u16,
    node: String,
    stall: PathBuf,
    _process: Child,
}

impl TlsRelay {
    async fn start(root: &Path, server: SocketAddr) -> Self {
        async fn openssl(root: &Path, args: &[&str]) {
            let output = Command::new("openssl")
                .current_dir(root)
                .args(args)
                .output()
                .await
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        openssl(
            root,
            &[
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-noenc",
                "-days",
                "1",
                "-subj",
                "/CN=Disposable carrier CA",
                "-keyout",
                "ca.key",
                "-out",
                "ca.pem",
            ],
        )
        .await;
        openssl(
            root,
            &[
                "req",
                "-new",
                "-newkey",
                "rsa:2048",
                "-noenc",
                "-subj",
                "/CN=127.0.0.1",
                "-keyout",
                "server.key",
                "-out",
                "server.csr",
            ],
        )
        .await;
        tokio::fs::write(
            root.join("extensions"),
            b"subjectAltName=IP:127.0.0.1\nextendedKeyUsage=serverAuth\n",
        )
        .await
        .unwrap();
        openssl(
            root,
            &[
                "x509",
                "-req",
                "-in",
                "server.csr",
                "-CA",
                "ca.pem",
                "-CAkey",
                "ca.key",
                "-CAcreateserial",
                "-days",
                "1",
                "-extfile",
                "extensions",
                "-out",
                "server.pem",
            ],
        )
        .await;
        let mut process = Command::new("node")
            .current_dir(root)
            .args(["-e", RELAY, &server.port().to_string()])
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut port = String::new();
        BufReader::new(process.stdout.take().unwrap())
            .read_line(&mut port)
            .await
            .unwrap();
        let (port, node) = port.trim().split_once(' ').unwrap();
        let port: u16 = port.parse().unwrap();
        let ca = tokio::fs::read(root.join("ca.pem")).await.unwrap();
        let link = Link::with_space_test_ca(
            &format!("https://127.0.0.1:{port}"),
            reqwest::Certificate::from_pem(&ca).unwrap(),
        )
        .unwrap();
        Self {
            link,
            port,
            node: node.to_owned(),
            stall: root.join("stall"),
            _process: process,
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn enrolled_runner_multiplexes_maximum_results_and_fences_lost_or_erased_spaces() {
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
            let metadata = if space == ids.personal {
                let empty = calternal_plugin_host::space_channel::SpaceFrame {
                    v: 1,
                    epoch: Uuid::nil(),
                    seq: u64::MAX,
                    package: Some("calternal-head-fixture".into()),
                    digest: Some("0".repeat(64)),
                    ticket: Some(Uuid::nil()),
                    message: SpaceMessage::Reply {
                        callback_id: Uuid::nil(),
                        result: Ok(json!({
                            "metadata":{"title":"Enrolled","padding":""},
                            "revision":"0".repeat(35)
                        })),
                    },
                };
                let overhead = serde_json::to_vec(&empty).unwrap().len() + 1;
                json!({"title":"Enrolled", "padding": "x".repeat(MAX_TICKET_RESULT_FRAME - overhead)})
            } else {
                json!({"title":"Enrolled"})
            };
            content
                .write(
                    space,
                    None,
                    "notes",
                    None,
                    metadata,
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
                .unwrap()
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
                    digest: sha256_hex(child_source().as_bytes()),
                },
                if space == ids.personal {
                    "fixture.head"
                } else {
                    "fixture.other"
                }
                .into(),
                "notes".into(),
                child_source(),
            )
            .unwrap()
        });
        let registry = Registry::empty()
            .with_space_item_heads(Vec::from(approvals))
            .unwrap();
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
        Arc::new(FixtureBackend::new(&placement_root, relay.node)),
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
    let dispatch_read = |request, space, item, operation| {
        dispatch(
            state.clone(),
            Identity { id: ids.owner },
            Session::Static,
            request,
            operation,
            serde_json::from_value(json!({"space_id":space,"item_id":item})).unwrap(),
        )
    };
    let expected = |item: &Item| json!({"metadata":item.metadata, "revision":item.revision});
    assert_eq!(items[0].revision.len(), 35);
    let requests: Vec<_> = (0..MAX_ACTIVE_TICKETS)
        .map(|i| format!("fixture-burst-{i}"))
        .collect();
    let reads = futures_util::future::join_all(
        requests
            .iter()
            .map(|request| dispatch_read(request, ids.personal, items[0].id, "fixture.head")),
    );
    for answer in tokio::time::timeout(Duration::from_secs(15), reads)
        .await
        .unwrap()
    {
        assert_eq!(answer.unwrap(), expected(&items[0]));
    }
    assert!(
        state
            .registry
            .space_item_head(ids.personal, "fixture.head")
            .unwrap()
            .wake
            .awake
            .lock()
            .await
            .as_ref()
            .unwrap()
            .sender
            .is_live()
    );
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
    let answer_b = tokio::time::timeout(
        Duration::from_secs(3),
        dispatch_read("fixture-three", ids.shared, items[1].id, "fixture.other"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(answer_b, expected(&items[1]));
    assert!(tokio::fs::try_exists(format!("/proc/{pid}")).await.unwrap());
    let second_assignment = assigned.recv().await.unwrap();
    assert!(matches!(second_assignment, Down::Assign { space_id, .. } if space_id == ids.shared));
    supervisor.await.unwrap();
    tokio::time::timeout(
        Duration::from_secs(5).saturating_sub(lost.elapsed()),
        async {
            while tokio::fs::try_exists(format!("/proc/{pid}")).await.unwrap() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        },
    )
    .await
    .expect("product supervisor must stop and reap the package");
    assert!(lost.elapsed() >= Duration::from_secs(3));
    let awake = state
        .registry
        .space_item_head(ids.shared, "fixture.other")
        .unwrap()
        .wake
        .awake
        .lock()
        .await
        .as_ref()
        .unwrap()
        .clone();
    assert!(awake.sender.is_live());
    let storage = state.store.shared_content();
    let space = ids.shared;
    let reader = tokio::task::block_in_place(|| storage.item_head_reader(space)).unwrap();
    dispatch(
        state.clone(),
        Identity { id: ids.owner },
        Session::Static,
        "erase-space",
        "spaces.delete",
        serde_json::from_value(json!({"space_id":ids.shared})).unwrap(),
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        while awake.sender.is_live() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("owner erasure must fence the awake carrier");
    tokio::task::block_in_place(|| {
        assert!(reader.read("notes", items[1].id).is_err());
        assert!(storage.item_head_reader(space).is_err());
    });
    let erased = root
        .path()
        .join("appdata")
        .join("spaces")
        .join(space.to_string());
    let mut fds = tokio::fs::read_dir("/proc/self/fd").await.unwrap();
    while let Some(fd) = fds.next_entry().await.unwrap() {
        if let Ok(open) = tokio::fs::read_link(fd.path()).await {
            assert!(
                !open.starts_with(&erased),
                "erased SQLite handle remains open: {open:?}"
            );
        }
    }
    let Down::Assign { spec, .. } = second_assignment else {
        unreachable!()
    };
    let work = placement_root
        .join("workers")
        .join(spec.thread_id.to_string())
        .join("work");
    tokio::time::timeout(Duration::from_secs(5), async {
        while tokio::fs::try_exists(&work).await.unwrap() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    server.abort();
    let _ = server.await;
    drop(root);
}
