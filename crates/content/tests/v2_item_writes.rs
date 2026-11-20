//! Checked create and edit through the production content writer.
use calternal_content::{
    CheckedItemBody, CheckedItemPlacement, Content, FileFormat, Placement, Request,
};
use calternal_roots::Roots;
use calternal_types::{AccountId, RevisionRef, SpaceId};
use serde_json::json;
use std::time::Duration;

#[test]
fn checked_rename_and_append_keep_exact_revisions_and_placement() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let content = Content::open(
        Roots {
            appdata: root.join("appdata"),
            userdata_ssd: root.join("userdata-ssd"),
            userdata_hdd: root.join("userdata-hdd"),
        },
        root.join("secrets"),
        Duration::from_secs(30),
        calternal_content::DEFAULT_RECEIPT_RETENTION,
        |_| {},
    )
    .unwrap();
    let space = SpaceId::new();
    let account = AccountId::new();
    let kind = "test-log";
    content
        .register_kind(
            space,
            kind,
            Placement::Visible {
                folder: "Logs".into(),
            },
            FileFormat::Opaque {
                extension: "md".into(),
            },
        )
        .unwrap();
    let request = |id| Request {
        account,
        via: None,
        id,
        operation: "test.write",
        fingerprint: id,
    };
    let receipt = |item: &calternal_content::Item| json!({"revision":item.revision});
    let create = |id| {
        content.write_checked_item(
            space,
            kind,
            None,
            None,
            CheckedItemPlacement::Create {
                folders: &["Example".into()],
                name: "Conversation.md",
            },
            json!({"title":"Original title"}),
            CheckedItemBody::Replace(b"original"),
            true,
            request(id),
            || Ok(()),
            |_| Ok(()),
            receipt,
        )
    };
    let first = create("first").unwrap();
    let duplicate = create("collision").unwrap();
    let path = |id| content.file_path(space, kind, id).unwrap();
    assert!(path(first.id).ends_with("Logs/Example/Conversation.md"));
    assert!(path(duplicate.id).ends_with("Logs/Example/Conversation 2.md"));
    assert!(create("first").is_err());
    let saved_receipt = content.receipt(space, account, "first").unwrap().unwrap();
    assert_eq!(saved_receipt.item, Some(first.id));
    assert_eq!(saved_receipt.result["revision"], first.revision);

    let refused = content.write_checked_item(
        space,
        kind,
        Some(first.id),
        Some(&RevisionRef(first.revision.clone())),
        CheckedItemPlacement::KeepFolder {
            name: Some("Forbidden.md"),
        },
        json!({"title":"Forbidden"}),
        CheckedItemBody::Replace(b"forbidden"),
        true,
        request("refused"),
        || Err(calternal_types::Error::forbidden("writer revoked")),
        |_| Ok(()),
        receipt,
    );
    assert_eq!(
        refused.unwrap_err().code,
        calternal_types::ErrorCode::Forbidden
    );
    assert!(
        content
            .receipt(space, account, "refused")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        content.get(space, first.id).unwrap().revision,
        first.revision
    );

    let saved = content
        .write_checked_item(
            space,
            kind,
            Some(first.id),
            Some(&RevisionRef(first.revision.clone())),
            CheckedItemPlacement::KeepFolder {
                name: Some("Renamed.md"),
            },
            json!({"title":"New title"}),
            CheckedItemBody::Replace(b"replacement"),
            true,
            request("save"),
            || Ok(()),
            |_| Ok(()),
            receipt,
        )
        .unwrap();
    assert_eq!(saved.metadata["title"], "New title");
    assert_eq!(saved.bytes, b"replacement");
    assert!(path(first.id).ends_with("Logs/Example/Renamed.md"));
    content.materialize(space, first.id).unwrap();
    assert_eq!(std::fs::read(path(first.id)).unwrap(), b"replacement");

    let stale = content
        .write_checked_item(
            space,
            kind,
            Some(first.id),
            Some(&RevisionRef(first.revision)),
            CheckedItemPlacement::KeepFolder { name: None },
            json!({}),
            CheckedItemBody::Append(b" stale"),
            true,
            request("stale"),
            || Ok(()),
            |_| Ok(()),
            receipt,
        )
        .unwrap_err();
    assert_eq!(stale.code, calternal_types::ErrorCode::Conflict);

    let appended = content
        .write_checked_item(
            space,
            kind,
            Some(first.id),
            Some(&RevisionRef(saved.revision)),
            CheckedItemPlacement::KeepFolder { name: None },
            json!({}),
            CheckedItemBody::Append(b" append"),
            false,
            request("append"),
            || Ok(()),
            |_| Ok(()),
            receipt,
        )
        .unwrap();
    assert_eq!(appended.bytes, b"replacement append");
    assert_eq!(appended.metadata["title"], "New title");
    content.materialize(space, first.id).unwrap();
    assert_eq!(
        std::fs::read(path(first.id)).unwrap(),
        b"replacement append"
    );
    let conn = rusqlite::Connection::open_with_flags(
        root.join(format!("appdata/spaces/{space}/content.db")),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let protected: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM item_discovery_protection WHERE item=?1)",
            [first.id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert!(protected);
}
