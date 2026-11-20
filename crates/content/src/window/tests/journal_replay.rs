//! Catch-up and restart checks against the durable content and journal.

use super::*;

pub(super) fn verify_inherited_memberships(
    content: &Content,
    space: SpaceId,
    actor: AccountId,
    item: Uuid,
    source: &std::path::Path,
) {
    // Model a release that committed its memberships before scope admission.
    let conn = Connection::open(source).unwrap();
    let trigger: String = conn
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name='window_member_limit'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    conn.execute_batch("DROP TRIGGER window_member_limit")
        .unwrap();
    let inherited = content
        .create_notes_collection(
            space,
            "inherited".into(),
            vec![item],
            request(actor, "inherited"),
        )
        .unwrap();
    conn.execute_batch(&trigger).unwrap();
    project(content, space);
    assert_eq!(
        result(member_window(content, space)).count.unwrap().value,
        0
    );
    let skipped: String = replay::reader(source)
        .unwrap()
        .query_row(
            "SELECT reason FROM window_skipped WHERE item=?1",
            [item.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(skipped, "effective_scope_limit_exceeded");
    let id = inherited["id"].as_str().unwrap().parse().unwrap();
    let original = content.get(space, id).unwrap();
    let mut metadata = original.metadata;
    metadata["title"] = json!("Retitled");
    let bytes = serde_json::to_vec(&metadata).unwrap();
    let edited = content
        .write(
            space,
            Some(id),
            "collections",
            Some(&original.revision),
            metadata,
            &bytes,
            request(actor, "retitle-inherited"),
            |_| json!({}),
        )
        .unwrap();
    content
        .set_collection_members(
            space,
            id,
            &edited.revision,
            vec![],
            request(actor, "repair"),
        )
        .unwrap();
    project(content, space);
    assert_eq!(
        result(member_window(content, space)).count.unwrap().value,
        1
    );
}

fn member_window(content: &Content, space: SpaceId) -> WindowReply {
    content
        .window(
            space,
            &WindowRequest::Window {
                request: range(0, 10, WindowAnchor::Start),
            },
            None,
            || {
                Ok(WindowAccess {
                    scope: WindowScope::SpaceNotes,
                    audience: WindowAudience::Member,
                    count: WindowCountMode::Exact,
                    view: Uuid::nil(),
                })
            },
        )
        .unwrap()
}

fn assert_loading(reply: WindowReply) {
    assert!(matches!(&reply, WindowReply::Loading(_)), "{reply:?}");
    let wire = serde_json::to_value(reply).unwrap();
    assert_eq!(wire["rows"], json!([]));
    assert!(wire.get("count").is_none());
    assert!(wire["result_version"].is_null());
    assert!(wire["window_ref"].is_null());
}

fn seed_note(content: &Content, space: SpaceId, actor: AccountId) -> crate::Item {
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
    content
        .write(
            space,
            None,
            "notes",
            None,
            json!({"title":"First"}),
            b"body",
            request(actor, "first"),
            |_| json!({}),
        )
        .unwrap()
}

#[test]
fn large_collection_replay_persists_a_bounded_membership_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let content = content(&dir);
    let space = SpaceId::new();
    let actor = AccountId::new();
    let first = seed_note(&content, space, actor);
    let mut members = vec![first.id];
    for n in 1..=128 {
        let key = format!("member-{n}");
        members.push(
            content
                .write(
                    space,
                    None,
                    "notes",
                    None,
                    json!({"title": key}),
                    b"body",
                    request(actor, &key),
                    |_| json!({}),
                )
                .unwrap()
                .id,
        );
    }
    project(&content, space);
    let collection = content
        .create_notes_collection(space, "Large".into(), members, request(actor, "large"))
        .unwrap();
    let id = collection["id"].as_str().unwrap();
    assert!(content.advance_window(space, true).unwrap());
    let source = crate::db::space_path(&content.roots.appdata, space);
    let reader = replay::reader(&source).unwrap();
    let progress: (bool, Option<String>, i64) = reader
        .query_row(
            "SELECT ready,member_after,(SELECT total FROM window_scopes WHERE collection=?1)
         FROM window_publication",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert!(!progress.0);
    assert!(progress.1.is_some());
    assert_eq!(progress.2, 128);
    assert_loading(member_window(&content, space));
    project(&content, space);
    let count: i64 = reader
        .query_row(
            "SELECT total FROM window_scopes WHERE collection=?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 129);

    // Simulate restarting with a sidecar from an earlier journal epoch.
    let sidecar = Connection::open(replay::path(&source)).unwrap();
    sidecar
        .execute("UPDATE window_publication SET epoch='previous-epoch'", [])
        .unwrap();
    let count_rows = || {
        reader
            .query_row("SELECT count(*) FROM window_rows", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap()
    };
    let old_rows = count_rows();
    assert!(old_rows > 128);
    assert!(content.advance_window(space, true).unwrap());
    assert_eq!(count_rows(), old_rows - 128);
    assert_loading(member_window(&content, space));
    project(&content, space);
    assert_eq!(count_rows(), old_rows);
    assert_eq!(
        result(member_window(&content, space)).count.unwrap().value,
        129
    );
}

#[test]
fn consecutive_windows_return_the_attached_reader_to_its_slot() {
    let dir = tempfile::tempdir().unwrap();
    let content = content(&dir);
    let space = SpaceId::new();
    seed_note(&content, space, AccountId::new());
    project(&content, space);

    assert_eq!(
        result(member_window(&content, space)).count.unwrap().value,
        1
    );
    let slot = content.pin_slot(space).unwrap();
    let reader = slot
        .windows
        .take_reader()
        .expect("first request returned its reader");
    reader
        .conn
        .execute_batch("CREATE TEMP TABLE reused_reader (test INTEGER)")
        .unwrap();
    slot.windows.put_reader(reader, &content.window_handles);
    assert_eq!(
        result(member_window(&content, space)).count.unwrap().value,
        1
    );
    let reader = slot
        .windows
        .take_reader()
        .expect("second request returned its reader");
    reader
        .conn
        .query_row("SELECT count(*) FROM temp.reused_reader", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap();
}

#[test]
fn a_full_retained_connection_cache_still_serves_other_spaces() {
    let dir = tempfile::tempdir().unwrap();
    let content = content(&dir);
    let actor = AccountId::new();
    let mut pins = Vec::new();
    for _ in 0..8 {
        let space = SpaceId::new();
        pins.push(content.pin_slot(space).unwrap());
        seed_note(&content, space, actor);
        project(&content, space);
        assert_eq!(
            result(member_window(&content, space)).count.unwrap().value,
            1
        );
    }
    let space = SpaceId::new();
    let last = content.pin_slot(space).unwrap();
    seed_note(&content, space, actor);
    project(&content, space);
    assert_eq!(
        result(member_window(&content, space)).count.unwrap().value,
        1
    );
    assert!(last.windows.take_reader().is_none());
    let first = pins.first().unwrap();
    assert!(first.windows.take_reader().is_some());
}

#[test]
fn relevant_save_between_materialization_and_delivery_releases_no_rows() {
    let dir = tempfile::tempdir().unwrap();
    let content = content(&dir);
    let space = SpaceId::new();
    let actor = AccountId::new();
    seed_note(&content, space, actor);
    project(&content, space);
    let calls = Cell::new(0);
    let reply = content
        .window(
            space,
            &WindowRequest::Window {
                request: range(0, 10, WindowAnchor::Start),
            },
            None,
            || {
                calls.set(calls.get() + 1);
                if calls.get() == 2 {
                    content
                        .write(
                            space,
                            None,
                            "notes",
                            None,
                            json!({"title":"Concurrent"}),
                            b"body",
                            request(actor, "concurrent"),
                            |_| json!({}),
                        )
                        .unwrap();
                }
                Ok(WindowAccess {
                    scope: WindowScope::SpaceNotes,
                    audience: WindowAudience::Member,
                    count: WindowCountMode::Exact,
                    view: Uuid::nil(),
                })
            },
        )
        .unwrap();
    assert_loading(reply);
    assert_eq!(calls.get(), 3);
}

#[test]
fn sustained_writes_leave_no_exact_count_until_the_last_journal_page() {
    let dir = tempfile::tempdir().unwrap();
    let content = content(&dir);
    let space = SpaceId::new();
    let actor = AccountId::new();
    let mut note = seed_note(&content, space, actor);
    project(&content, space);
    assert_eq!(
        result(member_window(&content, space)).count.unwrap().value,
        1
    );
    let mut save = |i| {
        let key = format!("save-{i}");
        note = content
            .write(
                space,
                Some(note.id),
                "notes",
                Some(&note.revision),
                json!({"title":"First"}),
                key.as_bytes(),
                request(actor, &key),
                |_| json!({}),
            )
            .unwrap();
    };
    for i in 1..=260 {
        save(i);
    }
    content
        .write(
            space,
            None,
            "notes",
            None,
            json!({"title":"Second"}),
            b"body",
            request(actor, "second"),
            |_| json!({}),
        )
        .unwrap();
    assert_loading(member_window(&content, space));
    assert!(content.advance_window(space, true).unwrap());
    assert_loading(member_window(&content, space));
    for i in 261..=324 {
        save(i);
    }
    assert!(content.advance_window(space, true).unwrap());
    assert_loading(member_window(&content, space));
    assert!(!content.advance_window(space, true).unwrap());
    let reply = result(member_window(&content, space));
    assert_eq!(reply.count.unwrap().value, 2);
    assert_eq!(reply.rows.len(), 2);
}

#[test]
fn kill_before_publication_and_lost_sidecar_wal_replay_from_durable_journal() {
    if let Some(path) = std::env::var_os("CALTERNAL_WINDOW_CRASH_FIXTURE") {
        let dir = tempfile::TempDir::with_prefix_in("roots", &path).unwrap();
        let content = content(&dir);
        let space = SpaceId::new();
        let actor = AccountId::new();
        seed_note(&content, space, actor);
        project(&content, space);

        let source = crate::db::space_path(&content.roots.appdata, space);
        let sidecar = replay::path(&source);
        let conn = Connection::open(&sidecar).unwrap();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .unwrap();
        std::fs::copy(&sidecar, std::path::Path::new(&path).join("checkpoint.db")).unwrap();

        content
            .write(
                space,
                None,
                "notes",
                None,
                json!({"title":"Second"}),
                b"body",
                request(actor, "second"),
                |_| json!({}),
            )
            .unwrap();
        if std::env::var_os("CALTERNAL_WINDOW_DROP_WAL").is_some() {
            project(&content, space);
            assert_eq!(
                result(member_window(&content, space)).count.unwrap().value,
                2
            );
        }
        std::fs::write(
            std::path::Path::new(&path).join("ready"),
            serde_json::to_vec(&(dir.path(), space)).unwrap(),
        )
        .unwrap();
        loop {
            std::thread::park();
        }
    }
    for drop_wal in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let module = module_path!().split_once("::").unwrap().1;
        let name = format!(
            "{module}::kill_before_publication_and_lost_sidecar_wal_replay_from_durable_journal"
        );
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", &name])
            .env("CALTERNAL_WINDOW_CRASH_FIXTURE", root.path());
        if drop_wal {
            command.env("CALTERNAL_WINDOW_DROP_WAL", "1");
        }
        let mut child = command.spawn().unwrap();
        let ready = root.path().join("ready");
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let (path, space): (std::path::PathBuf, SpaceId) = loop {
            if let Ok(bytes) = std::fs::read(&ready)
                && let Ok(value) = serde_json::from_slice(&bytes)
            {
                break value;
            }
            if child.try_wait().unwrap().is_some() || std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("crash fixture did not commit the second note");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());
        let roots = Roots {
            appdata: path.join("appdata"),
            userdata_ssd: path.join("ssd"),
            userdata_hdd: path.join("hdd"),
        };
        let source = crate::db::space_path(&roots.appdata, space);
        if drop_wal {
            let sidecar = replay::path(&source);
            // Restore only the last checkpointed derivative. The authoritative
            // content and journal files retain both acknowledged writes.
            for suffix in ["db-wal", "db-shm"] {
                match std::fs::remove_file(sidecar.with_extension(suffix)) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => panic!("remove sidecar transient: {error}"),
                }
            }
            std::fs::copy(root.path().join("checkpoint.db"), sidecar).unwrap();
        }
        let reopened = Content::open(
            roots,
            path.join("secrets"),
            Duration::ZERO,
            crate::DEFAULT_RECEIPT_RETENTION,
            |_| {},
        )
        .unwrap();
        assert_loading(member_window(&reopened, space));
        project(&reopened, space);
        let ready = result(member_window(&reopened, space));
        assert_eq!(ready.count.unwrap().value, 2);
        assert!(ready.rows.iter().any(|row| row.fields["title"] == "Second"));
    }
}
