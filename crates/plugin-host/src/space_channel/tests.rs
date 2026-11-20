use super::*;
use calternal_types::SpaceId;
use serde_json::json;
use tokio::io::duplex;
use uuid::Uuid;

#[tokio::test]
async fn handshake_checks_assignment_and_moves_application_frames() {
    let package = PackagePin {
        package: "calternal-notes".into(),
        digest: "a".repeat(64),
    };
    let space = SpaceId::new();
    let assignment = Uuid::new_v4();
    let (host_io, runtime_io) = duplex(4096);
    let (host, runtime) = tokio::join!(
        SpaceChannel::host(host_io, space, assignment, vec![package.clone()]),
        SpaceChannel::runtime(runtime_io, space, assignment),
    );
    let host = host.unwrap();
    let mut runtime = runtime.unwrap();
    assert_eq!(host.epoch(), runtime.epoch());
    let ticket = Uuid::new_v4();
    host.send(
        &package,
        ticket,
        SpaceMessage::Invoke {
            operation: "head".into(),
            input: json!({"id":"fixture"}),
            deadline_ms: 60_000,
            author_context: crate::space_channel::AuthorContext {
                request_id: "fixture-request".into(),
                actor: calternal_types::AccountId::new(),
                authored_at: "2026-11-20T00:00:00Z".into(),
            },
        },
    )
    .unwrap();
    let received = tokio::time::timeout(Duration::from_secs(1), runtime.receive())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(received.frame().ticket, Some(ticket));
    assert_eq!(received.frame().seq, 1);
    assert!(matches!(
        received.frame().message,
        SpaceMessage::Invoke { .. }
    ));

    let (host_io, runtime_io) = duplex(4096);
    let (_, mismatch) = tokio::join!(
        SpaceChannel::host(host_io, space, assignment, vec![package]),
        SpaceChannel::runtime(runtime_io, space, Uuid::new_v4()),
    );
    assert!(mismatch.is_err());
}

#[test]
fn control_frames_require_a_complete_header_and_exact_message_shape() {
    let canonical = json!({
        "v": 1,
        "epoch": Uuid::new_v4(),
        "seq": 1,
        "package": null,
        "digest": null,
        "ticket": null,
        "message": { "type": "renew" },
    });
    assert!(parse::<SpaceFrame>(format!("{canonical}\n").as_bytes()).is_ok());
    let mut unknown = canonical.clone();
    unknown["message"]["unexpected"] = json!(true);
    assert!(parse::<SpaceFrame>(format!("{unknown}\n").as_bytes()).is_err());
    let mut missing = canonical;
    missing.as_object_mut().unwrap().remove("ticket");
    assert!(parse::<SpaceFrame>(format!("{missing}\n").as_bytes()).is_err());
}

#[tokio::test]
async fn full_windows_wait_and_deliver_each_maximum_result() {
    let package = PackagePin {
        package: "calternal-notes".into(),
        digest: "a".repeat(64),
    };
    let space = SpaceId::new();
    let assignment = Uuid::new_v4();
    let (host_io, runtime_io) = duplex(4096);
    let (host, runtime) = tokio::join!(
        SpaceChannel::host(host_io, space, assignment, vec![package.clone()]),
        SpaceChannel::runtime(runtime_io, space, assignment),
    );
    let mut host = host.unwrap();
    let mut runtime = runtime.unwrap();
    let ticket = Uuid::new_v4();
    let callback_id = Uuid::new_v4();
    for _ in 0..PER_PACKAGE {
        host.send(
            &package,
            ticket,
            SpaceMessage::Invoke {
                operation: "fixture.head".into(),
                input: json!({}),
                deadline_ms: 1_000,
                author_context: crate::space_channel::AuthorContext {
                    request_id: "fixture-request".into(),
                    actor: calternal_types::AccountId::new(),
                    authored_at: "2026-11-20T00:00:00Z".into(),
                },
            },
        )
        .unwrap();
        runtime
            .send(
                &package,
                ticket,
                SpaceMessage::Call {
                    callback_id,
                    method: SpaceMethod::ItemHead,
                    input: json!({}),
                },
            )
            .unwrap();
    }
    let mut retained = Vec::new();
    for _ in 0..PER_PACKAGE {
        retained.push(host.receive().await.unwrap());
        retained.push(runtime.receive().await.unwrap());
    }

    let result = |ticket, host_side| {
        let message = |content: String| {
            if host_side {
                SpaceMessage::Reply {
                    callback_id,
                    result: Ok(json!(content)),
                }
            } else {
                SpaceMessage::Complete {
                    result: Ok(json!(content)),
                }
            }
        };
        let empty = SpaceFrame {
            v: V,
            epoch: host.epoch(),
            seq: u64::MAX,
            package: Some(package.package.clone()),
            digest: Some(package.digest.clone()),
            ticket: Some(ticket),
            message: message(String::new()),
        };
        let overhead = empty.encode(MAX_LINE).unwrap().len();
        message("x".repeat(MAX_TICKET_RESULT_FRAME - overhead))
    };
    let mut sends = tokio::task::JoinSet::new();
    let tickets: Vec<_> = (0..MAX_ACTIVE_TICKETS).map(|_| Uuid::new_v4()).collect();
    for &ticket in &tickets {
        let sender = host.sender();
        let reply = result(ticket, true);
        let pin = package.clone();
        sends.spawn(async move { sender.send_checked_wait(&pin, ticket, reply, || true).await });
        let sender = runtime.sender();
        let complete = result(ticket, false);
        let pin = package.clone();
        sends.spawn(async move { sender.send_wait(&pin, ticket, complete).await });
    }
    assert!(
        timeout(Duration::from_millis(30), sends.join_next())
            .await
            .is_err()
    );
    let live = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let guard = live.clone();
    let sender = host.sender();
    let pin = package.clone();
    let ticket = Uuid::new_v4();
    let waiting_reply = result(ticket, true);
    let mut revoked = tokio::spawn(async move {
        sender
            .send_checked_wait(&pin, ticket, waiting_reply, move || {
                guard.load(std::sync::atomic::Ordering::Acquire)
            })
            .await
    });
    assert!(
        timeout(Duration::from_millis(30), &mut revoked)
            .await
            .is_err()
    );
    live.store(false, std::sync::atomic::Ordering::Release);
    assert_eq!(
        timeout(Duration::from_millis(500), &mut revoked)
            .await
            .unwrap()
            .unwrap(),
        Err(Error::Refused)
    );
    drop(retained);
    timeout(Duration::from_secs(2), async {
        let mut replies = std::collections::HashSet::new();
        let mut completions = std::collections::HashSet::new();
        for _ in 0..MAX_ACTIVE_TICKETS {
            replies.insert(runtime.receive().await.unwrap().frame().ticket.unwrap());
            completions.insert(host.receive().await.unwrap().frame().ticket.unwrap());
        }
        assert_eq!(replies, tickets.into_iter().collect());
        assert_eq!(completions, replies);
        while let Some(sent) = sends.join_next().await {
            sent.unwrap().unwrap();
        }
    })
    .await
    .unwrap();
    assert!(host.is_live() && runtime.is_live());
}
