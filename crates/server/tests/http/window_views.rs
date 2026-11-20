#[path = "window_recovery.rs"]
mod recovery;

use super::*;

fn call(srv: &Server, token: &str, path: &str, body: Option<Value>, lease: &Value) -> Resp {
    let auth = format!("Bearer {token}");
    let mut headers = vec![
        ("Authorization", auth.as_str()),
        ("Content-Type", "application/json"),
    ];
    if !lease.is_null() {
        headers.push((
            "x-calternal-view-lease",
            lease["lease_ref"].as_str().unwrap(),
        ));
        headers.push((
            "x-calternal-cache-partition",
            lease["cache_partition"].as_str().unwrap(),
        ));
    }
    let method = if body.is_some() { "POST" } else { "GET" };
    let body = body.map(|body| body.to_string());
    raw(
        srv.port,
        method,
        path,
        &headers,
        body.as_deref().map(str::as_bytes),
    )
    .unwrap()
}

fn ready(srv: &Server, token: &str, path: &str, body: Value, lease: &Value) -> Resp {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let reply = call(srv, token, path, Some(body.clone()), lease);
        if reply.body["state"] != "loading" && reply.status != 429 {
            return reply;
        }
        if reply.body["state"] == "loading" {
            assert_eq!(reply.body["rows"], json!([]));
            assert!(reply.body.get("count").is_none());
        }
        assert!(
            Instant::now() < deadline,
            "window never published: {}",
            reply.body
        );
        // A cold retry must leave enough count starts for the remaining requests.
        std::thread::sleep(Duration::from_millis(1050));
    }
}

fn requested(bind: &str, anchor: &str) -> Value {
    json!({"type":"window","request":{"bind":bind,"anchor":anchor,"from":0,"to":10}})
}

fn open(srv: &Server, path: &str, count: &str) -> Value {
    let discovery = call(
        srv,
        &srv.owner,
        &format!("{path}?count={count}"),
        None,
        &Value::Null,
    );
    assert_eq!(discovery.status, 200, "{}", discovery.body);
    let response = call(
        srv,
        &srv.owner,
        path,
        Some(json!({"type":"view_open","view_ref":discovery.body["view_ref"]})),
        &Value::Null,
    );
    assert_eq!(response.status, 200, "{}", response.body);
    assert!(matches!(
        response.body["audience"].as_str(),
        Some("own_space_member" | "member")
    ));
    response.body["lease"].clone()
}

#[test]
fn unrelated_thread_commits_keep_ready_notes_available_while_replay_is_blocked() {
    let dir = TempDir::new("v2-disjoint");
    let plugins = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let srv = start_full(&dir.0, None, None, Some(&plugins));
    create(&srv, "first-note", &srv.personal, "durable body");
    let view = format!("/api/v2/spaces/{}/notes-view", srv.personal);
    let window = format!("{view}/window");
    let lease = open(&srv, &view, "exact");
    let initial = ready(
        &srv,
        &srv.owner,
        &window,
        requested("notes", "start"),
        &lease,
    );
    assert_eq!(initial.status, 200);
    let sidecar = rusqlite::Connection::open(
        dir.0
            .join("appdata/spaces")
            .join(&srv.personal)
            .join("windows-v1.db"),
    )
    .unwrap();
    sidecar.execute_batch("BEGIN IMMEDIATE").unwrap();
    let thread = op(
        &srv,
        &srv.owner,
        "unrelated-thread",
        "threads.create",
        json!({"space_id":srv.personal,"title":"Unrelated"}),
    );
    assert_eq!(thread.status, 200, "{}", thread.body);
    let after_thread = open(&srv, &view, "exact");
    let current = call(
        &srv,
        &srv.owner,
        &window,
        Some(requested("notes", "start")),
        &after_thread,
    );
    assert_eq!(current.status, 200, "{}", current.body);
    assert_eq!(current.body["state"], "ready", "{}", current.body);
    assert_eq!(current.body["count"]["value"], 1);

    create(&srv, "relevant-note", &srv.personal, "another note");
    let after_note = open(&srv, &view, "exact");
    let withheld = call(
        &srv,
        &srv.owner,
        &window,
        Some(requested("notes", "start")),
        &after_note,
    );
    assert_eq!(withheld.body["state"], "loading", "{}", withheld.body);
    assert_eq!(withheld.body["rows"], json!([]));
    assert!(withheld.body.get("count").is_none());
}

#[test]
fn collection_writers_refuse_scope_overflow_with_http_422() {
    let dir = TempDir::new("v2-member-cap");
    let srv = start(&dir.0, None);
    let note = create(&srv, "limit-note", &srv.personal, "body");
    let member = note["id"].as_str().expect("note identifier");
    let write = |n, members: Value| {
        op(
            &srv,
            &srv.owner,
            &format!("collection-{n}"),
            "collections.create",
            json!({"space_id":srv.personal,"title":format!("Collection {n}"),"members":members}),
        )
    };
    for n in 0..31 {
        let reply = write(n, json!([member]));
        assert_eq!(reply.status, 200, "{}", reply.body);
    }
    let overflow = write(31, json!([member]));
    let empty = write(32, json!([]));
    assert_eq!(empty.status, 200, "{}", empty.body);
    let collection = &empty.body["data"];
    let update = op(
        &srv,
        &srv.owner,
        "add-overflow",
        "collections.set_members",
        json!({"id":collection["id"],"base_revision":collection["revision"],"members":[member]}),
    );
    for reply in [overflow, update] {
        assert_eq!(reply.status, 422, "{}", reply.body);
        assert_eq!(
            reply.body["error"]["code"],
            "effective_scope_limit_exceeded"
        );
    }
}

#[test]
fn space_count_quota_is_independent_for_colliding_space_ids() {
    let dir = TempDir::new("v2-quotas");
    let srv = start(&dir.0, None);
    let mut seen = std::collections::HashMap::new();
    let pair = (0..33)
        .find_map(|n| {
            let data = op(
                &srv,
                &srv.owner,
                &format!("space-{n}"),
                "spaces.create",
                json!({"name":format!("Quota fixture {n}")}),
            );
            assert_eq!(data.status, 200, "{}", data.body);
            let id = data.body["data"]["space"]["id"]
                .as_str()
                .unwrap()
                .to_owned();
            let bucket = uuid::Uuid::parse_str(&id).unwrap().as_bytes()[15] % 32;
            seen.insert(bucket, id.clone()).map(|first| (first, id))
        })
        .unwrap();
    let a = format!("/api/v2/spaces/{}/notes-view", pair.0);
    let b = format!("/api/v2/spaces/{}/notes-view", pair.1);
    let first = open(&srv, &a, "exact");
    let second = open(&srv, &b, "exact");
    for n in 0..5 {
        let answer = call(
            &srv,
            &srv.owner,
            &format!("{a}/window"),
            Some(requested("notes", "start")),
            &first,
        );
        assert_eq!(answer.status, if n == 4 { 429 } else { 200 });
    }
    let answer = call(
        &srv,
        &srv.owner,
        &format!("{b}/window"),
        Some(requested("notes", "start")),
        &second,
    );
    assert_eq!(answer.status, 200, "{}", answer.body);
}

#[test]
fn sealed_windows_bind_to_the_actor_lease_partition_and_count_policy() {
    let dir = TempDir::new("v2-window");
    let srv = start(&dir.0, None);
    create(&srv, "first-note", &srv.personal, "unpublished body");
    let view = format!("/api/v2/spaces/{}/notes-view", srv.personal);
    let window = format!("{view}/window");
    let exact = open(&srv, &view, "exact");
    let first = ready(
        &srv,
        &srv.owner,
        &window,
        requested("notes", "start"),
        &exact,
    );
    assert_eq!(first.status, 200, "{}", first.body);
    assert_eq!(first.body["rows"][0]["fields"], json!({"title":"T"}));
    assert_eq!(first.body["count"], json!({"mode":"exact","value":1}));
    assert!(!first.body.to_string().contains("unpublished body"));
    let end = call(
        &srv,
        &srv.owner,
        &window,
        Some(requested("notes", "end")),
        &exact,
    );
    assert_eq!(end.status, 200);
    assert_eq!(end.body["anchor"], "end");
    assert_eq!(end.body["rows"], first.body["rows"]);

    let unsupported = call(
        &srv,
        &srv.owner,
        &window,
        Some(requested("guessed-bind", "start")),
        &exact,
    );
    assert_eq!(unsupported.status, 422);
    assert_eq!(
        unsupported.body,
        json!({"status":422,"code":"unsupported_query_plan","reason":"unsupported_plan"})
    );
    let foreign = call(
        &srv,
        &srv.guest,
        &window,
        Some(requested("notes", "start")),
        &exact,
    );
    assert_eq!(foreign.status, 404);

    let none = open(&srv, &view, "none");
    let start = call(
        &srv,
        &srv.owner,
        &window,
        Some(requested("notes", "start")),
        &none,
    );
    assert_eq!(start.status, 200, "{}", start.body);
    assert!(start.body.get("count").is_none());
    let end = call(
        &srv,
        &srv.owner,
        &window,
        Some(requested("notes", "end")),
        &none,
    );
    assert_eq!(end.status, 422);
    assert_eq!(
        end.body,
        json!({"status":422,"code":"unsupported_query_plan","reason":"end_unavailable"})
    );

    let renewal = call(
        &srv,
        &srv.owner,
        &format!("{view}/renew"),
        Some(json!({
            "type":"lease_renew","lease_ref":none["lease_ref"],"cache_partition":none["cache_partition"],
            "grant_revision":none["grant_revision"],"renewal_id":"r1","result_versions":{}
        })),
        &Value::Null,
    );
    assert_eq!(renewal.status, 200, "{}", renewal.body);
    assert_eq!(renewal.body["request_lease_ref"], none["lease_ref"]);
    assert_eq!(renewal.body["retained_results"], json!([]));
    assert_ne!(renewal.body["lease"]["lease_ref"], none["lease_ref"]);
    let stale = call(
        &srv,
        &srv.owner,
        &window,
        Some(requested("notes", "start")),
        &none,
    );
    assert_eq!(stale.status, 404);
    let current = call(
        &srv,
        &srv.owner,
        &window,
        Some(requested("notes", "start")),
        &renewal.body["lease"],
    );
    assert_eq!(current.status, 200, "{}", current.body);
    assert_eq!(current.body["result_version"], start.body["result_version"]);
}

#[test]
fn space_revocation_releases_no_live_window_rows_or_count() {
    let dir = TempDir::new("v2-revoke");
    let srv = start(&dir.0, None);
    create(&srv, "shared-note", &srv.shared, "shared content");
    let who = call(&srv, &srv.guest, "/api/v1/describe", None, &Value::Null);
    let view = format!("/api/v2/spaces/{}/notes-view", srv.shared);
    let discover = call(&srv, &srv.guest, &view, None, &Value::Null);
    assert_eq!(discover.status, 200, "{}", discover.body);
    let opened = call(
        &srv,
        &srv.guest,
        &view,
        Some(json!({
            "type":"view_open","view_ref":discover.body["view_ref"]
        })),
        &Value::Null,
    );
    assert_eq!(opened.status, 200, "{}", opened.body);
    let lease = &opened.body["lease"];
    let window = format!("{view}/window");
    assert_eq!(
        ready(
            &srv,
            &srv.guest,
            &window,
            requested("notes", "start"),
            lease
        )
        .status,
        200
    );
    let sidecar = rusqlite::Connection::open(
        dir.0
            .join("appdata/spaces")
            .join(&srv.shared)
            .join("windows-v1.db"),
    )
    .unwrap();
    sidecar.busy_timeout(Duration::from_secs(2)).unwrap();
    create(
        &srv,
        "worker-polled-note",
        &srv.shared,
        "more shared content",
    );
    // Observe replay without an HTTP window request waking the worker.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let caught_up: bool = sidecar
            .query_row(
                "SELECT (SELECT ready FROM window_publication) AND EXISTS(
                    SELECT 1 FROM window_scopes
                    WHERE collection='notes' AND audience='space' AND total=2)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        if caught_up {
            break;
        }
        assert!(Instant::now() < deadline, "worker did not poll the journal");
        std::thread::sleep(Duration::from_millis(20));
    }
    sidecar.execute_batch("BEGIN IMMEDIATE").unwrap();
    create(&srv, "unpublished-note", &srv.shared, "new body");
    let stale = call(
        &srv,
        &srv.guest,
        &window,
        Some(requested("notes", "start")),
        lease,
    );
    assert_eq!(stale.status, 200);
    assert_eq!(stale.body["state"], "loading");
    assert_eq!(stale.body["rows"], json!([]));
    assert!(stale.body.get("count").is_none());
    let revoked = op(
        &srv,
        &srv.owner,
        "revoke-window-role",
        "spaces.remove_member",
        json!({"space_id":srv.shared, "account_id":who.body["principal"]["id"]}),
    );
    assert_eq!(revoked.status, 200, "{}", revoked.body);
    let rejected = call(
        &srv,
        &srv.guest,
        &window,
        Some(requested("notes", "end")),
        lease,
    );
    assert_eq!(rejected.status, 404);
    assert!(rejected.body.get("rows").is_none());
    assert!(rejected.body.get("count").is_none());
}

#[cfg(feature = "test-plugins")]
#[test]
fn installed_reading_list_serves_only_the_member_list_and_declared_fields() {
    let dir = TempDir::new("v2-reading-list");
    let mut command = server_command(&dir.0, None, None, None);
    command.env("CALTERNAL_TEST_READING_LIST", "1");
    let srv = start_server(&dir.0, command);
    let personal = format!("/api/v2/spaces/{}/reading-list-view", srv.personal);
    let guest = call(&srv, &srv.guest, "/api/v1/describe", None, &Value::Null);
    let guest_id = guest.body["principal"]["id"].as_str().unwrap();
    let guest_spaces = described_spaces(srv.port, &srv.guest);
    let foreign_space = guest_spaces
        .iter()
        .find(|space| **space != srv.shared)
        .unwrap();

    let auth = rusqlite::Connection::open(dir.0.join("appdata/system/auth.db")).unwrap();
    let owns_item_only_grant: bool = auth.query_row(
        "SELECT EXISTS(SELECT 1 FROM item_grants WHERE space_id=?1 AND account_id=?2 AND kind='notes')",
        [&srv.personal, guest_id], |row| row.get(0),
    ).unwrap();
    assert!(
        owns_item_only_grant,
        "the guest must have actual item authority"
    );
    assert_eq!(
        call(&srv, &srv.guest, &personal, None, &Value::Null).status,
        404
    );
    let foreign = format!("/api/v2/spaces/{foreign_space}/reading-list-view");
    assert_eq!(
        call(&srv, &srv.owner, &foreign, None, &Value::Null).status,
        404
    );

    let descriptor = call(&srv, &srv.owner, &personal, None, &Value::Null);
    assert_eq!(descriptor.status, 200, "{}", descriptor.body);
    assert_eq!(descriptor.body["bind"], "items");
    assert_eq!(descriptor.body["fields"], json!(["title", "added", "read"]));
    assert_eq!(descriptor.body["count"], "exact");
    assert_eq!(descriptor.body["description"]["binds"][0]["id"], "items");
    let lease = open(&srv, &personal, "exact");
    let url = format!("{personal}/window");
    let page = ready(
        &srv,
        &srv.owner,
        &url,
        json!({
            "type":"window","request":{"bind":"items","from":0,"to":2},
        }),
        &lease,
    );
    assert_eq!(page.status, 200, "{}", page.body);
    let entries = page.body["rows"].as_array().unwrap();
    assert_eq!(
        entries[0]["fields"],
        json!({"title":"Second article","added":"2026-11-19T12:00:00.000Z","read":true})
    );
    assert_eq!(entries[1]["fields"]["title"], "Third article");
    assert_eq!(page.body["count"], json!({"mode":"exact","value":3}));
    let next = call(
        &srv,
        &srv.owner,
        &url,
        Some(json!({
            "type":"window","request":{"bind":"items","from":2,"to":3},
        })),
        &lease,
    );
    assert_eq!(next.status, 200, "{}", next.body);
    assert_eq!(next.body["rows"][0]["fields"]["title"], "First article");
    let end = call(
        &srv,
        &srv.owner,
        &url,
        Some(json!({
            "type":"window","request":{"bind":"items","anchor":"end","from":0,"to":1},
        })),
        &lease,
    );
    assert_eq!(end.status, 200, "{}", end.body);
    assert_eq!(end.body["rows"][0]["fields"]["title"], "First article");
    let rejected = call(
        &srv,
        &srv.owner,
        &url,
        Some(requested("reminders", "start")),
        &lease,
    );
    assert_eq!(rejected.status, 422);
    assert_eq!(rejected.body["reason"], "unsupported_plan");
    assert_eq!(
        call(
            &srv,
            &srv.guest,
            &url,
            Some(requested("items", "start")),
            &lease
        )
        .status,
        404
    );
    let notes = format!("/api/v2/spaces/{}/notes-view/window", srv.personal);
    assert_eq!(
        call(
            &srv,
            &srv.owner,
            &notes,
            Some(requested("items", "start")),
            &lease
        )
        .status,
        404
    );
}
