//! Exact-revision body pages through the real space writer and reader.
use calternal_content::{Content, FileFormat, Placement, Request};
use calternal_roots::Roots;
use calternal_types::{AccountId, RevisionRef, SpaceId};
use serde_json::json;
use std::time::Duration;

#[test]
fn pinned_range_reads_the_old_body_after_head_advance() {
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
    let mut body = vec![b'a'; 65536];
    body.extend_from_slice(b"tail");
    let original = content
        .write(
            space,
            None,
            kind,
            None,
            json!({}),
            &body,
            request("one"),
            |_| json!({}),
        )
        .unwrap();
    let revision = RevisionRef(original.revision.clone());
    let first = content
        .read_revision_page(space, original.id, kind, &revision, 0, 65536)
        .unwrap();
    assert_eq!(first, (body[..65536].to_vec(), Some(65536)));

    content
        .write(
            space,
            Some(original.id),
            kind,
            Some(&revision.0),
            json!({}),
            b"new head",
            request("two"),
            |_| json!({}),
        )
        .unwrap();
    let last = content
        .read_revision_page(space, original.id, kind, &revision, 65536, 65536)
        .unwrap();
    assert_eq!(last, (b"tail".to_vec(), None));
    let eof = content
        .read_revision_page(space, original.id, kind, &revision, body.len() as u64, 1)
        .unwrap();
    assert_eq!(eof, (vec![], None));

    assert!(
        content
            .read_revision_page(space, original.id, kind, &revision, 0, 65537)
            .is_err()
    );
    assert!(
        content
            .read_revision_page(space, original.id, "another-kind", &revision, 0, 1)
            .is_err()
    );
}
