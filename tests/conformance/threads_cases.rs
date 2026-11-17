//! Every thread operation is exercised with real passkey sessions and foreign spaces.
use super::*;

#[tokio::test]
async fn thread_operations_hide_foreign_targets_and_keep_children_after_archive() {
    let f = Fixture::new().await;
    let created = f
        .op(
            1,
            "threads.create",
            json!({"space_id":f.spaces[0],"title":"Parent"}),
        )
        .await;
    created.ok();
    let id = created.body["data"]["thread"]["id"].as_str().unwrap();
    let missing = uuid::Uuid::now_v7().to_string();
    let mut rows = vec![];
    for op in [
        "threads.create",
        "threads.list",
        "threads.get",
        "threads.send",
        "threads.stop",
        "threads.archive",
        "threads.delete",
    ] {
        for who in [0, 2, 3] {
            let (foreign, absent) = match op {
                "threads.create" => (
                    json!({"space_id":f.spaces[0],"title":"probe"}),
                    json!({"space_id":missing,"title":"probe"}),
                ),
                "threads.list" => (json!({"space_id":f.spaces[0]}), json!({"space_id":missing})),
                "threads.send" => (
                    json!({"id":id,"message":"must not cross spaces"}),
                    json!({"id":missing,"message":"must not cross spaces"}),
                ),
                _ => (json!({"id":id}), json!({"id":missing})),
            };
            let a = f.op(who, op, foreign).await;
            let b = f.op(who, op, absent).await;
            assert_eq!(a.status, 404, "{op}/{who}: {}", a.body);
            assert_eq!(a.signature(), b.signature(), "{op}/{who}");
            rows.push(json!({"operation":op,"account":who,"equal":true}));
        }
    }
    let wrong_space = f
        .op(
            1,
            "threads.send",
            json!({"id":id,"space_id":f.spaces[1],"message":"wrong hint"}),
        )
        .await;
    assert_eq!(wrong_space.status, 404);
    let child=f.op(1,"threads.create",json!({"space_id":f.spaces[0],"title":"Child","parent_thread":id,"spawned_by":uuid::Uuid::now_v7()})).await;
    child.ok();
    let child_id = &child.body["data"]["thread"]["id"];
    f.op(1, "threads.archive", json!({"id":child_id}))
        .await
        .ok();
    let children = f
        .op(
            1,
            "threads.list",
            json!({"space_id":f.spaces[0],"parent_thread":id}),
        )
        .await;
    children.ok();
    assert_eq!(children.body["data"]["threads"][0]["id"], *child_id);
    let wrong_kind = f.op(1, "resolve", json!({"kind":"notes","id":id})).await;
    assert_eq!(wrong_kind.status, 404);
    let deleted = f.op(1, "threads.delete", json!({"id":child_id})).await;
    deleted.ok();
    let tombstone = f.op(1, "threads.get", json!({"id":child_id})).await;
    tombstone.ok();
    assert_eq!(
        tombstone.body["data"]["thread"]["metadata"]["deleted"],
        true
    );
    let trash = f
        .dir
        .path()
        .join("userdata-ssd/spaces")
        .join(&f.spaces[0])
        .join(".trash")
        .join(format!("{}.md", child_id.as_str().unwrap()));
    assert!(trash.is_file());
    let descendants = f
        .op(
            1,
            "threads.list",
            json!({"space_id":f.spaces[0],"parent_thread":id}),
        )
        .await;
    descendants.ok();
    assert_eq!(descendants.body["data"]["threads"][0]["id"], *child_id);
    assert_eq!(
        descendants.body["data"]["threads"][0]["metadata"]["deleted"],
        true
    );

    evidence("threads-isolation", json!(rows));
}

#[tokio::test]
async fn thread_send_crash_after_append_retries_into_exactly_one_turn() {
    let mut f = Fixture::new().await;
    let created = f
        .op(
            1,
            "threads.create",
            json!({"space_id":f.spaces[0],"title":"Crash fixture"}),
        )
        .await;
    created.ok();
    let id = created.body["data"]["thread"]["id"].clone();
    f.stop();
    std::fs::write(f.dir.path().join("thread-send-fault"), b"armed").unwrap();
    f.start().await;
    let input = json!({"id":id,"message":"one durable message"});
    let result = f
        .client
        .post(format!("{}/api/v1/operations", f.base))
        .header("host", "localhost:18080")
        .header("origin", ORIGIN)
        .header("cookie", &f.cookies[1])
        .json(&json!({"request_id":"send-crash","operation":"threads.send","input":input}))
        .send()
        .await;
    assert!(result.is_err(), "send should crash before reply");
    f.stop();
    std::fs::remove_file(f.dir.path().join("thread-send-fault")).unwrap();
    f.start().await;
    let saved = f.op(1, "threads.get", json!({"id":id})).await;
    saved.ok();
    assert_eq!(
        saved.body["data"]["thread"]["markdown"]
            .as_str()
            .unwrap()
            .matches("one durable message")
            .count(),
        1
    );
    let first = f
        .op_id(1, "threads.send", input.clone(), "send-crash")
        .await;
    first.ok();
    let replay = f.op_id(1, "threads.send", input, "send-crash").await;
    replay.ok();
    assert_eq!(
        first.body["data"]["job"]["id"],
        replay.body["data"]["job"]["id"]
    );
    assert_eq!(first.body["data"]["job"]["space_id"], f.spaces[0]);
    let changed = f
        .op_id(
            1,
            "threads.send",
            json!({"id":id,"message":"different"}),
            "send-crash",
        )
        .await;
    assert_eq!(changed.status, 409);
    f.op(1, "threads.stop", json!({"id":id})).await.ok();
    f.op(1, "threads.delete", json!({"id":id})).await.ok();
    evidence(
        "threads-send-crash",
        json!({"single_message":true,"single_job":true,"space":f.spaces[0]}),
    );
}

#[tokio::test]
async fn threads_follow_the_independent_reader_editor_manager_and_owner_matrix() {
    let f = Fixture::new().await;
    for (index, can_write) in [(2, false), (3, true), (4, true)] {
        let space = &f.spaces[index];
        let owner = f
            .op(
                1,
                "threads.create",
                json!({"space_id":space,"title":"Role matrix"}),
            )
            .await;
        owner.ok();
        let thread = &owner.body["data"]["thread"]["id"];
        f.op(2, "threads.get", json!({"id":thread})).await.ok();
        f.op(2, "threads.list", json!({"space_id":space}))
            .await
            .ok();
        for operation in ["threads.create", "threads.archive", "threads.delete"] {
            let target = f
                .op(
                    1,
                    "threads.create",
                    json!({"space_id":space,"title":operation}),
                )
                .await;
            target.ok();
            let input = if operation == "threads.create" {
                json!({"space_id":space,"title":"Member created"})
            } else {
                json!({"id":target.body["data"]["thread"]["id"]})
            };
            let reply = f.op(2, operation, input).await;
            assert_eq!(
                reply.status,
                if can_write { 200 } else { 403 },
                "{index}/{operation}: {}",
                reply.body
            );
        }
        let send = f
            .op(
                2,
                "threads.send",
                json!({"id":thread,"message":"role matrix prompt"}),
            )
            .await;
        assert_eq!(
            send.status,
            if can_write { 200 } else { 403 },
            "{index}/send: {}",
            send.body
        );
        let stop = f.op(2, "threads.stop", json!({"id":thread})).await;
        assert_eq!(
            stop.status,
            if can_write { 200 } else { 403 },
            "{index}/stop: {}",
            stop.body
        );
    }
}

#[tokio::test]
async fn editor_operations_cannot_replace_standing_instructions_in_any_space() {
    let f = Fixture::new().await;
    let own = &f.spaces[3]; // account 2 is Editor here, and has no access to private 0.
    let foreign = &f.spaces[0];
    let mut paths = vec![];
    for space in [own, foreign] {
        let folder = f
            .dir
            .path()
            .join("userdata-ssd/spaces")
            .join(space)
            .join("AI");
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(&folder).unwrap();
        let path = folder.join("AGENTS.md");
        std::fs::write(&path, format!("Manager context for {space}")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        paths.push((path, format!("Manager context for {space}")));
    }
    let note = f
        .op(
            2,
            "notes.create",
            json!({"space_id":own,"title":"../AI/AGENTS","markdown":"Editor rewrite attempt"}),
        )
        .await;
    note.ok();
    let thread = f
        .op(
            2,
            "threads.create",
            json!({"space_id":own,"title":"../AGENTS"}),
        )
        .await;
    thread.ok();
    assert_eq!(
        f.op(
            2,
            "notes.create",
            json!({"space_id":foreign,"title":"AGENTS","markdown":"foreign rewrite"})
        )
        .await
        .status,
        404
    );
    assert_eq!(
        f.op(
            2,
            "threads.create",
            json!({"space_id":foreign,"title":"AGENTS"})
        )
        .await
        .status,
        404
    );
    for (path, expected) in paths {
        assert_eq!(std::fs::read_to_string(path).unwrap(), expected);
    }
    evidence(
        "standing-instruction-isolation",
        json!({"editor_kind_writes_preserve_reserved_file":true,"foreign_space_hidden":true}),
    );
}

#[tokio::test]
async fn omitted_send_mode_uses_the_senders_saved_mode_not_another_members_choice() {
    let f = Fixture::new().await;
    let space = &f.spaces[3];
    let created = f
        .op(
            1,
            "threads.create",
            json!({"space_id":space,"title":"Per-person modes","mode":"least_access"}),
        )
        .await;
    created.ok();
    let thread = &created.body["data"]["thread"]["id"];
    for (who, mode, expected) in [
        (1, None, "least_access"),
        (2, Some("full_access"), "full_access"),
        (1, None, "least_access"),
    ] {
        let mut input = json!({"id":thread,"message":"mode isolation fixture"});
        if let Some(mode) = mode {
            input["mode"] = json!(mode);
        }
        let sent = f.op(who, "threads.send", input).await;
        sent.ok();
        let job = &sent.body["data"]["job"]["id"];
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let event = f.op(who, "agent.events", json!({"job_id":job})).await;
            event.ok();
            if event.body["data"]["job"]["checkpoint"]["lease_id"].is_string() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "mode admission did not reach its queued checkpoint"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let saved = f.op(who, "threads.get", json!({"id":thread})).await;
        saved.ok();
        assert_eq!(saved.body["data"]["thread"]["metadata"]["mode"], expected);
        assert_eq!(
            saved.body["data"]["thread"]["metadata"]["mode_for"],
            f.ids[who]
        );
        f.op(who, "threads.stop", json!({"id":thread})).await.ok();
    }
    f.op(1, "threads.delete", json!({"id":thread})).await.ok();
}
