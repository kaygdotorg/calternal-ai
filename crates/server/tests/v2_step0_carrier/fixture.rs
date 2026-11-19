//! Enrollment and real subprocesses for the disposable carrier.
use super::wire;
use calternal_machine::{
    identity::MachineKey,
    protocol::{EnrolRequest, Owner, SessionFinish},
};
use calternal_plugin_host::{Authority, Frame, decode_frame, encode_frame};
use calternal_server::machines::{MachineService, Settings};
use calternal_types::SpaceId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{BufReader as ChildReader, Write},
    path::Path,
    process::{Child, ChildStdout, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use uuid::Uuid;

pub(super) fn source() -> std::path::PathBuf {
    std::env::var_os("STEP0_PACKAGE")
        .map(Into::into)
        .unwrap_or_else(|| {
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/v2_step0_carrier/package.cjs"
            )
            .into()
        })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Hello {
    pub(super) token: String,
    pub(super) space: SpaceId,
    pub(super) v: u8,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Welcome {
    pub(super) v: u8,
    pub(super) space: SpaceId,
    pub(super) epoch: Uuid,
    pub(super) credit: usize,
    pub(super) control: usize,
    pub(super) lease_ms: u128,
}

pub(super) struct Enrollment {
    pub(super) service: Arc<MachineService>,
    pub(super) auth: calternal_auth::spaces::Spaces,
    pub(super) id: Uuid,
    pub(super) token: String,
    pub(super) authority: Authority,
    pub(super) allowed: Vec<SpaceId>,
}

pub(super) fn enroll(dir: &Path, extra_spaces: usize) -> Enrollment {
    let auth = calternal_auth::spaces::Spaces::open(&dir.join("system/auth.db")).unwrap();
    let ids = auth.seed_development().unwrap();
    let account = ids.owner;
    let service = MachineService::open(dir, auth.machines(), Settings::default()).unwrap();
    let (code, _) = service.issue_code(Owner::Account { id: account }).unwrap();
    let key = MachineKey::generate().unwrap();
    let id = service
        .enrol(&EnrolRequest {
            code,
            public_key: key.public_key_b64(),
            name: "carrier-fixture".into(),
            os: "linux".into(),
            arch: "x86_64".into(),
            direct: false,
        })
        .unwrap()
        .machine_id;
    let challenge = service.session_start(id).unwrap();
    let session = service
        .session_finish(&SessionFinish {
            machine_id: id,
            signature: key.sign_session(id, &challenge),
            challenge,
            boot_id: Uuid::new_v4(),
            protocol: calternal_machine::protocol::PROTOCOL_VERSION,
            guest_memory_mib: None,
            backend: "podman".into(),
        })
        .unwrap();
    let mut allowed = vec![ids.personal, ids.shared];
    for i in 0..extra_spaces {
        allowed.push(auth.create(account, &format!("Idle {i}")).unwrap().id);
    }
    Enrollment {
        service,
        auth,
        id,
        token: session.token,
        authority: Authority {
            account_id: account,
            space_id: allowed[0],
            grant: "fixture".into(),
        },
        allowed,
    }
}

struct WorkerProcess {
    child: Child,
    responses: ChildReader<ChildStdout>,
}

pub(super) struct Packages {
    processes: [Mutex<WorkerProcess>; 2],
    pids: [u32; 2],
    stopped: AtomicBool,
}
impl Packages {
    pub(super) fn start() -> Self {
        let start = |_| {
            let source = source();
            let mut child = Command::new("node")
                .arg("--permission")
                .arg(format!("--allow-fs-read={}", source.display()))
                .arg(source)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            let responses = ChildReader::new(child.stdout.take().unwrap());
            Mutex::new(WorkerProcess { child, responses })
        };
        let processes = std::array::from_fn(start);
        let pids = std::array::from_fn(|i| processes[i].lock().unwrap().child.id());
        Self {
            processes,
            pids,
            stopped: AtomicBool::new(false),
        }
    }

    pub(super) fn request(&self, package: usize, frame: Value) -> Frame {
        let mut worker = self.processes[package].lock().unwrap();
        let bytes = encode_frame(&decode_frame(&wire::encode(&frame)).unwrap()).unwrap();
        worker
            .child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(&bytes)
            .unwrap();
        let response = wire::child_line(&mut worker.responses).unwrap();
        assert!(wire::strict(&response));
        decode_frame(&response).unwrap()
    }

    pub(super) fn stop(&self) -> usize {
        if self.stopped.swap(true, Ordering::AcqRel) {
            return 0;
        }
        // Kill both direct children before taking their I/O locks. A blocking
        // read from a child cannot indefinitely delay its lease teardown.
        for pid in self.pids {
            // Child is still owned and has not been reaped.
            unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
        }
        self.processes
            .iter()
            .map(|p| {
                let mut child = p.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                usize::from(child.child.wait().is_ok())
            })
            .sum()
    }
}
impl Drop for Packages {
    fn drop(&mut self) {
        self.stop();
    }
}
