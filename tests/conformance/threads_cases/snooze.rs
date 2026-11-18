//! Shared, receipted snooze deadlines on the live thread API.
use super::*;

#[tokio::test]
async fn snooze_is_shared_receipted_and_expires_without_a_write() {
    let f = Fixture::new().await;
    let space = &f.spaces[3];
    let created = f
        .op(
            1,
            "threads.create",
            json!({"space_id":space,"title":"Snoozed"}),
        )
        .await;
    created.ok();
    let id = &created.body["data"]["thread"]["id"];
    let mut journal =
        crate::live_cases::Live::open(&f, 2, json!([{"space_id":space,"bookmark":null}]))
            .await
            .unwrap();
    assert_eq!(journal.next().await.unwrap()["type"], "ready");

    let until = now_ms() + 2_500;
    let input = json!({"space_id":space,"id":id,"until_ms":until});
    let set = f
        .op_id(1, "threads.snooze", input.clone(), "test-snooze")
        .await;
    set.ok();
    let entry = journal.next().await.unwrap();
    assert_eq!(entry["entry"]["kind"], "threads.snooze");
    assert_eq!(entry["entry"]["data"]["snoozed_until_ms"], until);
    let list = f.op(2, "threads.list", json!({"space_id":space})).await;
    list.ok();
    assert_eq!(list.body["data"]["threads"][0]["snoozed_until_ms"], until);
    let got = f.op(2, "threads.get", json!({"id":id})).await;
    got.ok();
    assert_eq!(got.body["data"]["thread"]["snoozed_until_ms"], until);

    let remaining = until.saturating_sub(now_ms());
    tokio::time::sleep(Duration::from_millis(remaining as u64 + 25)).await;
    let replay = f.op_id(1, "threads.snooze", input, "test-snooze").await;
    replay.ok();
    assert_eq!(replay.body["data"], set.body["data"]);
    let list = f.op(2, "threads.list", json!({"space_id":space})).await;
    list.ok();
    assert!(list.body["data"]["threads"][0]["snoozed_until_ms"].is_null());
    assert_eq!(
        list.body["data"]["threads"][0]["revision"],
        got.body["data"]["thread"]["revision"]
    );

    let clear = f
        .op(2, "threads.snooze", json!({"id":id,"until_ms":null}))
        .await;
    clear.ok();
    assert!(clear.body["data"]["snoozed_until_ms"].is_null());
    let got = f.op(1, "threads.get", json!({"id":id})).await;
    got.ok();
    assert!(
        got.body["data"]["thread"]["metadata"]
            .get("snoozed_until_ms")
            .is_none()
    );
}

#[tokio::test]
async fn subscription_retry_snoozes_until_the_reported_reset() {
    use base64::{Engine, engine::general_purpose::STANDARD};

    let f = Fixture::new().await;
    let space = &f.spaces[0];
    let mut machine = crate::requests_cases::Machine::enroll(&f).await;
    f.op(
        1,
        "agent.settings",
        json!({"retry_settings":{"provider_errors":false,"subscription_limits":true}}),
    )
    .await
    .ok();
    let created = f
        .op(
            1,
            "threads.create",
            json!({"space_id":space,"title":"Subscription wait"}),
        )
        .await;
    created.ok();
    let id = &created.body["data"]["thread"]["id"];
    f.op(
        1,
        "threads.send",
        json!({"id":id,"message":"Run the fixture"}),
    )
    .await
    .ok();
    let exec = super::title::next_exec(&f, &mut machine).await;
    let reset = now_ms() + 60_000;
    let frames = [
        json!({"version":1,"seq":1,"event":{"type":"error","class":"usage_limit",
            "billing":"subscription","code":"usage_limit","reset_at_ms":reset,"message":"Fixture reset"}}),
        json!({"version":1,"seq":2,"event":{"type":"done","outcome":"failed"}}),
    ];
    let stream = format!("{}\n{}\n", frames[0], frames[1]);
    super::title::reply(&f, &machine, &exec, &STANDARD.encode(stream)).await;

    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let list = f.op(1, "threads.list", json!({"space_id":space})).await;
        list.ok();
        let row = &list.body["data"]["threads"][0];
        if row["turn_reason"]["rule"] == "subscription_reset" {
            assert_eq!(row["turn_reason"]["next_attempt_ms"], reset);
            assert_eq!(row["snoozed_until_ms"], reset);
            assert!(row["metadata"].get("snoozed_until_ms").is_none());
            break;
        }
        assert!(Instant::now() < deadline, "{row}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    f.op(1, "threads.snooze", json!({"id":id,"until_ms":null}))
        .await
        .ok();
    let open = f.op(1, "threads.get", json!({"id":id})).await;
    open.ok();
    assert_eq!(open.body["data"]["thread"]["snoozed_until_ms"], reset);
    f.op(1, "threads.stop", json!({"id":id})).await.ok();
    let open = f.op(1, "threads.get", json!({"id":id})).await;
    open.ok();
    assert!(open.body["data"]["thread"]["snoozed_until_ms"].is_null());
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
