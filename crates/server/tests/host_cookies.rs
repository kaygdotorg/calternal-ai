#[path = "account_common/mod.rs"]
mod common;
use axum::{body::Body, http::Request};
use common::*;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn duplicate_host_headers_and_stale_requests_never_clear_new_session() {
    let app = app();
    let (old_host, mut device) = first_owner(&app, "Owner").await;
    let start = post(&app, "/api/v1/auth/login/start", None, json!({})).await;
    let credential = device.sign(&start);
    let old_name = old_host.replacen("__Host-", "", 1);
    let signed = post(
        &app,
        "/api/v1/auth/login/finish",
        Some(&old_name),
        json!({"ceremony":start.body["ceremony"],"credential":credential}),
    )
    .await;
    assert_eq!(signed.status, 200);
    assert!(
        signed
            .cookie
            .as_ref()
            .unwrap()
            .starts_with("__Host-calternal_session=ci1_")
    );
    let new_host = signed.session();
    for cookies in [
        format!("{old_host}; {new_host}"),
        format!("{new_host}; {old_host}"),
    ] {
        let response = get(&app, "/api/v1/auth/status", Some(&cookies)).await;
        assert_eq!(response.status, 401);
        assert!(response.cookie.is_none());
    }
    let response = app
        .router
        .clone()
        .oneshot(
            Request::get("/api/v1/auth/status")
                .header("host", "localhost:8080")
                .header("cookie", &old_host)
                .header("cookie", &new_host)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    assert!(!response.headers().contains_key("set-cookie"));
    let logout = post(&app, "/api/v1/auth/logout", Some(&old_host), json!({})).await;
    assert_eq!(logout.status, 204);
    assert!(logout.cookie.is_none());
    let expired = get(&app, "/api/v1/auth/status", Some(&old_host)).await;
    assert_eq!(expired.body["signed_in"], false);
    assert!(expired.cookie.is_none());
    assert_eq!(
        get(&app, "/api/v1/auth/me", Some(&new_host)).await.status,
        200
    );
}
