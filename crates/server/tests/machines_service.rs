//! `MachineService` in process: session proofs, fencing and back-pressure,
//! asserted on exact error codes.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use calternal_core::ErrorCode;
use calternal_machine::identity::MachineKey;
use calternal_machine::protocol::{
    Capacity, Chunk, ChunkEvent, Down, EnrolRequest, ExitInfo, LeaseReport, OutputPost, Owner,
    SandboxState, SessionFinish, Stream, SyncRequest,
};
use calternal_server::machines::{MachineService, Settings};
use calternal_types::{AccountId, SpaceId};
use uuid::Uuid;

/// A service over a fresh auth.db with one existing account (machine authority
/// requires owners to exist). Authority calls go to the auth worker, so they run
/// outside the async worker, as the HTTP layer does.
fn service(dir: &std::path::Path) -> (Arc<MachineService>, AccountId) {
    let settings = Settings {
        lease_ttl: Duration::from_millis(300),
        poll_hold: Duration::from_millis(100),
        output_buffer_bytes: 64,
        ..Settings::default()
    };
    tokio::task::block_in_place(|| {
        let spaces = calternal_auth::spaces::Spaces::open(&dir.join("system/auth.db")).unwrap();
        let owner = spaces.seed_development().unwrap().owner;
        (
            MachineService::open(dir, spaces.machines(), settings).unwrap(),
            owner,
        )
    })
}

struct M {
    id: Uuid,
    key: MachineKey,
    token: String,
}

fn enrol(svc: &MachineService, owner: AccountId) -> (Uuid, MachineKey) {
    tokio::task::block_in_place(|| enrol_blocking(svc, owner))
}

fn enrol_blocking(svc: &MachineService, owner: AccountId) -> (Uuid, MachineKey) {
    let (code, _) = svc.issue_code(Owner::Account { id: owner }).unwrap();
    let key = MachineKey::generate().unwrap();
    let req = EnrolRequest {
        code,
        public_key: key.public_key_b64(),
        name: "m".into(),
        os: "linux".into(),
        arch: "x86_64".into(),
        direct: false,
    };
    let resp = svc.enrol(&req).unwrap();
    assert_eq!(
        svc.enrol(&req).unwrap_err().code,
        ErrorCode::NotFound,
        "code replay"
    );
    (resp.machine_id, key)
}

fn connect(svc: &MachineService, id: Uuid, key: MachineKey, boot: Uuid) -> M {
    tokio::task::block_in_place(|| {
        let challenge = svc.session_start(id).unwrap();
        let finish = SessionFinish {
            machine_id: id,
            signature: key.sign_session(id, &challenge),
            challenge,
            boot_id: boot,
            protocol: calternal_machine::protocol::PROTOCOL_VERSION,
            backend: "podman".into(),
        };
        let token = svc.session_finish(&finish).unwrap().token;
        assert_eq!(
            svc.session_finish(&finish).unwrap_err().code,
            ErrorCode::Unauthorized,
            "challenge replay"
        );
        M { id, key, token }
    })
}

async fn sync(svc: &Arc<MachineService>, m: &M, leases: Vec<LeaseReport>) -> Vec<Down> {
    let req = SyncRequest {
        discards: vec![],
        ack: 0,
        capacity: Capacity {
            cpus: 1,
            memory_bytes: 0,
            max_sandboxes: 2,
            running_sandboxes: 0,
        },
        draining: false,
        leases,
    };
    svc.sync(m.id, &m.token, req)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.message)
        .collect()
}

fn data(seq: u64, n: usize) -> Chunk {
    let data = base64::engine::general_purpose::STANDARD.encode(vec![b'x'; n]);
    Chunk {
        seq,
        event: ChunkEvent::Data {
            stream: Stream::Stdout,
            data,
        },
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn sessions_need_the_machine_key() {
    let dir = tempfile::tempdir().unwrap();
    let (svc, owner) = service(dir.path());
    let (id, _key) = enrol(&svc, owner);
    let wrong = MachineKey::generate().unwrap();
    let challenge = svc.session_start(id).unwrap();
    let forged = SessionFinish {
        machine_id: id,
        signature: wrong.sign_session(id, &challenge),
        challenge,
        boot_id: Uuid::new_v4(),
        protocol: calternal_machine::protocol::PROTOCOL_VERSION,
        backend: "podman".into(),
    };
    assert_eq!(
        svc.session_finish(&forged).unwrap_err().code,
        ErrorCode::Unauthorized
    );
    assert_eq!(
        svc.authenticate("cm1_nonsense").unwrap_err().code,
        ErrorCode::Unauthorized
    );
    assert_eq!(
        svc.session_start(Uuid::new_v4()).unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_fence_and_wrong_machine_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (svc, owner) = service(dir.path());
    let (a_id, a_key) = enrol(&svc, owner);
    let (b_id, b_key) = enrol(&svc, owner);
    let a = connect(&svc, a_id, a_key, Uuid::new_v4());
    sync(&svc, &a, vec![]).await;
    let run = svc
        .request_run(SpaceId::new(), owner, Uuid::new_v4(), None)
        .unwrap();
    let exec = svc
        .exec(owner, run.lease_id, vec!["true".into()], vec![])
        .unwrap();
    let msgs = sync(&svc, &a, vec![]).await;
    let Some(Down::Assign {
        fence, lease_id, ..
    }) = msgs.first().cloned()
    else {
        panic!("{msgs:?}")
    };
    assert!(matches!(msgs.get(1), Some(Down::Exec { exec_id, .. }) if *exec_id == exec));
    let post = |fence, chunks| OutputPost {
        lease_id,
        fence,
        exec_id: exec,
        chunks,
    };

    // Back-pressure: a 64-byte buffer takes the first 60 bytes, not the next 60.
    let ack = svc
        .output(a.id, post(fence, vec![data(1, 60), data(2, 60)]))
        .unwrap();
    assert_eq!((ack.acked, ack.retry_after_ms.is_some()), (1, true));
    let view = svc
        .read_output(owner, exec, 1, Duration::ZERO)
        .await
        .unwrap();
    assert!(view.chunks.is_empty());
    assert_eq!(
        svc.output(a.id, post(fence, vec![data(2, 60)]))
            .unwrap()
            .acked,
        2
    );

    // Another machine cannot deliver for A's lease.
    assert_eq!(
        svc.output(b_id, post(fence, vec![data(3, 1)]))
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );

    // A goes silent past the TTL: the lease is fenced and stays interrupted.
    tokio::time::sleep(Duration::from_millis(400)).await;
    svc.sweep().unwrap();
    assert_eq!(
        svc.run(owner, lease_id).unwrap().state,
        calternal_server::machines::LeaseState::Interrupted
    );
    let b = connect(&svc, b_id, b_key, Uuid::new_v4());
    svc.resume_run(owner, lease_id).unwrap();
    let to_b = sync(&svc, &b, vec![]).await;
    let Some(Down::Assign {
        fence: new_fence, ..
    }) = to_b.first().cloned()
    else {
        panic!("{to_b:?}")
    };
    assert!(new_fence > fence);
    assert_eq!(
        svc.authenticate(&a.token).unwrap_err().code,
        ErrorCode::Unauthorized,
        "A's session dropped"
    );
    // A reconnects; its old fence is still refused and it is told to stop.
    let a = connect(&svc, a.id, a.key, Uuid::new_v4());
    assert_eq!(
        svc.output(a.id, post(fence, vec![data(3, 1)]))
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    let report = LeaseReport {
        lease_id,
        fence,
        sandbox: SandboxState::Running,
    };
    let to_a = sync(&svc, &a, vec![report]).await;
    assert!(
        to_a.iter()
            .any(|m| matches!(m, Down::Stop { lease_id: l, .. } if *l == lease_id)),
        "{to_a:?}"
    );
    // The exec that ran on A ended as lost, after the data A delivered in time.
    let view = svc
        .read_output(owner, exec, 0, Duration::ZERO)
        .await
        .unwrap();
    assert_eq!(
        view.exit.and_then(|e| e.error).as_deref(),
        Some("the machine stopped responding")
    );
    // B delivers with the new fence.
    let exit = Chunk {
        seq: 1,
        event: ChunkEvent::Exit(ExitInfo {
            code: Some(0),
            ..ExitInfo::default()
        }),
    };
    let ack = svc
        .output(
            b.id,
            OutputPost {
                lease_id,
                fence: new_fence,
                exec_id: Uuid::new_v4(),
                chunks: vec![exit],
            },
        )
        .unwrap();
    assert_eq!(ack.acked, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_backend_selection_follows_owner_consent_and_pool_cannot_be_direct() {
    let dir = tempfile::tempdir().unwrap();
    let (svc, owner) = service(dir.path());
    let (id, key) = enrol(&svc, owner);
    let handshake = |backend: &str| {
        let challenge = svc.session_start(id).unwrap();
        svc.session_finish(&SessionFinish {
            machine_id: id,
            signature: key.sign_session(id, &challenge),
            challenge,
            boot_id: Uuid::new_v4(),
            protocol: calternal_machine::protocol::PROTOCOL_VERSION,
            backend: backend.into(),
        })
    };
    assert_eq!(handshake("auto").unwrap().backend, "podman");
    assert_eq!(
        tokio::task::block_in_place(|| svc.set_direct(AccountId::new(), id, true))
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    tokio::task::block_in_place(|| svc.set_direct(owner, id, true)).unwrap();
    assert_eq!(handshake("auto").unwrap().backend, "direct");
    tokio::task::block_in_place(|| svc.set_direct(owner, id, false)).unwrap();
    assert_eq!(handshake("auto").unwrap().backend, "podman");
    assert_eq!(handshake("direct").unwrap_err().code, ErrorCode::Forbidden);
    let (code, _) = tokio::task::block_in_place(|| svc.issue_code(Owner::Pool)).unwrap();
    let pool_key = MachineKey::generate().unwrap();
    let pool = tokio::task::block_in_place(|| {
        svc.enrol(&EnrolRequest {
            code,
            public_key: pool_key.public_key_b64(),
            name: "pool".into(),
            os: "linux".into(),
            arch: "x86_64".into(),
            direct: false,
        })
    })
    .unwrap()
    .machine_id;
    let challenge = svc.session_start(pool).unwrap();
    let grant = svc
        .session_finish(&SessionFinish {
            machine_id: pool,
            signature: pool_key.sign_session(pool, &challenge),
            challenge,
            boot_id: Uuid::new_v4(),
            protocol: calternal_machine::protocol::PROTOCOL_VERSION,
            backend: "auto".into(),
        })
        .unwrap();
    assert_eq!(grant.backend, "podman");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_discard_is_durable_and_only_its_machine_can_acknowledge() {
    use calternal_machine::protocol::DiscardReport;
    let dir = tempfile::tempdir().unwrap();
    let (svc, account) = service(dir.path());
    let (id, key) = enrol(&svc, account);
    let machine = connect(&svc, id, key, Uuid::new_v4());
    sync(&svc, &machine, vec![]).await;
    let run = svc
        .request_run(SpaceId::new(), account, Uuid::new_v4(), None)
        .unwrap();
    svc.release_run(account, run.lease_id).unwrap();
    drop(svc);
    let (svc, _) = service(dir.path()); // Offline requests must survive restart.
    assert_eq!(
        tokio::task::block_in_place(|| svc.discard_run(account, run.lease_id, false)).unwrap(),
        "pending"
    );
    assert_eq!(
        tokio::task::block_in_place(|| svc.discard_run(AccountId::new(), run.lease_id, false))
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    drop(svc);
    let (svc, _) = service(dir.path());
    let machine = connect(&svc, machine.id, machine.key, Uuid::new_v4());
    let messages = sync(&svc, &machine, vec![]).await;
    let request = messages
        .iter()
        .find_map(|m| {
            if let Down::Discard { request_id, .. } = m {
                Some(*request_id)
            } else {
                None
            }
        })
        .expect("discard was not recovered");
    let (other_id, key) = enrol(&svc, account);
    let other = connect(&svc, other_id, key, Uuid::new_v4());
    let report = |id, success| SyncRequest {
        ack: 1000,
        capacity: Capacity::default(),
        draining: false,
        leases: vec![],
        discards: vec![DiscardReport {
            request_id: id,
            success,
        }],
    };
    svc.sync(other.id, &other.token, report(request, true))
        .await
        .unwrap();
    assert_eq!(
        tokio::task::block_in_place(|| svc.discard_run(account, run.lease_id, false)).unwrap(),
        "pending"
    );
    svc.sync(machine.id, &machine.token, report(request, false))
        .await
        .unwrap();
    assert_eq!(
        tokio::task::block_in_place(|| svc.discard_run(account, run.lease_id, false)).unwrap(),
        "failed"
    );
    assert_eq!(
        tokio::task::block_in_place(|| svc.discard_run(account, run.lease_id, true)).unwrap(),
        "pending"
    );
    let messages = sync(&svc, &machine, vec![]).await;
    let next = messages
        .iter()
        .find_map(|m| {
            if let Down::Discard { request_id, .. } = m {
                Some(*request_id)
            } else {
                None
            }
        })
        .unwrap();
    assert_ne!(request, next);
    svc.sync(machine.id, &machine.token, report(request, false))
        .await
        .unwrap();
    svc.sync(machine.id, &machine.token, report(next, true))
        .await
        .unwrap();
    assert_eq!(
        tokio::task::block_in_place(|| svc.discard_run(account, run.lease_id, false)).unwrap(),
        "done"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pending_discard_never_replays_over_a_replacement_turn() {
    let dir = tempfile::tempdir().unwrap();
    let (svc, account) = service(dir.path());
    let (id, key) = enrol(&svc, account);
    let machine = connect(&svc, id, key, Uuid::new_v4());
    sync(&svc, &machine, vec![]).await;
    let space = SpaceId::new();
    let thread = Uuid::new_v4();
    let old = svc.request_run(space, account, thread, None).unwrap();
    svc.release_run(account, old.lease_id).unwrap();
    assert_eq!(
        tokio::task::block_in_place(|| svc.discard_run(account, old.lease_id, false)).unwrap(),
        "pending"
    );
    let replacement = svc.request_run(space, account, thread, None).unwrap();
    let machine = connect(&svc, machine.id, machine.key, Uuid::new_v4());
    let messages = sync(&svc, &machine, vec![]).await;
    assert!(
        !messages.iter().any(|m| matches!(m, Down::Discard { .. })),
        "stale deletion could remove replacement work"
    );
    assert_eq!(
        tokio::task::block_in_place(|| svc.discard_run(account, old.lease_id, false))
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    svc.release_run(account, replacement.lease_id).unwrap();
    assert_eq!(
        tokio::task::block_in_place(|| svc.discard_run(account, old.lease_id, false))
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
}
