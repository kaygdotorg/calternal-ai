//! Installation-scoped harness administration and owner-only live controls.
use super::*;

pub(super) const OPERATIONS: &[&str] = &[
    "harnesses.list",
    "harnesses.stage",
    "harnesses.activate",
    "harnesses.rollback",
    "agent.steer",
    "agent.approval",
];

#[tokio::test]
async fn harness_catalog_requires_installation_authority_and_a_tested_image() {
    let f = Fixture::new().await;
    let catalog = f.op(0, "harnesses.list", json!({})).await;
    catalog.ok();
    let rows = catalog.body["data"]["harnesses"].as_array().unwrap();
    assert!(
        rows.iter()
            .any(|row| row["id"] == "calternal-claude-code" && row["bundled"] == true)
    );
    assert_eq!(f.op(1, "harnesses.list", json!({})).await.status, 403);

    let id = "calternal-claude-code";
    let image = format!("sha256:{}", "a".repeat(64));
    let select = json!({ "id": id, "image": image });
    let offer = json!({
        "pin": {"version":"0.0.1", "image":image, "package_sha256":"a".repeat(64),
            "guest_image_digest":null},
        "login_command":"fixture login", "login_locations":["https://example.invalid/device"]
    });
    let stage = json!({"id":id, "space_id":f.spaces[0],
        "machine_id":uuid::Uuid::now_v7(), "offer":offer});
    assert_eq!(f.op(1, "harnesses.stage", stage).await.status, 403);
    for operation in ["harnesses.activate", "harnesses.rollback"] {
        assert_eq!(f.op(1, operation, select.clone()).await.status, 403);
        assert!(f.op(0, operation, select.clone()).await.status >= 400);
    }

    let job = uuid::Uuid::now_v7();
    assert_eq!(
        f.op(1, "agent.steer", json!({"job_id":job, "prompt":"fixture"}))
            .await
            .status,
        404
    );
    assert_eq!(
        f.op(
            1,
            "agent.approval",
            json!({"job_id":job, "approval_id":"unknown", "approve":true})
        )
        .await
        .status,
        404
    );
}
