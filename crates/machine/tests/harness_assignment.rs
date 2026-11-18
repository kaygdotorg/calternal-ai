//! A harness assignment carries the selected provider and workspace policy.
use calternal_machine::protocol::Down;
use serde_json::json;
use uuid::Uuid;

#[test]
fn harness_policy_survives_machine_transport() {
    let account = Uuid::now_v7();
    let assignment = json!({
        "type": "assign_harness",
        "lease_id": Uuid::now_v7(),
        "fence": 2,
        "space_id": Uuid::now_v7(),
        "account_id": account,
        "spec": {
            "account_id": account,
            "thread_id": Uuid::now_v7(),
            "image": null,
            "worker": null,
            "limits": {},
        },
        "harness": {
            "provider_id": "community-fixture",
            "login_scope": {"scope": "account", "id": account},
            "workspace_read_only": true,
            "guest_image_digest": null,
        },
    });
    let parsed: Down = serde_json::from_value(assignment.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(parsed).unwrap()["harness"],
        assignment["harness"]
    );
}
