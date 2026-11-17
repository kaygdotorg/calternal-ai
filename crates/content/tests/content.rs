//! Physical separation and restart recovery on disposable roots.
use calternal_content::{Content, FaultPoint, FileFormat, Placement, Request};
use calternal_types::{AccountId, ErrorCode, SpaceId};
use calternal_roots::Roots;
use serde_json::json;
use std::{path::Path, time::Duration};

fn open(path: &Path, fault: fn(FaultPoint)) -> Content {
    Content::open(
        Roots {
            appdata: path.join("appdata"),
            userdata_ssd: path.join("userdata-ssd"),
            userdata_hdd: path.join("userdata-hdd"),
        },
        path.join("secrets"),
        Duration::ZERO,
        fault,
    )
    .unwrap()
}
fn request(account: AccountId, id: &str) -> Request<'_> {
    Request {
        account,
        id,
        operation: "example.create",
        fingerprint: id,
    }
}
#[test]
fn spaces_have_separate_databases_and_foreign_ids_are_absent() {
    let dir = tempfile::tempdir().unwrap();
    let content = open(dir.path(), |_| {});
    let a = SpaceId::new();
    let b = SpaceId::new();
    let actor = AccountId::new();
    content
        .register_kind(
            a,
            "notes",
            Placement::Visible {
                folder: "Notes".into(),
            },
            FileFormat::Markdown,
        )
        .unwrap();
    content
        .register_kind(
            b,
            "chats",
            Placement::Visible {
                folder: "Test chats".into(),
            },
            FileFormat::Opaque {
                extension: "txt".into(),
            },
        )
        .unwrap();
    let one = content
        .write(
            a,
            None,
            "notes",
            None,
            json!({"title":"secret"}),
            b"one",
            request(actor, "one"),
            |i| json!({"id":i.id}),
        )
        .unwrap();
    content
        .write(
            b,
            None,
            "chats",
            None,
            json!({}),
            b"two",
            request(actor, "two"),
            |i| json!({"id":i.id}),
        )
        .unwrap();
    let pa = dir.path().join(format!("appdata/spaces/{a}/content.db"));
    let pb = dir.path().join(format!("appdata/spaces/{b}/content.db"));
    assert!(pa.is_file() && pb.is_file());
    assert_ne!(pa, pb);
    use std::os::unix::fs::MetadataExt;
    assert_ne!(
        std::fs::metadata(pa).unwrap().ino(),
        std::fs::metadata(pb).unwrap().ino()
    );
    assert_eq!(
        content.get(b, one.id).unwrap_err().code,
        ErrorCode::NotFound
    );
    assert_eq!(content.locate("tasks", one.id).unwrap(), None);
    assert_eq!(content.get(a, one.id).unwrap().bytes, b"one");
    assert_eq!(
        content.receipt(a, actor, "one").unwrap().unwrap().result["id"],
        one.id.to_string()
    );
    assert!(content.receipt(b, actor, "one").unwrap().is_none());
    assert_eq!(content.journal(a).unwrap().len(), 1);
    content.sweep();
    assert_eq!(content.get(a, one.id).unwrap().bytes, b"one");
}

fn interrupt(point: FaultPoint) {
    if point == FaultPoint::AfterCommit {
        panic!("simulated interruption")
    }
}
#[test]
fn restart_finishes_content_commit_before_file_placement_and_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let a = SpaceId::new();
    let actor = AccountId::new();
    let content = open(dir.path(), interrupt);
    content
        .register_kind(
            a,
            "notes",
            Placement::Visible {
                folder: "Notes".into(),
            },
            FileFormat::Markdown,
        )
        .unwrap();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        content.write(
            a,
            None,
            "notes",
            None,
            json!({}),
            b"durable",
            request(actor, "save"),
            |i| json!({"id":i.id}),
        )
    }));
    drop(content);
    let content = open(dir.path(), |_| {});
    let items = content.list(a, "notes", 100).unwrap();
    assert_eq!(items.len(), 1);
    let item = content.get(a, items[0].id).unwrap();
    assert_eq!(
        std::fs::read_to_string(content.file_path(a, &item.kind, item.id).unwrap()).unwrap(),
        format!("---\ncalternal-id: {}\n---\ndurable", item.id)
    );
    assert!(content.receipt(a, actor, "save").unwrap().is_some());
    assert_eq!(content.journal(a).unwrap().len(), 1);
}

#[test]
fn human_names_collisions_renames_and_registered_hidden_placement() {
    let dir = tempfile::tempdir().unwrap();
    let content = open(dir.path(), |_| {});
    let space = SpaceId::new();
    let actor = AccountId::new();
    content
        .register_kind(
            space,
            "notes",
            Placement::Visible {
                folder: "Notes".into(),
            },
            FileFormat::Markdown,
        )
        .unwrap();
    let one = content
        .write(
            space,
            None,
            "notes",
            None,
            json!({"title":"Café"}),
            b"original",
            request(actor, "one"),
            |i| json!({"id":i.id}),
        )
        .unwrap();
    let two = content
        .write(
            space,
            None,
            "notes",
            None,
            json!({"title":"CAFE\u{301}"}),
            b"second",
            request(actor, "two"),
            |i| json!({"id":i.id}),
        )
        .unwrap();
    assert_eq!(
        content
            .file_path(space, "notes", one.id)
            .unwrap()
            .file_name()
            .unwrap(),
        "Café.md"
    );
    assert_eq!(
        content
            .file_path(space, "notes", two.id)
            .unwrap()
            .file_name()
            .unwrap(),
        "CAFÉ 2.md"
    );
    let renamed = content
        .write(
            space,
            Some(one.id),
            "notes",
            Some(&one.revision),
            json!({"title":"CAFÉ"}),
            b"edited",
            request(actor, "rename"),
            |i| json!({"id":i.id}),
        )
        .unwrap();
    assert_eq!(renamed.id, one.id);
    let path = content.file_path(space, "notes", one.id).unwrap();
    assert_eq!(path.file_name().unwrap(), "CAFÉ.md");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        format!("---\ncalternal-id: {}\n---\nedited", one.id)
    );
    assert!(!path.with_file_name("Café.md").exists());
    assert_eq!(content.journal(space).unwrap().len(), 3);
    content
        .register_kind(
            space,
            "views",
            Placement::Hidden {
                plugin_id: "example".into(),
            },
            FileFormat::Opaque {
                extension: "json".into(),
            },
        )
        .unwrap();
    let hidden = content
        .write(
            space,
            None,
            "views",
            None,
            json!({"title":"Saved view"}),
            b"{}",
            request(actor, "hidden"),
            |i| json!({"id":i.id}),
        )
        .unwrap();
    assert!(
        content
            .file_path(space, "views", hidden.id)
            .unwrap()
            .ends_with(".calternal/example/views/Saved view.json")
    );
}

#[test]
fn collections_retain_ids_membership_and_account_separation() {
    let dir = tempfile::tempdir().unwrap();
    let content = open(dir.path(), |_| {});
    let space = SpaceId::new();
    let a = AccountId::new();
    let b = AccountId::new();
    content
        .register_kind(
            space,
            "collections",
            Placement::Hidden {
                plugin_id: "collections".into(),
            },
            FileFormat::Opaque {
                extension: "json".into(),
            },
        )
        .unwrap();
    let collection = content
        .ensure_standard_collection(space, a, "notes", "all", "All notes")
        .unwrap();
    let again = content
        .ensure_standard_collection(space, a, "notes", "all", "New label")
        .unwrap();
    assert_eq!(collection.id, again.id);
    assert!(
        content
            .file_path(space, "collections", collection.id)
            .unwrap()
            .ends_with(".calternal/collections/collections/All notes.json")
    );
    let member = uuid::Uuid::now_v7();
    let updated = content
        .set_collection_members(
            space,
            collection.id,
            &collection.revision,
            vec![member],
            request(a, "membership"),
        )
        .unwrap();
    assert_eq!(updated.collection.members, vec![member]);
    let personal = content
        .create_account_collection(a, "Across spaces", "notes", vec![member])
        .unwrap();
    assert_eq!(
        content
            .account_collection(a, personal.id)
            .unwrap()
            .collection
            .members,
        vec![member]
    );
    assert_eq!(
        content.account_collection(b, personal.id).unwrap_err().code,
        ErrorCode::NotFound
    );
    assert_eq!(
        content.get(space, personal.id).unwrap_err().code,
        ErrorCode::NotFound
    );
    drop(content);
    let content = open(dir.path(), |_| {});
    assert_eq!(
        content
            .ensure_standard_collection(space, a, "notes", "all", "All")
            .unwrap()
            .id,
        collection.id
    );
    assert_eq!(
        content
            .collection(space, collection.id)
            .unwrap()
            .collection
            .members,
        vec![member]
    );
}
