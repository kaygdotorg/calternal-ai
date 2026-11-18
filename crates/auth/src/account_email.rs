//! Account email verification. SMTP transport belongs to the host.

use crate::db::sql;
use crate::{AccountId, Auth, Error, ErrorCode, InstallationId, Result, session, spaces, token};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::{Value, json};
use std::sync::Arc;

/// A delivery for the host's private mail queue. Never log this value.
pub struct AccountMail {
    /// Destination verified mail address, or address being verified.
    pub to: String,
    /// Generic security message subject.
    pub subject: &'static str,
    /// A short-lived proof, if this message requests one.
    pub code: Option<String>,
}

pub(crate) fn email(raw: &str) -> Result<String> {
    let invalid = || Error::invalid("invalid email address");
    let raw = raw.trim();
    let (local, domain) = raw.rsplit_once('@').ok_or_else(invalid)?;
    if raw.len() > 254
        || local.is_empty()
        || local.len() > 64
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
        || !local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&b))
        || !domain.contains('.')
        || domain.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        return Err(invalid());
    }
    Ok(format!("{}@{}", local, domain.to_ascii_lowercase()))
}

pub(crate) const PROOF_TTL: i64 = 600_000;

pub(crate) fn numeric_code() -> Result<String> {
    let mut rng = token::random32()?;
    loop {
        for bytes in rng.chunks_exact(4) {
            let n = u32::from_be_bytes(bytes.try_into().expect("four bytes"));
            if n < 4_200_000_000 {
                return Ok(format!("{:08}", n % 100_000_000));
            }
        }
        rng = token::random32()?;
    }
}

pub(crate) fn opaque_id() -> Result<String> {
    token::new_session_token().map(|id| id[4..].to_owned())
}

pub(crate) fn refused() -> Error {
    Error::invalid("the code or challenge does not work")
}

pub(crate) fn throttled() -> Error {
    Error::new(ErrorCode::TooManyRequests, "account proof rate limited")
}

type Caller = Option<(AccountId, InstallationId)>;

async fn binding(auth: &Auth, caller: Caller, raw_token: Option<&str>) -> Result<Vec<u8>> {
    match (caller, raw_token) {
        (Some((user, installation)), None) => {
            session::require_fresh(&auth.inner, user, Some(installation), false).await?;
            Ok(auth
                .inner
                .invite_key
                .account_proof("email-installation", &installation.to_string()))
        }
        (None, Some(raw_token)) => {
            let grant = crate::setup::check(&auth.inner, raw_token, "Email verification").await?;
            if !(grant.first_owner || grant.invite) {
                return Err(refused());
            }
            Ok(grant.token_hash)
        }
        _ => Err(refused()),
    }
}

pub(crate) fn claim_email(
    tx: &Transaction<'_>,
    binding: &[u8],
    id: &str,
    email: &str,
    now: i64,
) -> Result<()> {
    let used = tx
        .execute(
            "DELETE FROM account_email_challenges WHERE id=?1 AND binding=?2 AND email=?3
         AND user_id IS NULL AND confirmed=1 AND expires_at>?4",
            params![id, binding, email, now],
        )
        .map_err(sql)?;
    if used != 1 {
        return Err(refused());
    }
    Ok(())
}

pub(crate) fn save_email(
    tx: &Transaction<'_>,
    user: AccountId,
    email: &str,
    verified_at: Option<i64>,
) -> Result<()> {
    if verified_at.is_some() {
        let taken: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM account_details
            WHERE email=?1 AND verified_at IS NOT NULL AND user_id<>?2)",
                params![email, user.to_string()],
                |r| r.get(0),
            )
            .map_err(sql)?;
        if taken {
            return Err(Error::new(
                ErrorCode::Conflict,
                "verified address unavailable",
            ));
        }
    }
    let user = user.to_string();
    // An UPSERT update would override the recovery trigger's OR IGNORE policy.
    let changed = tx
        .execute(
            "UPDATE account_details SET email=?2, verified_at=?3 WHERE user_id=?1",
            params![user, email, verified_at],
        )
        .map_err(sql)?;
    if changed == 0 {
        tx.execute(
            "INSERT INTO account_details(user_id,email,verified_at) VALUES(?1,?2,?3)",
            params![user, email, verified_at],
        )
        .map_err(sql)?;
    }
    Ok(())
}

fn check_fresh(tx: &Transaction<'_>, auth: &crate::Inner, caller: Caller) -> Result<()> {
    if let Some((user, install)) = caller {
        session::check_fresh(tx, &auth.config, user, install, false)?;
    }
    Ok(())
}

impl Auth {
    /// Issues an email challenge bound to this installation or signup token.
    /// In no-mail self-hosted mode, a signed-in caller can store unverified email.
    /// The host supplies the trusted source and must deliver any returned mail privately.
    pub async fn start_account_email(
        &self,
        caller: Caller,
        raw_token: Option<&str>,
        address: &str,
        source: &str,
        mail: bool,
    ) -> Result<(Value, Option<AccountMail>)> {
        let address = email(address)?;
        if !self.inner.email_sends.attempt(&format!("source:{source}")) {
            return Err(throttled());
        }
        let binding = binding(self, caller, raw_token).await?;
        let target = caller.map_or_else(|| address.clone(), |(u, _)| u.to_string());
        if !self.inner.email_sends.attempt(&format!("target:{target}")) {
            return Err(throttled());
        }
        let inner = Arc::clone(&self.inner);
        self.inner.db.run(move |c| {
            let tx = c.transaction_with_behavior(TransactionBehavior::Immediate).map_err(sql)?;
            check_fresh(&tx, &inner, caller)?;
            let now = inner.config.clock.now();
            if !mail {
                let (user, _) = caller.ok_or_else(refused)?;
                let old = crate::account_recovery::verified_address(&tx, user)?;
                // A mail outage must not displace an already verified address.
                if old.is_some() {
                    return Err(Error::new(ErrorCode::Conflict, "mail is needed to change this address"));
                }
                save_email(&tx, user, &address, None)?;
                spaces::commit(tx)?;
                return Ok((json!({"email_status":"unverified"}), None));
            }
            tx.execute("DELETE FROM account_email_challenges WHERE expires_at<=?1 OR binding=?2", params![now,binding]).map_err(sql)?;
            let open: i64 = tx.query_row("SELECT count(*) FROM account_email_challenges", [], |r| r.get(0)).map_err(sql)?;
            if open >= 2048 { return Err(throttled()); }
            let id = opaque_id()?;
            let code = numeric_code()?;
            let proof = inner.invite_key.account_proof(&id, &code);
            let expires = now + PROOF_TTL;
            tx.execute("INSERT INTO account_email_challenges(id,user_id,binding,email,proof,expires_at,attempts)
                VALUES(?1,?2,?3,?4,?5,?6,5)", params![id,caller.map(|(u,_)|u.to_string()),binding,address,proof,expires]).map_err(sql)?;
            spaces::commit(tx)?;
            Ok((json!({"challenge_id":id,"expires_at":expires}),
                Some(AccountMail { to:address, subject:"Verify your Calternal email address", code:Some(code) })))
        }).await
    }

    /// Checks the typed code and either binds it to enrollment or saves a verified address.
    /// Incorrect attempts consume the matching challenge's budget before returning a refusal.
    pub async fn confirm_account_email(
        &self,
        caller: Caller,
        raw_token: Option<&str>,
        id: &str,
        code: &str,
        source: &str,
    ) -> Result<(Value, Option<AccountMail>)> {
        if !self
            .inner
            .recovery_proofs
            .attempt(&format!("source:{source}"))
        {
            return Err(throttled());
        }
        let binding = binding(self, caller, raw_token).await?;
        let inner = Arc::clone(&self.inner);
        let id = id.to_owned();
        let proof = inner.invite_key.account_proof(&id, code);
        self.inner
            .db
            .run(move |c| {
                let tx = c
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(sql)?;
                check_fresh(&tx, &inner, caller)?;
                let now = inner.config.clock.now();
                let row = tx
                    .query_row(
                        "SELECT email,proof FROM account_email_challenges
                WHERE id=?1 AND binding=?2 AND expires_at>?3 AND confirmed=0 AND attempts>0",
                        params![id, binding, now],
                        |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?)),
                    )
                    .optional()
                    .map_err(sql)?;
                let Some((address, expected)) = row else {
                    return Err(refused());
                };
                if !token::same_hash(
                    proof.as_slice().try_into().map_err(Error::internal)?,
                    &expected,
                ) {
                    tx.execute(
                        "UPDATE account_email_challenges SET attempts=attempts-1 WHERE id=?1",
                        [id],
                    )
                    .map_err(sql)?;
                    spaces::commit(tx)?;
                    return Err(refused());
                }
                let result = if let Some((user, _)) = caller {
                    let old = crate::account_recovery::verified_address(&tx, user)?;
                    save_email(&tx, user, &address, Some(now))?;
                    tx.execute(
                        "DELETE FROM account_recovery_flows WHERE user_id=?1",
                        [user.to_string()],
                    )
                    .map_err(sql)?;
                    tx.execute(
                        "DELETE FROM account_recovery_grants WHERE user_id=?1 AND method='email'",
                        [user.to_string()],
                    )
                    .map_err(sql)?;
                    tx.execute("DELETE FROM account_email_challenges WHERE id=?1", [id])
                        .map_err(sql)?;
                    crate::account_recovery::audit(
                        &tx,
                        user,
                        "email",
                        "address_changed",
                        "account",
                        now,
                    )?;
                    (
                        json!({"email_status":"verified"}),
                        old.map(|to| AccountMail {
                            to,
                            subject: "Your Calternal email address changed",
                            code: None,
                        }),
                    )
                } else {
                    tx.execute(
                        "UPDATE account_email_challenges SET confirmed=1 WHERE id=?1",
                        [&id],
                    )
                    .map_err(sql)?;
                    (
                        json!({"email_status":"verified","verification_id":id}),
                        None,
                    )
                };
                spaces::commit(tx)?;
                Ok(result)
            })
            .await
    }

    /// Reads the current verified address for a signed-in account's security notice.
    /// One indexed read. The host must supply the account from its authenticated session.
    pub async fn security_email(&self, user: AccountId) -> Result<Option<String>> {
        self.inner
            .db
            .run(move |c| crate::account_recovery::verified_address(c, user))
            .await
    }
}
