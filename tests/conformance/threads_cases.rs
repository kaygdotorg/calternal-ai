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
        "threads.rename",
        "threads.unarchive",
        "threads.delete",
    ] {
        for who in [0, 2, 3] {
            let (foreign, absent) = match op {
                "threads.create" => (
                    json!({"space_id":f.spaces[0],"title":"probe"}),
                    json!({"space_id":missing,"title":"probe"}),
                ),
                "threads.list" => (json!({"space_id":f.spaces[0]}), json!({"space_id":missing})),
                "threads.rename" => (
                    json!({"id":id,"title":"foreign"}),
                    json!({"id":missing,"title":"missing"}),
                ),
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
    let document = saved.body["data"]["thread"]["markdown"].as_str().unwrap();
    let stamp = saved.body["data"]["thread"]["metadata"]["last_activity_at"]
        .as_str()
        .unwrap();
    assert!(document.contains(&format!(" time:{stamp} -->")));
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
    let reopened = f.op(1, "threads.get", json!({"id":id})).await;
    reopened.ok();
    assert_eq!(
        reopened.body["data"]["job"]["id"],
        first.body["data"]["job"]["id"]
    );
    let list = f
        .op(1, "threads.list", json!({"space_id":f.spaces[0]}))
        .await;
    list.ok();
    let row = &list.body["data"]["threads"][0];
    assert_eq!(row["id"], id);
    assert_eq!(row["metadata"]["message_count"], 1);
    assert!(row["metadata"]["model"].is_string());
    assert!(matches!(
        row["turn_state"].as_str(),
        Some("queued" | "running")
    ));
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
        for operation in [
            "threads.create",
            "threads.archive",
            "threads.rename",
            "threads.unarchive",
            "threads.delete",
        ] {
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
            } else if operation == "threads.rename" {
                json!({"id":target.body["data"]["thread"]["id"],"title":"Member renamed"})
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
    let mut previous_stop: Option<(usize, String)> = None;
    for (index, (who, mode, expected)) in [
        (1, None, "least_access"),
        (2, Some("full_access"), "full_access"),
        (1, None, "least_access"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut input = json!({"id":thread,"message":"mode isolation fixture"});
        if let Some(mode) = mode {
            input["mode"] = json!(mode);
        }
        let sent = f.op(who, "threads.send", input).await;
        sent.ok();
        let job = &sent.body["data"]["job"]["id"];
        let deadline = Instant::now() + Duration::from_secs(10);
        let lease = loop {
            let event = f.op(who, "agent.events", json!({"job_id":job})).await;
            event.ok();
            if event.body["data"]["job"]["checkpoint"]["lease_id"].is_string() {
                break event.body["data"]["job"]["checkpoint"]["lease_id"]
                    .as_str()
                    .unwrap()
                    .to_owned();
            }
            assert!(
                Instant::now() < deadline,
                "mode admission did not reach its queued checkpoint for account {who}: {}",
                event.body
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        };
        if let Some((previous_who, request_id)) = &previous_stop {
            f.op_id(
                *previous_who,
                "threads.stop",
                json!({"id":thread}),
                request_id,
            )
            .await
            .ok();
            let current = f
                .req(
                    "GET",
                    &format!("/api/v1/machines/runs/{lease}"),
                    Some(&f.cookies[who]),
                    json!({}),
                    Some(ORIGIN),
                )
                .await;
            current.ok();
            assert_ne!(
                current.body["state"], "ended",
                "replayed stop ended a later lease"
            );
        }
        let saved = f.op(who, "threads.get", json!({"id":thread})).await;
        saved.ok();
        assert_eq!(saved.body["data"]["thread"]["metadata"]["mode"], expected);
        assert_eq!(
            saved.body["data"]["thread"]["metadata"]["mode_for"],
            f.ids[who]
        );
        let request_id = format!("mode-stop-{index}");
        f.op_id(who, "threads.stop", json!({"id":thread}), &request_id)
            .await
            .ok();
        let ended = f
            .req(
                "GET",
                &format!("/api/v1/machines/runs/{lease}"),
                Some(&f.cookies[who]),
                json!({}),
                Some(ORIGIN),
            )
            .await;
        ended.ok();
        assert_eq!(
            ended.body["state"], "ended",
            "stop returned before its lease ended"
        );
        previous_stop = Some((who, request_id));
    }
    f.op(1, "threads.delete", json!({"id":thread})).await.ok();
}

#[tokio::test]
async fn saved_thread_events_replay_and_stay_in_the_owning_space() {
    use super::{jobs_cases::Sse, live_cases::Live};

    let f = Fixture::new().await;
    let space = &f.spaces[0];
    let mut owner = Live::open(&f, 1, json!([{"space_id":space}]))
        .await
        .ok()
        .unwrap();
    let bookmark = owner.next().await.unwrap()["bookmark"].clone();
    drop(owner);
    let mut other = Live::open(&f, 2, json!([{"space_id":f.spaces[1]}]))
        .await
        .ok()
        .unwrap();
    assert_eq!(other.next().await.unwrap()["type"], "ready");

    let created = f
        .op(
            1,
            "threads.create",
            json!({"space_id":space,"title":"Original"}),
        )
        .await;
    created.ok();
    let thread = &created.body["data"]["thread"]["id"];
    let renamed = f
        .op(1, "threads.rename", json!({"id":thread,"title":"Renamed"}))
        .await;
    renamed.ok();
    assert_eq!(renamed.body["data"]["thread"]["id"], *thread);
    assert_eq!(
        renamed.body["data"]["thread"]["metadata"]["title"],
        "Renamed"
    );
    let chats = f
        .dir
        .path()
        .join("userdata-ssd/spaces")
        .join(space)
        .join("AI/Chats");
    assert!(!chats.join("Original.md").exists());
    assert!(
        std::fs::read_to_string(chats.join("Renamed.md"))
            .unwrap()
            .contains("title: \"Renamed\"")
    );
    f.op(1, "threads.archive", json!({"id":thread})).await.ok();
    let restored = f.op(1, "threads.unarchive", json!({"id":thread})).await;
    restored.ok();
    assert_eq!(
        restored.body["data"]["thread"]["metadata"]["archived"],
        false
    );
    let sent = f
        .op(
            1,
            "threads.send",
            json!({"id":thread,"message":"Recorded","model":"fixture-model"}),
        )
        .await;
    sent.ok();
    let job = &sent.body["data"]["job"]["id"];
    // The requests lane uses this same jobs transition to park a turn. Inject
    // only that state in this disposable fixture, without provisioning a request.
    let database =
        Connection::open(f.appdata().join("spaces").join(space).join("jobs.db")).unwrap();
    database.busy_timeout(Duration::from_secs(5)).unwrap();
    let changed = database
        .execute(
            "UPDATE jobs SET state='waiting' WHERE id=?1 AND state='running'",
            [job.as_str().unwrap()],
        )
        .unwrap();
    if changed == 0 {
        // Admission and claim may race the fixture. Taking the state from admitted
        // also fences a later claim; either path leaves the same durable wait.
        assert_eq!(database.execute("UPDATE jobs SET state='waiting' WHERE id=?1 AND state IN ('admitted','running')", [job.as_str().unwrap()]).unwrap(), 1);
    }
    let reopened = f.op(1, "threads.get", json!({"id":thread})).await;
    reopened.ok();
    assert_eq!(reopened.body["data"]["job"]["id"], *job);
    assert_eq!(reopened.body["data"]["job"]["state"], "waiting");
    let job_id = reopened.body["data"]["job"]["id"].as_str().unwrap();
    let mut job_stream = Sse::open(&f, &f.cookies[1], job_id, None).await.unwrap();
    let (event, _, snapshot) = job_stream.next().await.unwrap();
    assert_eq!(event, "snapshot");
    assert_eq!(snapshot["job"]["id"], *job);
    let after = format!("{}:0", snapshot["epoch"].as_str().unwrap());
    drop(job_stream);
    let mut resumed = Sse::open(&f, &f.cookies[1], job_id, Some(&after))
        .await
        .unwrap();
    let (_, _, snapshot) = resumed.next().await.unwrap();
    assert_eq!(snapshot["job"]["id"], *job);
    let hidden_job = Sse::open(&f, &f.cookies[2], job_id, Some(&after))
        .await
        .err()
        .unwrap();
    let missing_job = Sse::open(&f, &f.cookies[2], &uuid::Uuid::now_v7().to_string(), None)
        .await
        .err()
        .unwrap();
    assert_eq!(hidden_job, missing_job);
    let list = f.op(1, "threads.list", json!({"space_id":space})).await;
    list.ok();
    let row = &list.body["data"]["threads"][0];
    assert_eq!(row["turn_state"], "waiting");
    assert_eq!(row["metadata"]["model"], "fixture-model");
    f.op(1, "threads.stop", json!({"id":thread})).await.ok();

    let mut owner = Live::open(&f, 1, json!([{"space_id":space,"bookmark":bookmark}]))
        .await
        .ok()
        .unwrap();
    let (changes, states) = tokio::time::timeout(Duration::from_secs(10), async {
        let mut changes = Vec::new();
        let mut states = Vec::new();
        while states.last().map(String::as_str) != Some("cancelled") {
            let frame = owner.next().await.unwrap();
            if frame["type"] != "entry" || frame["entry"]["item"] != *thread {
                continue;
            }
            assert_eq!(frame["space_id"], *space);
            let entry = &frame["entry"];
            let kind = entry["kind"].as_str().unwrap();
            if kind == "threads.turn_state_changed" {
                assert_eq!(entry["data"]["job_id"], *job);
                states.push(entry["data"]["job_state"].as_str().unwrap().to_owned());
            } else {
                changes.push(kind.to_owned());
            }
        }
        (changes, states)
    })
    .await
    .expect("turn changes must arrive on the subscribed journal");
    assert_eq!(
        changes,
        [
            "threads.create",
            "threads.rename",
            "threads.archive",
            "threads.unarchive",
            "threads.send"
        ]
    );
    assert_eq!(states.first().map(String::as_str), Some("admitted"));
    assert!(states.iter().any(|state| state == "waiting"));

    // A visible barrier after the foreign mutations detects accidental delivery
    // without relying on a fixed sleep or an idle event timeout.
    f.op(
        2,
        "threads.create",
        json!({"space_id":f.spaces[1],"title":"Visible barrier"}),
    )
    .await
    .ok();
    let frame = other.next().await.unwrap();
    assert_eq!(frame["space_id"], f.spaces[1]);
    assert_eq!(frame["entry"]["kind"], "threads.create");
    assert_ne!(frame["entry"]["item"], *thread);
    let foreign = Live::open(&f, 2, json!([{"space_id":space,"bookmark":bookmark}])).await;
    let absent = Live::open(
        &f,
        2,
        json!([{"space_id":uuid::Uuid::now_v7(),"bookmark":bookmark}]),
    )
    .await;
    assert_eq!(foreign.err().unwrap(), absent.err().unwrap());
}

#[tokio::test]
async fn fresh_personal_and_owned_spaces_list_empty_from_an_unrelated_working_directory() {
    let mut f = Fixture::new().await;
    let signup = f.op(0, "spaces.list", json!({})).await;
    signup.ok();
    let personal = signup.body["data"]["spaces"][0]["id"].clone();
    f.stop();
    f.start_with_plugin_root(false).await;

    // A duplicate display name must never mark a second space as personal.
    let created = f.op(0, "spaces.create", json!({"name":"Personal"})).await;
    created.ok();
    let new_space = &created.body["data"]["space"];
    assert_eq!(new_space["personal"], false);
    let spaces = f.op(0, "spaces.list", json!({})).await;
    spaces.ok();
    let spaces = spaces.body["data"]["spaces"].as_array().unwrap();
    let personal_rows: Vec<_> = spaces.iter().filter(|s| s["personal"] == true).collect();
    assert_eq!(personal_rows.len(), 1);
    assert_eq!(personal_rows[0]["id"], personal);

    for space in [&personal, &new_space["id"]] {
        let list = f.op(0, "threads.list", json!({"space_id":space})).await;
        list.ok();
        assert_eq!(list.body["data"]["threads"], json!([]));
        let foreign = f.op(1, "threads.list", json!({"space_id":space})).await;
        let absent = f
            .op(1, "threads.list", json!({"space_id":uuid::Uuid::now_v7()}))
            .await;
        assert_eq!(foreign.status, 404);
        assert_eq!(foreign.signature(), absent.signature());
    }
}
