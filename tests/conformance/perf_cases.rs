//! Acknowledged saves survive immediate process death before deferred placement.
use super::*;

#[tokio::test]
async fn acknowledged_save_survives_immediate_restart_with_isolation() {
    let mut f = Fixture::new().await;
    let created = f
        .op(
            1,
            "notes.create",
            json!({"space_id":f.spaces[0],"title":"Durable","markdown":"initial"}),
        )
        .await;
    created.ok();
    let mut note = created.body["data"]["note"].clone();
    for i in 0..4 {
        let request = format!("acknowledged-{i}");
        let input = json!({"id":note["id"],"base_revision":note["revision"],"markdown":format!("durable {i}")});
        let saved = f.op_id(1, "notes.update", input.clone(), &request).await;
        saved.ok();
        f.stop();
        f.start().await;
        let read = f.op(1, "notes.get", json!({"id":note["id"]})).await;
        read.ok();
        assert_eq!(read.body["data"]["note"], saved.body["data"]["note"]);
        let replay = f.op_id(1, "notes.update", input, &request).await;
        replay.ok();
        assert_eq!(replay.body, saved.body);
        assert_eq!(
            f.op(2, "notes.get", json!({"id":note["id"]})).await.status,
            404
        );
        note = saved.body["data"]["note"].clone();
    }
}

#[tokio::test]
async fn purge_blocks_old_body_receipt_replay_with_foreign_isolation() {
    let f = Fixture::new().await;
    let input =
        json!({"space_id":f.spaces[0],"title":"Receipt purge","markdown":"private erased body"});
    let saved = f
        .op_id(1, "notes.create", input.clone(), "create-before-purge")
        .await;
    saved.ok();
    let id = saved.body["data"]["note"]["id"].clone();
    f.op(1, "notes.delete", json!({"id":id})).await.ok();
    f.op_id(1, "trash.purge", json!({"id":id}), "permanent-purge")
        .await
        .ok();
    f.op_id(1, "trash.purge", json!({"id":id}), "permanent-purge")
        .await
        .ok();
    let replay = f
        .op_id(1, "notes.create", input.clone(), "create-before-purge")
        .await;
    assert_eq!(
        replay.status, 404,
        "purged content must not return through an older receipt"
    );
    assert_eq!(
        f.op_id(2, "notes.create", input, "create-before-purge")
            .await
            .status,
        404
    );
}
