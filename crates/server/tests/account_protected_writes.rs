//! The live account API writes with the installed recovery capture triggers.
#[path = "account_common/mod.rs"]
mod common;
#[path = "account_live_common/mod.rs"]
mod live;
#[path = "machines_common/mod.rs"]
mod machines;

use common::{Device, Res};
use live::Host;
use serde_json::{Value, json};

fn trace(path: &str, response: &Res) {
    let code = response.code();
    println!("{path} -> {} {}", response.status.as_u16(), code);
}

async fn change(host: &Host, path: &str, cookie: Option<&str>, body: Value) -> Res {
    let response = host.post(path, cookie, body).await;
    trace(path, &response);
    response
}

async fn add(host: &Host, cookie: &str) -> Res {
    let start = change(host, "passkeys/add/start", Some(cookie), json!({})).await;
    assert_eq!(start.status, 200, "{}", start.body);
    let mut device = Device::new();
    change(
        host,
        "passkeys/add/finish",
        Some(cookie),
        json!({
            "ceremony":start.body["ceremony"], "credential":device.create_at(&host.origin, &start),
            "name":"Second device",
        }),
    )
    .await
}

fn assert_capture_is_live(host: &Host) {
    let db = rusqlite::Connection::open(host.appdata().join("system/auth.db")).unwrap();
    let enabled: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='trigger' AND name='security_recovery_account_details_UPDATE')",
        [], |row| row.get(0),
    ).unwrap();
    assert!(
        enabled,
        "protected account writes must run with recovery capture"
    );
}

#[tokio::test]
async fn first_owner_no_mail_persists_all_five_account_changes() {
    let host = Host::start(|_| {});
    let status = host.get("status", None).await;
    assert_eq!(status.body["mail_available"], false);
    assert_eq!(status.body["email_required"], false);
    let (setup, fresh, original, _) = host.owner().await;
    assert_eq!(original.len(), 10);
    println!("setup/finish -> 200; 10 recovery codes returned");
    assert_capture_is_live(&host);
    assert_eq!(
        host.get("me", Some(&fresh)).await.body["second_passkey_prompt"],
        true
    );
    let dismissed = change(&host, "passkeys/prompt/dismiss", Some(&setup), json!({})).await;
    let after_dismiss = host.get("me", Some(&fresh)).await.body["second_passkey_prompt"].clone();
    println!("GET me prompt after dismissal -> {after_dismiss}");

    let added = add(&host, &setup).await;
    let keys = host.get("passkeys", Some(&fresh)).await;
    let count = keys.body["passkeys"].as_array().unwrap().len();
    println!("GET passkeys after enrollment -> {count}");

    let regenerated = change(&host, "recovery/codes/regenerate", Some(&fresh), json!({})).await;
    let codes = regenerated.body["recovery_codes"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let changed = change(
        &host,
        "username/change",
        Some(&fresh),
        json!({"username":"new-handle"}),
    )
    .await;
    let email = change(
        &host,
        "email/start",
        Some(&fresh),
        json!({"email":"owner@example.test"}),
    )
    .await;
    let profile = host.get("me", Some(&fresh)).await;
    println!(
        "GET me after updates -> username={}, email_status={}",
        profile.body["user"]["username"], profile.body["email_status"]
    );

    assert_eq!(dismissed.status, 200, "{}", dismissed.body);
    assert_eq!(after_dismiss, false);
    assert_eq!(added.status, 200, "{}", added.body);
    assert_eq!(count, 2);
    assert_eq!(regenerated.status, 200);
    assert_eq!(codes.len(), 10);
    assert!(
        codes
            .iter()
            .all(|code| !original.contains(&code.as_str().unwrap().to_owned()))
    );
    assert_eq!(changed.status, 200);
    assert_eq!(profile.body["user"]["username"], "new-handle");
    assert_eq!(email.status, 200, "{}", email.body);
    assert_eq!(profile.body["email_status"], "unverified");
    assert_eq!(profile.body["email"], "owner@example.test");
}

#[tokio::test]
async fn account_descriptions_update_existing_rows_under_capture() {
    let host = Host::start(|_| {});
    let (setup, fresh, _, _) = host.owner().await;
    assert_capture_is_live(&host);
    let first_address = change(
        &host,
        "email/start",
        Some(&fresh),
        json!({"email":"first@example.test"}),
    )
    .await;
    assert_eq!(first_address.status, 200);
    let dismiss = change(&host, "passkeys/prompt/dismiss", Some(&setup), json!({})).await;
    let second_address = change(
        &host,
        "email/start",
        Some(&fresh),
        json!({"email":"second@example.test"}),
    )
    .await;
    let profile = host.get("me", Some(&fresh)).await;
    println!(
        "GET me after existing-row updates -> prompt={}, email={}, email_status={}",
        profile.body["second_passkey_prompt"], profile.body["email"], profile.body["email_status"]
    );
    assert_eq!(dismiss.status, 200, "{}", dismiss.body);
    assert_eq!(second_address.status, 200, "{}", second_address.body);
    assert_eq!(profile.body["second_passkey_prompt"], false);
    assert_eq!(profile.body["email"], "second@example.test");
    assert_eq!(profile.body["email_status"], "unverified");
    let repeated = change(&host, "passkeys/prompt/dismiss", Some(&setup), json!({})).await;
    assert_eq!(repeated.status, 200, "{}", repeated.body);
}

#[tokio::test]
async fn replacement_enrollment_updates_existing_account_details() {
    let host = Host::start(|_| {});
    let (_, fresh, original, _) = host.owner().await;
    assert_capture_is_live(&host);
    assert_eq!(
        change(&host, "passkeys/prompt/dismiss", Some(&fresh), json!({}))
            .await
            .status,
        200
    );
    let started = change(&host, "recovery/start", None, json!({"method":"code"})).await;
    assert_eq!(started.status, 200);
    let proven = change(
        &host,
        "recovery/prove",
        None,
        json!({
            "flow_id":started.body["flow_id"], "code":original[0],
        }),
    )
    .await;
    assert_eq!(proven.status, 200, "{}", proven.body);
    let start = change(
        &host,
        "recovery/enroll/start",
        None,
        json!({
            "enrollment_grant":proven.body["enrollment_grant"],
        }),
    )
    .await;
    assert_eq!(start.status, 200, "{}", start.body);
    let mut replacement = Device::new();
    let finished = change(&host, "recovery/enroll/finish", None, json!({
        "ceremony":start.body["ceremony"],"credential":replacement.create_at(&host.origin, &start),
    })).await;
    let previous = host.get("me", Some(&fresh)).await;
    println!("GET me with pre-recovery cookie -> {}", previous.status);
    assert_eq!(finished.status, 200, "{}", finished.body);
    assert_eq!(finished.body["sign_in_required"], true);
    assert!(finished.cookie.is_none());
    assert_eq!(previous.status, 401);
    let start = change(&host, "login/start", None, json!({})).await;
    assert_eq!(start.status, 200, "{}", start.body);
    let signed_in = change(&host, "login/finish", None, json!({
        "ceremony":start.body["ceremony"], "credential":replacement.sign_at(&host.origin, &start),
    })).await;
    assert_eq!(signed_in.status, 200, "{}", signed_in.body);
    assert_eq!(
        signed_in.body["recovery_codes"].as_array().unwrap().len(),
        10
    );
    let keys = host.get("passkeys", Some(&signed_in.session())).await;
    assert_eq!(keys.body["passkeys"].as_array().unwrap().len(), 1);
    let profile = host.get("me", Some(&signed_in.session())).await;
    assert_eq!(profile.body["second_passkey_prompt"], true);
}

#[tokio::test]
async fn production_account_post_routes_run_with_recovery_capture() {
    let host = Host::start(|_| {});
    assert_capture_is_live(&host);
    // Owner drives setup/start, setup/finish and both reverify routes through the
    // production socket. Keep all normally exposed account POSTs in this trail.
    let (setup, fresh, _, mut device) = host.owner().await;
    let start = change(&host, "login/start", None, json!({})).await;
    assert_eq!(start.status, 200);
    let login = change(
        &host,
        "login/finish",
        None,
        json!({
            "ceremony":start.body["ceremony"], "credential":device.sign_at(&host.origin, &start),
        }),
    )
    .await;
    assert_eq!(login.status, 200, "{}", login.body);
    let login = login.session();
    let first = host.get("passkeys", Some(&login)).await.body["passkeys"][0]["id"].clone();
    let added = add(&host, &setup).await;
    assert_eq!(added.status, 200, "{}", added.body);
    let second = &added.body["passkey"]["id"];
    let renamed = change(
        &host,
        "passkeys/rename",
        Some(&login),
        json!({"id":second,"name":"Phone"}),
    )
    .await;
    assert_eq!(renamed.status, 200);
    assert!(
        host.get("passkeys", Some(&login)).await.body["passkeys"]
            .as_array()
            .unwrap()
            .iter()
            .any(|key| key["id"] == *second && key["name"] == "Phone")
    );
    assert_eq!(
        change(&host, "passkeys/remove", Some(&login), json!({"id":first}))
            .await
            .status,
        200
    );
    assert_eq!(
        host.get("passkeys", Some(&login)).await.body["passkeys"][0]["id"],
        *second
    );

    let obsolete = host.get("me", Some(&setup)).await.body["installation"].clone();
    let revoked = change(
        &host,
        "installations/revoke",
        Some(&login),
        json!({"installation":obsolete}),
    )
    .await;
    assert_eq!(revoked.status, 200, "{}", revoked.body);
    assert_eq!(host.get("me", Some(&setup)).await.status, 401);
    assert_eq!(
        change(&host, "logout", Some(&fresh), json!({}))
            .await
            .status,
        204
    );
    assert_eq!(host.get("me", Some(&fresh)).await.status, 401);

    assert_eq!(
        change(
            &host,
            "username/change",
            Some(&login),
            json!({"username":"matrix-owner"})
        )
        .await
        .status,
        200
    );
    assert_eq!(
        change(
            &host,
            "email/start",
            Some(&login),
            json!({"email":"matrix@example.test"})
        )
        .await
        .status,
        200
    );
    assert_eq!(
        change(&host, "passkeys/prompt/dismiss", Some(&login), json!({}))
            .await
            .status,
        200
    );
    let codes = change(&host, "recovery/codes/regenerate", Some(&login), json!({})).await;
    assert_eq!(codes.status, 200, "{}", codes.body);
    let no_mail = change(
        &host,
        "email/confirm",
        Some(&login),
        json!({
            "challenge_id":"not-issued", "code":"00000000",
        }),
    )
    .await;
    assert_eq!(no_mail.code(), "email_delivery_failed");
    let status = change(&host, "recovery/status", None, json!({"flow_id":"unknown"})).await;
    assert_eq!(status.status, 200, "{}", status.body);
    let vetos = change(&host, "recovery/veto", Some(&login), json!({})).await;
    assert_eq!(vetos.status, 200, "{}", vetos.body);

    let recovery = change(&host, "recovery/start", None, json!({"method":"code"})).await;
    assert_eq!(recovery.status, 200, "{}", recovery.body);
    let proof = change(
        &host,
        "recovery/prove",
        None,
        json!({
            "flow_id":recovery.body["flow_id"], "code":codes.body["recovery_codes"][0],
        }),
    )
    .await;
    assert_eq!(proof.status, 200, "{}", proof.body);
    let recovery = change(
        &host,
        "recovery/enroll/start",
        None,
        json!({"enrollment_grant":proof.body["enrollment_grant"]}),
    )
    .await;
    assert_eq!(recovery.status, 200, "{}", recovery.body);
    let mut key = Device::new();
    let recovered = change(&host, "recovery/enroll/finish", None, json!({
        "ceremony":recovery.body["ceremony"], "credential":key.create_at(&host.origin, &recovery),
    })).await;
    assert_eq!(recovered.status, 200, "{}", recovered.body);
    assert_eq!(host.get("me", Some(&login)).await.status, 401);
}
