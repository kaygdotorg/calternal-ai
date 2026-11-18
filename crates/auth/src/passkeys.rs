//! Managing your own passkeys, once you are signed in.
//!
//! These are the actions behind Settings → Account → Passkeys. Adding one is
//! two steps, like every WebAuthn prompt: `start` hands the browser its
//! options, `finish` checks the answer. The open prompt is tied to the
//! person who started it, so nobody can finish somebody else's.
//!
//! The one rule: an account always keeps at least one passkey. Removing the
//! last would lock everyone out (only `calternal user recover` could fix it),
//! so the count and the delete happen in one transaction.
//!
//! A passkey is named by its credential id, base64url without padding.
//! Names are ours, not the device's, and anyone can rename them.

use crate::Inner;
use crate::ceremony::Pending;
use crate::db::{now_ms, sql};
use crate::{AccountId, Error, ErrorCode, Result};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rusqlite::params;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use webauthn_rs::prelude::*;

/// Longest passkey name kept.
const MAX_NAME: usize = 40;

/// One passkey as Settings shows it. Times are milliseconds since the epoch.
#[derive(Serialize)]
pub struct PasskeyInfo {
    /// Base64url credential identifier.
    pub id: String,
    /// Printable display label.
    pub name: String,
    /// Registration time, Unix milliseconds.
    pub created_at: i64,
    /// Last assertion time, Unix milliseconds, if used.
    pub last_used_at: Option<i64>,
}

/// A name as typed: printable, trimmed, short. Empty becomes `fallback`.
pub(crate) fn clean_label(name: &str, fallback: &str) -> String {
    let s: String = name
        .chars()
        .filter(|c| !crate::session::hidden_char(*c))
        .take(MAX_NAME)
        .collect();
    let s = s.trim();
    if s.is_empty() {
        fallback.into()
    } else {
        s.into()
    }
}

fn id_of(cred_id: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(cred_id)
}

fn cred_of(id: &str) -> Result<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(id)
        .map_err(|_| Error::invalid("passkey is not an id"))
}

/// This account's passkeys, oldest first.
pub(crate) async fn list(inner: &Inner, user: AccountId) -> Result<Vec<PasskeyInfo>> {
    inner
        .db
        .run(move |c| {
            let mut st = c
                .prepare(
                    "SELECT cred_id, name, created_at, last_used_at FROM passkeys
                     WHERE user_id = ?1 ORDER BY created_at, rowid",
                )
                .map_err(sql)?;
            let rows = st
                .query_map(params![user.to_string()], |r| {
                    Ok(PasskeyInfo {
                        id: id_of(&r.get::<_, Vec<u8>>(0)?),
                        name: r.get(1)?,
                        created_at: r.get(2)?,
                        last_used_at: r.get(3)?,
                    })
                })
                .map_err(sql)?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(sql)
        })
        .await
}

/// Starts adding a passkey. Returns the prompt id and the options for
/// `navigator.credentials.create`. Passkeys you already have are listed as
/// excluded, so the same device can't be added twice.
pub(crate) async fn start_add(inner: &Arc<Inner>, user: AccountId) -> Result<(String, Value)> {
    let u = user.to_string();
    let (name, username): (String, String) = inner
        .db
        .run(move |c| {
            c.query_row(
                "SELECT name, username FROM users WHERE id = ?1",
                params![u],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(sql)
        })
        .await?;
    let existing = crate::passkey::existing_cred_ids(inner, user).await?;
    let (ccr, state) = inner
        .webauthn
        .start_passkey_registration(user.as_uuid(), &username, &name, Some(existing))
        .map_err(Error::internal)?;
    let mut options = serde_json::to_value(&ccr).map_err(Error::internal)?;
    crate::passkey::require_discoverable(&mut options);
    let id = inner.ceremonies.insert(Pending::Add { state, user })?;
    Ok((id, options))
}

/// Finishes adding a passkey and saves it under `name`.
/// Its insertion and optional recovery witness share an IMMEDIATE transaction.
pub(crate) async fn finish_add(
    inner: &Arc<Inner>,
    user: AccountId,
    ceremony: &str,
    credential: Value,
    name: &str,
) -> Result<PasskeyInfo> {
    let Pending::Add { state, user: owner } = inner.ceremonies.take_add(ceremony, user)? else {
        return Err(Error::new(
            ErrorCode::PasskeyFailed,
            "prompt was not for adding a passkey",
        ));
    };
    if owner != user {
        return Err(Error::new(
            ErrorCode::PasskeyFailed,
            "prompt belongs to someone else",
        ));
    }
    let reg: RegisterPublicKeyCredential = serde_json::from_value(credential)
        .map_err(|e| Error::new(ErrorCode::PasskeyFailed, e.to_string()))?;
    let passkey = inner
        .webauthn
        .finish_passkey_registration(&reg, &state)
        .map_err(|e| Error::new(ErrorCode::PasskeyFailed, e.to_string()))?;
    let cred_id = passkey.cred_id().as_ref().to_vec();
    let json = serde_json::to_string(&passkey).map_err(Error::internal)?;
    let name = clean_label(name, "Passkey");
    let now = now_ms();
    let row = PasskeyInfo {
        id: id_of(&cred_id),
        name: name.clone(),
        created_at: now,
        last_used_at: None,
    };
    inner
        .db
        .run(move |c| {
            let tx = c
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(sql)?;
            let added = tx
                .execute(
                    "INSERT OR IGNORE INTO passkeys (cred_id, user_id, passkey, created_at, name)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![cred_id, user.to_string(), json, now, name],
                )
                .map_err(sql)?;
            if added != 1 {
                return Err(Error::new(
                    ErrorCode::PasskeyFailed,
                    "passkey already registered",
                ));
            }
            crate::spaces::commit(tx)
        })
        .await?;
    Ok(row)
}

/// Renames one of your passkeys and publishes its protected account metadata
/// through the configured recovery barrier before returning the name kept.
pub(crate) async fn rename(inner: &Inner, user: AccountId, id: &str, name: &str) -> Result<String> {
    let cred = cred_of(id)?;
    let name = clean_label(name, "");
    if name.is_empty() {
        return Err(Error::invalid("a passkey needs a name"));
    }
    let kept = name.clone();
    let changed = inner
        .db
        .run(move |c| {
            let tx = c
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(sql)?;
            let changed = tx
                .execute(
                    "UPDATE passkeys SET name = ?3 WHERE cred_id = ?1 AND user_id = ?2",
                    params![cred, user.to_string(), name],
                )
                .map_err(sql)?;
            crate::spaces::commit(tx)?;
            Ok(changed)
        })
        .await?;
    if changed == 0 {
        return Err(Error::new(ErrorCode::NotFound, "no such passkey"));
    }
    Ok(kept)
}

/// Removes one of your passkeys, unless it is the last.
/// A configured recovery sink durably publishes removal before acknowledgement.
pub(crate) async fn remove(inner: &Inner, user: AccountId, id: &str) -> Result<()> {
    let cred = cred_of(id)?;
    inner
        .db
        .run(move |c| {
            let tx = c
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(sql)?;
            let u = user.to_string();
            let have: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM passkeys WHERE user_id = ?1",
                    params![u],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            let mine: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM passkeys WHERE user_id = ?1 AND cred_id = ?2",
                    params![u, cred],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            if mine == 0 {
                return Err(Error::new(ErrorCode::NotFound, "no such passkey"));
            }
            if have <= 1 {
                return Err(Error::new(ErrorCode::LastPasskey, "only passkey"));
            }
            tx.execute(
                "DELETE FROM passkeys WHERE user_id = ?1 AND cred_id = ?2",
                params![u, cred],
            )
            .map_err(sql)?;
            crate::spaces::commit(tx)
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed_short_and_never_empty() {
        assert_eq!(clean_label("  MacBook \n", "Passkey"), "MacBook");
        assert_eq!(clean_label("   ", "Passkey"), "Passkey");
        assert_eq!(clean_label("a\u{061c}b\u{202e}c\u{2069}", "Passkey"), "abc");
        assert_eq!(clean_label(&"a".repeat(100), "x").chars().count(), MAX_NAME);
    }

    #[test]
    fn ids_round_trip() {
        let id = id_of(&[1, 2, 250, 251]);
        assert_eq!(cred_of(&id).unwrap(), vec![1, 2, 250, 251]);
        assert!(cred_of("not base64!").is_err());
    }
}
