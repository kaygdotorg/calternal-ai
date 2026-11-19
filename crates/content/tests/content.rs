//! Physical separation and restart recovery on disposable roots.
use calternal_content::{Content, FaultPoint, FileFormat, Placement, Request};
use calternal_roots::Roots;
use calternal_types::{AccountId, ErrorCode, SpaceId};
use serde_json::json;
use std::{path::Path, time::Duration};

fn open(path: &Path, fault: fn(FaultPoint)) -> Content {
    open_retaining(path, fault, calternal_content::DEFAULT_RECEIPT_RETENTION)
}
fn open_retaining(path: &Path, fault: fn(FaultPoint), retention: Duration) -> Content {
    Content::open(
        Roots {
            appdata: path.join("appdata"),
            userdata_ssd: path.join("userdata-ssd"),
            userdata_hdd: path.join("userdata-hdd"),
        },
        path.join("secrets"),
        Duration::ZERO,
        retention,
        fault,
    )
    .unwrap()
}
/// Observe the post-fsync bookkeeping mark without forcing publication or placement.
fn wait_for_placement(path: &Path, space: SpaceId, item: uuid::Uuid) {
    let conn = rusqlite::Connection::open_with_flags(
        path.join(format!("appdata/spaces/{space}/content.db")),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    loop {
        let done: bool = conn
            .query_row(
                "SELECT mat_state='materialized' FROM items WHERE id=?1",
                [item.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        if done {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "userdata placement exceeded one second"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn request(account: AccountId, id: &str) -> Request<'_> {
    Request {
        account,
        via: None,
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
    let via = calternal_types::Via::Agent {
        thread: uuid::Uuid::now_v7(),
    };
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
            Request {
                via: Some(&via),
                ..request(actor, "save")
            },
            |i| json!({"id":i.id}),
        )
    }));
    // Observe the SQLite commit immediately after the injected interruption,
    // before reopening the space, publishing its receipt or placing the file.
    let sqlite =
        rusqlite::Connection::open(dir.path().join(format!("appdata/spaces/{a}/content.db")))
            .unwrap();
    let projected: (String, String, u32) = sqlite
        .query_row(
            "SELECT p.item,i.id,s.total FROM window_rows p
         JOIN items i ON i.id=p.item
         JOIN window_scopes s USING(collection,audience)
         WHERE p.collection='notes' AND p.audience='space'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(projected.0, projected.1);
    assert_eq!(projected.2, 1);
    drop(sqlite);
    drop(content);
    let content = open(dir.path(), |_| {});
    let items = content.list(a, "notes", 100).unwrap();
    assert_eq!(items.len(), 1);
    let item = content.get(a, items[0].id).unwrap();
    assert_eq!(item.id.to_string(), projected.0);
    assert_eq!(
        std::fs::read_to_string(content.file_path(a, &item.kind, item.id).unwrap()).unwrap(),
        format!(
            "---\ncalternal-id: {}\nformat: calternal-note/v1\n---\ndurable",
            item.id
        )
    );
    let receipt = content.receipt(a, actor, "save").unwrap().unwrap();
    assert_eq!(receipt.result, json!({"id":item.id}));
    let journal = content.journal(a).unwrap();
    assert_eq!(journal.len(), 1);
    assert_eq!(
        journal[0].data["actor"],
        json!(calternal_types::Actor {
            account: actor,
            via: Some(via)
        })
    );
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
    wait_for_placement(dir.path(), space, one.id);
    let path = content.file_path(space, "notes", one.id).unwrap();
    assert_eq!(path.file_name().unwrap(), "CAFÉ.md");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        format!(
            "---\ncalternal-id: {}\nformat: calternal-note/v1\n---\nedited",
            one.id
        )
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

/// Work in one space never waits for another: while a write in space A is stalled
/// inside its commit (holding A's lock), a read in an open space B and the cold open
/// of a third space C both finish promptly.
#[test]
fn a_slow_write_in_one_space_does_not_delay_other_spaces() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static STALL: AtomicBool = AtomicBool::new(false);
    static STALLED: AtomicBool = AtomicBool::new(false);
    fn stall(point: FaultPoint) {
        if point == FaultPoint::AfterCommit && STALL.swap(false, Ordering::SeqCst) {
            STALLED.store(true, Ordering::SeqCst);
            std::thread::sleep(Duration::from_secs(3));
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let content = open(dir.path(), stall);
    let (a, b, c, actor) = (
        SpaceId::new(),
        SpaceId::new(),
        SpaceId::new(),
        AccountId::new(),
    );
    let notes = || Placement::Visible {
        folder: "Notes".into(),
    };
    for space in [a, b, c] {
        content
            .register_kind(space, "notes", notes(), FileFormat::Markdown)
            .unwrap();
    }
    let in_b = content
        .write(
            b,
            None,
            "notes",
            None,
            json!({"title":"B"}),
            b"b\n",
            request(actor, "b"),
            |_| json!({}),
        )
        .unwrap();
    // Close C so the probe below has to open it again.
    drop(content);
    let content = open(dir.path(), stall);
    content.list(b, "notes", 10).unwrap();
    std::thread::scope(|scope| {
        STALL.store(true, Ordering::SeqCst);
        let slow = scope.spawn(|| {
            content
                .write(
                    a,
                    None,
                    "notes",
                    None,
                    json!({"title":"A"}),
                    b"a\n",
                    request(actor, "a"),
                    |_| json!({}),
                )
                .unwrap()
        });
        while !STALLED.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
        let started = std::time::Instant::now();
        assert_eq!(content.get(b, in_b.id).unwrap().bytes, b"b\n");
        assert!(content.list(c, "notes", 10).unwrap().is_empty());
        let waited = started.elapsed();
        assert!(
            !slow.is_finished(),
            "the stalled write must still be running"
        );
        assert!(
            waited < Duration::from_secs(1),
            "other spaces waited {waited:?}"
        );
        slow.join().unwrap();
    });
}

/// Receipts are bounded: within the retention a reused request ID is refused; once
/// its receipt has been pruned (at the latest when the space next opens), the same
/// ID runs as a new request.
#[test]
fn a_request_id_is_new_again_after_receipt_retention() {
    let dir = tempfile::tempdir().unwrap();
    let (space, actor) = (SpaceId::new(), AccountId::new());
    let retention = Duration::from_millis(50);
    let write = |content: &Content, text: &[u8]| {
        content.write(
            space,
            None,
            "notes",
            None,
            json!({"title": "T"}),
            text,
            Request {
                account: actor,
                via: None,
                id: "same",
                operation: "example.create",
                fingerprint: std::str::from_utf8(text).unwrap(),
            },
            |_| json!({}),
        )
    };
    let content = open_retaining(dir.path(), |_| {}, retention);
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
    write(&content, b"first\n").unwrap();
    assert_eq!(
        write(&content, b"second\n").unwrap_err().code,
        ErrorCode::Conflict
    );
    drop(content);
    std::thread::sleep(retention * 4);
    let content = open_retaining(dir.path(), |_| {}, retention);
    write(&content, b"second\n").expect("a pruned receipt makes the ID new again");
}

/// Startup must not inspect inactive space databases. Their own first access
/// validates them before exposing any data, without blocking a healthy space.
#[test]
fn startup_defers_inactive_space_validation_until_first_use() {
    let dir = tempfile::tempdir().unwrap();
    let broken = SpaceId::new();
    let path = dir.path().join(format!("appdata/spaces/{broken}"));
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("content.db"), b"invalid sqlite").unwrap();
    let content = open(dir.path(), |_| {});
    assert!(content.list(broken, "notes", 1).is_err());
    assert!(content.list(SpaceId::new(), "notes", 1).unwrap().is_empty());
}

/// Acknowledgement needs the SSD transactions, while the lifecycle worker makes
/// the real file durable without requiring another request to that space.
#[test]
fn acknowledged_content_materializes_without_another_request() {
    let dir = tempfile::tempdir().unwrap();
    let content = open_retaining(
        dir.path(),
        |_| {},
        calternal_content::DEFAULT_RECEIPT_RETENTION,
    );
    let (space, actor) = (SpaceId::new(), AccountId::new());
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
    let item = content
        .write(
            space,
            None,
            "notes",
            None,
            json!({"title":"Async"}),
            b"durable",
            request(actor, "async"),
            |_| json!({}),
        )
        .unwrap();
    let path = dir
        .path()
        .join(format!("userdata-ssd/spaces/{space}/Notes/Async.md"));
    wait_for_placement(dir.path(), space, item.id);
    assert!(std::fs::read_to_string(path).unwrap().ends_with("durable"));
}

#[test]
fn dormant_outbox_recovery_preserves_the_original_checkpoint_event() {
    let root = tempfile::tempdir().unwrap();
    let (space, actor) = (SpaceId::new(), AccountId::new());
    let request = Request {
        account: actor,
        via: None,
        id: "checkpoint:1",
        operation: "threads.checkpoint",
        fingerprint: "original",
    };
    let reply = json!({"append": [{"text": "exactly once"}]});
    let content = open(root.path(), |_| {});
    let journal = content.live_journal(space).unwrap();
    let space_root = root.path().join(format!("appdata/spaces/{space}"));
    let db = rusqlite::Connection::open(space_root.join("content.db")).unwrap();
    db.execute_batch(
        "CREATE TRIGGER stop_retirement BEFORE DELETE ON outbox
         BEGIN SELECT RAISE(FAIL, 'interrupted after publication'); END;",
    )
    .unwrap();
    content
        .set_control(space, request, "checkpoint", |_| {
            Ok((json!(true), reply.clone()))
        })
        .expect_err("journal commit must precede the interrupted retirement");
    assert_eq!(content.journal(space).unwrap().len(), 1);
    db.execute_batch("DROP TRIGGER stop_retirement").unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM outbox", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
    drop(db);
    drop(journal);
    drop(content);
    let db = rusqlite::Connection::open(space_root.join("index.db")).unwrap();
    db.execute("UPDATE idempotency SET at_ms=0", []).unwrap();
    drop(db);
    let recovered = open(root.path(), |_| {});
    let stored = recovered
        .receipt(space, actor, "checkpoint:1")
        .unwrap()
        .unwrap();
    assert_eq!(stored.result, reply);
    let events = recovered.journal(space).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data["append"], reply["append"]);
}

#[test]
fn opted_in_kind_merges_a_stale_replace_after_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let space = SpaceId::new();
    let actor = AccountId::new();
    let content = open(dir.path(), |_| {});
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
    content.enable_live_editing(space, "documents").unwrap();
    let write = |content: &Content, id, base, key, bytes: &[u8]| {
        content.write(
            space,
            id,
            "documents",
            base,
            json!({"title":"Shared"}),
            bytes,
            request(actor, key),
            |item| json!({"id":item.id}),
        )
    };
    let base = write(&content, None, None, "seed", b"Start\nMiddle\nEnd\n").unwrap();
    write(
        &content,
        Some(base.id),
        Some(&base.revision),
        "person",
        b"Start!\nMiddle\nEnd\n",
    )
    .unwrap();
    drop(content);
    let reopened = open(dir.path(), |_| {});
    let merged = write(
        &reopened,
        Some(base.id),
        Some(&base.revision),
        "agent",
        b"Start\nMiddle\nEnd!\n",
    )
    .unwrap();
    assert_eq!(merged.bytes, b"Start!\nMiddle\nEnd!\n");
}

#[test]
fn live_patch_preserves_concurrent_typing_and_append_uses_the_head() {
    use calternal_content::{LiveEdit, LiveTextEdit};
    let dir = tempfile::tempdir().unwrap();
    let content = open(dir.path(), |_| {});
    let space = SpaceId::new();
    let actor = AccountId::new();
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
    content.enable_live_editing(space, "documents").unwrap();
    let base = content
        .write(
            space,
            None,
            "documents",
            None,
            json!({"title":"Document"}),
            b"abc",
            request(actor, "seed"),
            |_| json!({}),
        )
        .unwrap();
    content
        .write(
            space,
            Some(base.id),
            "documents",
            Some(&base.revision),
            base.metadata.clone(),
            b"ab!c",
            request(actor, "typing"),
            |_| json!({}),
        )
        .unwrap();
    let patch = [LiveTextEdit {
        at: 1,
        delete: 2,
        insert: String::new(),
    }];
    let patched = content
        .edit_live(
            space,
            base.id,
            "documents",
            Some(&base.revision),
            LiveEdit::Patch(&patch),
            request(actor, "patch"),
            |_| json!({}),
        )
        .unwrap();
    assert_eq!(patched.bytes, b"a!");
    let appended = content
        .edit_live(
            space,
            base.id,
            "documents",
            None,
            LiveEdit::Append("z"),
            request(actor, "append"),
            |_| json!({}),
        )
        .unwrap();
    assert_eq!(appended.bytes, b"a!z");
    assert_eq!(appended.metadata, base.metadata);
}

#[test]
fn outside_edit_uses_the_proven_file_base_and_merges_a_stale_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let content = open(dir.path(), |_| {});
    let space = SpaceId::new();
    let actor = AccountId::new();
    let kind = "documents";
    content
        .register_kind(
            space,
            kind,
            Placement::Visible {
                folder: "Documents".into(),
            },
            FileFormat::Markdown,
        )
        .unwrap();
    content.enable_live_editing(space, kind).unwrap();
    let first = content
        .write(
            space,
            None,
            kind,
            None,
            json!({"title":"Document"}),
            b"Start\nEnd\n",
            request(actor, "outside-seed"),
            |_| json!({}),
        )
        .unwrap();
    content.materialize(space, first.id).unwrap();
    let file = content.file_path(space, kind, first.id).unwrap();
    let original = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, original.replace("End\n", "End outside\n")).unwrap();

    assert!(
        content
            .reconcile_room_file(space, kind, first.id, actor)
            .unwrap()
    );
    assert!(
        !content
            .reconcile_room_file(space, kind, first.id, AccountId::new())
            .unwrap()
    );
    let merged = content
        .write(
            space,
            Some(first.id),
            kind,
            Some(&first.revision),
            first.metadata,
            b"Start agent\nEnd\n",
            request(actor, "outside-stale"),
            |_| json!({}),
        )
        .unwrap();
    assert_eq!(merged.bytes, b"Start agent\nEnd outside\n");
    content.materialize(space, first.id).unwrap();
    assert!(
        std::fs::read_to_string(file)
            .unwrap()
            .ends_with("Start agent\nEnd outside\n")
    );
}
