//! Lifecycle guards serialize external admissions and Trash on the content owner.
use calternal_content::{Content, FileFormat, Placement, Request};
use calternal_types::{AccountId, Error, SpaceId};
use serde_json::json;
use std::time::Duration;
#[test]
fn guarded_trash_preserves_identity_history_and_refuses_new_admission() {
    let dir = tempfile::tempdir().unwrap();
    let roots = calternal_roots::Roots {
        appdata: dir.path().join("appdata"),
        userdata_ssd: dir.path().join("ssd"),
        userdata_hdd: dir.path().join("hdd"),
    };
    let content = Content::open(
        roots,
        dir.path().join("secrets"),
        Duration::ZERO,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
        |_| {},
    )
    .unwrap();
    let (space, account) = (SpaceId::new(), AccountId::new());
    content
        .register_kind(
            space,
            "threads",
            Placement::VisibleTree {
                folder: "AI".into(),
                subdirectories: vec!["Chats".into()],
            },
            FileFormat::Markdown,
        )
        .unwrap();
    let req = |id| Request {
        account,
        via: None,
        id,
        operation: "example.lifecycle",
        fingerprint: id,
    };
    let item = content
        .write(
            space,
            None,
            "threads",
            None,
            json!({"title":"Readable"}),
            b"messages",
            req("create"),
            |i| json!({"id":i.id}),
        )
        .unwrap();
    let original = content.file_path(space, "threads", item.id).unwrap();
    assert!(
        content
            .trash(
                &item,
                json!({"title":"Readable","deleted":true}),
                b"messages",
                req("blocked"),
                |i| json!({"id":i.id}),
                || Err(Error::conflict("active turn"))
            )
            .is_err()
    );
    assert!(original.is_file());
    assert_eq!(content.get(space, item.id).unwrap().revision, item.revision);
    let deletion = content
        .trash(
            &item,
            json!({"title":"Readable","deleted":true}),
            b"messages",
            req("delete"),
            |i| json!({"id":i.id}),
            || Ok(()),
        )
        .unwrap();
    let path = content.file_path(space, "threads", item.id).unwrap();
    assert_eq!(
        path.file_name().unwrap().to_str().unwrap(),
        format!("{}.md", item.id)
    );
    assert!(path.parent().unwrap().ends_with(".trash"));
    assert!(!original.exists());
    assert_eq!(content.locate("threads", item.id).unwrap(), Some(space));
    let denied = content.admit_item(space, item.id, req("admit"), |current, _| {
        if current.metadata["deleted"] == true {
            return Err(Error::not_found("item not found"));
        }
        panic!("deleted head reached external admission")
    });
    assert!(denied.is_err());
    assert_eq!(
        content.history(space, item.id, 100, None).unwrap().0.len(),
        2
    );
    assert_eq!(deletion.id, item.id);
    assert!(
        content
            .receipt(space, account, "blocked")
            .unwrap()
            .is_none()
    );
}
