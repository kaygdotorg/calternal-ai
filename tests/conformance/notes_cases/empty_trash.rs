use super::*;

#[tokio::test]
async fn empty_trash_requires_confirmation_and_manager_authority() {
    let mut f = Fixture::new().await;
    let space = f.spaces[3].clone();
    let old = f.note(1, &space).await;
    f.stop();
    let content = f.appdata().join("spaces").join(&space).join("content.db");
    restore_content_v6_fixture(&content);
    f.start().await;
    let auth = rusqlite::Connection::open_with_flags(
        f.appdata().join("system/auth.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    assert_eq!(
        auth.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        14,
    );
    drop(auth);
    let managed = data(
        f.op(2, "trash.empty.preview", json!({"space_id":f.spaces[4]}))
            .await,
    );
    assert_eq!(managed["count"], 0);
    let manager_space = &f.spaces[4];
    let manager_note = f.note(2, manager_space).await;
    let manager_target = json!({"id":manager_note["id"]});
    f.op(2, "notes.delete", manager_target.clone()).await.ok();
    f.op(2, "trash.purge", manager_target).await.ok();
    let id = &old["id"];
    let target = json!({"id":id});
    f.op(2, "notes.delete", target.clone()).await.ok();
    f.op(2, "notes.restore", target.clone()).await.ok();
    f.op(2, "notes.delete", target.clone()).await.ok();
    let scope = json!({"space_id":space});
    let trash = data(f.op(2, "trash.list", scope.clone()).await);
    assert_eq!(trash["notes"][0]["days_remaining"], 30);
    assert_eq!(f.op(2, "trash.purge", target).await.status, 403);
    assert_eq!(
        f.op(2, "trash.empty.preview", scope.clone()).await.status,
        403
    );

    let preview = data(f.op(1, "trash.empty.preview", scope.clone()).await);
    assert_eq!(preview["count"], 1);
    assert!(preview["bytes"].as_u64().unwrap() > 0);
    let other = f.note(1, &space).await;
    f.op(1, "notes.delete", json!({"id":other["id"]}))
        .await
        .ok();
    let confirm = |p: &Value| json!({"space_id":space,"confirmation":p["confirmation"]});
    assert_eq!(f.op(1, "trash.empty", confirm(&preview)).await.status, 409);
    let preview = data(f.op(1, "trash.empty.preview", scope.clone()).await);
    assert_eq!(preview["count"], 2);
    let start = confirm(&preview);
    assert_eq!(f.op(2, "trash.empty", start.clone()).await.status, 403);
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
        .json(&json!({"operation":"trash.empty","request_id":"empty-1","input":start}))
        .send()
        .await;
    assert!(lost.is_err());
    f.stop();
    std::fs::remove_file(fault).unwrap();
    f.start().await;
    let admitted = data(f.op_id(1, "trash.empty", start.clone(), "empty-1").await);
    assert_eq!(admitted["count"], 2);
    assert_eq!(
        admitted,
        data(f.op_id(1, "trash.empty", start, "empty-1").await)
    );
    let job = json!({"space_id":space,"job_id":admitted["job_id"]});
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let status = data(f.op(1, "trash.empty.status", job.clone()).await);
            if status["state"] == "done" {
                assert_eq!(status["purged"], 2);
                assert_eq!(status["skipped"], 0);
                break;
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(data(f.op(1, "trash.list", scope).await)["notes"], json!([]));
    assert_eq!(address(&f, 2, id).await["state"], "deleted");
}

fn restore_content_v6_fixture(path: &std::path::Path) {
    let mut db = rusqlite::Connection::open(path).unwrap();
    let tx = db.transaction().unwrap();
    // Undo migrations 9, 8 and 7 without changing the saved note. Reinstall
    // the v5 collection index and its triggers because v9 replaces one of them.
    tx.execute_batch(
        "DROP TRIGGER window_collection_order_forget;
         DROP TRIGGER window_kind_register;
         DROP VIEW window_note_candidates;
         DROP VIEW window_trash_notes;
         DROP VIEW window_generic_candidates;
         DROP VIEW window_generic_heads;
         DROP VIEW window_live_notes;
         DROP TABLE window_collection_newest;
         DROP TABLE window_kind_orders;
         DROP TABLE window_configuration;
         DROP TRIGGER collection_members_insert;
         DROP TRIGGER collection_members_update;
         DROP TRIGGER collection_members_delete;
         DROP TABLE note_collection_members;
         DROP TABLE notes_trash_targets;
         DROP TABLE notes_trash_jobs;
         DROP TABLE notes_rename_steps;
         DROP TABLE notes_rename_jobs;
         DROP TABLE IF EXISTS _calternal_schema_compatibility;",
    )
    .unwrap();
    tx.execute_batch(include_str!(
        "../../../crates/content/src/collection_members.sql"
    ))
    .unwrap();
    tx.pragma_update(None, "user_version", 6).unwrap();
    tx.commit().unwrap();
}
