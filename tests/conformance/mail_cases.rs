//! Real passkey sessions, seeded package operations, and account-owned Mail setup.
use super::*;

#[tokio::test]
async fn mail_operations_and_the_password_prompt_follow_personal_authority() {
    let f = Fixture::new().await;
    let mine = f.op(1, "spaces.list", json!({})).await;
    mine.ok();
    let personal = mine.body["data"]["spaces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|space| space["personal"] == true)
        .unwrap()["id"]
        .as_str()
        .unwrap();
    let missing = uuid::Uuid::now_v7().to_string();
    let endpoint = json!({"host":"generated.mail.test","port":993,"username":"reader"});
    let account = json!({"account_id":missing});
    let folder = json!({"account_id":missing,"folder":"INBOX"});
    let target = json!({"account_id":missing,"folder":"INBOX","uid_validity":1,"uid":1});
    let operations = [
        ("add-account", endpoint.clone()),
        ("list-accounts", json!({})),
        ("list-folders", account),
        ("list-headers", folder),
        ("read-message", target.clone()),
        ("mark-read", target),
        ("open-snapshot", json!({"item_id":missing})),
    ];
    for (name, mut input) in operations {
        input["space_id"] = json!(personal);
        let operation = format!("calternal-mail.{name}");
        let signed_out = f
            .req(
                "POST",
                "/api/v1/operations",
                None,
                json!({"request_id":"without-session","operation":operation,"input":input}),
                Some(ORIGIN),
            )
            .await;
        assert_eq!(signed_out.status, 401, "{operation}");
        let foreign = f.op(2, &operation, input.clone()).await;
        assert_eq!(foreign.status, 404, "{operation}");
        input["space_id"] = json!(missing);
        let absent = f.op(2, &operation, input).await;
        assert_eq!(foreign.signature(), absent.signature(), "{operation}");
    }
    for space in [personal, &f.spaces[0]] {
        f.op(
            1,
            "plugins.enable",
            json!({"space_id":space,"plugin_id":"calternal-mail"}),
        )
        .await
        .ok();
    }

    let mut input = endpoint;
    input["space_id"] = json!(&f.spaces[0]);
    let shared = f.op(1, "calternal-mail.add-account", input.clone()).await;
    assert!(shared.status >= 400, "shared space created a Mail prompt");
    input["space_id"] = json!(personal);
    let added = f.op(1, "calternal-mail.add-account", input.clone()).await;
    added.ok();
    let id = &added.body["data"]["id"];
    let request = &added.body["data"]["secret_request_id"];
    assert!(id.is_string() && request.is_string());
    assert_eq!(added.body["data"]["ready"], false);
    let again = f.op(1, "calternal-mail.add-account", input).await;
    again.ok();
    assert_eq!(&again.body["data"]["id"], id);
    assert_eq!(&again.body["data"]["secret_request_id"], request);

    let own = f.op(1, "secrets.list", json!({})).await;
    own.ok();
    assert_eq!(&own.body["data"]["requests"][0]["id"], request);
    let other = f.op(2, "secrets.list", json!({})).await;
    other.ok();
    assert_eq!(other.body["data"]["requests"], json!([]));

    let accounts = f
        .op(
            1,
            "calternal-mail.list-accounts",
            json!({"space_id":personal}),
        )
        .await;
    accounts.ok();
    assert_eq!(&accounts.body["data"]["accounts"][0]["id"], id);
}
