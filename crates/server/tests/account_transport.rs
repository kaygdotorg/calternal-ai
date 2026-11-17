//! Transport integration for freshness, account-private passkeys and session authority.
#[path = "account_common/mod.rs"]
mod common;
use axum::{body::Body, extract::ConnectInfo, http::Request};
use calternal_server::account_http::{HttpAuth, source_key};
use common::*;
use serde_json::json;

#[tokio::test]
async fn passkey_management_keeps_the_last_key_and_uses_fresh_proof() {
    let app = app();
    let (cookie, _) = first_owner(&app, "Owner").await;
    let list = get(&app, "/api/v1/auth/passkeys", Some(&cookie)).await;
    assert_eq!(list.status, 200);
    let first = list.body["passkeys"][0]["id"].clone();
    let removed = post(
        &app,
        "/api/v1/auth/passkeys/remove",
        Some(&cookie),
        json!({"id":first}),
    )
    .await;
    assert_eq!(removed.code(), "last_passkey");
    let start = post(
        &app,
        "/api/v1/auth/passkeys/add/start",
        Some(&cookie),
        json!({}),
    )
    .await;
    assert_eq!(start.status, 200, "{}", start.body);
    let mut key = Device::new();
    let credential = key.create(&start);
    let add = post(
        &app,
        "/api/v1/auth/passkeys/add/finish",
        Some(&cookie),
        json!({"ceremony":start.body["ceremony"],"credential":credential,"name":"Phone"}),
    )
    .await;
    assert_eq!(add.status, 200, "{}", add.body);
    let id = add.body["passkey"]["id"].clone();
    let renamed = post(
        &app,
        "/api/v1/auth/passkeys/rename",
        Some(&cookie),
        json!({"id":id,"name":"  Renamed  "}),
    )
    .await;
    assert_eq!(renamed.status, 200);
    assert_eq!(renamed.body["name"], "Renamed");
    let removed = post(
        &app,
        "/api/v1/auth/passkeys/remove",
        Some(&cookie),
        json!({"id":first}),
    )
    .await;
    assert_eq!(removed.status, 200);
    let list = get(&app, "/api/v1/auth/passkeys", Some(&cookie)).await;
    assert_eq!(list.body["passkeys"].as_array().unwrap().len(), 1);
    assert_eq!(list.body["passkeys"][0]["id"], id);
}
#[tokio::test]
async fn reverify_is_bound_to_current_account_and_purpose() {
    let app = app();
    let (cookie, mut device) = first_owner(&app, "Owner").await;
    let start = post(
        &app,
        "/api/v1/auth/reverify/start",
        Some(&cookie),
        json!({}),
    )
    .await;
    assert_eq!(start.status, 200);
    let credential = device.sign(&start);
    let wrong = post(
        &app,
        "/api/v1/auth/login/finish",
        None,
        json!({"ceremony":start.body["ceremony"],"credential":credential}),
    )
    .await;
    assert_eq!(wrong.code(), "passkey_failed");
    let correct = post(
        &app,
        "/api/v1/auth/reverify/finish",
        Some(&cookie),
        json!({"ceremony":start.body["ceremony"],"credential":credential}),
    )
    .await;
    assert_eq!(correct.status, 200, "{}", correct.body);
    assert_ne!(correct.session(), cookie);
    assert_eq!(
        get(&app, "/api/v1/auth/me", Some(&cookie)).await.status,
        200
    );
    let replay = post(
        &app,
        "/api/v1/auth/reverify/finish",
        Some(&cookie),
        json!({"ceremony":start.body["ceremony"],"credential":credential}),
    )
    .await;
    assert_eq!(replay.code(), "passkey_failed");
}
#[tokio::test]
async fn session_authenticates_an_account_and_never_implies_space_authority() {
    let app = app();
    let (cookie, _) = first_owner(&app, "Owner").await;
    let discovery = get(&app, "/api/v1/describe", Some(&cookie)).await;
    assert_eq!(discovery.status, 200);
    assert_eq!(discovery.body["principal"]["spaces"], json!([]));
    let me = get(&app, "/api/v1/auth/me", Some(&cookie)).await;
    assert_eq!(me.body["user"]["role"], "admin");
    let request =
        json!({"request_id":"denied","operation":"notes.list","input":{"space_id":calternal_types::SpaceId::new()}});
    let refused = post(&app, "/api/v1/operations", Some(&cookie), request).await;
    assert_eq!(refused.status, 404);
    let created = post(
        &app,
        "/api/v1/operations",
        Some(&cookie),
        json!({"request_id":"create","operation":"spaces.create","input":{"name":"Space"}}),
    )
    .await;
    assert_eq!(created.status, 200, "{}", created.body);
    let space = created.body["data"]["space"]["id"].clone();
    assert_ne!(space, me.body["user"]["id"]);
    let note=post(&app,"/api/v1/operations",Some(&cookie),json!({"request_id":"note","operation":"notes.create","input":{"space_id":space,"title":"Note","markdown":"test"}})).await;
    assert_eq!(note.status, 200, "{}", note.body);
    let resolved=post(&app,"/api/v1/operations",Some(&cookie),json!({"request_id":"resolve","operation":"resolve","input":{"kind":"notes","id":note.body["data"]["note"]["id"]}})).await;
    assert_eq!(resolved.status, 200, "{}", resolved.body);
    assert_eq!(resolved.body["data"]["space_id"], space);
    let revoked = post(
        &app,
        "/api/v1/auth/installations/revoke",
        Some(&cookie),
        json!({"installation":me.body["installation"]}),
    )
    .await;
    assert_eq!(revoked.status, 200);
    assert_eq!(
        get(&app, "/api/v1/describe", Some(&cookie)).await.status,
        401
    );
}
#[test]
fn forwarding_stops_at_first_untrusted_hop_and_rejects_malformed_chains() {
    let app = app();
    let mut config = calternal_auth::AuthConfig::new(ORIGIN).unwrap();
    config.set_trusted_proxies("127.0.0.0/8, ::1/128").unwrap();
    let service = HttpAuth {
        auth: app.auth.clone(),
        config,
    };
    let request = |peer: &str, xff: &str| {
        Request::builder()
            .extension(ConnectInfo(peer.parse::<std::net::SocketAddr>().unwrap()))
            .header("x-forwarded-for", xff)
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        source_key(
            &service,
            &request("127.0.0.1:123", "198.51.100.1, 192.0.2.1, 127.0.0.2")
        ),
        "192.0.2.1"
    );
    assert_eq!(
        source_key(&service, &request("192.0.2.2:123", "198.51.100.1")),
        "192.0.2.2"
    );
    assert_eq!(
        source_key(&service, &request("127.0.0.1:123", "junk, 192.0.2.1")),
        "127.0.0.1"
    );
    assert!(
        calternal_auth::AuthConfig::new("https://example.org")
            .unwrap()
            .set_trusted_proxies("127.0.0.1/33")
            .is_err()
    );
    assert_eq!(
        source_key(&service, &request("[2001:db8:1:2::3]:123", "198.51.100.1")),
        "2001:db8:1:2::/64"
    );
    assert_eq!(
        source_key(&service, &request("[::ffff:192.0.2.2]:123", "198.51.100.1")),
        "192.0.2.2"
    );
}
#[tokio::test]
async fn issued_cookie_is_host_only_secure_and_does_not_include_a_domain() {
    let app = app();
    let (cookie, mut device) = first_owner(&app, "Owner").await;
    let response = login(&app, &mut device).await;
    let header = response.cookie.unwrap();
    assert!(header.starts_with("__Host-calternal_session="));
    assert!(header.ends_with("; Path=/; HttpOnly; SameSite=Lax; Max-Age=7776000; Secure"));
    assert!(!header.contains("Domain="));
    assert_eq!(
        post(&app, "/api/v1/auth/logout", Some(&cookie), json!({}))
            .await
            .status,
        204
    );
}

#[tokio::test]
async fn stale_account_proof_requires_reverification_before_adding_a_passkey() {
    let app = app();
    let (cookie, mut device) = first_owner(&app, "Owner").await;
    let db = rusqlite::Connection::open(app.dir.path().join("appdata/system/auth.db")).unwrap();
    db.execute(
        "UPDATE installations SET assertion_at=0, registration_session=0",
        [],
    )
    .unwrap();
    drop(db);
    let stale = post(
        &app,
        "/api/v1/auth/passkeys/add/start",
        Some(&cookie),
        json!({}),
    )
    .await;
    assert_eq!(stale.code(), "passkey_reverification_required");
    let start = post(
        &app,
        "/api/v1/auth/reverify/start",
        Some(&cookie),
        json!({}),
    )
    .await;
    let credential = device.sign(&start);
    let verified = post(
        &app,
        "/api/v1/auth/reverify/finish",
        Some(&cookie),
        json!({"ceremony":start.body["ceremony"],"credential":credential}),
    )
    .await;
    assert_eq!(verified.status, 200);
    let fresh = post(
        &app,
        "/api/v1/auth/passkeys/add/start",
        Some(&verified.session()),
        json!({}),
    )
    .await;
    assert_eq!(fresh.status, 200, "{}", fresh.body);
}
