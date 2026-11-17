#[path = "account_common/mod.rs"]
mod common;
use axum::{body::Body, http::Request};
use common::*;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
#[ignore = "2026-11-17 user decision: no legacy compatibility; obsolete cookie clearing is intentionally absent"]
async fn old_name_is_ignored_and_cleared_once_without_touching_host_session() {
    let app = app();
    let (host, _) = first_owner(&app, "Owner").await;
    let old = host.replacen("__Host-", "", 1);
    for (cookies, signed_in) in [
        (old.clone(), false),
        (format!("{old}; {host}"), true),
        (format!("{host}; {old}; {old}"), true),
    ] {
        let response = app
            .router
            .clone()
            .oneshot(
                Request::get("/api/v1/auth/status")
                    .header("host", "localhost:8080")
                    .header("cookie", cookies)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let clearing: Vec<_> = response.headers().get_all("set-cookie").iter().collect();
        assert_eq!(clearing.len(), 1);
        assert_eq!(
            clearing[0],
            "calternal_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
        );
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()["signed_in"],
            signed_in
        );
    }
    assert_eq!(get(&app, "/api/v1/auth/me", Some(&old)).await.status, 401);
    assert_eq!(get(&app, "/api/v1/auth/me", Some(&host)).await.status, 200);
    assert!(
        get(&app, "/api/v1/auth/status", None)
            .await
            .cookie
            .is_none()
    );
}

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

#[tokio::test]
#[ignore = "2026-11-17 user decision: no old schema compatibility work; source migration fixture retained for reference"]
async fn upgrading_schema_six_removes_cookie_state_and_keeps_passkeys() {
    let app = app();
    let (host, _) = first_owner(&app, "Owner").await;
    let path = app.dir.path().join("appdata/system/auth.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    // `app()` creates the current schema. Remove v11 tables before modeling
    // the v6 database this test upgrades.
    remove_v11_sharing_schema(&db).unwrap();
    db.execute_batch("PRAGMA foreign_keys=OFF; DROP TRIGGER account_identity_separate; DROP TABLE space_receipts; DROP TABLE development_directory; DROP TABLE space_memberships; DROP TABLE spaces; DROP TABLE space_role_capabilities; DROP TABLE space_roles;").unwrap();
    db.execute_batch(
        "DROP TRIGGER installation_scopes_insert;
         DROP TRIGGER installation_scopes_update;
         ALTER TABLE installations DROP COLUMN registration_session;
         ALTER TABLE installations DROP COLUMN assertion_at;
         ALTER TABLE installations DROP COLUMN scopes;
         ALTER TABLE installations ADD COLUMN host_cookie INTEGER NOT NULL DEFAULT 0;
         PRAGMA user_version=6;",
    )
    .unwrap();
    drop(db);
    let reopened = calternal_auth::Auth::open(
        &app.dir.path().join("appdata"),
        calternal_auth::AuthConfig::new(ORIGIN).unwrap(),
    )
    .unwrap();
    let db = rusqlite::Connection::open(path).unwrap();
    assert!(db.prepare("SELECT host_cookie FROM installations").is_err());
    assert_eq!(
        db.query_row("SELECT count(*) FROM passkeys", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let response =
        calternal_server::account_http::routes::<()>(calternal_server::account_http::HttpAuth {
            auth: reopened,
            config: calternal_auth::AuthConfig::new(ORIGIN).unwrap(),
        })
        .oneshot(
            Request::get("/api/v1/auth/me")
                .header("cookie", host)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}
