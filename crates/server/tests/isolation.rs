//! Two people on one server (#409): nothing one does with the other's
//! ids reaches the other, and the answer is the same "not found" a made-up
//! id gets. The whole-server suite (every action, every room) lives in
//! `crates/calternal/src/isolation.rs`; this file covers what needs real
//! sessions: devices, passkeys and the session cookie.

#[path = "account_common/mod.rs"]
mod common;

use axum::body::Body;
use common::*;
use serde_json::{Value, json};

/// Signs in a second person, made with `calternal user create`.
async fn second_person(app: &App, name: &str) -> String {
    let made = app.auth.create(name).await.unwrap();
    let start = post(
        app,
        "/api/v1/auth/setup/start",
        None,
        json!({ "token": made.token }),
    )
    .await;
    assert_eq!(start.status, 200, "{:?}", start.body);
    let mut dev = Device::new();
    let cred = dev.create(&start);
    let fin = post(
        app,
        "/api/v1/auth/setup/finish",
        None,
        json!({ "ceremony": start.body["ceremony"], "credential": cred, "device": "Phone" }),
    )
    .await;
    assert_eq!(fin.status, 200, "{:?}", fin.body);
    // Setup sessions are not a fresh sign-in; sign in for sensitive actions.
    login(app, &mut dev).await.session()
}

async fn result(app: &App, cookie: &str, name: &str) -> Value {
    let r = action(app, cookie, name, "", json!(null)).await;
    assert_eq!(r.status, 200, "{name}: {:?}", r.body);
    r.body["result"].clone()
}

/// Status and error code: all a client learns from a refusal.
fn outcome(r: &Res) -> (u16, String) {
    (r.status.as_u16(), r.code().to_owned())
}

#[tokio::test]
async fn devices_and_passkeys_of_another_person_are_not_found() {
    let app = app();
    let (alice, _) = first_owner(&app, "Alice").await;
    let bob = second_person(&app, "Bob").await;

    let a_install = result(&app, &alice, "account.installations").await["installations"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let a_passkey = result(&app, &alice, "account.passkeys").await["passkeys"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let made_up_install = uuid::Uuid::now_v7().to_string();
    let made_up_passkey = "AAAAAAAAAAAAAAAAAAAAAA".to_owned();

    let tries = |install: &str, passkey: &str| {
        vec![
            ("account.revoke", json!({ "installation": install })),
            (
                "account.passkey-rename",
                json!({ "id": passkey, "name": "mine now" }),
            ),
            ("account.passkey-remove", json!({ "id": passkey })),
        ]
    };
    for (n, ((name, real), (_, fake))) in tries(&a_install, &a_passkey)
        .into_iter()
        .zip(tries(&made_up_install, &made_up_passkey))
        .enumerate()
    {
        let r = action(&app, &bob, name, &format!("real-{n}"), real).await;
        let f = action(&app, &bob, name, &format!("fake-{n}"), fake).await;
        assert_eq!(
            outcome(&r),
            outcome(&f),
            "{name}: {:?} vs {:?}",
            r.body,
            f.body
        );
        assert_eq!(r.status, 404, "{name}: {:?}", r.body);
    }

    // Alice is still signed in (a refused revoke used to sign her out of
    // this process anyway), and her passkey is untouched.
    let me = get(&app, "/api/v1/auth/me", Some(&alice)).await;
    assert_eq!(me.status, 200, "{:?}", me.body);
    let keys = result(&app, &alice, "account.passkeys").await;
    assert_eq!(keys["passkeys"][0]["id"], a_passkey);
    assert_ne!(keys["passkeys"][0]["name"], "mine now");
    // Bob's own list never shows Alice's device.
    let his = result(&app, &bob, "account.installations").await;
    assert!(!his.to_string().contains(&a_install), "{his}");
}

#[tokio::test]
async fn another_persons_passkey_prompt_cannot_be_used_or_cancelled() {
    let app = app();
    let (alice, _) = first_owner(&app, "Alice").await;
    let bob = second_person(&app, "Bob").await;
    let start = action(&app, &alice, "account.passkey-add-start", "", json!(null)).await;
    assert_eq!(start.status, 200, "{:?}", start.body);
    let ceremony = start.body["result"]["ceremony"].clone();
    let flat = Res {
        status: start.status,
        cookie: None,
        body: start.body["result"].clone(),
    };
    let mut dev = Device::new();
    let cred = dev.create(&flat);

    let input = |c: &Value| json!({ "ceremony": c, "credential": cred, "name": "x" });
    let r = action(
        &app,
        &bob,
        "account.passkey-add-finish",
        "b1",
        input(&ceremony),
    )
    .await;
    let f = action(
        &app,
        &bob,
        "account.passkey-add-finish",
        "b2",
        input(&json!("0".repeat(32))),
    )
    .await;
    assert_eq!(outcome(&r), outcome(&f), "{:?} vs {:?}", r.body, f.body);
    assert_ne!(r.status, 200);

    // Alice's prompt is still there for her.
    let mine = action(
        &app,
        &alice,
        "account.passkey-add-finish",
        "a1",
        input(&ceremony),
    )
    .await;
    assert_eq!(mine.status, 200, "{:?}", mine.body);
}

#[tokio::test]
async fn a_cookie_sent_from_another_sites_page_does_not_count() {
    let app = app();
    let (alice, _) = first_owner(&app, "Alice").await;
    // A page on a sibling host (same site, so the browser sends the
    // cookie) tries the API and the live websocket.
    let evil = [
        ("cookie", alice.as_str()),
        ("origin", "http://preview.localhost:8080"),
    ];
    let r = send(&app, "GET", "/api/v1/describe", &evil, Body::empty()).await;
    assert_eq!(r.status, 401, "{:?}", r.body);
    let live = [
        ("cookie", alice.as_str()),
        ("origin", "http://preview.localhost:8080"),
        ("connection", "upgrade"),
        ("upgrade", "websocket"),
        ("sec-websocket-version", "13"),
        ("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="),
    ];
    let r = send(&app, "GET", "/api/v1/describe", &live, Body::empty()).await;
    assert_eq!(r.status, 401, "{:?}", r.body);
    let mut add = [
        ("idempotency-key", "k"),
        ("cookie", alice.as_str()),
        ("origin", "null"),
    ];
    let r = send(
        &app,
        "GET",
        "/api/v1/describe",
        &add,
        Body::from(r#"{"text":"hi"}"#),
    )
    .await;
    assert_eq!(r.status, 401, "{:?}", r.body);

    // From our own page the same cookie works.
    add[2] = ("origin", ORIGIN);
    let r = send(
        &app,
        "GET",
        "/api/v1/describe",
        &add,
        Body::from(r#"{"text":"hi"}"#),
    )
    .await;
    assert_eq!(r.status, 200, "{:?}", r.body);
    let r = get(&app, "/api/v1/describe", Some(&alice)).await;
    assert_eq!(r.status, 200, "{:?}", r.body);
}
