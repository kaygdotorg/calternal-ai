//! An ordinary worker pages its own kind through the live broker.

use super::*;
use std::collections::BTreeSet;

#[tokio::test]
async fn plugin_kind_page_is_bounded_and_its_cursor_is_scoped() {
    let f = Fixture::new().await;
    let space = &f.spaces[2];
    let other = &f.spaces[3];
    jobs_cases::enable(&f, 1, space).await;
    jobs_cases::enable(&f, 1, other).await;
    let mut expected = BTreeSet::new();
    for body in ["first body", "second body"] {
        let created = f
            .op(1, "jobs-demo.write", json!({"space_id":space,"text":body}))
            .await;
        created.ok();
        expected.insert(created.body["data"]["id"].as_str().unwrap().to_owned());
    }
    let first = f
        .op(1, "jobs-demo.list", page(space, "demo-notes", 1))
        .await;
    first.ok();
    let first = &first.body["data"];
    let cursor = first["next_cursor"].as_str().unwrap();
    assert!(cursor.starts_with('p'));
    let second = f.op(1, "jobs-demo.list", continuation(space, cursor)).await;
    second.ok();
    let second = &second.body["data"];
    assert!(second["next_cursor"].is_null());
    let mut actual = BTreeSet::new();
    for result in [first, second] {
        let rows = result["items"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].as_object().unwrap().len(), 4);
        actual.insert(rows[0]["id"].as_str().unwrap().to_owned());
    }
    assert_eq!(actual, expected);
    let whole = f
        .op(1, "jobs-demo.list", page(space, "demo-notes", 2))
        .await;
    whole.ok();
    assert_eq!(
        whole.body["data"]["items"],
        json!([first["items"][0], second["items"][0]])
    );
    let mut other_query = continuation(space, cursor);
    other_query["label"] = json!("other-filter");
    for (who, input) in [
        (1, page(space, "notes", 1)),
        (1, page(space, "demo-notes", 101)),
        (1, continuation(space, &format!("{cursor}x"))),
        (1, continuation(other, cursor)),
        (2, continuation(space, cursor)),
        (1, other_query),
        (3, page(space, "demo-notes", 1)),
    ] {
        let denied = f.op(who, "jobs-demo.list", input).await;
        assert_ne!(denied.status, 200, "{}", denied.body);
        assert!(denied.body.get("data").is_none());
    }
}

#[tokio::test]
async fn plugin_kind_page_refuses_a_worker_revoked_after_admission() {
    let mut f = Fixture::new().await;
    let space = f.spaces[2].clone();
    jobs_cases::enable(&f, 1, &space).await;
    f.stop();
    gate_runtime(&f);
    f.start().await;
    let request = f.op(2, "jobs-demo.list", page(&space, "demo-notes", 1));
    tokio::pin!(request);
    let marker = f.dir.path().join("worker-entered");
    tokio::select! {
        result = &mut request => panic!("worker finished before gate: {}", result.body),
        () = async {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !marker.exists() {
                assert!(Instant::now() < deadline, "worker did not reach gate");
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        } => {}
    }
    f.op(
        1,
        "spaces.remove_member",
        json!({"space_id":space,"account_id":f.ids[2]}),
    )
    .await
    .ok();
    std::fs::write(f.dir.path().join("worker-release"), "").unwrap();
    let denied = request.await;
    assert_ne!(denied.status, 200);
    assert!(denied.body.get("data").is_none());
}

fn page(space: &str, kind: &str, limit: u16) -> Value {
    json!({"space_id":space,"kind":kind,"limit":limit})
}

fn continuation(space: &str, cursor: &str) -> Value {
    json!({"space_id":space,"kind":"demo-notes","cursor":cursor,"limit":1})
}

#[tokio::test]
async fn invalid_plugin_output_after_write_acknowledges_the_durable_effect_once() {
    let mut f = Fixture::new().await;
    let space = f.spaces[0].clone();
    let foreign = f.spaces[1].clone();
    jobs_cases::enable(&f, 1, &space).await;
    jobs_cases::enable(&f, 2, &foreign).await;
    let op = "jobs-demo.write";
    let rid = "invalid-plugin-output-write";
    let input = json!({"space_id":space,"text":"bad-output-after-write"});
    let invalid = f
        .op_id(1, op, json!({"space_id":space,"text":5}), rid)
        .await;
    assert_eq!(invalid.status, 400);
    let first = f.op_id(1, op, input.clone(), rid).await;
    first.ok();
    let committed = &first.body["data"]["committed_write"];
    assert!(uuid::Uuid::parse_str(committed["id"].as_str().unwrap()).is_ok());
    assert!(committed["revision"].is_string());

    f.stop();
    f.start().await;
    let replay = f.op_id(1, op, input.clone(), rid).await;
    replay.ok();
    assert_eq!(replay.body["data"], first.body["data"]);
    let changed = f
        .op_id(1, op, json!({"space_id":space,"text":"different"}), rid)
        .await;
    assert_eq!(changed.status, 409);
    let denied = f.op_id(2, op, input, rid).await;
    assert_eq!(denied.status, 404);
    let rows = f
        .op(1, "jobs-demo.list", page(&space, "demo-notes", 10))
        .await;
    rows.ok();
    assert_eq!(rows.body["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(rows.body["data"]["items"][0]["id"], committed["id"]);
    let empty = f
        .op(2, "jobs-demo.list", page(&foreign, "demo-notes", 10))
        .await;
    empty.ok();
    assert_eq!(empty.body["data"]["items"], json!([]));
}

#[tokio::test]
async fn retry_recovers_a_plugin_write_committed_before_server_exit() {
    let mut f = Fixture::new().await;
    let space = f.spaces[0].clone();
    jobs_cases::enable(&f, 1, &space).await;
    let op = "jobs-demo.write";
    let rid = "interrupted-plugin-output-write";
    let input = json!({"space_id":space,"text":"pause-after-write"});
    let item = {
        let writing = f.op_id(1, op, input.clone(), rid);
        tokio::pin!(writing);
        tokio::select! {
            result = &mut writing => panic!("worker answered before the crash: {}", result.body),
            item = async {
                let deadline = Instant::now() + Duration::from_secs(2);
                loop {
                    let rows = f.op(1, "jobs-demo.list", page(&space, "demo-notes", 10)).await;
                    rows.ok();
                    let items = rows.body["data"]["items"].as_array().unwrap();
                    if let Some(item) = items.first() {
                        assert_eq!(items.len(), 1);
                        break item.clone();
                    }
                    assert!(Instant::now() < deadline, "broker write never committed");
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            } => item,
        }
    };
    f.stop();
    // The retry must use the journal, even without a machine to run this worker.
    f.remote_workers = true;
    f.start().await;
    let replay = f.op_id(1, op, input, rid).await;
    replay.ok();
    let committed = &replay.body["data"]["committed_write"];
    assert_eq!(committed["id"], item["id"]);
    assert_eq!(committed["revision"], item["revision"]);
}
