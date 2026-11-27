use super::*;
use calternal_types::{AccountId, SpaceId};
use std::path::Path;

#[test]
fn records_keep_two_actors_in_ledger_order() {
    // Slice 1 exposes only personal spaces. Exercise the record ledger
    // with two already-authorized contexts without adding a sharing API.
    let dir = tempfile::tempdir().unwrap();
    let mut space = Space::open(dir.path(), SpaceId::new(), 7 * 86_400_000).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins-v2/calternal-chat");
    space.install(&fixture, true).unwrap();
    let actor = Actor::plugin(AccountId::new(), "calternal-chat");
    let other = Actor::plugin(AccountId::new(), "calternal-chat");
    let call = |space: &mut Space, actor: &Actor, change| {
        write(
            space,
            actor,
            Write {
                changes: vec![serde_json::from_value(change).unwrap()],
                preview: false,
            },
            None,
        )
    };
    let output = call(
        &mut space,
        &actor,
        json!({"op":"add","kind":"calternal-chat.chats", "fields":{"title":"Two writers"}}),
    )
    .unwrap();
    let item = output["changes"][0]["id"].clone();
    let output = call(
        &mut space,
        &actor,
        json!({"op":"append record","item":item,"record":{"body":"one"}}),
    )
    .unwrap();
    let first = output["changes"][0]["anchor"].clone();
    call(
        &mut space,
        &other,
        json!({"op":"append record","item":item,"record":{"body":"two"}}),
    )
    .unwrap();
    let denied = call(
        &mut space,
        &other,
        json!({"op":"remove record","item":item,"anchor":first}),
    )
    .unwrap_err();
    assert_eq!(denied.code, ErrorCode::Forbidden);
    let id = serde_json::from_value(item.clone()).unwrap();
    let records = space.records(id).unwrap();
    assert_eq!(records[0].author, actor.account.to_string());
    assert_eq!(records[1].author, other.account.to_string());
    assert!(records[0].ordinal < records[1].ordinal);
    call(
        &mut space,
        &actor,
        json!({"op":"remove record","item":item,"anchor":first}),
    )
    .unwrap();
    let records = space.records(id).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].author, other.account.to_string());
}

#[test]
fn restart_finishes_a_durable_batch_without_replacing_its_first_placement() {
    let dir = tempfile::tempdir().unwrap();
    let id = SpaceId::new();
    let space = Space::open(dir.path(), id, 7 * 86_400_000).unwrap();
    let placements = [
        space
            .stage("Fixture/one.md".into(), None, Some(b"first"))
            .unwrap(),
        space
            .stage("Fixture/two.md".into(), None, Some(b"second"))
            .unwrap(),
    ];
    space
        .db
        .execute(
            "INSERT INTO publication VALUES(1,?1)",
            [serde_json::to_string(&placements).unwrap()],
        )
        .unwrap();
    // Simulate interruption after the first destination became durable.
    let fixture = dir
        .path()
        .join("spaces")
        .join(id.to_string())
        .join("Fixture");
    calternal_fsio::create_dir_all_durable(&fixture).unwrap();
    calternal_fsio::write_atomic(&fixture.join("one.md"), b"first").unwrap();
    drop(space);
    let reopened = Space::open(dir.path(), id, 7 * 86_400_000).unwrap();
    assert_eq!(
        reopened.read_optional("Fixture/one.md").unwrap().unwrap(),
        b"first"
    );
    assert_eq!(
        reopened.read_optional("Fixture/two.md").unwrap().unwrap(),
        b"second"
    );
    assert!(reopened.pending_dir.names().unwrap().is_empty());
}
