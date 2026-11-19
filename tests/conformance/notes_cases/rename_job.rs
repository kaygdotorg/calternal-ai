use super::*;

#[tokio::test]
async fn linked_rename_batches_are_undoable_by_job_id() {
    let f = Fixture::new().await;
    let space = &f.spaces[3];
    f.op(
        1,
        "plugins.enable",
        json!({"space_id":space,"plugin_id":"calternal-notes"}),
    )
    .await
    .ok();
    let create = async |title: &str, markdown: &str| {
        data(
            f.op(
                2,
                "notes.create",
                json!({"space_id":space,"title":title,"markdown":markdown}),
            )
            .await,
        )["note"]["id"]
            .clone()
    };
    let root = create("Target", "# Target\n\nOriginal").await;
    let mut sources = Vec::new();
    for i in 0..12 {
        sources.push(create(&format!("Source {i}"), "[authored](<Target.md#^anchor>)").await);
    }
    let stale = restored_revision(&f, &root).await;
    f.op(
        2,
        "notes.update",
        json!({"space_id":space,"id":root,"base_revision":stale,"markdown":"# Target\n\nUpdated original"}),
    )
    .await
    .ok();
    let rejected = f
        .op(
            2,
            "notes.rename",
            json!({"id":root,"base_revision":stale,"title":"Retitled"}),
        )
        .await;
    assert_eq!(rejected.status, 409);
    let bytes = committed_text_bytes(&f, space);
    let rename =
        json!({"id":root,"base_revision":restored_revision(&f, &root).await,"title":"Retitled"});
    let job = data(f.op(2, "notes.rename", rename).await);
    let status = json!({"space_id":space,"job_id":job["job_id"]});
    wait(&f, &status, "done").await;
    let title_growth = ("Retitled".len() - "Target".len()) as u64;
    assert_eq!(
        committed_text_bytes(&f, space),
        bytes + title_growth * (1 + sources.len() as u64)
    );
    for id in &sources {
        let note = data(f.op(2, "notes.get", json!({"id":id})).await);
        assert_eq!(
            note["note"]["markdown"],
            "[authored](<Retitled.md#^anchor>)"
        );
    }
    data(f.op(2, "notes.rename.undo", status.clone()).await);
    wait(&f, &status, "undone").await;
    assert_eq!(committed_text_bytes(&f, space), bytes);
    for id in sources {
        let note = data(f.op(2, "notes.get", json!({"id":id})).await);
        assert_eq!(note["note"]["markdown"], "[authored](<Target.md#^anchor>)");
    }
    let note = data(f.op(2, "notes.get", json!({"id":root})).await);
    assert_eq!(note["note"]["markdown"], "# Target\n\nUpdated original");
    let content = f.appdata().join("spaces").join(space).join("content.db");
    let db =
        rusqlite::Connection::open_with_flags(content, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let current_checkpoints: i64 = db
        .query_row(
            "SELECT count(*) FROM room_checkpoints JOIN items ON item=id AND room_checkpoints.seq=items.seq
             WHERE kind='notes'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(current_checkpoints, 13);
}

async fn wait(f: &Fixture, input: &Value, expected: &str) {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let status = data(f.op(2, "notes.rename.status", input.clone()).await);
            assert_eq!(status["total"], 12);
            if status["state"] == expected {
                assert_eq!(status["remaining"], 0);
                assert_eq!(status["skipped"], 0);
                return;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn late_rename_destination_collision_retires_the_unpublished_plan() {
    let mut f = Fixture::new().await;
    let space = f.spaces[2].clone();
    let root = f.note(1, &space).await;
    let source = data(
        f.op(
            1,
            "notes.create",
            json!({
                "space_id":space,"title":"Link","markdown":"[to the note](<Secret.md>)"
            }),
        )
        .await,
    )["note"]["id"]
        .clone();
    let before = committed_text_bytes(&f, &space);
    let rename = json!({"id":root["id"],"base_revision":root["revision"],"title":"Competing"});
    let fault = f.dir.path().join("content-fault");
    f.stop();
    std::fs::write(&fault, "after_commit").unwrap();
    f.start().await;
    let lost = f
        .client
        .post(format!("{}/api/v1/operations", f.base))
        .header("host", "localhost:18080")
        .header("origin", ORIGIN)
        .header("cookie", &f.cookies[1])
        .json(&json!({"operation":"notes.rename","request_id":"loses-destination","input":rename}))
        .send()
        .await;
    assert!(lost.is_err());
    f.stop();
    std::fs::remove_file(fault).unwrap();
    let folder = f
        .dir
        .path()
        .join("userdata-ssd/spaces")
        .join(&space)
        .join("Notes");
    let competitor = folder.join("Competing.md");
    std::fs::write(&competitor, b"outside note").unwrap();
    f.start().await;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let available = f.op(1, "notes.get", json!({"id":root["id"]})).await;
            if available.status == 200 {
                assert_eq!(available.body["data"]["note"]["revision"], root["revision"]);
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let unchanged = data(f.op(1, "notes.get", json!({"id":source})).await);
    assert_eq!(unchanged["note"]["markdown"], "[to the note](<Secret.md>)");
    assert_eq!(committed_text_bytes(&f, &space), before);
    let next = data(f.op(1, "notes.rename", rename).await);
    assert_eq!(next["note"]["title"], "Competing");
    assert!(folder.join("Competing 2.md").is_file());
    assert_eq!(std::fs::read(competitor).unwrap(), b"outside note");
}
