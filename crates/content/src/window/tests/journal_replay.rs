//! Catch-up and restart checks against the durable content and journal.

use super::*;

pub(super) fn verify_inherited_memberships(
    content: &Content,
    space: SpaceId,
    actor: AccountId,
    item: Uuid,
    source: &std::path::Path,
) {
    // Model memberships created by a release before scope admission. Opening
    // such a space preserves the lists, but replay cannot publish a partial set.
    Connection::open(&source)
        .unwrap()
        .execute_batch("DROP TRIGGER window_member_limit")
        .unwrap();
    let inherited = content
        .create_notes_collection(
            space,
            "inherited".into(),
            vec![item],
            request(actor, "inherited"),
        )
        .unwrap();
    let error = content.advance_window(space, true).unwrap_err();
    assert_eq!(error.code, ErrorCode::EffectiveScopeLimitExceeded);
    assert_loading(member_window(content, space));
    let id = inherited["id"].as_str().unwrap().parse().unwrap();
    content
        .set_collection_members(
            space,
            id,
            inherited["revision"].as_str().unwrap(),
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
