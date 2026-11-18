//! Proof and grant state for replacing lost passkeys.

use crate::account_email::{
    self, AccountMail, PROOF_TTL, numeric_code, opaque_id, refused, throttled,
};
use crate::db::sql;
use crate::{AccountId, Auth, InstallationId, Result, session, spaces, token};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::{Value, json};
use std::sync::Arc;

const HOLD: i64 = 3 * 86_400_000;
const CLAIM_WINDOW: i64 = 86_400_000;
const GRANT_TTL: i64 = 900_000;
const HISTORY: i64 = 90 * 86_400_000;

pub(crate) fn verified_address(c: &Connection, user: AccountId) -> Result<Option<String>> {
    c.query_row(
        "SELECT email FROM account_details WHERE user_id=?1 AND verified_at IS NOT NULL",
        [user.to_string()],
        |r| r.get(0),
    )
    .optional()
    .map_err(sql)
}

pub(crate) fn audit(
    tx: &Transaction<'_>,
    user: AccountId,
    method: &str,
    action: &str,
    actor: &str,
    now: i64,
) -> Result<()> {
    tx.execute(
        "INSERT INTO account_security_events VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            uuid::Uuid::now_v7().to_string(),
            user.to_string(),
            method,
            action,
            actor,
            now
        ],
    )
    .map_err(sql)?;
    tx.execute("DELETE FROM account_security_events WHERE user_id=?1 AND id NOT IN
        (SELECT id FROM account_security_events WHERE user_id=?1 ORDER BY at DESC,id DESC LIMIT 256)",
        [user.to_string()]).map_err(sql)?;
    Ok(())
}

fn parse_user(raw: &str) -> Result<AccountId> {
    AccountId::parse(raw).map_err(crate::Error::internal)
}

fn grant(
    tx: &Transaction<'_>,
    inner: &crate::Inner,
    user: AccountId,
    method: &str,
    now: i64,
) -> Result<String> {
    let secret = token::new_setup_token()?;
    let canonical = token::normalize_setup_token(&secret).ok_or_else(refused)?;
    let hash = inner
        .invite_key
        .account_proof("replacement-grant", &canonical);
    tx.execute(
        "DELETE FROM account_recovery_grants WHERE expires_at<=?1 OR user_id=?2",
        params![now, user.to_string()],
    )
    .map_err(sql)?;
    tx.execute(
        "INSERT INTO account_recovery_grants VALUES(?1,?2,?3,?4)",
        params![hash, user.to_string(), method, now + GRANT_TTL],
    )
    .map_err(sql)?;
    Ok(secret)
}

fn matching_proof(inner: &crate::Inner, id: &str, code: &str, proof: &[u8]) -> bool {
    let candidate = inner.invite_key.account_proof(id, code);
    candidate
        .as_slice()
        .try_into()
        .is_ok_and(|h| token::same_hash(h, proof))
}

fn notice(to: String, subject: &'static str, code: Option<String>) -> AccountMail {
    AccountMail { to, subject, code }
}

impl Auth {
    /// Prunes one bounded batch of expired account proofs and audit rows.
    /// Returns whether another batch may be due. Uses one protected commit.
    pub async fn purge_account_security(&self) -> Result<bool> {
        let now = self.inner.config.clock.now();
        self.inner
            .db
            .run(move |c| {
                let tx = c
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(sql)?;
                let mut backlog = false;
                for table in [
                    "account_email_challenges",
                    "account_recovery_flows",
                    "account_recovery_grants",
                ] {
                    let removed = tx
                        .execute(
                            &format!(
                                "DELETE FROM {table} WHERE rowid IN
                    (SELECT rowid FROM {table} WHERE expires_at<=?1 LIMIT 128)"
                            ),
                            [now],
                        )
                        .map_err(sql)?;
                    backlog |= removed == 128;
                }
                let removed = tx
                    .execute(
                        "DELETE FROM account_security_events WHERE id IN
                (SELECT id FROM account_security_events WHERE at<=?1 LIMIT 128)",
                        [now - HISTORY],
                    )
                    .map_err(sql)?;
                spaces::commit(tx)?;
                Ok(backlog || removed == 128)
            })
            .await
    }

    /// Starts a recovery proof. Unknown email addresses receive the same wire shape.
    /// The host supplies a trusted source, and privately sends any returned message.
    pub async fn start_recovery(
        &self,
        method: &str,
        address: Option<&str>,
        source: &str,
        mail: bool,
    ) -> Result<(Value, Option<AccountMail>)> {
        let id = opaque_id()?;
        if method == "code" && address.is_none() {
            return Ok((json!({"flow_id":format!("code_{id}")}), None));
        }
        if method != "email" || !mail {
            return Err(refused());
        }
        if !self.inner.email_sends.attempt(&format!("source:{source}")) {
            return Err(throttled());
        }
        let address = account_email::email(address.ok_or_else(refused)?)?;
        let inner = Arc::clone(&self.inner);
        self.inner.db.run(move |c| {
            let tx = c.transaction_with_behavior(TransactionBehavior::Immediate).map_err(sql)?;
            let now = inner.config.clock.now();
            let account = tx.query_row("SELECT d.user_id FROM account_details d JOIN users u ON u.id=d.user_id
                WHERE d.email=?1 AND d.verified_at IS NOT NULL AND u.disabled_at IS NULL",
                [&address], |r| r.get::<_,String>(0)).optional().map_err(sql)?;
            let mut result = (json!({"flow_id":id}),None);
            if let Some(account) = account.filter(|u| inner.email_sends.attempt(&format!("target:{u}"))) {
                tx.execute("DELETE FROM account_recovery_flows WHERE expires_at<=?1 OR (user_id=?2 AND stage='proof')",
                    params![now,account]).map_err(sql)?;
                let code = numeric_code()?;
                let proof = inner.invite_key.account_proof(&id, &code);
                let inserted = tx.execute("INSERT INTO account_recovery_flows(id,user_id,email,stage,proof,expires_at,proof_until,attempts)
                    VALUES(?1,?2,?3,'proof',?4,?5,?5,5) ON CONFLICT(user_id) DO NOTHING",
                    params![id,account,address,proof,now+PROOF_TTL]).map_err(sql)?;
                if inserted == 1 {
                    result.1 = Some(notice(address,"Confirm your Calternal recovery request",Some(code)));
                }
                spaces::commit(tx)?;
            }
            Ok(result)
        }).await
    }

    /// Consumes a backup code or advances a mailbox proof under a protected commit.
    /// Unknown, expired, vetoed and exhausted proofs share a generic refusal.
    pub async fn prove_recovery(
        &self,
        id: &str,
        code: &str,
        source: &str,
        mail: bool,
    ) -> Result<(Value, Option<AccountMail>)> {
        if !self
            .inner
            .recovery_proofs
            .attempt(&format!("source:{source}"))
        {
            return Err(throttled());
        }
        let id = id.to_owned();
        let code = code.trim().to_ascii_lowercase();
        let inner = Arc::clone(&self.inner);
        self.inner.db.run(move |c| {
            let tx = c.transaction_with_behavior(TransactionBehavior::Immediate).map_err(sql)?;
            let now = inner.config.clock.now();
            if id.starts_with("code_") {
                let hash = inner.invite_key.account_proof("recovery-code", &code);
                let user = tx.query_row("SELECT k.user_id FROM account_recovery_codes k JOIN users u ON u.id=k.user_id
                    WHERE k.hash=?1 AND u.disabled_at IS NULL", [&hash], |r| r.get::<_,String>(0))
                    .optional().map_err(sql)?.ok_or_else(refused)?;
                if !inner.recovery_proofs.attempt(&format!("target:{user}")) { return Err(throttled()); }
                tx.execute("DELETE FROM account_recovery_codes WHERE hash=?1",[hash]).map_err(sql)?;
                let user = parse_user(&user)?;
                let value = grant(&tx,&inner,user,"code",now)?;
                audit(&tx,user,"code","started","recovery",now)?;
                let msg = verified_address(&tx,user)?.filter(|_| mail).map(|to| notice(to,"Calternal recovery started",None));
                spaces::commit(tx)?;
                return Ok((json!({"enrollment_grant":value}),msg));
            }
            if !mail { return Err(refused()); }
            let row = tx.query_row("SELECT f.user_id,f.email,f.stage,f.proof,f.hold_until
                FROM account_recovery_flows f JOIN users u ON u.id=f.user_id
                JOIN account_details d ON d.user_id=f.user_id
                WHERE f.id=?1 AND f.expires_at>?2 AND f.proof_until>?2 AND f.attempts>0 AND f.stage IN ('proof','claim')
                AND u.disabled_at IS NULL AND d.email=f.email AND d.verified_at IS NOT NULL",
                params![id,now], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Vec<u8>>(3)?,r.get::<_,Option<i64>>(4)?)))
                .optional().map_err(sql)?.ok_or_else(refused)?;
            let (user,email,stage,proof,hold) = row;
            if !inner.recovery_proofs.attempt(&format!("target:{user}")) { return Err(throttled()); }
            if !matching_proof(&inner,&id,&code,&proof) || (stage=="claim" && hold.is_none_or(|at| at>now)) {
                tx.execute("UPDATE account_recovery_flows SET attempts=attempts-1 WHERE id=?1",[id]).map_err(sql)?;
                spaces::commit(tx)?;
                return Err(refused());
            }
            let user = parse_user(&user)?;
            let result = if stage == "proof" {
                let hold = now+HOLD;
                tx.execute("UPDATE account_recovery_flows SET stage='waiting',hold_until=?2,expires_at=?3,
                    proof=zeroblob(32),attempts=0 WHERE id=?1",params![id,hold,hold+CLAIM_WINDOW]).map_err(sql)?;
                audit(&tx,user,"email","started","recovery",now)?;
                (json!({"hold_until":hold}),Some(notice(email,"Calternal recovery waiting period started",None)))
            } else {
                tx.execute("DELETE FROM account_recovery_flows WHERE id=?1",[id]).map_err(sql)?;
                (json!({"enrollment_grant":grant(&tx,&inner,user,"email",now)?}),None)
            };
            spaces::commit(tx)?;
            Ok(result)
        }).await
    }

    /// Reads recovery progress and sends the claim code when the hold elapses.
    /// An expired or exhausted claim proof can be reissued within its claim window.
    pub async fn recovery_status(
        &self,
        id: &str,
        source: &str,
        mail: bool,
    ) -> Result<(Value, Option<AccountMail>)> {
        let id = id.to_owned();
        let source = format!("source:{source}");
        let inner = Arc::clone(&self.inner);
        self.inner.db.run(move |c| {
            let tx = c.transaction_with_behavior(TransactionBehavior::Immediate).map_err(sql)?;
            let now = inner.config.clock.now();
            let row = tx.query_row("SELECT f.user_id,f.email,f.stage,f.hold_until,f.proof_until,f.attempts FROM account_recovery_flows f
                JOIN account_details d ON d.user_id=f.user_id JOIN users u ON u.id=f.user_id
                WHERE f.id=?1 AND f.expires_at>?2 AND d.email=f.email AND d.verified_at IS NOT NULL AND u.disabled_at IS NULL",
                params![id,now], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<i64>>(3)?,r.get::<_,i64>(4)?,r.get::<_,i64>(5)?)))
                .optional().map_err(sql)?;
            let Some((user,email,stage,hold,proof_until,attempts)) = row else { return Ok((json!({"stage":"proof"}),None)); };
            if mail && ((stage == "waiting" && hold.is_some_and(|at| at<=now))
                || (stage == "claim" && (proof_until<=now || attempts==0))) {
                if !inner.email_sends.attempt(&source) || !inner.email_sends.attempt(&format!("target:{user}")) { return Err(throttled()); }
                let code = numeric_code()?;
                let proof = inner.invite_key.account_proof(&id,&code);
                tx.execute("UPDATE account_recovery_flows SET stage='claim',proof=?2,attempts=5,proof_until=min(expires_at,?3) WHERE id=?1",
                    params![id,proof,now+PROOF_TTL]).map_err(sql)?;
                spaces::commit(tx)?;
                return Ok((json!({"stage":"claim","hold_until":hold}),Some(notice(email,"Finish Calternal recovery",Some(code)))));
            }
            Ok((json!({"stage":stage,"hold_until":hold}),None))
        }).await
    }

    /// Cancels email recovery for an account after fresh session proof.
    /// An already-issued email enrollment grant is invalidated in the same commit.
    pub async fn veto_recovery(
        &self,
        user: AccountId,
        installation: InstallationId,
    ) -> Result<bool> {
        let inner = Arc::clone(&self.inner);
        self.inner
            .db
            .run(move |c| {
                let tx = c
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(sql)?;
                session::check_fresh(&tx, &inner.config, user, installation, false)?;
                let id = user.to_string();
                let flows = tx
                    .execute("DELETE FROM account_recovery_flows WHERE user_id=?1", [&id])
                    .map_err(sql)?;
                let grants = tx
                    .execute(
                        "DELETE FROM account_recovery_grants WHERE user_id=?1 AND method='email'",
                        [id],
                    )
                    .map_err(sql)?;
                let cancelled = flows + grants > 0;
                if cancelled {
                    audit(
                        &tx,
                        user,
                        "email",
                        "vetoed",
                        "account",
                        inner.config.clock.now(),
                    )?;
                }
                spaces::commit(tx)?;
                Ok(cancelled)
            })
            .await
    }

    /// Issues an enrollment-only grant after the local operator verified identity.
    /// The host must enforce an installation-admin boundary and protect the output.
    pub async fn operator_recovery(&self, user: AccountId, reason: &str) -> Result<String> {
        if !matches!(reason, "lost_passkeys" | "compromised" | "mail_unavailable") {
            return Err(refused());
        }
        let inner = Arc::clone(&self.inner);
        let reason = reason.to_owned();
        self.inner
            .db
            .run(move |c| {
                let tx = c
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(sql)?;
                let enabled: bool = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM users WHERE id=?1 AND disabled_at IS NULL)",
                        [user.to_string()],
                        |r| r.get(0),
                    )
                    .map_err(sql)?;
                if !enabled {
                    return Err(refused());
                }
                let now = inner.config.clock.now();
                let value = grant(&tx, &inner, user, "admin", now)?;
                audit(&tx, user, "admin", &reason, "operator", now)?;
                spaces::commit(tx)?;
                Ok(value)
            })
            .await
    }

    /// Reads up to 256 recent private recovery/security events for the authenticated account.
    pub async fn recovery_history(&self, user: AccountId) -> Result<Vec<Value>> {
        let cutoff = self.inner.config.clock.now() - HISTORY;
        self.inner.db.run(move |c| {
            let mut stmt = c.prepare_cached("SELECT method,action,at,actor FROM account_security_events
                WHERE user_id=?1 AND at>?2 ORDER BY at DESC,id DESC LIMIT 256").map_err(sql)?;
            stmt.query_map(params![user.to_string(),cutoff], |r| Ok(json!({"method":r.get::<_,String>(0)?,
                "action":r.get::<_,String>(1)?,"at":r.get::<_,i64>(2)?,"actor":r.get::<_,String>(3)?})))
                .map_err(sql)?.collect::<rusqlite::Result<Vec<_>>>().map_err(sql)
        }).await
    }

    /// Checks the hashed grant and starts a WebAuthn prompt for the bound account.
    /// Grant redemption stays transactional with replacement enrollment.
    pub async fn start_replacement(&self, raw: &str, source: &str) -> Result<(String, Value)> {
        if !self
            .inner
            .setup_failures
            .attempt(&format!("source:{source}"))
        {
            return Err(throttled());
        }
        let canonical = token::normalize_setup_token(raw).ok_or_else(refused)?;
        let hash = self
            .inner
            .invite_key
            .account_proof("replacement-grant", &canonical);
        let lookup = hash.clone();
        let now = self.inner.config.clock.now();
        let (user,name) = self.inner.db.run(move |c| {
            c.query_row("SELECT u.id,u.name FROM account_recovery_grants g JOIN users u ON u.id=g.user_id
                WHERE g.hash=?1 AND g.expires_at>?2 AND u.disabled_at IS NULL", params![lookup,now],
                |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional().map_err(sql)?.ok_or_else(refused)
        }).await?;
        crate::passkey::start_registration(
            &self.inner,
            crate::setup::Redeemable {
                token_hash: hash,
                user: parse_user(&user)?,
                name,
                first_owner: false,
                invite: false,
                signup: None,
                replacement: true,
            },
        )
        .await
    }

    /// Enrolls one replacement passkey and revokes the account's former access.
    /// Returns only a verified-address security notice. It never creates a session.
    pub async fn finish_replacement(
        &self,
        ceremony: &str,
        credential: Value,
    ) -> Result<Option<AccountMail>> {
        let (r, credential_id, passkey_json) =
            crate::passkey::verify_registration(&self.inner, ceremony, credential, true)?;
        let inner = Arc::clone(&self.inner);
        self.inner.db.run(move |c| {
            let tx = c.transaction_with_behavior(TransactionBehavior::Immediate).map_err(sql)?;
            let now = inner.config.clock.now();
            let user = r.user.to_string();
            let method = tx.query_row("DELETE FROM account_recovery_grants WHERE hash=?1 AND user_id=?2 AND expires_at>?3
                AND EXISTS(SELECT 1 FROM users WHERE id=?2 AND disabled_at IS NULL) RETURNING method",
                params![r.token_hash,user,now], |r| r.get::<_,String>(0)).optional().map_err(sql)?.ok_or_else(refused)?;
            // Insert first so the replacement cannot reuse an existing credential.
            tx.execute("INSERT INTO passkeys(cred_id,user_id,passkey,created_at,name) VALUES(?1,?2,?3,?4,'Replacement passkey')",
                params![credential_id,user,passkey_json,now]).map_err(sql)?;
            tx.execute("DELETE FROM passkeys WHERE user_id=?1 AND cred_id<>?2",params![user,credential_id]).map_err(sql)?;
            let installations = {
                let mut stmt = tx.prepare_cached("UPDATE installations SET revoked_at=?2 WHERE user_id=?1 AND revoked_at IS NULL RETURNING id").map_err(sql)?;
                stmt.query_map(params![user,now], |r| r.get::<_,String>(0)).map_err(sql)?
                    .collect::<rusqlite::Result<Vec<_>>>().map_err(sql)?
            };
            let installations = installations.iter().map(|id| id.parse().map_err(crate::Error::internal)).collect::<Result<Vec<InstallationId>>>()?;
            for table in ["oidc_links",
                "account_recovery_codes","account_recovery_flows","account_email_challenges","setup_tokens"] {
                tx.execute(&format!("DELETE FROM {table} WHERE user_id=?1"),[&user]).map_err(sql)?;
            }
            tx.execute("INSERT INTO account_details(user_id,replacement_pending,prompt_dismissed) VALUES(?1,1,0)
                ON CONFLICT(user_id) DO UPDATE SET replacement_pending=1,prompt_dismissed=0", [&user]).map_err(sql)?;
            audit(&tx,r.user,&method,"completed","recovery",now)?;
            let mail = verified_address(&tx,r.user)?.map(|to| notice(to,"Calternal account recovery completed",None));
            spaces::commit_with(tx,|| inner.sessions.forget_installations(installations))?;
            Ok(mail)
        }).await
    }
}
