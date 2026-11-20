//! Window recovery through the live server and the operator socket.
use super::*;
use std::os::unix::fs::MetadataExt;

#[test]
fn corrupt_projection_is_rebuilt_before_releasing_rows() {
    let dir = TempDir::new("v2-corrupt");
    let srv = start(&dir.0, None);
    create(&srv, "first-note", &srv.personal, "durable body");
    let path = dir
        .0
        .join("appdata/spaces")
        .join(&srv.personal)
        .join("windows-v1.db");
    std::fs::write(path, b"invalid SQLite database").unwrap();
    let view = format!("/api/v2/spaces/{}/notes-view", srv.personal);
    let window = format!("{view}/window");
    let lease = open(&srv, &view, "exact");
    let request = requested("notes", "start");
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let reply = call(&srv, &srv.owner, &window, Some(request.clone()), &lease);
        match reply.status {
            200 if reply.body["state"] == "ready" => {
                assert_eq!(reply.body["count"]["value"], 1);
                break;
            }
            200 => {
                assert_eq!(reply.body["state"], "loading");
                assert_eq!(reply.body["rows"], json!([]));
                assert!(reply.body.get("count").is_none());
            }
            503 => {
                assert_eq!(reply.body["error"]["code"], "unavailable");
                assert!(reply.body.get("rows").is_none());
                assert!(reply.body.get("count").is_none());
            }
            429 => {}
            _ => panic!("unexpected {}: {}", reply.status, reply.body),
        }
        assert!(Instant::now() < deadline, "projection did not rebuild");
        std::thread::sleep(Duration::from_millis(1050));
    }
}

#[test]
fn unopenable_projection_persists_unavailability_until_it_can_rebuild() {
    let dir = TempDir::new("v2-unopenable");
    let srv = start(&dir.0, None);
    create(&srv, "first-note", &srv.personal, "durable body");
    let path = dir
        .0
        .join("appdata/spaces")
        .join(&srv.personal)
        .join("windows-v1.db");
    // An unexpected directory cannot be opened as SQLite or removed as a file.
    std::fs::create_dir(&path).unwrap();
    let view = format!("/api/v2/spaces/{}/notes-view", srv.personal);
    let lease = open(&srv, &view, "exact");
    let endpoint = format!("{view}/window");
    let request = requested("notes", "start");
    // The first authorized request wakes the worker if the slot is dormant.
    let first = call(&srv, &srv.owner, &endpoint, Some(request.clone()), &lease);
    assert!(matches!(first.status, 200 | 503), "{}", first.body);
    let failure = path.with_file_name("windows-v1.failure");
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if std::fs::read_to_string(&failure).is_ok_and(|body| body.starts_with("replay_failed ")) {
            break;
        }
        assert!(Instant::now() < deadline, "failure was not persisted");
        std::thread::sleep(Duration::from_millis(25));
    }
    let failed = call(&srv, &srv.owner, &endpoint, Some(request.clone()), &lease);
    assert_eq!(failed.status, 503, "{}", failed.body);
    assert_eq!(failed.body["error"]["code"], "unavailable");
    assert!(path.is_dir());
    std::fs::remove_dir(path).unwrap();
    loop {
        let reply = call(&srv, &srv.owner, &endpoint, Some(request.clone()), &lease);
        if reply.body["state"] == "ready" {
            assert_eq!(reply.body["count"]["value"], 1);
            break;
        }
        assert!(matches!(reply.status, 200 | 429 | 503), "{}", reply.body);
        assert!(
            Instant::now() < deadline,
            "window could not recover: {}",
            reply.body
        );
        std::thread::sleep(Duration::from_millis(1050));
    }
    assert!(!failure.exists());
}

fn window_status(dir: &TempDir, srv: &Server) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_calternal"))
        .args([
            "--json",
            "window-status",
            &srv.personal,
            "--installation-id",
            INSTALLATION,
            "--appdata",
        ])
        .arg(dir.0.join("appdata"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}

// Model a prior disk/source fault. The online write path stores valid JSON,
// and fixture-only SQL must bypass indexes and triggers that parse metadata.
fn poison(source: &rusqlite::Connection, item: &str) -> String {
    use rusqlite::config::DbConfig;
    source
        .set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false)
        .unwrap();
    let tx = source.unchecked_transaction().unwrap();
    let index: String = tx
        .query_row(
            "SELECT group_concat(sql,';') FROM sqlite_schema
             WHERE name IN ('standard_collections','item_title_key')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    tx.execute_batch("DROP INDEX standard_collections; DROP INDEX item_title_key")
        .unwrap();
    tx.execute("UPDATE items SET metadata='{' WHERE id=?1", [item])
        .unwrap();
    tx.commit().unwrap();
    index
}

#[test]
fn poison_metadata_stops_retries_without_replacing_sidecar() {
    let dir = TempDir::new("v2-poison");
    let srv = start(&dir.0, None);
    create(&srv, "healthy", &srv.personal, "durable body");
    let view = format!("/api/v2/spaces/{}/notes-view", srv.personal);
    let endpoint = format!("{view}/window");
    let lease = open(&srv, &view, "exact");
    let request = requested("notes", "start");
    ready(&srv, &srv.owner, &endpoint, request.clone(), &lease);
    let space = dir.0.join("appdata/spaces").join(&srv.personal);
    let path = space.join("windows-v1.db");
    let held = std::fs::File::open(&path).unwrap();
    let valid_inode = held.metadata().unwrap().ino();
    let sidecar = rusqlite::Connection::open(&path).unwrap();
    sidecar.busy_timeout(Duration::from_secs(2)).unwrap();
    sidecar.execute_batch("BEGIN IMMEDIATE").unwrap();
    let note = create(&srv, "poison", &srv.personal, "this was normal");
    let source = rusqlite::Connection::open(space.join("content.db")).unwrap();
    source.busy_timeout(Duration::from_secs(2)).unwrap();
    poison(&source, note["id"].as_str().unwrap());
    drop(sidecar);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let status = window_status(&dir, &srv);
        assert_eq!(std::fs::metadata(&path).unwrap().ino(), valid_inode);
        if status["retries_exhausted"] == true {
            assert_eq!(status["failure"], "malformed_source_metadata");
            assert_eq!(status["skipped_items"], 0);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "retry limit not reached: {status}"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    let marker = space.join("windows-v1.failure");
    assert!(
        std::fs::read_to_string(marker)
            .unwrap()
            .starts_with("malformed_source_metadata 5 ")
    );
    drop(srv);
    let restarted = start(&dir.0, None);
    let lease = open(&restarted, &view, "exact");
    let reply = call(
        &restarted,
        &restarted.owner,
        &endpoint,
        Some(request),
        &lease,
    );
    assert_eq!(reply.status, 503, "{}", reply.body);
    assert_eq!(reply.body["error"]["code"], "unavailable");
    assert_eq!(std::fs::metadata(path).unwrap().ino(), valid_inode);
}

#[test]
fn old_sidecar_schema_rebuilds_to_current_version() {
    let dir = TempDir::new("v2-old-schema");
    let srv = start(&dir.0, None);
    create(&srv, "first-note", &srv.personal, "durable body");
    let path = dir
        .0
        .join("appdata/spaces")
        .join(&srv.personal)
        .join("windows-v1.db");
    let old = rusqlite::Connection::open(&path).unwrap();
    old.execute_batch("CREATE TABLE old_sidecar_marker (id INTEGER); PRAGMA user_version=1")
        .unwrap();
    drop(old);
    let view = format!("/api/v2/spaces/{}/notes-view", srv.personal);
    let lease = open(&srv, &view, "exact");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let reply = call(
            &srv,
            &srv.owner,
            &format!("{view}/window"),
            Some(requested("notes", "start")),
            &lease,
        );
        if reply.body["state"] == "ready" {
            assert_eq!(reply.body["count"]["value"], 1);
            break;
        }
        assert!(matches!(reply.status, 200 | 429 | 503), "{}", reply.body);
        assert!(Instant::now() < deadline, "old sidecar did not rebuild");
        std::thread::sleep(Duration::from_millis(600));
    }
    let new = rusqlite::Connection::open(path).unwrap();
    let version: i64 = new
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);
    let legacy: bool = new
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='old_sidecar_marker')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!legacy);
}

#[test]
fn admin_counts_inherited_collection_omissions_without_hiding_notes() {
    let dir = TempDir::new("v2-omit");
    let log = dir.0.join("warnings.log");
    let mut cmd = server_command(&dir.0, None, None, None);
    cmd.stderr(Stdio::from(std::fs::File::create(&log).unwrap()));
    let srv = start_server(&dir.0, cmd);
    let note = create(&srv, "retained-note", &srv.personal, "body");
    let space = dir.0.join("appdata/spaces").join(&srv.personal);
    let source = rusqlite::Connection::open(space.join("content.db")).unwrap();
    source.busy_timeout(Duration::from_secs(2)).unwrap();
    source
        .execute_batch("DROP TRIGGER window_member_limit")
        .unwrap();
    for n in 0..32 {
        let reply = op(
            &srv,
            &srv.owner,
            &format!("inherited-{n}"),
            "collections.create",
            json!({"space_id":srv.personal,"title":format!("Inherited {n}"),"members":[note["id"]]}),
        );
        assert_eq!(reply.status, 200, "{}", reply.body);
    }
    let view = format!("/api/v2/spaces/{}/notes-view", srv.personal);
    let lease = open(&srv, &view, "exact");
    let reply = ready(
        &srv,
        &srv.owner,
        &format!("{view}/window"),
        requested("notes", "start"),
        &lease,
    );
    assert_eq!(reply.body["count"]["value"], 1);
    assert_eq!(reply.body["rows"].as_array().unwrap().len(), 1);
    let status = window_status(&dir, &srv);
    assert_eq!(status["skipped_items"], 1);
    assert!(status["failure"].is_null());
    let update = op(
        &srv,
        &srv.owner,
        "edit-over-limit-note",
        "notes.update",
        json!({"id":note["id"],"base_revision":note["revision"],
            "space_id":srv.personal,"markdown":"updated"}),
    );
    assert_eq!(update.status, 200, "{}", update.body);
    let newer = ready(
        &srv,
        &srv.owner,
        &format!("{view}/window"),
        requested("notes", "start"),
        &lease,
    );
    assert_ne!(newer.body["result_version"], reply.body["result_version"]);
    let warnings = std::fs::read_to_string(log).unwrap();
    assert_eq!(
        warnings
            .matches("window omitted explicit collections")
            .count(),
        1,
        "{warnings}"
    );
}

#[test]
fn failed_reader_retries_in_place_after_metadata_is_repaired() {
    let dir = TempDir::new("v2-reader");
    let srv = start(&dir.0, None);
    let note = create(&srv, "first-note", &srv.personal, "body");
    let view = format!("/api/v2/spaces/{}/notes-view", srv.personal);
    let endpoint = format!("{view}/window");
    let lease = open(&srv, &view, "exact");
    let request = requested("notes", "start");
    ready(&srv, &srv.owner, &endpoint, request.clone(), &lease);
    let space = dir.0.join("appdata/spaces").join(&srv.personal);
    let path = space.join("windows-v1.db");
    let inode = std::fs::metadata(&path).unwrap().ino();
    let source = rusqlite::Connection::open(space.join("content.db")).unwrap();
    source.busy_timeout(Duration::from_secs(2)).unwrap();
    let id = note["id"].as_str().unwrap();
    let metadata: String = source
        .query_row("SELECT metadata FROM items WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    // Leave the journal head unchanged so this fault reaches row materialization.
    let index = poison(&source, id);
    let unavailable = call(&srv, &srv.owner, &endpoint, Some(request.clone()), &lease);
    assert_eq!(unavailable.status, 503, "{}", unavailable.body);
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let status = window_status(&dir, &srv);
        if status["failure"] == "malformed_source_metadata" {
            assert_eq!(status["retries_exhausted"], false);
            break;
        }
        assert!(Instant::now() < deadline, "{status}");
        std::thread::sleep(Duration::from_millis(50));
    }
    let repair = source.unchecked_transaction().unwrap();
    repair
        .execute("UPDATE items SET metadata=?2 WHERE id=?1", [id, &metadata])
        .unwrap();
    repair.execute_batch(&index).unwrap();
    repair.commit().unwrap();
    loop {
        let reply = call(&srv, &srv.owner, &endpoint, Some(request.clone()), &lease);
        if reply.body["state"] == "ready" {
            assert_eq!(reply.body["count"]["value"], 1);
            break;
        }
        assert!(matches!(reply.status, 200 | 429 | 503), "{}", reply.body);
        assert!(Instant::now() < deadline, "reader did not recover");
        std::thread::sleep(Duration::from_millis(300));
    }
    loop {
        if window_status(&dir, &srv)["failure"].is_null() {
            break;
        }
        assert!(Instant::now() < deadline, "reader failure was not cleared");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(std::fs::metadata(path).unwrap().ino(), inode);
}
