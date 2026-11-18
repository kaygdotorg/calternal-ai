use super::*;
use crate::{Content, FileFormat, Placement, Request};
use calternal_types::AccountId;
use rusqlite::Connection;
use serde_json::json;

#[test]
fn placement_keeps_retrying_when_the_slot_is_busy_after_a_failed_mark() {
    let dir = tempfile::tempdir().unwrap();
    let roots = calternal_roots::Roots {
        appdata: dir.path().join("appdata"),
        userdata_ssd: dir.path().join("ssd"),
        userdata_hdd: dir.path().join("hdd"),
    };
    let content = Content::open(
        roots.clone(),
        dir.path().join("secrets"),
        Duration::from_secs(60),
        crate::DEFAULT_RECEIPT_RETENTION,
        |_| {},
    )
    .unwrap();
    let space = SpaceId::new();
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
    // Drive the same worker step directly. Holding this guard parks the real
    // worker before it can take a slot, even after an acknowledged write.
    let _worker = lock(&content.root_guard);
    let item = content
        .write(
            space,
            None,
            "notes",
            None,
            json!({"title": "Retry"}),
            b"latest revision",
            Request {
                account: AccountId::new(),
                via: None,
                id: "save",
                operation: "notes.create",
                fingerprint: "save",
            },
            |item| json!({"id": item.id}),
        )
        .unwrap();
    let slot = lock(&content.spaces).get(&space).unwrap().clone();
    let observer = Connection::open(crate::db::space_path(&roots.appdata, space)).unwrap();
    observer
        .execute_batch(
            "CREATE TRIGGER fail_mark BEFORE UPDATE OF mat_state ON items
             WHEN NEW.mat_state = 'materialized'
             BEGIN SELECT RAISE(FAIL, 'test placement bookkeeping failure'); END;",
        )
        .unwrap();
    assert!(
        place_pending(&slot, |_| {}),
        "failed mark did not schedule retry"
    );
    let state = || {
        observer
            .query_row(
                "SELECT revision,mat_state,(SELECT count(*) FROM outbox) FROM items WHERE id=?1",
                [item.id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, u32>(2)?,
                    ))
                },
            )
            .unwrap()
    };
    assert_eq!(state(), (item.revision.clone(), "pending".into(), 0));
    let file = roots
        .userdata_ssd
        .join(format!("spaces/{space}/Notes/Retry.md"));
    let bytes = std::fs::read(&file).unwrap();
    assert!(bytes.ends_with(b"latest revision"));
    observer.execute_batch("DROP TRIGGER fail_mark").unwrap();

    // A direct user of the slot (such as the search worker's head check)
    // releases it without unparking the placement worker.
    let busy = lock(&slot.space);
    let retry = place_pending(&slot, |_| {});
    drop(busy);
    assert!(retry, "busy slot lost the pending revision's short retry");
    assert!(!place_pending(&slot, |_| {}));
    assert_eq!(state(), (item.revision, "materialized".into(), 0));
    assert_eq!(std::fs::read(file).unwrap(), bytes);
}
