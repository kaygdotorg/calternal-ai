//! Breaking sign-in on purpose: junk, oversized and cross-site requests,
//! forged sessions and floods. Every refusal must be a friendly, coded
//! error, never a crash or a leak.

#[path = "account_common/mod.rs"]
mod common;

use axum::body::Body;
use common::*;
use serde_json::json;

const POSTS: [&str; 5] = [
    "/api/v1/auth/setup/start",
    "/api/v1/auth/setup/finish",
    "/api/v1/auth/login/start",
    "/api/v1/auth/login/finish",
    "/api/v1/auth/logout",
];

fn assert_friendly(r: &Res, code: &str) {
    assert_eq!(r.code(), code, "{:?}", r.body);
    let e = &r.body["error"];
    assert!(e["message"].as_str().is_some_and(|m| m.ends_with('.')));
    assert!(e["request_id"].as_str().is_some());
    assert!(e.get("detail").is_none(), "details never reach clients");
}

#[tokio::test]
async fn oversized_bodies_are_refused() {
    let app = app();
    let big = format!("{{\"token\":\"{}\"}}", "a".repeat(100 * 1024));
    for path in POSTS.iter().filter(|p| !p.ends_with("logout")) {
        let r = send(&app, "POST", path, &[], Body::from(big.clone())).await;
        assert_eq!(r.status, 413, "{path}");
        assert_friendly(&r, "too_large");
    }
}

#[tokio::test]
async fn bad_json_and_unknown_fields_are_refused() {
    let app = app();
    for body in [
        "{",
        "[]",
        "null",
        "\"x\"",
        "{\"token\": 5}",
        "{\"token\":\"x\",\"evil\":1}",
    ] {
        for path in [
            "/api/v1/auth/setup/start",
            "/api/v1/auth/setup/finish",
            "/api/v1/auth/login/finish",
        ] {
            let r = send(&app, "POST", path, &[], Body::from(body)).await;
            assert_eq!(r.status, 400, "{path} {body}");
            assert_friendly(&r, "invalid_input");
        }
    }
    let r = send(
        &app,
        "POST",
        "/api/v1/auth/login/start",
        &[],
        Body::from("{oops"),
    )
    .await;
    assert_friendly(&r, "invalid_input");
}

#[tokio::test]
async fn non_json_requests_are_refused() {
    let app = app();
    for path in POSTS {
        let r = send(
            &app,
            "POST",
            path,
            &[("content-type", "application/x-www-form-urlencoded")],
            Body::from("token=x"),
        )
        .await;
        assert_eq!(r.status, 400, "{path}");
        assert_friendly(&r, "invalid_input");
    }
}

#[tokio::test]
async fn other_sites_cannot_act_for_you() {
    let app = app();
    let (cookie, _) = first_owner(&app, "kayg").await;
    for origin in [
        "https://evil.example",
        "http://localhost:9999",
        "null",
        "http://localhost:8080.evil",
    ] {
        for path in POSTS {
            let r = send(
                &app,
                "POST",
                path,
                &[("origin", origin), ("cookie", &cookie)],
                Body::from("{}"),
            )
            .await;
            assert_eq!(r.status, 403, "{path} from {origin}");
            assert_friendly(&r, "forbidden");
        }
    }
    // The session survived all of that.
    assert_eq!(
        get(&app, "/api/v1/auth/me", Some(&cookie)).await.status,
        200
    );
}

#[tokio::test]
async fn forged_sessions_get_nowhere() {
    let app = app();
    let (cookie, _) = first_owner(&app, "kayg").await;
    let real = cookie.split_once('=').unwrap().1;
    let mut flipped = real.to_owned();
    let last = flipped.pop().unwrap();
    flipped.push(if last == 'a' { 'b' } else { 'a' });
    let huge = "a".repeat(8000);
    let forged = [
        "__Host-calternal_session=".to_owned(),
        "__Host-calternal_session=ci1_short".to_owned(),
        format!("__Host-calternal_session={flipped}"),
        format!("__Host-calternal_session={huge}"),
        format!("__Host-calternal_session=ci1_{}", "z".repeat(52)),
        "__Host-calternal_session=../../etc/passwd".to_owned(),
        "__Host-calternal_session=ci1_é".to_owned(),
    ];
    for c in &forged {
        let r = get(&app, "/api/v1/auth/me", Some(c)).await;
        assert_eq!(r.status, 401, "{c}");
        assert_friendly(&r, "unauthorized");
        let r = action(&app, c, "hello.say", "k", json!({ "text": "x" })).await;
        assert_eq!(r.status, 401);
    }
    let r = send(
        &app,
        "GET",
        "/api/v1/auth/me",
        &[("authorization", "Bearer nope")],
        Body::empty(),
    )
    .await;
    assert_eq!(r.status, 401);
}

#[tokio::test]
async fn junk_setup_tokens_are_refused_then_rate_limited() {
    let app = app_with(|c| c.setup_failures_per_minute = 5);
    let _real = app.auth.first_owner_token().await.unwrap();
    let junk = [
        String::new(),
        "x".into(),
        "a".repeat(52),
        "😀".repeat(30),
        "a".repeat(10_000),
        "' OR 1=1 --".into(),
    ];
    let mut refused = 0;
    for t in junk.iter().cycle().take(5) {
        let r = post(
            &app,
            "/api/v1/auth/setup/start",
            None,
            json!({ "token": t, "name": "k" }),
        )
        .await;
        assert_friendly(&r, "invite_invalid");
        refused += 1;
    }
    assert_eq!(refused, 5);
    let r = post(
        &app,
        "/api/v1/auth/setup/start",
        None,
        json!({ "token": "x", "name": "k" }),
    )
    .await;
    assert_eq!(r.status, 429);
    assert_friendly(&r, "too_many_requests");
}

#[tokio::test]
async fn anonymous_prompt_flood_preserves_login_setup_and_add() {
    let app = app_with(|c| c.max_ceremonies = 5);
    let (cookie, _) = first_owner(&app, "owner").await;
    for _ in 0..10 {
        let r = post(&app, "/api/v1/auth/login/start", None, json!({})).await;
        assert_eq!(r.status, 200, "{:?}", r.body);
    }
    let made = app.auth.create("member").await.unwrap();
    let token = made.token.as_str();
    let r = post(
        &app,
        "/api/v1/auth/setup/start",
        None,
        json!({"token": token, "name":"member"}),
    )
    .await;
    assert_eq!(r.status, 200, "{:?}", r.body);
    let r = action(
        &app,
        &cookie,
        "account.passkey-add-start",
        "add",
        json!(null),
    )
    .await;
    assert_eq!(r.status, 200, "{:?}", r.body);
}

#[tokio::test]
async fn duplicate_session_cookies_never_choose_an_identity() {
    let app = app();
    let (cookie, _) = first_owner(&app, "owner").await;
    let member = app.auth.create("member").await.unwrap();
    let start = post(
        &app,
        "/api/v1/auth/setup/start",
        None,
        json!({"token":member.token,"name":"member"}),
    )
    .await;
    assert_eq!(start.status, 200, "{:?}", start.body);
    let credential = Device::new().create(&start);
    let finished = post(
        &app,
        "/api/v1/auth/setup/finish",
        None,
        json!({"ceremony":start.body["ceremony"],"credential":credential}),
    )
    .await;
    assert_eq!(finished.status, 200, "{:?}", finished.body);
    let other = finished.session();
    for cookies in [format!("{cookie}; {other}"), format!("{other}; {cookie}")] {
        let r = get(&app, "/api/v1/auth/me", Some(&cookies)).await;
        assert_eq!(r.status, 401);
        assert!(r.cookie.is_none());
    }
}

#[tokio::test]
async fn garbage_credentials_never_crash() {
    // Each shape gets a fresh account so back-off cannot mask validation.
    for (path, junk) in [
        ("", json!({})),
        ("", json!([])),
        ("", json!("x")),
        ("", json!(null)),
        ("/response/signature", json!("AAAA")),
        ("/response/authenticatorData", json!("")),
        ("/response/clientDataJSON", json!("e30")),
        ("/response/userHandle", json!("not-a-uuid")),
        ("/rawId", json!("")),
    ] {
        let app = app();
        let (_, mut dev) = first_owner(&app, "kayg").await;
        let start = post(&app, "/api/v1/auth/login/start", None, json!({})).await;
        let cred = if path.is_empty() {
            junk
        } else {
            let mut cred = dev.sign(&start);
            *cred.pointer_mut(path).unwrap() = junk;
            cred
        };
        let r = post(
            &app,
            "/api/v1/auth/login/finish",
            None,
            json!({ "ceremony": start.body["ceremony"], "credential": cred }),
        )
        .await;
        assert_eq!(r.status, 401, "{:?}", r.body);
        assert_friendly(&r, "passkey_failed");
    }
}

#[tokio::test]
async fn nothing_about_the_server_leaks_before_sign_in() {
    let app = app();
    first_owner(&app, "Very Secret Name").await;
    let s = get(&app, "/api/v1/auth/status", None).await;
    assert_eq!(
        s.body,
        json!({ "setup": false, "signed_in": false, "email_required": false, "mail_available": false })
    );
    let start = post(&app, "/api/v1/auth/login/start", None, json!({})).await;
    let text = start.body.to_string();
    assert!(
        !text.contains("Very Secret"),
        "sign-in prompt leaks a name: {text}"
    );
    assert!(start.body["options"].get("mediation").is_none());
}

#[tokio::test]
async fn one_source_cannot_lock_out_another_and_cannot_spoof_forwarding() {
    use axum::{extract::ConnectInfo, http::Request};
    use std::net::SocketAddr;
    use tower::ServiceExt;
    let app = app_with(|c| c.setup_failures_per_minute = 2);
    let token = app.auth.first_owner_token().await.unwrap().unwrap();
    for (ip, token, expected) in [
        ("192.0.2.1:1000", "junk", 400),
        ("192.0.2.1:1000", "junk", 400),
        ("192.0.2.1:1000", "junk", 429),
        ("192.0.2.2:1000", token.as_str(), 200),
    ] {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/auth/setup/start")
            .extension(ConnectInfo(ip.parse::<SocketAddr>().unwrap()))
            .header("host", "localhost:8080")
            .header("content-type", "application/json")
            .header("origin", ORIGIN)
            // Changing this header must not give the attacker a new bucket.
            .header("x-forwarded-for", format!("198.51.100.{expected}"))
            .body(Body::from(
                json!({"token": token,"name":"owner"}).to_string(),
            ))
            .unwrap();
        let response = app.router.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status().as_u16(), expected);
    }
}

#[tokio::test]
async fn login_source_limit_does_not_block_another_client() {
    use axum::{extract::ConnectInfo, http::Request};
    use std::net::SocketAddr;
    use tower::ServiceExt;
    let app = app_with(|c| {
        c.login_starts_per_minute = 2;
        // Leave room for two first-source prompts and one second-source prompt.
        c.max_ceremonies = 3;
    });
    for (ip, expected) in [
        ("192.0.2.1:1000", 200),
        ("192.0.2.1:1000", 200),
        ("192.0.2.1:1000", 429),
        ("192.0.2.2:1000", 200),
    ] {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login/start")
            .extension(ConnectInfo(ip.parse::<SocketAddr>().unwrap()))
            .header("host", "localhost:8080")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "198.51.100.1")
            .body(Body::from("{}"))
            .unwrap();
        assert_eq!(
            app.router
                .clone()
                .oneshot(req)
                .await
                .unwrap()
                .status()
                .as_u16(),
            expected
        );
    }
}
