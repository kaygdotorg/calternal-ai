//! Private account settings and one-time backup credentials.

use crate::db::sql;
use crate::{AccountId, Auth, Error, ErrorCode, InstallationId, Result, session, spaces};
use rusqlite::{Connection, Transaction, TransactionBehavior, params};
use serde_json::{Value, json};

pub(crate) fn username(raw: &str) -> Result<String> {
    let value = raw.trim().to_ascii_lowercase();
    if !(3..=32).contains(&value.len())
        || value.split('-').any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
        || uuid::Uuid::parse_str(&value).is_ok()
        || matches!(
            value.as_str(),
            "admin" | "administrator" | "root" | "owner" | "system" | "support" | "calternal"
        )
    {
        return Err(Error::invalid("invalid username"));
    }
    Ok(value)
}

pub(crate) fn check_username(c: &Connection, username: &str, user: AccountId) -> Result<()> {
    let taken: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE username=?1 AND id<>?2)",
            params![username, user.to_string()],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if taken {
        return Err(Error::new(ErrorCode::Conflict, "username already taken"));
    }
    Ok(())
}

pub(crate) fn rotate_codes(
    tx: &Transaction<'_>,
    key: &crate::invite_key::Key,
    user: AccountId,
) -> Result<Vec<String>> {
    let user = user.to_string();
    tx.execute(
        "DELETE FROM account_recovery_codes WHERE user_id=?1",
        [&user],
    )
    .map_err(sql)?;
    let mut insert = tx
        .prepare_cached("INSERT INTO account_recovery_codes(hash,user_id) VALUES(?1,?2)")
        .map_err(sql)?;
    (0..10)
        .map(|_| {
            let code = crate::token::random32()?[..16]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            insert
                .execute(params![key.account_proof("recovery-code", &code), user])
                .map_err(sql)?;
            Ok(code)
        })
        .collect()
}

impl Auth {
    /// Changes only the account's presentation handle in one protected commit.
    /// Requires a current, recently asserted browser or native installation.
    pub async fn change_username(
        &self,
        user: AccountId,
        installation: InstallationId,
        raw: &str,
    ) -> Result<String> {
        let username = username(raw)?;
        let config = self.inner.config.clone();
        self.inner
            .db
            .run(move |c| {
                let tx = c
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(sql)?;
                session::check_fresh(&tx, &config, user, installation, false)?;
                check_username(&tx, &username, user)?;
                tx.execute(
                    "UPDATE users SET username=?1 WHERE id=?2",
                    params![username, user.to_string()],
                )
                .map_err(sql)?;
                spaces::commit(tx)?;
                Ok(username)
            })
            .await
    }

    /// Returns private email, recovery status, second-passkey prompt and code availability.
    /// The host must supply an authenticated account. Uses one database query.
    pub async fn account_security_status(&self, user: AccountId) -> Result<Value> {
        let now = self.inner.config.clock.now();
        self.inner.db.run(move |c| {
            c.query_row(
                "SELECT d.email, d.verified_at IS NOT NULL,
                  COALESCE(d.prompt_dismissed,0)=0 AND (SELECT count(*) FROM passkeys WHERE user_id=u.id)=1,
                  EXISTS(SELECT 1 FROM account_recovery_codes WHERE user_id=u.id),
                  EXISTS(SELECT 1 FROM account_recovery_flows WHERE user_id=u.id AND stage<>'proof' AND expires_at>?2)
                  OR EXISTS(SELECT 1 FROM account_recovery_grants WHERE user_id=u.id AND method='email' AND expires_at>?2)
                 FROM users u LEFT JOIN account_details d ON d.user_id=u.id WHERE u.id=?1",
                params![user.to_string(),now], |r| {
                    let email: Option<String> = r.get(0)?;
                    let verified: bool = r.get(1)?;
                    let status = if verified { "verified" } else if email.is_some() { "unverified" } else { "missing" };
                    Ok(json!({"email":email,"email_status":status,"second_passkey_prompt":r.get::<_,bool>(2)?,
                        "recovery_codes_configured":r.get::<_,bool>(3)?,"email_recovery_pending":r.get::<_,bool>(4)?}))
                }
            ).map_err(sql)
        }).await
    }

    /// Hides the second-passkey prompt for a signed-in account in one commit.
    pub async fn dismiss_second_passkey(&self, user: AccountId) -> Result<()> {
        self.inner
            .db
            .run(move |c| {
                let tx = c
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(sql)?;
                let user = user.to_string();
                // UPSERT's update overrides the recovery trigger's OR IGNORE policy.
                if tx
                    .execute(
                        "UPDATE account_details SET prompt_dismissed=1 WHERE user_id=?1",
                        [&user],
                    )
                    .map_err(sql)?
                    == 0
                {
                    tx.execute(
                        "INSERT INTO account_details(user_id,prompt_dismissed) VALUES(?1,1)",
                        [&user],
                    )
                    .map_err(sql)?;
                }
                spaces::commit(tx)
            })
            .await
    }

    /// Replaces the whole recovery-code set in one protected commit and returns it once.
    /// Requires a recent check with the session's sign-in method. The caller must send this result privately.
    pub async fn regenerate_recovery_codes(
        &self,
        user: AccountId,
        installation: InstallationId,
    ) -> Result<Vec<String>> {
        let inner = self.inner.clone();
        self.inner
            .db
            .run(move |c| {
                let tx = c
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(sql)?;
                session::check_fresh(&tx, &inner.config, user, installation, false)?;
                let codes = rotate_codes(&tx, &inner.invite_key, user)?;
                spaces::commit(tx)?;
                Ok(codes)
            })
            .await
    }
}

#[cfg(test)]
#[path = "account_details_tests.rs"]
mod account_details_tests;
