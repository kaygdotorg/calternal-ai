use super::*;

#[test]
fn slice_after_returns_only_subsequent_records_in_order() {
    let records = vec![
        json!({"anchor": "a1", "body": "one"}),
        json!({"anchor": "a2", "body": "two"}),
        json!({"anchor": "a3", "body": "three"}),
    ];
    let (mode, delta, after) = slice_after(&records, Some("a1"));
    assert_eq!(mode, "delta");
    assert_eq!(after.as_deref(), Some("a1"));
    assert_eq!(delta.len(), 2);
    assert_eq!(delta[0]["anchor"], "a2");
    assert_eq!(delta[1]["anchor"], "a3");
}

#[test]
fn slice_after_falls_back_to_snapshot_for_unknown_anchor() {
    let records = vec![
        json!({"anchor": "a1", "body": "one"}),
        json!({"anchor": "a2", "body": "two"}),
    ];
    let (mode, out, after) = slice_after(&records, Some("gone"));
    assert_eq!(mode, "snapshot");
    assert!(after.is_none());
    assert_eq!(out.len(), 2);
}

#[test]
fn hub_notifies_only_matching_space() {
    let mut hub = Hub::default();
    let mut other_space = Hub::default();
    let owner = AccountId::new();
    let other = AccountId::new();
    let mut rx1 = hub.subscribe(owner);
    let mut rx1_other = hub.subscribe(other);
    let mut rx2 = other_space.subscribe(owner);
    hub.notify(None);
    assert!(rx1.try_recv().is_ok());
    assert!(rx1_other.try_recv().is_ok());
    assert!(rx2.try_recv().is_err());

    // Simulate an idle peer in a multi-person space. Private changes
    // must neither enqueue an event for the peer nor lag its channel.
    for _ in 0..(SPACE_CHANNEL_CAPACITY + 1) {
        hub.notify(Some(owner));
    }
    assert!(matches!(
        rx1.try_recv(),
        Err(broadcast::error::TryRecvError::Lagged(_))
    ));
    assert!(matches!(
        rx1_other.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    ));
    assert!(matches!(
        rx2.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    ));
}
