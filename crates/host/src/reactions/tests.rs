//! Pure writes have the same replay disposition with and without an outbox.

use super::*;
use calternal_auth::{Auth, AuthConfig};

#[test]
fn pure_write_replay_does_not_report_a_commit() {
    let root = tempfile::tempdir().unwrap();
    let space = SpaceId::new();
    let auth = Auth::open(
        &root.path().join("auth"),
        AuthConfig::new("http://localhost").unwrap(),
    )
    .unwrap();
    let account = crate::block_on(auth.create("Owner")).unwrap().unwrap().id;
    let host = Host::new(root.path().into(), auth, 7).unwrap();
    let slot = host.space_slot(space).unwrap();
    let mut storage = Space::open(root.path(), space, 86_400_000).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins-v2/calternal-reading-list");
    storage
        .install(crate::installation::bundle(&source).unwrap(), true, None)
        .unwrap();
    let write = || Call {
        call: "s.write".into(),
        input: serde_json::json!({"changes":[{"op":"add","kind":"calternal-reading-list.reading",
            "fields":{"title":"Example","url":"https://example.com"}}]}),
        idempotency_key: Some("gesture".into()),
        slot_selection: None,
    };
    let owner = Actor::account(account);
    let (first, disposition, _) = storage.observe_write(&owner, write(), &[], &slot).unwrap();
    assert!(disposition == Disposition::Committed);
    assert!(!has_outbox(&storage.db).unwrap());
    let (replay, disposition, _) = storage.observe_write(&owner, write(), &[], &slot).unwrap();
    assert_eq!(first, replay);
    assert!(disposition == Disposition::Replayed);
}
