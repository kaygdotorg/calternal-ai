//! Browser subscription isolation and retention setup through production operation routing.
use super::*;

/// Account-scoped operations exercised by this module.
pub(super) const OPERATIONS: &[&str] = &[
    "push.subscribe",
    "push.unsubscribe",
    "push.devices",
    "push.test",
];

#[tokio::test]
async fn subscriptions_follow_the_signed_in_browser_and_survive_restart() {
    let mut f = Fixture::new().await;
    f.stop();
    std::fs::write(f.dir.path().join("web-push-local"), []).unwrap();
    f.start().await;
    let initial = f.op(0, "push.devices", json!({})).await;
    initial.ok();
    let key = initial.body["data"]["application_server_key"]
        .as_str()
        .unwrap();
    assert_eq!(key.len(), 87);
    assert_eq!(initial.body["data"]["devices"], json!([]));
    assert_eq!(f.op(0, "push.test", json!({})).await.status, 404);
    let auth = "BwcHBwcHBwcHBwcHBwcHBw";
    let endpoint = "https://fcm.googleapis.com/fixture-only";
    assert_eq!(
        f.op(
            0,
            "push.subscribe",
            json!({
                "endpoint":"https://example.invalid/not-a-provider","p256dh":key,"auth":auth
            })
        )
        .await
        .status,
        400
    );
    let added = f
        .op(
            0,
            "push.subscribe",
            json!({
                "endpoint":endpoint,"p256dh":key,"auth":auth
            }),
        )
        .await;
    added.ok();
    let devices = added.body["data"]["devices"].as_array().unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["current"], true);
    assert!(!added.body.to_string().contains(endpoint));
    assert!(!added.body.to_string().contains(auth));
    let device = devices[0]["device_id"].as_str().unwrap();
    let foreign = f
        .op(1, "push.unsubscribe", json!({"device_id":device}))
        .await;
    foreign.ok();
    assert_eq!(foreign.body["data"]["removed"], false);
    let queued = f.op(0, "push.test", json!({})).await;
    queued.ok();
    assert_eq!(queued.body["data"]["queued"], true);
    let cooldown = f.op(0, "push.test", json!({})).await;
    assert_eq!(cooldown.status, 429);
    assert_eq!(cooldown.body["error"]["code"], "rate_limited");
    assert_eq!(
        cooldown.body["error"]["message"],
        "Register this browser or wait a minute before testing again."
    );
    let seconds = cooldown.headers["retry-after"].parse::<u64>().unwrap();
    assert!((1..=60).contains(&seconds));
    assert_eq!(
        cooldown.body["error"]["details"]["retry_after_seconds"],
        seconds
    );
    f.stop();
    f.start().await;
    let after = f.op(0, "push.devices", json!({})).await;
    after.ok();
    assert_eq!(after.body["data"]["application_server_key"], key);
    assert_eq!(after.body["data"]["devices"][0]["device_id"], device);
    assert!(!after.body.to_string().contains(endpoint));
    let removed = f
        .op(0, "push.unsubscribe", json!({"device_id":device}))
        .await;
    removed.ok();
    assert_eq!(removed.body["data"]["removed"], true);
    assert_eq!(
        f.op(0, "push.devices", json!({})).await.body["data"]["devices"],
        json!([])
    );
}

#[tokio::test]
async fn thread_push_contains_click_routing_and_no_message_text() {
    let mut f = Fixture::new().await;
    f.stop();
    std::fs::write(f.dir.path().join("web-push-local"), []).unwrap();
    f.start().await;
    let devices = f.op(1, "push.devices", json!({})).await;
    devices.ok();
    f.op(
        1,
        "push.subscribe",
        json!({
            "endpoint": "https://fcm.googleapis.com/thread-fixture",
            "p256dh": devices.body["data"]["application_server_key"],
            "auth": "BwcHBwcHBwcHBwcHBwcHBw",
        }),
    )
    .await
    .ok();

    let mut machine = crate::requests_cases::Machine::enroll(&f).await;
    let space = &f.spaces[0];
    let title = "A thread to open";
    let created = f
        .op(1, "threads.create", json!({"space_id":space,"title":title}))
        .await;
    created.ok();
    let thread = &created.body["data"]["thread"]["id"];
    let message = "fixture message must stay out of the notification";
    f.op(1, "threads.send", json!({"id":thread,"message":message}))
        .await
        .ok();
    let deadline = Instant::now() + Duration::from_secs(15);
    let exec = loop {
        assert!(Instant::now() < deadline, "fixture turn did not start");
        if let Some(exec) = machine
            .sync(&f, json!([]))
            .await
            .into_iter()
            .find(|frame| frame["type"] == "exec")
        {
            break exec;
        }
    };
    // A provider text_delta followed by a successful completion.
    const TURN: &str = "eyJ2ZXJzaW9uIjoxLCJzZXEiOjEsImV2ZW50Ijp7InR5cGUiOiJ0ZXh0X2RlbHRhIiwidGV4dCI6IkFzc2lzdGFudCBmaXh0dXJlIn19CnsidmVyc2lvbiI6MSwic2VxIjoyLCJldmVudCI6eyJ0eXBlIjoiZG9uZSIsIm91dGNvbWUiOiJkb25lIn19Cg==";
    machine
        .output(
            &f,
            &exec,
            json!([
                {"kind":"data","seq":1,"stream":"stdout","data":TURN},
                {"kind":"exit","seq":2,"code":0}
            ]),
        )
        .await;

    let capture = f.appdata().join("web-push-fixture.json");
    let deadline = Instant::now() + Duration::from_secs(15);
    let raw = loop {
        if let Ok(raw) = std::fs::read_to_string(&capture) {
            break raw;
        }
        assert!(
            Instant::now() < deadline,
            "fixture thread push was not captured"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    assert!(!raw.contains(message));
    assert!(!raw.contains("Assistant fixture"));
    assert_eq!(
        serde_json::from_str::<Value>(&raw).unwrap(),
        json!({"title":title,"status":"finished","space_id":space,"thread_id":thread})
    );
}
