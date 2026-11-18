//! Notes scope, index, and lifecycle checks through passkey-authenticated HTTP.
use super::*;

pub(super) fn foreign_cases(
    space: &str,
    note: &Value,
    missing: &str,
) -> Vec<(&'static str, Value, Value)> {
    let mut cases = Vec::new();
    for op in ["notes.delete", "notes.restore", "trash.purge"] {
        cases.push((op, json!({"id":note["id"]}), json!({"id":missing})));
    }
    cases.push((
        "notes.rename",
        json!({"id":note["id"],"base_revision":note["revision"],"title":"renamed"}),
        json!({"id":missing,"base_revision":note["revision"],"title":"renamed"}),
    ));
    for op in ["trash.list", "tags.list"] {
        cases.push((op, json!({"space_id":space}), json!({"space_id":missing})));
    }
    for op in ["notes.lookup", "tags.lookup"] {
        cases.push((
            op,
            json!({"space_id":space,"prefix":""}),
            json!({"space_id":missing,"prefix":""}),
        ));
    }
    cases
}

fn data(r: Answer) -> Value {
    r.ok();
    r.body["data"].clone()
}

async fn address(f: &Fixture, who: usize, id: &Value) -> Value {
    data(f.op(who, "resolve", json!({"kind":"notes","id":id})).await)
}

async fn wait_for_placement(path: &Path) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !path.is_file() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("note placement should finish after save acknowledgment");
}

#[tokio::test]
async fn notes_index_and_trash_follow_the_current_space_and_head() {
    let f = Fixture::new().await;
    let space = &f.spaces[2];
    let other = &f.spaces[1];
    let note = data(f.op(1, "notes.create", json!({
        "space_id":space,"title":"Résumé Plan","markdown":"---\ntags: [work/q4, home]\n---\n# Résumé Plan\n#work/q4 #work/q3 `#hidden`"
    })).await)["note"].clone();
    let id = &note["id"];
    let item = json!({"id":id});
    let tags = |space: &str| json!({"space_id":space});
    let lookup = |space: &str, prefix: &str| json!({"space_id":space,"prefix":prefix});
    let paths = f.dir.path().join("userdata-ssd/spaces").join(space);
    let old = paths.join("Notes/Résumé Plan.md");
    let trashed = paths
        .join(".trash")
        .join(format!("{}.md", id.as_str().unwrap()));
    wait_for_placement(&old).await;
    let visible = data(f.op(2, "tags.list", tags(space)).await);
    assert_eq!(
        visible["tags"],
        json!([
            {"name":"home","count":1},{"name":"work","count":1},
            {"name":"work/q3","count":1},{"name":"work/q4","count":1}
        ])
    );
    assert_eq!(
        data(f.op(2, "tags.lookup", lookup(space, "work/")).await)["tags"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        data(f.op(2, "notes.lookup", lookup(space, "RÉSU")).await)["notes"][0]["id"],
        *id
    );
    assert_eq!(
        data(
            f.op(2, "notes.list", json!({"space_id":space,"tag":"work"}))
                .await
        )["notes"][0]["id"],
        *id
    );
    assert_eq!(
        data(f.op(2, "tags.list", tags(other)).await)["tags"],
        json!([])
    );
    assert_eq!(
        data(f.op(2, "notes.lookup", lookup(other, "")).await)["notes"],
        json!([])
    );

    assert_eq!(
        data(f.op(2, "notes.lookup", lookup(space, "RÉSU")).await)["notes"][0]["link_target"],
        "Résumé Plan.md"
    );
    let sibling = data(f.op(1, "notes.create", json!({
        "space_id":space,"title":"Links",
        "markdown":"# Links\n[Résumé Plan](<Résumé Plan.md#^block>) and [**custom**](<Résumé Plan.md>). `[code](<Résumé Plan.md>)`"
    })).await)["note"]["id"].clone();
    let cross = data(
        f.op(
            2,
            "notes.create",
            json!({
                "space_id":other,"title":"Foreign Links","markdown":"[link](<Résumé Plan.md>)"
            }),
        )
        .await,
    )["note"]["id"]
        .clone();
    let rename = json!({"id":id,"title":"New Plan","base_revision":note["revision"]});
    assert_eq!(f.op(2, "notes.rename", rename.clone()).await.status, 403);
    assert_eq!(f.op(2, "notes.delete", item.clone()).await.status, 403);
    let renamed = data(f.op(1, "notes.rename", rename.clone()).await);
    assert!(
        renamed["note"]["markdown"]
            .as_str()
            .unwrap()
            .contains("# New Plan\n")
    );
    let linked = data(f.op(2, "notes.get", json!({"id":sibling})).await);
    assert_eq!(
        linked["note"]["markdown"],
        "# Links\n[New Plan](<New Plan.md#^block>) and [**custom**](<New Plan.md>). `[code](<Résumé Plan.md>)`"
    );
    let foreign = data(f.op(2, "notes.get", json!({"id":cross})).await);
    assert_eq!(foreign["note"]["markdown"], "[link](<Résumé Plan.md>)");
    assert!(!old.exists());
    assert!(paths.join("Notes/New Plan.md").is_file());
    assert_eq!(address(&f, 2, id).await["state"], "active");
    assert_eq!(f.op(1, "notes.rename", rename).await.status, 409);

    let deleted = data(
        f.op_id(1, "notes.delete", item.clone(), "delete-retry")
            .await,
    );
    assert_eq!(
        deleted,
        data(
            f.op_id(1, "notes.delete", item.clone(), "delete-retry")
                .await
        )
    );
    assert!(trashed.is_file());
    assert_eq!(address(&f, 2, id).await["state"], "in_trash");
    assert_eq!(f.op(2, "notes.get", item.clone()).await.status, 404);
    assert_eq!(f.op(2, "notes.restore", item.clone()).await.status, 403);
    assert_eq!(f.op(2, "trash.purge", item.clone()).await.status, 403);
    assert_eq!(
        data(f.op(2, "trash.list", tags(space)).await)["notes"][0]["id"],
        *id
    );
    assert_eq!(
        data(f.op(2, "trash.list", tags(other)).await)["notes"],
        json!([])
    );
    assert_eq!(
        data(f.op(2, "tags.list", tags(space)).await)["tags"],
        json!([])
    );
    assert_eq!(
        data(f.op(2, "notes.lookup", lookup(space, "New")).await)["notes"],
        json!([])
    );

    data(
        f.op(
            1,
            "notes.create",
            json!({"space_id":space,"title":"New Plan","markdown":"# New Plan\nreplacement"}),
        )
        .await,
    );
    let restored = data(f.op(1, "notes.restore", item.clone()).await);
    assert_eq!(restored["note"]["id"], *id);
    assert!(!trashed.exists());
    let restored = paths.join("Notes/New Plan 2.md");
    assert!(restored.is_file());
    assert_eq!(address(&f, 2, id).await["state"], "active");
    assert_eq!(
        data(f.op(2, "tags.lookup", lookup(space, "home")).await)["tags"],
        json!([{"name":"home","count":1}])
    );
    data(f.op(1, "notes.update", json!({
        "id":id,"base_revision":restored_revision(&f, id).await,"markdown":"# New Plan\n#new-tag"
    })).await);
    assert_eq!(
        data(f.op(2, "tags.list", tags(space)).await)["tags"],
        json!([{"name":"new-tag","count":1}])
    );
    assert_eq!(f.op(1, "trash.purge", item.clone()).await.status, 409);

    data(f.op(1, "notes.delete", item.clone()).await);
    let mut live = live_cases::Live::open(&f, 2, json!([{"space_id":space,"bookmark":null}]))
        .await
        .ok()
        .unwrap();
    assert_eq!(live.next().await.unwrap()["type"], "ready");
    let purge = data(f.op_id(1, "trash.purge", item.clone(), "purge-retry").await);
    assert_eq!(
        purge,
        data(f.op_id(1, "trash.purge", item.clone(), "purge-retry").await)
    );
    let event = live.next().await.unwrap();
    assert_eq!(event["entry"]["kind"], "trash.purge");
    assert_eq!(event["entry"]["item"], *id);
    assert_eq!(purge, json!({"id":id,"state":"deleted"}));
    assert!(!restored.exists() && !trashed.exists());
    assert_eq!(address(&f, 2, id).await["state"], "deleted");
    assert_eq!(f.op(2, "notes.history", item.clone()).await.status, 404);
    assert_eq!(f.op(1, "notes.restore", item.clone()).await.status, 404);
    assert_eq!(
        data(f.op(2, "trash.list", tags(space)).await)["notes"],
        json!([])
    );
}

#[tokio::test]
async fn notes_moves_recover_after_the_file_move_before_placement_bookkeeping() {
    let mut f = Fixture::new().await;
    let space = f.spaces[0].clone();
    let note = f.note(1, &space).await;
    let id = note["id"].as_str().unwrap();
    let paths = f.dir.path().join("userdata-ssd/spaces").join(&space);
    let original = paths.join("Notes/Secret.md");
    let trashed = paths.join(".trash").join(format!("{id}.md"));
    let fault = f.dir.path().join("content-fault");
    // Arm the move fault only after the initial save has placed its file.
    wait_for_placement(&original).await;

    for (op, expected, moved, old, rid) in [
        (
            "notes.delete",
            "in_trash",
            &trashed,
            &original,
            "crash-delete",
        ),
        (
            "notes.restore",
            "active",
            &original,
            &trashed,
            "crash-restore",
        ),
        (
            "notes.delete",
            "in_trash",
            &trashed,
            &original,
            "crash-delete-again",
        ),
    ] {
        f.stop();
        std::fs::write(&fault, "after_placement").unwrap();
        f.start().await;
        let lost = f
            .client
            .post(format!("{}/api/v1/operations", f.base))
            .header("host", "localhost:18080")
            .header("origin", ORIGIN)
            .header("cookie", &f.cookies[1])
            .json(&json!({"operation":op,"request_id":rid,"input":{"id":id}}))
            .send()
            .await;
        assert!(lost.is_err());
        assert!(moved.is_file() && !old.exists());
        f.stop();
        std::fs::remove_file(&fault).unwrap();
        f.start().await;
        assert_eq!(address(&f, 1, &json!(id)).await["state"], expected);
        data(f.op_id(1, op, json!({"id":id}), rid).await);
        assert!(moved.is_file() && !old.exists());
    }
    f.stop();
    let db = Connection::open(f.appdata().join("spaces").join(&space).join("content.db")).unwrap();
    db.execute(
        "UPDATE items SET updated_at='2000-01-01T00:00:00.000Z' WHERE id=?1",
        [id],
    )
    .unwrap();
    drop(db);
    f.start().await;
    for _ in 0..100 {
        if address(&f, 1, &json!(id)).await["state"] == "deleted" && !trashed.exists() {
            let db = Connection::open(f.appdata().join("spaces").join(&space).join("content.db"))
                .unwrap();
            let count: i64 = db
                .query_row("SELECT count(*) FROM versions WHERE item=?1", [id], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(count, 0);
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("startup retention did not purge the expired note");
}

async fn restored_revision(f: &Fixture, id: &Value) -> Value {
    data(f.op(1, "notes.get", json!({"id":id})).await)["note"]["revision"].clone()
}

#[tokio::test]
async fn linked_rename_recovers_before_replaying_its_receipt() {
    let mut f = Fixture::new().await;
    let space = f.spaces[0].clone();
    let note = f.note(1, &space).await;
    let source = data(
        f.op(
            1,
            "notes.create",
            json!({
                "space_id":space,"title":"Source","markdown":"[Secret](<Secret.md#^anchor>)"
            }),
        )
        .await,
    )["note"]["id"]
        .clone();
    let request = json!({"id":note["id"],"title":"Renamed","base_revision":note["revision"]});
    let fault = f.dir.path().join("content-fault");
    f.stop();
    std::fs::write(&fault, "after_placement").unwrap();
    f.start().await;
    let lost = f
        .client
        .post(format!("{}/api/v1/operations", f.base))
        .header("host", "localhost:18080")
        .header("origin", ORIGIN)
        .header("cookie", &f.cookies[1])
        .json(&json!({"operation":"notes.rename","request_id":"linked-rename","input":request}))
        .send()
        .await;
    assert!(lost.is_err());
    f.stop();
    std::fs::remove_file(fault).unwrap();
    f.start().await;
    let replay = data(f.op_id(1, "notes.rename", request, "linked-rename").await);
    assert_eq!(replay["note"]["id"], note["id"]);
    let folder = f
        .dir
        .path()
        .join("userdata-ssd/spaces")
        .join(space)
        .join("Notes");
    assert!(!folder.join("Secret.md").exists());
    assert!(folder.join("Renamed.md").is_file());
    let indexed = data(f.op(1, "notes.get", json!({"id":source})).await);
    assert_eq!(
        indexed["note"]["markdown"],
        "[Renamed](<Renamed.md#^anchor>)"
    );
    let disk = std::fs::read_to_string(folder.join("Source.md")).unwrap();
    assert!(disk.ends_with("[Renamed](<Renamed.md#^anchor>)"));
}
