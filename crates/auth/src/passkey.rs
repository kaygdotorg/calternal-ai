//! Passkeys, through WebAuthn.
//!
//! Registration always asks for a discoverable passkey (one the device or
//! password manager remembers by itself), so signing in is one button: no
//! username to type. Every sign-in requires user verification (Face ID,
//! Touch ID, a PIN), and a passkey's signature counter must never go
//! backwards, which would hint at a cloned key.

use crate::Inner;
use crate::ceremony::Pending;
use crate::config::AuthConfig;
use crate::db::{now_ms, sql};
use crate::session::{self, InstallationKind};
use crate::setup::Redeemable;
use crate::{AccountId, Error, ErrorCode, InstallationId, Result};
use rusqlite::params;
use serde_json::Value;
use std::sync::Arc;
use webauthn_rs::prelude::*;

/// Builds the WebAuthn verifier for this host.
pub(crate) fn build(config: &AuthConfig) -> Result<Webauthn> {
    WebauthnBuilder::new(&config.rp_id, &config.origin)
        .and_then(|b| b.rp_name("Calternal").build())
        .map_err(|e| Error::internal(format!("passkey setup: {e}")))
}

fn failed(e: impl std::fmt::Display) -> Error {
    Error::new(ErrorCode::PasskeyFailed, e.to_string())
}

/// Starts registering a passkey for a checked setup token. Returns the
/// prompt id and the options for `navigator.credentials.create`.
pub(crate) async fn start_registration(
    inner: &Arc<Inner>,
    r: Redeemable,
) -> Result<(String, Value)> {
    let existing = existing_cred_ids(inner, r.user).await?;
    let username = crate::setup::username_for(&r.name);
    let (ccr, state) = inner
        .webauthn
        .start_passkey_registration(r.user.0, &username, &r.name, Some(existing))
        .map_err(Error::internal)?;
    let mut options = serde_json::to_value(&ccr).map_err(Error::internal)?;
    require_discoverable(&mut options);
    let id = inner.ceremonies.insert(Pending::Setup {
        state,
        token_hash: r.token_hash,
        user: r.user,
        name: r.name,
        first_owner: r.first_owner,
    })?;
    Ok((id, options))
}

/// Asks the browser for a discoverable passkey. The verifier doesn't care
/// either way; this only changes what the browser creates.
pub(crate) fn require_discoverable(options: &mut Value) {
    if let Some(sel) = options
        .pointer_mut("/publicKey/authenticatorSelection")
        .and_then(Value::as_object_mut)
    {
        sel.insert("residentKey".into(), "required".into());
        sel.insert("requireResidentKey".into(), true.into());
    }
}

/// Finishes registration: verifies the browser's answer, then uses up the
/// setup token and saves the passkey in one transaction.
pub(crate) async fn finish_registration(
    inner: &Arc<Inner>,
    account: AccountId,
    ceremony: &str,
    credential: Value,
    device: String,
) -> Result<(AccountId, InstallationId, String)> {
    let Pending::Setup {
        state,
        token_hash,
        user,
        name,
        first_owner,
    } = inner.ceremonies.take_setup(ceremony, account)?
    else {
        return Err(failed("prompt was not a registration"));
    };
    let reg: RegisterPublicKeyCredential = serde_json::from_value(credential).map_err(failed)?;
    let passkey = inner
        .webauthn
        .finish_passkey_registration(&reg, &state)
        .map_err(failed)?;
    let cred_id = passkey.cred_id().as_ref().to_vec();
    let json = serde_json::to_string(&passkey).map_err(Error::internal)?;
    let r = Redeemable {
        token_hash,
        user,
        name,
        first_owner,
    };
    let (installation, token) = crate::setup::redeem(inner, r, cred_id, json, device).await?;
    Ok((user, installation, token))
}

/// Starts a sign-in with any passkey for this host.
pub(crate) fn start_login(
    inner: &Arc<Inner>,
    user: AccountId,
    purpose: crate::LoginPurpose,
    source: &str,
) -> Result<(String, Value)> {
    let (rcr, state) = inner
        .webauthn
        .start_discoverable_authentication()
        .map_err(Error::internal)?;
    let mut options = serde_json::to_value(&rcr).map_err(Error::internal)?;
    // The sign-in screen has a button, so the browser should show its
    // passkey prompt right away instead of waiting in the autofill menu.
    if let Some(o) = options.as_object_mut() {
        o.remove("mediation");
    }
    let id = inner.ceremonies.insert(Pending::Login {
        state,
        user,
        purpose,
        source: source.to_owned(),
    })?;
    Ok((id, options))
}

/// Finishes a sign-in and creates a new Installation for this browser.
pub(crate) async fn finish_login(
    inner: &Arc<Inner>,
    ceremony: &str,
    credential: Value,
    device: String,
    expected_user: AccountId,
    purpose: crate::LoginPurpose,
) -> Result<(AccountId, InstallationId, String)> {
    let Pending::Login { state, .. } =
        inner
            .ceremonies
            .take_login(ceremony, expected_user, purpose)?
    else {
        return Err(failed("prompt was not a sign-in"));
    };
    let cred: PublicKeyCredential = serde_json::from_value(credential).map_err(failed)?;
    let (uuid, cred_id) = inner
        .webauthn
        .identify_discoverable_authentication(&cred)
        .map_err(failed)?;
    let user = AccountId(uuid);
    if expected_user != user {
        return Err(failed("passkey belongs to a different account"));
    }
    let cred_id = cred_id.to_vec();
    let mut keys = user_passkeys(inner, user).await?;
    let pos = keys
        .iter()
        .position(|k| k.cred_id().as_ref() == cred_id.as_slice())
        .ok_or_else(|| failed("unknown passkey"))?;
    if !inner.account_failures.ready(user.0) {
        return Err(Error::new(
            ErrorCode::TooManyRequests,
            "account sign-in back-off; try again shortly",
        ));
    }
    let discoverable: Vec<DiscoverableKey> = keys.iter().map(DiscoverableKey::from).collect();
    let result =
        match inner
            .webauthn
            .finish_discoverable_authentication(&cred, state, &discoverable)
        {
            Ok(result) => result,
            Err(e) => {
                return Err(if inner.account_failures.failed(user.0) {
                    Error::new(
                        ErrorCode::TooManyRequests,
                        "account sign-in back-off; try again shortly",
                    )
                } else {
                    failed(e)
                });
            }
        };
    inner.account_failures.clear(user.0);
    let changed = keys[pos].update_credential(&result) == Some(true);
    let json = changed
        .then(|| serde_json::to_string(&keys[pos]))
        .transpose()
        .map_err(Error::internal)?;
    let ttl = inner.config.session_ttl.as_millis() as i64;
    let (installation, token) = inner
        .db
        .run(move |c| {
            let tx = c.transaction().map_err(sql)?;
            let now = now_ms();
            match json {
                Some(j) => tx.execute(
                    "UPDATE passkeys SET passkey = ?2, last_used_at = ?3 WHERE cred_id = ?1",
                    params![cred_id, j, now],
                ),
                None => tx.execute(
                    "UPDATE passkeys SET last_used_at = ?2 WHERE cred_id = ?1",
                    params![cred_id, now],
                ),
            }
            .map_err(sql)?;
            let out = session::create(&tx, user, InstallationKind::Browser, &device, ttl, true)?;
            tx.commit().map_err(sql)?;
            Ok(out)
        })
        .await?;
    Ok((user, installation, token))
}

async fn user_passkeys(inner: &Arc<Inner>, user: AccountId) -> Result<Vec<Passkey>> {
    let u = user.to_string();
    let rows: Vec<String> = inner
        .db
        .run(move |c| {
            let mut st = c
                .prepare("SELECT passkey FROM passkeys WHERE user_id = ?1")
                .map_err(sql)?;
            let rows = st
                .query_map(params![u], |r| r.get::<_, String>(0))
                .map_err(sql)?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(sql)
        })
        .await?;
    rows.iter()
        .map(|j| serde_json::from_str(j).map_err(Error::internal))
        .collect()
}

pub(crate) async fn existing_cred_ids(
    inner: &Arc<Inner>,
    user: AccountId,
) -> Result<Vec<CredentialID>> {
    Ok(user_passkeys(inner, user)
        .await?
        .iter()
        .map(|k| k.cred_id().clone())
        .collect())
}
