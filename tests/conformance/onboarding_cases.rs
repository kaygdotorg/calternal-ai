//! Per-account onboarding state, its credentials boundary, and durable retries.

use super::*;

/// The operations this file covers, for the inventory in `isolation.rs`.
pub(super) const OPERATIONS: &[&str] = &["onboarding.state", "onboarding.finish"];

#[tokio::test]
async fn onboarding_is_account_bound_and_finish_is_durable_and_idempotent() {
    let mut f = Fixture::new().await;
    let fresh = f.op(0, "onboarding.state", json!({})).await;
    fresh.ok();
    assert_eq!(fresh.body["data"], json!({"finished":false}));
    for operation in ["onboarding.state", "onboarding.finish"] {
        let foreign = f.op(1, operation, json!({"account_id":f.ids[0]})).await;
        assert_eq!(foreign.status, 400, "{}", foreign.body);
    }
    let finished = f.op(0, "onboarding.finish", json!({})).await;
    finished.ok();
    assert_eq!(finished.body["data"], json!({"finished":true}));
    let again = f.op(0, "onboarding.finish", json!({})).await;
    again.ok();
    assert_eq!(again.body["data"], finished.body["data"]);
    let other = f.op(1, "onboarding.state", json!({})).await;
    other.ok();
    assert_eq!(other.body["data"], fresh.body["data"]);
    f.stop();
    f.start().await;
    let reloaded = f.op(0, "onboarding.state", json!({})).await;
    reloaded.ok();
    assert_eq!(reloaded.body["data"], finished.body["data"]);
}
