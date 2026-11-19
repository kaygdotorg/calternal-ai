//! Exercise the actual long-poll drain barrier with a queued direct-mode turn.
use super::*;
use crate::{
    identity::MachineKey,
    protocol::{ChunkEvent, Down, Envelope, OutputAck, OutputPost, Owner, SandboxSpec},
    sandbox::direct::DirectBackend,
    update::tests::{NEW, OLD, mock, staged},
};
use calternal_types::{AccountId, SpaceId};
use std::sync::atomic::{AtomicBool, Ordering};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn waits_for_the_turn_and_server_ack_before_replacing_the_program() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("calternal");
    std::fs::write(&target, OLD).unwrap();
    let release = staged(temp.path(), &target).await;
    let release_ready = Some(tokio::spawn(async move { Ok(release) }));

    let proceed = temp.path().join("finish-turn");
    let marker = temp.path().join("turn-finished");
    let allow_turn = proceed.clone();
    let saw_exit = Arc::new(AtomicBool::new(false));
    let reported_exit = saw_exit.clone();
    let lease_id = Uuid::new_v4();
    let account_id = AccountId::new();
    let command = vec![
        "/bin/sh".to_owned(),
        "-c".to_owned(),
        "while [ ! -f \"$1\" ]; do sleep 0.02; done; printf done > \"$2\"".to_owned(),
        "turn".to_owned(),
        proceed.display().to_string(),
        marker.display().to_string(),
    ];
    let assignment = vec![
        Envelope {
            seq: 1,
            message: Down::Assign {
                lease_id,
                fence: 1,
                space_id: SpaceId::new(),
                account_id,
                spec: SandboxSpec {
                    code_directory: None,
                    space_id: None,
                    provider_login: false,
                    shared_provider_sponsor: None,
                    account_id: account_id.as_uuid(),
                    thread_id: Uuid::new_v4(),
                    image: None,
                    worker: None,
                    limits: Default::default(),
                },
            },
        },
        Envelope {
            seq: 2,
            message: Down::Exec {
                interactive: false,
                lease_id,
                fence: 1,
                exec_id: Uuid::new_v4(),
                argv: command,
                stdin: String::new(),
                env: Vec::new(),
            },
        },
    ];
    let server = mock(move |path, body| {
        let value = match path {
            "/api/v1/machine/session/start" => serde_json::json!({"challenge":"dummy"}),
            "/api/v1/machine/session/finish" => serde_json::json!({
                "backend":"direct", "token":"fixture", "lease_ttl_ms": 30000, "poll_hold_ms": 100,
            }),
            "/api/v1/machine/sync" => {
                let req: SyncRequest = serde_json::from_slice(body).unwrap();
                if req.draining && !req.leases.is_empty() {
                    std::fs::write(&allow_turn, b"go").unwrap();
                }
                let messages = if req.ack == 0 {
                    assignment.clone()
                } else if req.ack == 2 && reported_exit.load(Ordering::Acquire) {
                    vec![Envelope {
                        seq: 3,
                        message: Down::Stop {
                            lease_id,
                            reason: "turn complete".into(),
                            keep_files: true,
                        },
                    }]
                } else {
                    Vec::new()
                };
                serde_json::to_value(SyncResponse { messages }).unwrap()
            }
            "/api/v1/machine/output" => {
                let post: OutputPost = serde_json::from_slice(body).unwrap();
                if post.chunks.iter().any(
                    |chunk| matches!(&chunk.event, ChunkEvent::Exit(exit) if exit.code == Some(0)),
                ) {
                    reported_exit.store(true, Ordering::Release);
                }
                serde_json::to_value(OutputAck {
                    acked: post.chunks.last().unwrap().seq,
                    retry_after_ms: None,
                })
                .unwrap()
            }
            _ => return None,
        };
        Some(serde_json::to_vec(&value).unwrap())
    })
    .await;
    let creds = Credentials {
        server: server.origin.clone(),
        machine_id: Uuid::new_v4(),
        owner: Owner::Account { id: account_id },
        key: MachineKey::generate().unwrap(),
    };
    let link = Arc::new(Link::new(&server.origin).unwrap());
    let runner = Arc::new(Runner::new(
        Arc::new(DirectBackend::new(temp.path().join("sandboxes"))),
        link.clone(),
        2,
    ));
    let mut release_ready = release_ready;
    let ready = Ready::default();
    let mut ack = 0;
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        session_loop(
            &creds,
            &link,
            &runner,
            Uuid::new_v4(),
            &ready,
            &mut ack,
            &mut release_ready,
        ),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    assert!(saw_exit.load(Ordering::Acquire));
    assert_eq!(std::fs::read(marker).unwrap(), b"done");
    assert_eq!(std::fs::read(&target).unwrap(), OLD);
    assert!(runner.reports().is_empty());
    result.publish().unwrap();
    assert_eq!(std::fs::read(target).unwrap(), NEW);
}
