//! Independent receipt-domain regressions through real passkey sessions.
use super::*;

#[tokio::test]
async fn directory_request_id_can_be_reused_in_another_space() {
    let f = Fixture::new().await;
    for space in [&f.spaces[2], &f.spaces[3]] {
        f.op_id(
            1,
            "spaces.set_role",
            json!({"space_id":space,"account_id":f.ids[2],"role":"editor"}),
            "directory-shared-id",
        )
        .await
        .ok();
    }
}

#[tokio::test]
async fn request_id_cannot_execute_content_and_directory_writes_in_one_space() {
    let f = Fixture::new().await;
    let space = &f.spaces[2];
    f.op_id(
        1,
        "notes.create",
        json!({"space_id":space,"title":"one request","markdown":"one effect"}),
        "one-write",
    )
    .await
    .ok();
    let second = f
        .op_id(
            1,
            "spaces.set_role",
            json!({"space_id":space,"account_id":f.ids[2],"role":"editor"}),
            "one-write",
        )
        .await;
    assert_eq!(
        second.status, 409,
        "one request ID executed two mutations: {}",
        second.body
    );
}

#[tokio::test]
async fn pending_request_ids_are_independent_between_spaces() {
    let f = Fixture::new().await;
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    for space in [&f.spaces[0], &f.spaces[2]] {
        let queued = json!({
            "space_id":space,"item_id":uuid::Uuid::now_v7().to_string(),"created_at_ms":created,
            "request":{"request_id":"pending-shared-id","operation":"notes.create",
                "input":{"space_id":space,"title":"queued","markdown":"private input"}}
        });
        f.op(1, "pending.queue", queued).await.ok();
    }
}

#[tokio::test]
async fn resaved_draft_cannot_survive_revocation_under_its_old_space_index() {
    let f = Fixture::new().await;
    let item = uuid::Uuid::now_v7().to_string();
    let first_space = &f.spaces[0];
    let second_space = &f.spaces[2];
    f.op(
        1,
        "drafts.save",
        json!({"space_id":first_space,"item_id":item,"text":"first"}),
    )
    .await
    .ok();
    let moved = f
        .op(
            1,
            "drafts.save",
            json!({"space_id":second_space,"item_id":item,"text":"must be erased"}),
        )
        .await;
    if moved.status >= 400 {
        // Refusing a change of a draft's owning space is also safe.
        return;
    }
    moved.ok();
    f.op(1, "spaces.delete", json!({"space_id":second_space}))
        .await
        .ok();
    let listed = f.op(1, "drafts.list", json!({})).await;
    listed.ok();
    assert!(
        !listed.body["data"]["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["item_id"] == item),
        "revoked-space draft was delivered through its stale old-space SQL index: {}",
        listed.body
    );
}

#[tokio::test]
async fn manager_can_cancel_a_job_started_by_another_account() {
    let f = Fixture::new().await;
    // Account 1 owns this space; account 2 is its Manager.
    let space = &f.spaces[4];
    let id =
        jobs_cases::start_job(&f, 1, space, json!({"steps":1000,"delay_ms":20,"every":10})).await;
    let response = f.client.post(format!("{}/api/v1/operations", f.base))
        .header("host", "localhost:18080").header("origin", ORIGIN)
        .header("cookie", &f.cookies[2])
        .json(&json!({"request_id":"manager-cancel","operation":"jobs.cancel","input":{"job_id":id}}))
        .send().await.expect("manager cancellation must return an HTTP response instead of panicking on the async worker");
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["data"]["job"]["state"], "cancelled");
}
