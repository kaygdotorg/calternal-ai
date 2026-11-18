//! Stable room bootstrap and wire edits across content-service restarts.
use calternal_content::{Content, FileFormat, Placement, Request};
use calternal_roots::Roots;
use calternal_types::{AccountId, SpaceId};
use serde_json::json;
use yrs::updates::decoder::Decode;
use yrs::{Doc, ReadTxn, Text, Transact, Update};

#[test]
fn initial_state_is_read_only_and_wire_replay_keeps_the_same_lineage() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let open = || {
        Content::open(
            Roots {
                appdata: root.join("appdata"),
                userdata_ssd: root.join("ssd"),
                userdata_hdd: root.join("hdd"),
            },
            root.join("secrets"),
            std::time::Duration::ZERO,
            calternal_content::DEFAULT_RECEIPT_RETENTION,
            |_| {},
        )
        .unwrap()
    };
    let space = SpaceId::new();
    let actor = AccountId::new();
    let request = |id| Request {
        account: actor,
        via: None,
        id,
        operation: "documents.update",
        fingerprint: id,
    };
    let content = open();
    content
        .register_kind(
            space,
            "documents",
            Placement::Visible {
                folder: "Documents".into(),
            },
            FileFormat::Markdown,
        )
        .unwrap();
    let item = content
        .write(
            space,
            None,
            "documents",
            None,
            json!({"title":"Document"}),
            b"abc",
            request("seed"),
            |_| json!({}),
        )
        .unwrap();
    content.enable_live_editing(space, "documents").unwrap();
    let before = content.journal(space).unwrap().len();
    let state = content.room_snapshot(space, "documents", item.id).unwrap();
    assert_eq!(content.journal(space).unwrap().len(), before);
    drop(content);

    let content = open();
    let after = content.room_snapshot(space, "documents", item.id).unwrap();
    assert_eq!(after.lineage, state.lineage);
    assert_eq!(after.update, state.update);

    let doc = Doc::new();
    let text = doc.get_or_insert_text("md");
    doc.transact_mut()
        .apply_update(Update::decode_v1(&state.update).unwrap())
        .unwrap();
    let base = doc.transact().state_vector();
    text.insert(&mut doc.transact_mut(), 3, "!");
    let edit = doc.transact().encode_diff_v1(&base);
    content
        .apply_room_update(
            space,
            "documents",
            item.id,
            &state.lineage,
            &edit,
            request("wire"),
            |_| Ok(()),
        )
        .unwrap();
    drop(content);

    let content = open();
    let after = content.room_snapshot(space, "documents", item.id).unwrap();
    assert_eq!(after.lineage, state.lineage);
    assert_eq!(content.get(space, item.id).unwrap().bytes, b"abc!");

    // Simulate a crash after the swap displaced an outside edit, before the
    // exchange discovered that the displaced bytes were not its base.
    drop(content);
    let folder = root.join(format!("ssd/spaces/{space}/Documents"));
    let path = folder.join("Document.md");
    let saved = std::fs::read(&path).unwrap();
    let temp = format!("{}abc-Document.md", calternal_fsio::TEMP_PREFIX);
    let intent = format!("{}intent-Document.md", calternal_fsio::TEMP_PREFIX);
    let base = b"expected old file";
    let outside = b"outside edit";
    let mut record = format!("{temp}\n").into_bytes();
    record.extend_from_slice(&(base.len() as u64).to_le_bytes());
    record.extend_from_slice(base);
    record.extend_from_slice(&saved);
    std::fs::write(folder.join(&temp), outside).unwrap();
    std::fs::write(folder.join(&intent), record).unwrap();

    let content = open();
    content.room_snapshot(space, "documents", item.id).unwrap();
    assert_eq!(std::fs::read(path).unwrap(), outside);
    assert!(!folder.join(temp).exists());
    assert!(!folder.join(intent).exists());
}
