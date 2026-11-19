use super::*;

#[test]
fn a_space_admits_32_rooms_with_32_participants_each() {
    let hub = Hub::default();
    let space = SpaceId::new();
    let account = AccountId::new();
    let mut retained = Vec::new();
    for _ in 0..ROOMS_PER_SPACE {
        let key = Key {
            space,
            kind: "documents".into(),
            item: Uuid::new_v4(),
        };
        for _ in 0..PARTICIPANTS {
            retained.push(hub.room(&key, account).unwrap().0);
        }
        assert!(hub.room(&key, account).is_err());
    }
    let extra = Key {
        space,
        kind: "documents".into(),
        item: Uuid::new_v4(),
    };
    assert!(hub.room(&extra, account).is_err());
    assert_eq!(retained.len(), 1_024);
    drop(retained);
    assert!(hub.room(&extra, account).is_ok());
}

#[tokio::test]
async fn last_grant_eviction_fences_preview_for_just_the_affected_account() {
    let hub = Hub::default();
    let alice = AccountId::new();
    let bob = AccountId::new();
    let key = Key {
        space: SpaceId::new(),
        kind: "documents".into(),
        item: Uuid::new_v4(),
    };
    let (alice, mut cut) = {
        let (lease, termination) = hub.room(&key, alice).unwrap();
        hub.evict_item_account(key.space, &key.kind, key.item, alice);
        (lease, termination)
    };
    let (_bob, other) = hub.room(&key, bob).unwrap();
    cut.changed().await.unwrap();
    assert_eq!(*cut.borrow(), Some(Termination::AccessRevoked));
    assert_eq!(*other.borrow(), None);
    let error = alice
        .room
        .preview(alice.id)
        .publish(&[0, 0], [])
        .unwrap_err();
    assert!(super::super::access_revoked(&error));
}
