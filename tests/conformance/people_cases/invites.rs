//! HTTP invitation isolation, races, and personal-space enrollment.
use super::*;

async fn link(
    f: &Fixture,
    actor: usize,
    space: &str,
    role: &str,
    expires_at: Option<i64>,
) -> (String, String) {
    let created = f
        .op(
            actor,
            "invites.create",
            json!({"space_id":space,"role":role,"expires_at":expires_at}),
        )
        .await;
    created.ok();
    let invite_id = created.body["data"]["invite_id"].as_str().unwrap();
    let result = f.op(actor, "invites.list", json!({"space_id":space})).await;
    result.ok();
    let row = result.body["data"]["invites"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["invite_id"] == invite_id)
        .unwrap();
    let url = row["link"].as_str().unwrap();
    assert!(url.starts_with(&format!("{ORIGIN}/invites/")));
    (invite_id.into(), url.rsplit('/').next().unwrap().into())
}

#[tokio::test]
async fn invitation_redemption_is_single_use_expiring_and_provisions_personal_space() {
    let mut f = Fixture::new().await;
    let space = f.spaces[0].clone();
    let (invite, token) = link(&f, 1, &space, "editor", None).await;
    assert_eq!(
        f.op(2, "invites.list", json!({"space_id":space}))
            .await
            .status,
        404
    );

    let wrong_space = f
        .op(
            2,
            "invites.revoke",
            json!({"space_id":f.spaces[1],"invite_id":invite}),
        )
        .await;
    assert_eq!(wrong_space.status, 404);
    assert_eq!(
        wrong_space.signature(),
        f.op(
            2,
            "invites.revoke",
            json!({"space_id":f.spaces[1],"invite_id":uuid::Uuid::now_v7()})
        )
        .await
        .signature()
    );
    let joined = f
        .op_id(
            3,
            "invites.redeem",
            json!({"secret":token}),
            "01912345-6789-7abc-8123-000000000001",
        )
        .await;
    joined.ok();
    assert_eq!(joined.body["data"]["space_id"], space);
    f.op_id(
        3,
        "invites.redeem",
        json!({"secret":token}),
        "01912345-6789-7abc-8123-000000000001",
    )
    .await
    .ok();
    let invalid = f
        .op(4, "invites.redeem", json!({"secret":"not-an-invitation"}))
        .await;
    assert_eq!(invalid.status, 404);
    assert_eq!(
        invalid.signature(),
        f.op(4, "invites.redeem", json!({"secret":token}))
            .await
            .signature()
    );

    f.op(
        1,
        "spaces.remove_member",
        json!({"space_id":space,"account_id":f.ids[3]}),
    )
    .await
    .ok();
    assert_eq!(
        invalid.signature(),
        f.op_id(
            3,
            "invites.redeem",
            json!({"secret":token}),
            "01912345-6789-7abc-8123-000000000001"
        )
        .await
        .signature()
    );

    let (revoked, token) = link(&f, 1, &space, "reader", None).await;
    f.op(
        1,
        "invites.revoke",
        json!({"space_id":space,"invite_id":revoked}),
    )
    .await
    .ok();
    assert_eq!(
        invalid.signature(),
        f.op(4, "invites.redeem", json!({"secret":token}))
            .await
            .signature()
    );

    let soon = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
        + 1000;
    let (_, token) = link(&f, 1, &space, "reader", Some(soon)).await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert_eq!(
        invalid.signature(),
        f.op(4, "invites.redeem", json!({"secret":token}))
            .await
            .signature()
    );

    let (_, token) = link(&f, 1, &space, "reader", None).await;
    // A first prompt left open does not consume the invite.
    let first = f
        .auth(
            "setup/start",
            None,
            json!({"token":token,"name":"Invited person"}),
        )
        .await;
    first.ok();
    let mut late_device = Device::new();
    let late = late_device.register(&first);
    f.enroll_named(&token, "Invited person").await;
    let second_finish = f
        .auth(
            "setup/finish",
            None,
            json!({"ceremony":first.body["ceremony"],"credential":late,"device":"late passkey"}),
        )
        .await;
    assert_eq!(second_finish.status, 400);
    assert_eq!(second_finish.body["error"]["code"], "setup_token_invalid");
    let who = f.ids.len() - 1;
    let scopes = f.op(who, "spaces.list", json!({})).await;
    scopes.ok();
    let scopes = scopes.body["data"]["spaces"].as_array().unwrap();
    let personals: Vec<_> = scopes.iter().filter(|s| s["personal"] == true).collect();
    assert_eq!(personals.len(), 1);
    assert_eq!(personals[0]["owner"], f.ids[who]);
    assert_ne!(personals[0]["id"], f.ids[who]);
    assert!(
        scopes
            .iter()
            .any(|s| s["id"] == space && s["role"] == "reader")
    );
    assert_eq!(
        invalid.signature(),
        f.op(4, "invites.redeem", json!({"secret":token}))
            .await
            .signature()
    );
    let used = f
        .auth("setup/start", None, json!({"token":token,"name":"Used"}))
        .await;
    let bogus = f
        .auth("setup/start", None, json!({"token":"bogus","name":"Used"}))
        .await;
    assert_eq!(used.status, 400);
    assert_eq!(used.signature(), bogus.signature());
}
