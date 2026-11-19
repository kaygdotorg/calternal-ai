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
