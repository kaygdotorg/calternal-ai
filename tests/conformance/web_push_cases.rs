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
    assert_eq!(f.op(0, "push.test", json!({})).await.status, 503);
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
