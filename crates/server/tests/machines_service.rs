//! `MachineService` in process: session proofs, fencing and back-pressure,
//! asserted on exact error codes.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use calternal_types::{AccountId, SpaceId};
use calternal_core::ErrorCode;
use calternal_machine::identity::MachineKey;
use calternal_machine::protocol::{
    Capacity, Chunk, ChunkEvent, Down, EnrolRequest, ExitInfo, LeaseReport, OutputPost, Owner,
    SandboxState, SessionFinish, Stream, SyncRequest,
};
use calternal_server::machines::{MachineService, Settings};
use uuid::Uuid;

fn service(dir: &std::path::Path) -> Arc<MachineService> {
    let settings = Settings {
        lease_ttl: Duration::from_millis(300),
        poll_hold: Duration::from_millis(100),
        output_buffer_bytes: 64,
        ..Settings::default()
    };
    MachineService::open(dir, settings).unwrap()
}

struct M {
    id: Uuid,
    key: MachineKey,
    token: String,
}

fn enrol(svc: &MachineService, owner: AccountId) -> (Uuid, MachineKey) {
    let (code, _) = svc.issue_code(Owner::Account { id: owner }).unwrap();
    let key = MachineKey::generate().unwrap();
    let req = EnrolRequest {
        code,
        public_key: key.public_key_b64(),
        name: "m".into(),
        os: "linux".into(),
        arch: "x86_64".into(),
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
    let challenge = svc.session_start(id).unwrap();
    let finish = SessionFinish {
        machine_id: id,
        signature: key.sign_session(id, &challenge),
        challenge,
        boot_id: boot,
        protocol: 1,
    };
    let token = svc.session_finish(&finish).unwrap().token;
    assert_eq!(
        svc.session_finish(&finish).unwrap_err().code,
        ErrorCode::Unauthorized,
        "challenge replay"
    );
    M { id, key, token }
}

async fn sync(svc: &Arc<MachineService>, m: &M, leases: Vec<LeaseReport>) -> Vec<Down> {
    let req = SyncRequest {
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
    let svc = service(dir.path());
    let (id, _key) = enrol(&svc, AccountId::new());
    let wrong = MachineKey::generate().unwrap();
    let challenge = svc.session_start(id).unwrap();
    let forged = SessionFinish {
        machine_id: id,
        signature: wrong.sign_session(id, &challenge),
        challenge,
        boot_id: Uuid::new_v4(),
        protocol: 1,
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
    let svc = service(dir.path());
    let owner = AccountId::new();
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

    // A goes silent past the TTL: the lease is fenced and moves to B.
    tokio::time::sleep(Duration::from_millis(400)).await;
    svc.sweep().unwrap();
    assert_eq!(
        svc.run(owner, lease_id).unwrap().state,
        calternal_server::machines::LeaseState::Waiting
    );
    let b = connect(&svc, b_id, b_key, Uuid::new_v4());
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
