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
    assert_eq!(response.body["audience"], "own_space_member");
    response.body["lease"].clone()
}

#[test]
fn sealed_windows_bind_to_the_actor_lease_partition_and_count_policy() {
    let dir = TempDir::new("v2-window");
    let srv = start(&dir.0, None);
    create(&srv, "first-note", &srv.personal, "unpublished body");
    let view = format!("/api/v2/spaces/{}/notes-view", srv.personal);
    let window = format!("{view}/window");
    let exact = open(&srv, &view, "exact");
    let first = call(
        &srv,
        &srv.owner,
        &window,
        Some(requested("notes", "start")),
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
        call(
            &srv,
            &srv.guest,
            &window,
            Some(requested("notes", "start")),
            lease
        )
        .status,
        200
    );
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
