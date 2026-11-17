//! Signing in, sessions and Installations.

#[path = "account_common/mod.rs"]
mod common;

use axum::body::Body;
use common::*;
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn sign_in_with_a_passkey_creates_a_new_installation() {
    let app = app();
    let (laptop, mut dev) = first_owner_setup_session(&app, "kayg").await;
    let res = login(&app, &mut dev).await;
    assert_eq!(res.status, 200, "{:?}", res.body);
    let phone = res.session();
    assert_ne!(laptop, phone);

    let list = action(&app, &phone, "account.installations", "", json!(null)).await;
    let rows = list.body["result"]["installations"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(rows.len(), 2);
    let current: Vec<_> = rows.iter().filter(|r| r["current"] == true).collect();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0]["name"], "Phone");
    assert_eq!(current[0]["kind"], "browser");
}

#[tokio::test]
async fn a_sign_in_answer_cannot_be_replayed() {
    let app = app();
    let (_, mut dev) = first_owner(&app, "kayg").await;
    let start = post(&app, "/api/v1/auth/login/start", None, json!({})).await;
    let body = json!({ "ceremony": start.body["ceremony"], "credential": dev.sign(&start) });
    assert_eq!(
        post(&app, "/api/v1/auth/login/finish", None, body.clone())
            .await
            .status,
        200
    );
    let again = post(&app, "/api/v1/auth/login/finish", None, body.clone()).await;
    assert_eq!(again.code(), "passkey_failed");

    // The same answer against a fresh prompt fails too: the challenge differs.
    let fresh = post(&app, "/api/v1/auth/login/start", None, json!({})).await;
    let mut swapped = body;
    swapped["ceremony"] = fresh.body["ceremony"].clone();
    let r = post(&app, "/api/v1/auth/login/finish", None, swapped).await;
    assert_eq!(r.code(), "passkey_failed");
}

#[tokio::test]
async fn an_unknown_passkey_is_refused() {
    let app = app();
    let (_, real) = first_owner(&app, "kayg").await;
    // A key registered on another server, claiming to be this owner.
    let other = common::app();
    let (_, mut stranger) = first_owner(&other, "x").await;
    stranger.user = real.user.clone();
    let r = login(&app, &mut stranger).await;
    assert_eq!(r.code(), "passkey_failed");
}

#[tokio::test]
async fn sign_out_ends_the_session_immediately() {
    let app = app();
    let (cookie, _) = first_owner(&app, "kayg").await;
    assert_eq!(
        get(&app, "/api/v1/auth/me", Some(&cookie)).await.status,
        200
    );
    let out = post(&app, "/api/v1/auth/logout", Some(&cookie), json!({})).await;
    assert_eq!(out.status, 204);
    assert!(out.cookie.is_none());
    let me = get(&app, "/api/v1/auth/me", Some(&cookie)).await;
    assert_eq!((me.status.as_u16(), me.code()), (401, "unauthorized"));
    let api = action(&app, &cookie, "hello.say", "k", json!({ "text": "x" })).await;
    assert_eq!(api.status, 401);
}

// Source live-feed cases are deferred; see ../PORTING.md.

#[tokio::test]
async fn sessions_expire_when_unused() {
    let app = app_with(|c| c.session_ttl = Duration::from_millis(80));
    let (cookie, _) = first_owner(&app, "kayg").await;
    assert_eq!(
        get(&app, "/api/v1/auth/me", Some(&cookie)).await.status,
        200
    );
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(
        get(&app, "/api/v1/auth/me", Some(&cookie)).await.status,
        401
    );
}

#[tokio::test]
async fn bearer_tokens_work_like_cookies() {
    let app = app();
    let (cookie, _) = first_owner(&app, "kayg").await;
    let token = cookie.split_once('=').unwrap().1.to_owned();
    let auth = format!("Bearer {token}");
    let r = send(
        &app,
        "GET",
        "/api/v1/auth/me",
        &[("authorization", &auth)],
        Body::empty(),
    )
    .await;
    assert_eq!(r.status, 200);
}

/// An old browser cookie must not establish a fresh live socket after logout, either.

#[tokio::test]
async fn foreign_status_cannot_sign_the_browser_out() {
    let app = app();
    let (cookie, _) = first_owner(&app, "kayg").await;
    let status = send(
        &app,
        "GET",
        "/api/v1/auth/status",
        &[
            ("cookie", &cookie),
            ("origin", "http://preview.localhost:8080"),
        ],
        Body::empty(),
    )
    .await;
    assert_eq!(status.body["signed_in"], false);
    assert!(status.cookie.is_none());
    assert_eq!(
        get(&app, "/api/v1/auth/me", Some(&cookie)).await.status,
        200
    );
}
