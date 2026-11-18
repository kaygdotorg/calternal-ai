//! Account enrollment, private email challenge and recovery regressions.
mod common;

use calternal_auth::Clock;
use common::*;
use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

#[tokio::test]
async fn hosted_enrollment_requires_a_code_bound_to_the_signup_token() {
    let app = app();
    let setup = app.auth.first_owner_token().await.unwrap().unwrap();
    let (challenge, mail) = app
        .auth
        .start_account_email(None, Some(&setup), "Owner@EXAMPLE.COM", "verify", true)
        .await
        .unwrap();
    let mail = mail.unwrap();
    assert_eq!(mail.to, "Owner@example.com");
    let id = challenge["challenge_id"].as_str().unwrap();
    assert!(
        app.auth
            .confirm_account_email(None, Some(&setup), id, "wrong", "verify")
            .await
            .is_err()
    );
    let (verified, _) = app
        .auth
        .confirm_account_email(None, Some(&setup), id, &mail.code.unwrap(), "verify")
        .await
        .unwrap();
    let verification = verified["verification_id"].as_str().unwrap();

    assert!(
        app.auth
            .start_setup_profile(
                &setup,
                "Owner",
                Some("owner-one"),
                None,
                None,
                true,
                "signup"
            )
            .await
            .is_err()
    );
    let (a, ceremony, options) = app
        .auth
        .start_setup_profile(
            &setup,
            "Owner",
            Some("Owner-One"),
            Some("Owner@example.com"),
            Some(verification),
            true,
            "signup",
        )
        .await
        .unwrap();
    let (account, _, _, codes) = app
        .auth
        .finish_http_setup(&ceremony, Device::new().create(&options), "Laptop")
        .await
        .unwrap();
    assert_eq!(a, account);
    assert_eq!(codes.len(), 10);
    assert_eq!(
        app.auth.account_profile(a).await.unwrap()["username"],
        "owner-one"
    );
    let status = app.auth.account_security_status(a).await.unwrap();
    assert_eq!(status["email_status"], "verified");
    assert_eq!(status["second_passkey_prompt"], true);
    app.auth.dismiss_second_passkey(a).await.unwrap();
    assert_eq!(
        app.auth.account_security_status(a).await.unwrap()["second_passkey_prompt"],
        false
    );
}

#[tokio::test]
async fn recovery_code_rotates_access_without_creating_a_session() {
    let app = app();
    let setup = app.auth.first_owner_token().await.unwrap().unwrap();
    let (a, ceremony, options) = app
        .auth
        .start_setup_profile(
            &setup,
            "Owner",
            Some("first-owner"),
            None,
            None,
            false,
            "signup",
        )
        .await
        .unwrap();
    let mut old = Device::new();
    let (_, _, session, codes) = app
        .auth
        .finish_http_setup(&ceremony, old.create(&options), "Laptop")
        .await
        .unwrap();
    assert!(app.auth.identify(&session).await.is_some());

    let (flow, _) = app
        .auth
        .start_recovery("code", None, "backup", false)
        .await
        .unwrap();
    let flow = flow["flow_id"].as_str().unwrap();
    let (proof, _) = app
        .auth
        .prove_recovery(flow, &codes[0], "backup", false)
        .await
        .unwrap();
    assert!(
        app.auth
            .prove_recovery(flow, &codes[0], "backup", false)
            .await
            .is_err()
    );
    let grant = proof["enrollment_grant"].as_str().unwrap();
    let (ceremony, options) = app.auth.start_replacement(grant, "backup").await.unwrap();
    let mut replacement = Device::new();
    app.auth
        .finish_replacement(&ceremony, replacement.create(&options))
        .await
        .unwrap();

    assert!(app.auth.identify(&session).await.is_none());
    assert!(app.auth.start_replacement(grant, "backup").await.is_err());
    assert!(
        app.auth
            .prove_recovery(flow, &codes[1], "backup", false)
            .await
            .is_err()
    );
    assert!(login(&app, a, &mut old).await.is_err());
    let (id, options) = app.auth.start_http_login(None, "backup").unwrap();
    let (same, _, _, new_codes) = app
        .auth
        .finish_http_login(None, &id, replacement.sign(&options), "New laptop")
        .await
        .unwrap();
    assert_eq!(same, a);
    assert_eq!(new_codes.len(), 10);
    assert_ne!(new_codes, codes);
    let (id, options) = app.auth.start_http_login(None, "backup").unwrap();
    assert!(
        app.auth
            .finish_http_login(None, &id, replacement.sign(&options), "New laptop")
            .await
            .unwrap()
            .3
            .is_empty()
    );
}

#[tokio::test]
async fn email_recovery_waits_for_claim_and_allows_the_owner_to_veto() {
    let clock = Arc::new(AtomicI64::new(calternal_types::now_ms()));
    let now = clock.clone();
    let app = app_with(|c| c.clock = Clock::from_fn(move || now.load(Ordering::Relaxed)));
    let (first, mut device) = first_owner(&app, "Owner").await;
    let (a, i, _) = login(&app, first.0, &mut device).await.unwrap();
    db(&app)
        .execute(
            "INSERT INTO account_details(user_id,email,verified_at) VALUES(?1,?2,?3)",
            rusqlite::params![
                a.to_string(),
                "owner@example.com",
                calternal_types::now_ms()
            ],
        )
        .unwrap();
    let (flow, message) = app
        .auth
        .start_recovery("email", Some("owner@example.com"), "first", true)
        .await
        .unwrap();
    let id = flow["flow_id"].as_str().unwrap();
    let code = message.unwrap().code.unwrap();
    app.auth
        .prove_recovery(id, &code, "proof", true)
        .await
        .unwrap();
    assert_eq!(
        app.auth.recovery_status(id, "proof", true).await.unwrap().0["stage"],
        "waiting"
    );
    assert!(
        app.auth
            .prove_recovery(id, &code, "proof", true)
            .await
            .is_err()
    );
    assert_eq!(
        app.auth.account_security_status(a).await.unwrap()["email_recovery_pending"],
        true
    );
    assert!(app.auth.veto_recovery(a, i).await.unwrap());
    assert_eq!(
        app.auth.account_security_status(a).await.unwrap()["email_recovery_pending"],
        false
    );
    assert_eq!(
        app.auth.recovery_status(id, "proof", true).await.unwrap().0["stage"],
        "proof"
    );

    let (flow, message) = app
        .auth
        .start_recovery("email", Some("owner@example.com"), "second", true)
        .await
        .unwrap();
    let id = flow["flow_id"].as_str().unwrap();
    let (hold, _) = app
        .auth
        .prove_recovery(id, &message.unwrap().code.unwrap(), "proof", true)
        .await
        .unwrap();
    clock.store(hold["hold_until"].as_i64().unwrap() + 1, Ordering::Relaxed);
    let (state, mail) = app.auth.recovery_status(id, "claim", true).await.unwrap();
    assert_eq!(state["stage"], "claim");
    let (proof, _) = app
        .auth
        .prove_recovery(id, &mail.unwrap().code.unwrap(), "proof", true)
        .await
        .unwrap();
    assert!(proof["enrollment_grant"].is_string());
}

#[tokio::test]
async fn local_operator_grant_works_without_mail_and_is_audited() {
    let app = app();
    let ((account, _, _), _) = first_owner(&app, "Owner").await;
    let grant = app
        .auth
        .operator_recovery(account, "mail_unavailable")
        .await
        .unwrap();
    let (id, opts) = app
        .auth
        .start_replacement(&grant, "operator")
        .await
        .unwrap();
    assert!(
        app.auth
            .finish_replacement(&id, Device::new().create(&opts))
            .await
            .unwrap()
            .is_none()
    );
    let history = app.auth.recovery_history(account).await.unwrap();
    assert_eq!(history[0]["action"], "completed");
    assert_eq!(history[1]["action"], "mail_unavailable");
}
