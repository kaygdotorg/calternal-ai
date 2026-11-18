//! Verified provider identity exercises the protected account authority.
mod common;

use calternal_auth::{ErrorCode, OidcProvider, OidcSignup};
use calternal_secrets::SecretValue;
use common::*;

fn provider() -> OidcProvider {
    OidcProvider {
        id: "company".into(),
        preset: "generic".into(),
        label: "Company".into(),
        issuer: "https://id.example.com".into(),
        client_id: "client".into(),
        scopes: vec!["profile".into()],
        display_name_claim: Some("name".into()),
    }
}

#[tokio::test]
async fn a_provider_credential_is_sealed_and_an_unknown_subject_requires_an_invite() {
    let app = app();
    let (initial, mut device) = first_owner(&app, "Owner").await;
    let (account, installation, _) = login(&app, initial.0, &mut device).await.unwrap();
    let caller = (account, installation);
    let secret = b"private-client-credential";
    app.auth
        .configure_oidc(
            caller,
            provider(),
            Some(SecretValue::new(secret.to_vec()).unwrap()),
        )
        .await
        .unwrap();
    app.auth
        .set_oidc_provider_state(caller, "company", "enable", false)
        .await
        .unwrap();
    let (p, revision) = app.auth.oidc_provider("company").await.unwrap();
    assert_eq!(
        app.auth
            .oidc_credential(&p, &revision)
            .await
            .unwrap()
            .expose(),
        secret
    );
    let encrypted: Vec<u8> = db(&app)
        .query_row("SELECT secret FROM oidc_providers", [], |r| r.get(0))
        .unwrap();
    assert!(!encrypted.windows(secret.len()).any(|bytes| bytes == secret));

    let refusal = app
        .auth
        .finish_oidc(
            p.id,
            revision,
            "foreign-subject".into(),
            Some("Owner".into()),
            None,
            None,
            None,
            "Browser".into(),
        )
        .await
        .unwrap_err();
    assert_eq!(refusal.code(), ErrorCode::SetupTokenInvalid);
}

#[tokio::test]
async fn only_a_fresh_settings_link_allows_provider_signin_and_unlink_revokes_it() {
    let app = app();
    let (initial, mut device) = first_owner(&app, "Owner").await;
    let (account, installation, _) = login(&app, initial.0, &mut device).await.unwrap();
    let admin = (account, installation);
    app.auth
        .configure_oidc(
            admin,
            provider(),
            Some(SecretValue::new(b"private".to_vec()).unwrap()),
        )
        .await
        .unwrap();
    app.auth
        .set_oidc_provider_state(admin, "company", "enable", false)
        .await
        .unwrap();
    let (p, revision) = app.auth.oidc_provider("company").await.unwrap();

    assert!(
        app.auth
            .finish_oidc(
                p.id.clone(),
                revision.clone(),
                "subject".into(),
                None,
                None,
                Some((account, initial.1)),
                None,
                "Browser".into()
            )
            .await
            .is_err()
    );
    assert!(
        app.auth
            .finish_oidc(
                p.id.clone(),
                revision.clone(),
                "subject".into(),
                None,
                None,
                Some(admin),
                None,
                "Browser".into()
            )
            .await
            .unwrap()
            .is_none()
    );
    let (same, install, session, _) = app
        .auth
        .finish_oidc(
            p.id.clone(),
            revision.clone(),
            "subject".into(),
            None,
            None,
            None,
            None,
            "Browser".into(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(same, account);
    assert!(app.auth.require_fresh(same, install, false).await.is_err());
    assert_eq!(
        app.auth
            .oidc_session((same, install))
            .await
            .unwrap()
            .unwrap()
            .provider,
        "company"
    );
    assert!(
        app.auth
            .finish_oidc(
                p.id.clone(),
                revision.clone(),
                "wrong-subject".into(),
                None,
                None,
                None,
                Some((same, install)),
                "Browser".into()
            )
            .await
            .is_err()
    );
    let (same, fresh, fresh_token, _) = app
        .auth
        .finish_oidc(
            p.id.clone(),
            revision.clone(),
            "subject".into(),
            None,
            None,
            None,
            Some((same, install)),
            "Browser".into(),
        )
        .await
        .unwrap()
        .unwrap();
    app.auth.require_fresh(same, fresh, false).await.unwrap();
    assert!(app.auth.identify(&session).await.is_some());
    assert!(app.auth.identify(&fresh_token).await.is_some());
    app.auth.unlink_oidc(admin, &p.id).await.unwrap();
    assert!(app.auth.identify(&session).await.is_none());
    assert!(app.auth.identify(&fresh_token).await.is_none());
    assert!(
        app.auth
            .finish_oidc(
                p.id,
                revision,
                "subject".into(),
                None,
                None,
                None,
                None,
                "Browser".into()
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn invited_oidc_signup_offers_a_passkey_and_has_a_last_method_guard() {
    let app = app();
    let (initial, mut device) = first_owner(&app, "Owner").await;
    let (owner, installation, _) = login(&app, initial.0, &mut device).await.unwrap();
    let admin = (owner, installation);
    app.auth
        .configure_oidc(
            admin,
            provider(),
            Some(SecretValue::new(b"private".to_vec()).unwrap()),
        )
        .await
        .unwrap();
    app.auth
        .set_oidc_provider_state(admin, "company", "enable", false)
        .await
        .unwrap();
    let (p, revision) = app.auth.oidc_provider("company").await.unwrap();
    let directory = db(&app);
    let space: String = directory
        .query_row(
            "SELECT id FROM spaces WHERE owner_account=?1",
            [owner.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    let space = calternal_types::SpaceId::parse(&space).unwrap();
    let admin_auth = app.auth.clone();
    let token = tokio::task::spawn_blocking(move || {
        let invite = admin_auth
            .create_invite(
                owner,
                space,
                "editor".into(),
                None,
                ("oidc-invite", "test", "{}"),
            )
            .unwrap();
        let invite_id = invite["invite_id"].as_str().unwrap();
        let listing = admin_auth.list_invites(owner, space, None).unwrap();
        let link = listing["invites"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["invite_id"] == invite_id)
            .unwrap()["link"]
            .as_str()
            .unwrap();
        link.rsplit('/').next().unwrap().to_owned()
    })
    .await
    .unwrap();
    let draft = OidcSignup {
        token,
        name: Some("Provider Person".into()),
        username: "provider-person".into(),
        email: None,
        verification_id: None,
    };
    let (user, _, token, codes) = app
        .auth
        .finish_oidc(
            p.id.clone(),
            revision.clone(),
            "new-subject".into(),
            None,
            Some(draft),
            None,
            None,
            "Browser".into(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_ne!(user, owner);
    assert_eq!(codes.len(), 10);
    assert!(app.auth.offer_first_passkey(user).await.unwrap());
    assert_eq!(app.auth.passkeys(user).await.unwrap().len(), 0);
    assert!(
        app.auth
            .set_oidc_provider_state(admin, &p.id, "disable", false)
            .await
            .is_err()
    );
    app.auth
        .set_oidc_provider_state(admin, &p.id, "disable", true)
        .await
        .unwrap();
    assert!(app.auth.identify(&token).await.is_none());
    assert!(
        app.auth
            .finish_oidc(
                p.id,
                revision,
                "new-subject".into(),
                None,
                None,
                None,
                None,
                "Browser".into()
            )
            .await
            .is_err()
    );
}
