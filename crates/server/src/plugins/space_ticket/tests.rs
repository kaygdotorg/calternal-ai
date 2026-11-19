use super::*;
use calternal_auth::{
    spaces::Spaces,
    token::{hash32, normalize_setup_token},
};
use calternal_plugin_host::space_channel::{PackagePin, SpaceFrame, SpaceMessage, SpaceMethod};
use serde_json::json;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[test]
fn ticket_is_refused_after_grant_revision_moves_or_epoch_changes() {
    let root = tempfile::tempdir().unwrap();
    let spaces = Spaces::open(&root.path().join("auth.db")).unwrap();
    let ids = spaces.seed_development().unwrap();
    let machine = spaces.machines();
    let (code, _) = machine.issue_code(None, Duration::from_secs(30)).unwrap();
    let machine = machine
        .redeem_code(
            hash32(&normalize_setup_token(&code).unwrap()),
            vec![3; 32],
            "fixture".into(),
            "linux".into(),
            "x86_64".into(),
            false,
        )
        .unwrap()
        .unwrap();
    let grants = spaces.grant_revision(ids.shared, machine.id).unwrap();
    let reader = spaces.invocation_reader(&grants).unwrap();
    let epoch = Uuid::new_v4();
    let pin = PackagePin {
        package: "calternal-fixture".into(),
        digest: "a".repeat(64),
    };
    let scope = Scope {
        actor: ids.guest,
        installation: None,
        package: pin.clone(),
        epoch,
        operation: "fixture.head".into(),
        kind: "notes".into(),
        item: Uuid::new_v4(),
    };
    assert!(Ticket::mint(&reader, scope.clone()).is_err());
    spaces
        .set_role(ids.owner, ids.shared, ids.guest, "editor")
        .unwrap();
    // The role write can reply before its worker publishes the clean latch.
    // A queued authority read joins publication before testing the new rights.
    let published = spaces.grant_revision(ids.shared, machine.id).unwrap();
    assert_eq!(reader.grants().current(), published.current());
    let ticket = Ticket::mint(&reader, scope.clone()).unwrap();
    let mut frame = SpaceFrame {
        v: 1,
        epoch,
        seq: 1,
        package: Some(pin.package),
        digest: Some(pin.digest),
        ticket: Some(ticket.id()),
        message: SpaceMessage::Call {
            callback_id: Uuid::new_v4(),
            method: SpaceMethod::ItemHead,
            input: json!({}),
        },
    };
    assert!(ticket.authorize_at(&frame, Instant::now()).is_ok());
    frame.epoch = Uuid::new_v4();
    assert!(ticket.authorize_at(&frame, Instant::now()).is_err());
    frame.epoch = epoch;
    assert!(
        ticket
            .authorize_at(&frame, Instant::now() + Duration::from_secs(60))
            .is_err()
    );
    spaces
        .set_role(ids.owner, ids.shared, ids.guest, "reader")
        .unwrap();
    assert!(ticket.authorize_at(&frame, Instant::now()).is_err());
    assert!(Ticket::mint(&reader, scope).is_err());
}
