use super::*;

#[tokio::test]
async fn empty_trash_requires_confirmation_and_manager_authority() {
    let mut f = Fixture::new().await;
    let space = f.spaces[3].clone();
    let old = f.note(1, &space).await;
    f.stop();
    let content = f.appdata().join("spaces").join(&space).join("content.db");
    let db = rusqlite::Connection::open(content).unwrap();
    db.execute_batch(
        "DROP TABLE notes_trash_targets; DROP TABLE notes_trash_jobs;
         DROP TABLE notes_rename_steps; DROP TABLE notes_rename_jobs;
         PRAGMA user_version=5;",
    )
    .unwrap();
    drop(db);
    let auth = rusqlite::Connection::open(f.appdata().join("system/auth.db")).unwrap();
    auth.execute_batch(
        "DELETE FROM space_role_capabilities WHERE capability='content.purge';
         PRAGMA user_version=14;",
    )
    .unwrap();
    drop(auth);
    f.start().await;
    let managed = data(
        f.op(2, "trash.empty.preview", json!({"space_id":f.spaces[4]}))
            .await,
    );
    assert_eq!(managed["count"], 0);
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
