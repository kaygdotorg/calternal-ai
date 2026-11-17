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
