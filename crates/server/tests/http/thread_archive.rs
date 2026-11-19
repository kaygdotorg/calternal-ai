//! Saved-send admission respects an archived thread head and records refusal.
use super::*;

fn plugins() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins")
}

fn create_thread(server: &Server) -> String {
    let response = op(
        server,
        &server.owner,
        "create-thread",
        "threads.create",
        json!({"space_id":server.personal,"title":"Archive admission fixture"}),
    );
    assert_eq!(response.status, 200, "{}", response.body);
    response.body["data"]["thread"]["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn set_archive(server: &Server, thread: &str, archived: bool) {
    let action = if archived {
        "threads.archive"
    } else {
        "threads.unarchive"
    };
    let response = op(server, &server.owner, action, action, json!({"id":thread}));
    assert_eq!(response.status, 200, "{}", response.body);
}

fn assert_no_turn(data: &Path, space: &str, thread: &str) {
    let path = data.join("appdata/spaces").join(space).join("jobs.db");
    if !path.exists() {
        return;
    }
    let jobs =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let exists = jobs
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM jobs WHERE json_extract(input,'$.request.thread_id')=?1)",
            [thread],
            |row| row.get::<_, bool>(0),
        )
        .unwrap();
    assert!(!exists, "refused send created a durable turn");
}

#[cfg(debug_assertions)]
#[test]
fn archived_head_refuses_saved_send_and_keeps_refusal_on_replay() {
    let data = TempDir::new("archive-saved-send");
    let plugins = plugins();
    let server = start_full(&data.0, Some("after_thread_message"), None, Some(&plugins));
    let thread = create_thread(&server);
    let input = json!({"id":thread,"message":"Saved before archive"});
    assert!(
        try_op(
            &server,
            &server.owner,
            "saved-send",
            "threads.send",
            input.clone()
        )
        .is_none(),
        "debug server should stop after committing the message"
    );
    drop(server);

    let server = start_full(&data.0, None, None, Some(&plugins));
    let saved = op(
        &server,
        &server.owner,
        "get-saved",
        "threads.get",
        json!({"id":thread}),
    );
    assert_eq!(saved.status, 200, "{}", saved.body);
    assert_eq!(saved.body["data"]["thread"]["metadata"]["message_count"], 1);

    set_archive(&server, &thread, true);
    let refused = op(
        &server,
        &server.owner,
        "saved-send",
        "threads.send",
        input.clone(),
    );
    assert_eq!(refused.status, 409, "{}", refused.body);
    assert_eq!(
        refused.body["error"]["message"],
        "This thread was archived before the turn could start. The message was saved."
    );
    assert_eq!(
        refused.body["error"]["details"]["reason"],
        "thread_archived"
    );
    assert_no_turn(&data.0, &server.personal, &thread);
    drop(server);

    let server = start_full(&data.0, None, None, Some(&plugins));
    set_archive(&server, &thread, false);
    let replay = op(&server, &server.owner, "saved-send", "threads.send", input);
    assert_eq!(replay.status, refused.status, "{}", replay.body);
    assert_eq!(replay.body, refused.body);
    assert_no_turn(&data.0, &server.personal, &thread);

    let fresh = op(
        &server,
        &server.owner,
        "new-send",
        "threads.send",
        json!({"id":thread,"message":"A new send after unarchive"}),
    );
    assert_eq!(fresh.status, 200, "{}", fresh.body);
    assert!(fresh.body["data"]["job"]["id"].is_string());
}

#[test]
fn archived_thread_refuses_send_before_saving_an_intent() {
    let data = TempDir::new("archive-before-send");
    let server = start_full(&data.0, None, None, Some(&plugins()));
    let thread = create_thread(&server);
    set_archive(&server, &thread, true);

    let refused = op(
        &server,
        &server.owner,
        "send-after-archive",
        "threads.send",
        json!({"id":thread,"message":"Must not be saved"}),
    );
    assert_eq!(refused.status, 409, "{}", refused.body);
    assert_eq!(refused.body["error"]["message"], "this thread is archived");
    let saved = op(
        &server,
        &server.owner,
        "get-archived",
        "threads.get",
        json!({"id":thread}),
    );
    assert_eq!(saved.status, 200, "{}", saved.body);
    assert_eq!(saved.body["data"]["thread"]["metadata"]["message_count"], 0);
    assert_no_turn(&data.0, &server.personal, &thread);
}
