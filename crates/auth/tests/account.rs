//! Library projections of the source setup/create/login/passkey/isolation cases.
mod common;
use calternal_auth::{ErrorCode, LoginPurpose};
use calternal_types::AccountId;
use common::*;
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn first_owner_signs_up_with_the_printed_token() {
    let app = app();
    let ((account, installation, token), _) = first_owner(&app, "  kayg ").await;
    assert_eq!(
        app.auth.find("kayg").await.unwrap(),
        (account, "kayg".into())
    );
    assert_eq!(
        app.auth.identify(&token).await,
        Some((account, installation))
    );
    assert_eq!(app.auth.first_owner_token().await.unwrap(), None);
    assert!(
        !app.dir.path().join("users").exists(),
        "identity enrollment creates no Home"
    );
}

#[tokio::test]
async fn a_restart_replaces_the_first_owner_token() {
    let app = app();
    let old = app.auth.first_owner_token().await.unwrap().unwrap();
    let new = app.auth.first_owner_token().await.unwrap().unwrap();
    assert_ne!(old, new);
    assert_eq!(
        app.auth
            .start_setup(&old, "k", "local")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::SetupTokenInvalid
    );
    assert!(
        app.auth
            .start_setup(&new.to_uppercase(), "k", "local")
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn first_owner_needs_a_name() {
    let app = app();
    let token = app.auth.first_owner_token().await.unwrap().unwrap();
    assert_eq!(
        app.auth
            .start_setup(&token, "  ", "local")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidInput
    );
}

#[tokio::test]
async fn an_expired_setup_token_is_refused() {
    let app = app_with(|c| c.setup_ttl = Duration::from_millis(30));
    let token = app.auth.first_owner_token().await.unwrap().unwrap();
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(
        app.auth
            .start_setup(&token, "k", "local")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::SetupTokenInvalid
    );
}

async fn storm(created: bool, n: usize) {
    let app = app();
    let token = if created {
        app.auth.create("Kayg").await.unwrap().token
    } else {
        app.auth.first_owner_token().await.unwrap().unwrap()
    };
    let mut finishes = Vec::new();
    for i in 0..n {
        let (account, id, options) = app
            .auth
            .start_setup(&token, &format!("p{i}"), "local")
            .await
            .unwrap();
        finishes.push((account, id, Device::new().create(&options)));
    }
    let mut tasks = Vec::new();
    for (account, id, cred) in finishes {
        let auth = app.auth.clone();
        tasks.push(tokio::spawn(async move {
            auth.finish_setup(account, &id, cred, "Mac").await
        }));
    }
    let mut ok = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(_) => ok += 1,
            Err(e) => assert_eq!(e.code(), ErrorCode::SetupTokenInvalid),
        }
    }
    assert_eq!(ok, 1, "exactly one finish may win the token");
    assert_eq!(app.auth.first_owner_token().await.unwrap(), None);
}
#[tokio::test]
async fn a_token_works_exactly_once_under_a_storm() {
    storm(false, 16).await;
}
#[tokio::test]
async fn an_enrolment_token_works_exactly_once_under_a_storm() {
    storm(true, 12).await;
}

#[tokio::test]
async fn a_finished_prompt_cannot_be_replayed() {
    let app = app();
    let token = app.auth.first_owner_token().await.unwrap().unwrap();
    let (account, id, options) = app.auth.start_setup(&token, "k", "local").await.unwrap();
    let cred = Device::new().create(&options);
    app.auth
        .finish_setup(account, &id, cred.clone(), "Mac")
        .await
        .unwrap();
    assert_eq!(
        app.auth
            .finish_setup(account, &id, cred, "Mac")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::PasskeyFailed
    );
}

#[tokio::test]
async fn recovery_adds_a_new_passkey_for_the_same_person() {
    let app = app();
    let (old, _) = first_owner(&app, "Kayg").await;
    assert_eq!(
        app.auth.recover("nobody").await.unwrap_err().code(),
        ErrorCode::NotFound
    );
    let (name, token) = app.auth.recover("kayg").await.unwrap();
    assert_eq!(name, "Kayg");
    let (new, mut phone) = enroll(&app, &token, "").await;
    assert_eq!(new.0, old.0);
    assert_eq!(login(&app, old.0, &mut phone).await.unwrap().0, old.0);
    assert_eq!(
        app.auth
            .start_setup(&token, "", "local")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::SetupTokenInvalid
    );
}

#[tokio::test]
async fn a_created_person_enrolls_their_first_passkey_and_is_the_same_person() {
    let app = app();
    let made = app.auth.create("Kayg").await.unwrap();
    assert_eq!(made.username, "kayg");
    assert!(AccountId::parse(&made.id.to_string()).is_ok());
    assert_eq!(app.auth.first_owner_token().await.unwrap(), None);
    let (signed, mut dev) = enroll(&app, &made.token, "").await;
    assert_eq!(signed.0, made.id);
    assert_eq!(app.auth.find("kayg").await.unwrap().1, "Kayg");
    assert_eq!(login(&app, made.id, &mut dev).await.unwrap().0, made.id);
    assert_eq!(
        app.auth
            .start_setup(&made.token, "", "local")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::SetupTokenInvalid
    );
    assert!(!app.dir.path().join("users").exists());
}

#[tokio::test]
async fn names_must_be_usable_and_unique() {
    let app = app();
    app.auth.create("Kayg").await.unwrap();
    assert_eq!(
        app.auth.create("KAYG").await.unwrap_err().code(),
        ErrorCode::InvalidInput
    );
    assert_eq!(
        app.auth.create("   ").await.unwrap_err().code(),
        ErrorCode::InvalidInput
    );
}

#[tokio::test]
async fn an_earlier_first_owner_token_stops_working() {
    let app = app();
    let first = app.auth.first_owner_token().await.unwrap().unwrap();
    app.auth.create("Kayg").await.unwrap();
    assert_eq!(
        app.auth
            .start_setup(&first, "stranger", "local")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::SetupTokenInvalid
    );
}

#[tokio::test]
async fn an_expired_enrolment_token_is_refused() {
    let app = app();
    let made = app.auth.create("Kayg").await.unwrap();
    db(&app)
        .execute("UPDATE setup_tokens SET expires_at = 0", [])
        .unwrap();
    assert_eq!(
        app.auth
            .start_setup(&made.token, "", "local")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::SetupTokenInvalid
    );
}

#[tokio::test]
async fn sign_in_with_a_passkey_creates_a_new_installation() {
    let app = app();
    let (laptop, mut dev) = first_owner(&app, "kayg").await;
    let phone = login(&app, laptop.0, &mut dev).await.unwrap();
    assert_ne!(laptop.2, phone.2);
    let list = app.auth.installations(laptop.0).await.unwrap();
    assert_eq!(list.len(), 2);
    let current: Vec<_> = list.iter().filter(|r| r.id == phone.1).collect();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].name, "Phone");
    assert_eq!(current[0].kind, calternal_auth::InstallationKind::Browser);
}

#[tokio::test]
async fn a_sign_in_answer_cannot_be_replayed() {
    let app = app();
    let (signed, mut dev) = first_owner(&app, "kayg").await;
    let (id, options) = app
        .auth
        .start_login(signed.0, LoginPurpose::SignIn, "local")
        .unwrap();
    let cred = dev.sign(&options);
    app.auth
        .finish_login(signed.0, LoginPurpose::SignIn, &id, cred.clone(), "Mac")
        .await
        .unwrap();
    assert_eq!(
        app.auth
            .finish_login(signed.0, LoginPurpose::SignIn, &id, cred.clone(), "Mac")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::PasskeyFailed
    );
    let (fresh, _) = app
        .auth
        .start_login(signed.0, LoginPurpose::SignIn, "local")
        .unwrap();
    assert_eq!(
        app.auth
            .finish_login(signed.0, LoginPurpose::SignIn, &fresh, cred, "Mac")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::PasskeyFailed
    );
}

#[tokio::test]
async fn an_unknown_passkey_is_refused() {
    let app = app();
    let (real, dev) = first_owner(&app, "kayg").await;
    let other = common::app();
    let (_, mut stranger) = first_owner(&other, "x").await;
    stranger.user = dev.user;
    assert_eq!(
        login(&app, real.0, &mut stranger).await.unwrap_err().code(),
        ErrorCode::PasskeyFailed
    );
}

#[tokio::test]
async fn sign_out_ends_the_session_immediately() {
    let app = app();
    let (signed, _) = first_owner(&app, "kayg").await;
    assert!(app.auth.identify(&signed.2).await.is_some()); // warm the cache
    app.auth.revoke(signed.0, signed.1).await.unwrap();
    assert_eq!(app.auth.identify(&signed.2).await, None); // next call
}

#[tokio::test]
async fn revoking_another_device_signs_it_out() {
    let app = app();
    let (laptop, mut dev) = first_owner(&app, "kayg").await;
    let phone = login(&app, laptop.0, &mut dev).await.unwrap();
    app.auth.identify(&phone.2).await.unwrap();
    app.auth.revoke(laptop.0, phone.1).await.unwrap();
    assert!(app.auth.identify(&phone.2).await.is_none());
    assert!(app.auth.identify(&laptop.2).await.is_some());
    assert_eq!(
        app.auth.revoke(laptop.0, phone.1).await.unwrap_err().code(),
        ErrorCode::NotFound
    );
}

#[tokio::test]
async fn sessions_expire_when_unused() {
    // An injected clock: expiry is decided by time passing, not by scheduling.
    let start = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let now = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(start));
    let clock = now.clone();
    let app = app_with(move |c| {
        c.session_ttl = Duration::from_secs(60);
        c.clock =
            calternal_auth::Clock::from_fn(move || clock.load(std::sync::atomic::Ordering::SeqCst));
    });
    let (signed, _) = first_owner(&app, "kayg").await;
    now.fetch_add(59_000, std::sync::atomic::Ordering::SeqCst);
    assert!(
        app.auth.identify(&signed.2).await.is_some(),
        "still live within the TTL"
    );
    now.fetch_add(60_001, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        app.auth.identify(&signed.2).await,
        None,
        "expired after an idle TTL"
    );
}

#[tokio::test]
async fn the_first_passkey_is_listed_with_the_device_it_was_made_on() {
    let app = app();
    let (signed, dev) = first_owner(&app, "kayg").await;
    let list = app.auth.passkeys(signed.0).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, dev.cred_id.unwrap());
    assert_eq!(list[0].name, "Test browser");
    assert!(list[0].created_at > 0);
    assert_eq!(list[0].last_used_at, None);
}

#[tokio::test]
async fn setup_registration_allows_a_second_passkey() {
    let app = app();
    let (signed, _) = first_owner(&app, "Alice").await;
    add(&app, &signed, "Phone").await;
    assert_eq!(app.auth.passkeys(signed.0).await.unwrap().len(), 2);
}

#[tokio::test]
async fn a_session_without_a_passkey_ceremony_needs_reverification() {
    let app = app();
    let (signed, _) = first_owner(&app, "Alice").await;
    db(&app)
        .execute("UPDATE installations SET assertion_at = NULL", [])
        .unwrap();
    assert_eq!(
        app.auth
            .start_add_passkey(signed.0, signed.1)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::PasskeyReverificationRequired
    );
}

#[tokio::test]
async fn reverification_requires_the_current_accounts_passkey() {
    let app = app();
    let (signed, mut owner) = first_owner(&app, "Alice").await;
    assert!(
        app.auth
            .require_fresh(signed.0, signed.1, true)
            .await
            .is_ok()
    );
    assert_eq!(
        app.auth
            .require_fresh(signed.0, signed.1, false)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::PasskeyReverificationRequired
    );
    let (id, options) = app
        .auth
        .start_login(signed.0, LoginPurpose::Reverify, "local")
        .unwrap();
    let fresh = app
        .auth
        .finish_login(
            signed.0,
            LoginPurpose::Reverify,
            &id,
            owner.sign(&options),
            "Mac",
        )
        .await
        .unwrap();
    app.auth
        .require_fresh(fresh.0, fresh.1, false)
        .await
        .unwrap();
    let registration: bool = db(&app)
        .query_row(
            "SELECT registration_session FROM installations WHERE id=?1",
            [fresh.1.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!registration);
    let made = app.auth.create("Bob").await.unwrap();
    let (_, mut other) = enroll(&app, &made.token, "").await;
    let (id, options) = app
        .auth
        .start_login(signed.0, LoginPurpose::Reverify, "local")
        .unwrap();
    assert_eq!(
        app.auth
            .finish_login(
                signed.0,
                LoginPurpose::Reverify,
                &id,
                other.sign(&options),
                "Mac"
            )
            .await
            .unwrap_err()
            .code(),
        ErrorCode::PasskeyFailed
    );
    assert_eq!(app.auth.identify(&fresh.2).await.unwrap().0, signed.0);
}

#[tokio::test]
async fn the_last_passkey_cannot_be_removed() {
    let app = app();
    let (signed, mut dev) = first_owner(&app, "kayg").await;
    let only = dev.cred_id.clone().unwrap();
    assert_eq!(
        app.auth
            .remove_passkey(signed.0, signed.1, &only)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::LastPasskey
    );
    assert_eq!(app.auth.passkeys(signed.0).await.unwrap().len(), 1);
    let (_, second) = add(&app, &signed, "Laptop").await;
    app.auth
        .remove_passkey(signed.0, signed.1, &only)
        .await
        .unwrap();
    assert_eq!(
        app.auth
            .remove_passkey(signed.0, signed.1, &second.id)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::LastPasskey
    );
    assert_eq!(
        login(&app, signed.0, &mut dev).await.unwrap_err().code(),
        ErrorCode::PasskeyFailed
    );
}

#[tokio::test]
async fn a_removed_passkey_that_is_gone_or_unknown_is_not_found() {
    let app = app();
    let (signed, _) = first_owner(&app, "kayg").await;
    add(&app, &signed, "Laptop").await;
    assert_eq!(
        app.auth
            .remove_passkey(signed.0, signed.1, "AAAA")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
    assert_eq!(
        app.auth
            .rename_passkey(signed.0, "AAAA", "x")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
    assert_eq!(app.auth.passkeys(signed.0).await.unwrap().len(), 2);
}

#[tokio::test]
async fn an_empty_name_is_refused_and_a_long_one_is_cut() {
    let app = app();
    let (signed, dev) = first_owner(&app, "kayg").await;
    let id = dev.cred_id.unwrap();
    assert_eq!(
        app.auth
            .rename_passkey(signed.0, &id, "   ")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidInput
    );
    app.auth
        .rename_passkey(signed.0, &id, &"n".repeat(300))
        .await
        .unwrap();
    assert_eq!(app.auth.passkeys(signed.0).await.unwrap()[0].name.len(), 40);
}

#[tokio::test]
async fn an_add_prompt_works_once_and_login_cannot_be_used_to_add() {
    let app = app();
    let (signed, _) = first_owner(&app, "kayg").await;
    let (id, options) = app
        .auth
        .start_add_passkey(signed.0, signed.1)
        .await
        .unwrap();
    let cred = Device::new().create(&options);
    app.auth
        .finish_add_passkey(signed.0, signed.1, &id, cred.clone(), "Phone")
        .await
        .unwrap();
    assert_eq!(
        app.auth
            .finish_add_passkey(signed.0, signed.1, &id, cred, "Phone")
            .await
            .err()
            .unwrap()
            .code(),
        ErrorCode::PasskeyFailed
    );
    let (login, _) = app
        .auth
        .start_login(signed.0, LoginPurpose::SignIn, "local")
        .unwrap();
    assert_eq!(
        app.auth
            .finish_add_passkey(signed.0, signed.1, &login, json!({}), "x")
            .await
            .err()
            .unwrap()
            .code(),
        ErrorCode::PasskeyFailed
    );
    assert_eq!(app.auth.passkeys(signed.0).await.unwrap().len(), 2);
}

#[tokio::test]
async fn the_same_device_cannot_be_added_twice() {
    let app = app();
    let (signed, dev) = first_owner(&app, "kayg").await;
    let (_, options) = app
        .auth
        .start_add_passkey(signed.0, signed.1)
        .await
        .unwrap();
    assert_eq!(
        options["publicKey"]["excludeCredentials"][0]["id"],
        dev.cred_id.unwrap()
    );
}

// New adaptation regression cases below.
#[tokio::test]
async fn sessions_carry_only_account_and_installation_identity() {
    let app = app();
    let (signed, _) = first_owner(&app, "kayg").await;
    let identity = serde_json::to_value(app.auth.identify(&signed.2).await.unwrap()).unwrap();
    assert_eq!(identity, json!([signed.0, signed.1]));
}

#[tokio::test]
async fn concurrent_last_key_removal_is_guarded_inside_the_transaction() {
    let app = app();
    let (signed, dev) = first_owner(&app, "kayg").await;
    let (_, second) = add(&app, &signed, "Phone").await;
    // Use two services/connections, so the database transaction protects the
    // invariant even outside the single worker's serialization.
    let other = calternal_auth::Auth::open(
        app.dir.path(),
        calternal_auth::AuthConfig::new(ORIGIN).unwrap(),
    )
    .unwrap();
    let first_id = dev.cred_id.unwrap();
    let (a, b) = tokio::join!(
        app.auth.remove_passkey(signed.0, signed.1, &first_id),
        other.remove_passkey(signed.0, signed.1, &second.id)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(a.err().or(b.err()).unwrap().code(), ErrorCode::LastPasskey);
    assert_eq!(app.auth.passkeys(signed.0).await.unwrap().len(), 1);
}

#[tokio::test]
async fn ceremony_account_and_purpose_binding_preserves_rightful_prompt() {
    let app = app();
    let (signed, mut dev) = first_owner(&app, "kayg").await;
    let (id, options) = app
        .auth
        .start_login(signed.0, LoginPurpose::Reverify, "local")
        .unwrap();
    let cred = dev.sign(&options);
    assert_eq!(
        app.auth
            .finish_login(
                AccountId::new(),
                LoginPurpose::Reverify,
                &id,
                cred.clone(),
                "Mac"
            )
            .await
            .unwrap_err()
            .code(),
        ErrorCode::PasskeyFailed
    );
    assert_eq!(
        app.auth
            .finish_login(signed.0, LoginPurpose::SignIn, &id, cred.clone(), "Mac")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::PasskeyFailed
    );
    app.auth
        .finish_login(signed.0, LoginPurpose::Reverify, &id, cred, "Mac")
        .await
        .unwrap();
    let (id, opts) = app
        .auth
        .start_add_passkey(signed.0, signed.1)
        .await
        .unwrap();
    let cred = Device::new().create(&opts);
    let made = app.auth.create("Bob").await.unwrap();
    let (bob, _) = enroll(&app, &made.token, "").await;
    assert_eq!(
        app.auth
            .finish_add_passkey(bob.0, bob.1, &id, cred.clone(), "x")
            .await
            .err()
            .unwrap()
            .code(),
        ErrorCode::PasskeyFailed
    );
    app.auth
        .finish_add_passkey(signed.0, signed.1, &id, cred, "Phone")
        .await
        .unwrap();
}

#[tokio::test]
async fn stale_foreign_and_revoked_proof_is_refused() {
    let app = app();
    let (signed, _) = first_owner(&app, "kayg").await;
    assert!(
        app.auth
            .require_fresh(AccountId::new(), signed.1, true)
            .await
            .is_err()
    );
    db(&app)
        .execute("UPDATE installations SET assertion_at=0", [])
        .unwrap();
    assert_eq!(
        app.auth
            .require_fresh(signed.0, signed.1, true)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::PasskeyReverificationRequired
    );
    app.auth.revoke(signed.0, signed.1).await.unwrap();
    assert_eq!(
        app.auth
            .require_fresh(signed.0, signed.1, true)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Forbidden
    );
}

#[tokio::test]
async fn foreign_revoke_does_not_fence_the_owners_session_or_keys() {
    let app = app();
    let (signed, dev) = first_owner(&app, "Alice").await;
    assert!(app.auth.identify(&signed.2).await.is_some());
    let other = app.auth.create("Bob").await.unwrap();
    assert_eq!(
        app.auth
            .revoke(other.id, signed.1)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
    assert!(app.auth.identify(&signed.2).await.is_some());
    assert_eq!(
        app.auth
            .rename_passkey(other.id, &dev.cred_id.unwrap(), "stolen")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
    assert!(app.auth.passkeys(other.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn ceremony_expires_and_zero_capacity_disables_prompts() {
    let app = app_with(|c| c.ceremony_ttl = Duration::ZERO);
    let made = app.auth.create("Alice").await.unwrap();
    let (account, id, options) = app
        .auth
        .start_setup(&made.token, "", "local")
        .await
        .unwrap();
    assert_eq!(
        app.auth
            .finish_setup(account, &id, Device::new().create(&options), "Mac")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::PasskeyFailed
    );
    let disabled = app_with(|c| c.max_ceremonies = 0);
    assert_eq!(
        disabled
            .auth
            .start_login(AccountId::new(), LoginPurpose::SignIn, "local")
            .unwrap_err()
            .code(),
        ErrorCode::TooManyRequests
    );
}

#[tokio::test]
async fn devices_and_passkeys_of_another_person_are_not_found() {
    let app = app();
    let (alice, _) = first_owner(&app, "Alice").await;
    let made = app.auth.create("Bob").await.unwrap();
    let (bob, _) = enroll(&app, &made.token, "").await;
    let alice_key = app.auth.passkeys(alice.0).await.unwrap()[0].id.clone();
    let fake_install = calternal_types::InstallationId::new();
    let fake_key = "AAAAAAAAAAAAAAAAAAAAAA";
    let real = app.auth.revoke(bob.0, alice.1).await.unwrap_err();
    let fake = app.auth.revoke(bob.0, fake_install).await.unwrap_err();
    assert_eq!(real.code(), fake.code());
    assert_eq!(real.code(), ErrorCode::NotFound);
    let real = app
        .auth
        .rename_passkey(bob.0, &alice_key, "mine now")
        .await
        .unwrap_err();
    let fake = app
        .auth
        .rename_passkey(bob.0, fake_key, "mine now")
        .await
        .unwrap_err();
    assert_eq!(real.code(), fake.code());
    assert_eq!(real.code(), ErrorCode::NotFound);
    let real = app
        .auth
        .remove_passkey(bob.0, bob.1, &alice_key)
        .await
        .unwrap_err();
    let fake = app
        .auth
        .remove_passkey(bob.0, bob.1, fake_key)
        .await
        .unwrap_err();
    assert_eq!(real.code(), fake.code());
    assert_eq!(real.code(), ErrorCode::NotFound);
    assert!(app.auth.identify(&alice.2).await.is_some());
    let keys = app.auth.passkeys(alice.0).await.unwrap();
    assert_eq!(keys[0].id, alice_key);
    assert_ne!(keys[0].name, "mine now");
    assert!(
        app.auth
            .installations(bob.0)
            .await
            .unwrap()
            .iter()
            .all(|i| i.id != alice.1)
    );
}

#[tokio::test]
async fn another_persons_passkey_prompt_cannot_be_used_or_cancelled() {
    let app = app();
    let (alice, _) = first_owner(&app, "Alice").await;
    let made = app.auth.create("Bob").await.unwrap();
    let (bob, _) = enroll(&app, &made.token, "").await;
    let (id, opts) = app.auth.start_add_passkey(alice.0, alice.1).await.unwrap();
    let cred = Device::new().create(&opts);
    let real = app
        .auth
        .finish_add_passkey(bob.0, bob.1, &id, cred.clone(), "x")
        .await
        .err()
        .unwrap();
    let fake = app
        .auth
        .finish_add_passkey(bob.0, bob.1, &"0".repeat(32), cred.clone(), "x")
        .await
        .err()
        .unwrap();
    assert_eq!(real.code(), fake.code());
    app.auth
        .finish_add_passkey(alice.0, alice.1, &id, cred, "x")
        .await
        .unwrap();
}

#[tokio::test]
async fn library_add_rename_and_remove_preserve_key_metadata() {
    let app = app();
    let (signed, _) = first_owner(&app, "Alice").await;
    let (_, phone) = add(&app, &signed, "  iPhone  ").await;
    let rows = app.auth.passkeys(signed.0).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].name, "iPhone");
    app.auth
        .rename_passkey(signed.0, &phone.id, "Mum's iPhone")
        .await
        .unwrap();
    assert_eq!(
        app.auth.passkeys(signed.0).await.unwrap()[1].name,
        "Mum's iPhone"
    );
    app.auth
        .remove_passkey(signed.0, signed.1, &phone.id)
        .await
        .unwrap();
    assert_eq!(app.auth.passkeys(signed.0).await.unwrap().len(), 1);
}

#[tokio::test]
async fn setup_token_expiry_is_rechecked_at_redemption_and_rolls_back() {
    let app = app();
    let token = app.auth.first_owner_token().await.unwrap().unwrap();
    let (account, id, opts) = app
        .auth
        .start_setup(&token, "Alice", "local")
        .await
        .unwrap();
    db(&app)
        .execute("UPDATE setup_tokens SET expires_at=0", [])
        .unwrap();
    assert_eq!(
        app.auth
            .finish_setup(account, &id, Device::new().create(&opts), "Mac")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::SetupTokenInvalid
    );
    let counts: (i64,i64,i64) = db(&app).query_row("SELECT (SELECT count(*) FROM users),(SELECT count(*) FROM passkeys),(SELECT count(*) FROM installations)", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(counts, (0, 0, 0));
}

#[tokio::test]
async fn setup_and_login_admission_preserve_source_and_verified_target_limits() {
    let app = app_with(|c| {
        c.setup_failures_per_minute = 1;
        c.login_starts_per_minute = 1;
    });
    let token = app.auth.first_owner_token().await.unwrap().unwrap();
    // Junk charges only its source, never a real token's target bucket.
    assert_eq!(
        app.auth
            .start_setup("junk", "Alice", "junk")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::SetupTokenInvalid
    );
    assert!(app.auth.start_setup(&token, "Alice", "good").await.is_ok());
    assert_eq!(
        app.auth
            .start_setup(&token, "Alice", "another")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::TooManyRequests
    );
    assert_eq!(
        app.auth
            .start_setup(&token, "Alice", "good")
            .await
            .unwrap_err()
            .code(),
        ErrorCode::TooManyRequests
    );
    let account = AccountId::new();
    app.auth
        .start_login(account, LoginPurpose::SignIn, "good")
        .unwrap();
    assert_eq!(
        app.auth
            .start_login(account, LoginPurpose::SignIn, "good")
            .unwrap_err()
            .code(),
        ErrorCode::TooManyRequests
    );
}
