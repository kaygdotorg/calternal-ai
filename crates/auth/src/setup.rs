//! Setup tokens: how the first owner gets in, and how someone who lost
//! every device gets back in.
//!
//! - While the server has no first account, the operator can issue a
//!   first-owner code. Pasting it on the first screen, with a name, creates
//!   the owner and their first passkey.
//! - `calternal user recover <username-or-uuid>` prints a recovery token for an
//!   existing person. Pasting it lets them add a new passkey.
//! - `calternal user create <name>` makes a person with no passkey yet
//!   (identity only; no content or directory is created)
//!   and prints a token that enrolls their first passkey. Once anyone
//!   exists, no first-owner token is issued.
//!
//! A token works exactly once, even if many requests race to use it, and
//! expires after `setup_ttl`. Only its hash is stored.

use crate::Inner;
#[cfg(test)]
use crate::db::open_conn;
use crate::db::{now_ms, sql};
use crate::session::{self, InstallationKind};
use crate::token::{hash, new_setup_token, normalize_setup_token};
use crate::{AccountId, Error, ErrorCode, InstallationId, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::sync::Arc;
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;

/// A setup token that checked out, ready for its passkey.
pub(crate) struct Redeemable {
    pub token_hash: Vec<u8>,
    pub user: AccountId,
    pub name: String,
    pub first_owner: bool,
    pub invite: bool,
    pub signup: Option<Signup>,
    pub replacement: bool,
}

pub(crate) struct Signup {
    pub username: String,
    pub email: Option<String>,
    pub verification: Option<String>,
}

pub(crate) type Enrollment = (InstallationId, String, Vec<String>);

/// A first-owner code and its exact database expiry. Keep the code private.
pub struct FirstOwnerCode {
    /// The one-time code presented to the operator.
    pub code: String,
    /// Expiry as Unix milliseconds.
    pub expires_ms: i64,
}

/// Verify a startup candidate or issue a replacement under the same transaction.
/// The optional protected recovery barrier precedes acknowledgement of writes.
pub(crate) async fn first_owner_code(
    inner: &Arc<Inner>,
    candidate: Option<String>,
) -> Result<Option<FirstOwnerCode>> {
    let ttl = inner.config.setup_ttl.as_millis() as i64;
    inner
        .db
        .run(move |c| {
            let tx = c.transaction().map_err(sql)?;
            let has_account: bool = tx
                .query_row("SELECT EXISTS(SELECT 1 FROM users)", [], |r| r.get(0))
                .map_err(sql)?;
            if has_account {
                return Ok(None);
            }
            if let Some((code, canonical)) = candidate
                .and_then(|code| normalize_setup_token(&code).map(|canonical| (code, canonical)))
            {
                let expires_ms = tx
                    .query_row(
                        "SELECT expires_at FROM setup_tokens
                     WHERE hash=?1 AND purpose='first-owner' AND used_at IS NULL AND expires_at>?2",
                        params![hash(&canonical), now_ms()],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(sql)?;
                if let Some(expires_ms) = expires_ms {
                    return Ok(Some(FirstOwnerCode { code, expires_ms }));
                }
            }
            tx.execute(
                "DELETE FROM setup_tokens WHERE purpose = 'first-owner' AND used_at IS NULL",
                [],
            )
            .map_err(sql)?;
            let token = new_setup_token()?;
            let expires_ms = insert_token(&tx, &token, "first-owner", None, ttl)?;
            crate::spaces::commit(tx)?;
            Ok(Some(FirstOwnerCode {
                code: token,
                expires_ms,
            }))
        })
        .await
}

/// Who `who` (an exact username or UUID) is: their id and name.
pub(crate) fn find_in(c: &Connection, who: &str) -> Result<(String, String)> {
    // Display names are never identity selectors, even when currently unique.
    // BINARY overrides the username column's case-insensitive uniqueness collation.
    let id = AccountId::parse(who).ok().map(|id| id.to_string());
    let mut st = c
        .prepare("SELECT id, name FROM users WHERE id = ?1 OR username = ?2 COLLATE BINARY")
        .map_err(sql)?;
    let mut rows = st
        .query_map(params![id, who], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(sql)?;
    let found = rows
        .next()
        .transpose()
        .map_err(sql)?
        .ok_or_else(|| Error::new(ErrorCode::NotFound, "use an exact username or UUID"))?;
    if rows.next().transpose().map_err(sql)?.is_some() {
        return Err(Error::invalid(
            "username and UUID identify different accounts",
        ));
    }
    Ok(found)
}

/// Replaces recovery tokens atomically, including the optional recovery witness.
pub(crate) fn recover_in(c: &mut Connection, who: &str, ttl: i64) -> Result<(String, String)> {
    let tx = c.transaction().map_err(sql)?;
    let (user, name) = find_in(&tx, who)?;
    crate::people::require_active(&tx, AccountId::parse(&user).map_err(Error::internal)?)?;
    tx.execute(
        "DELETE FROM setup_tokens WHERE purpose = 'recover' AND user_id = ?1 AND used_at IS NULL",
        params![user],
    )
    .map_err(sql)?;
    let token = new_setup_token()?;
    insert_token(&tx, &token, "recover", Some(&user), ttl)?;
    crate::spaces::commit(tx)?;
    Ok((name, token))
}

/// A person made by `calternal user create`.
#[derive(Debug, Clone)]
pub struct Created {
    /// Their identity; it does not select content.
    pub id: AccountId,
    /// Display name.
    pub name: String,
    /// Exact administrative username.
    pub username: String,
    /// One-time token that enrolls their first passkey.
    pub token: String,
}

/// Creates an account and token, waiting for configured protected recovery.
pub(crate) fn create_in(c: &mut Connection, name: &str, ttl: i64) -> Result<Created> {
    let name = clean_person_name(name).ok_or_else(|| Error::invalid("unusable account name"))?;
    let username = username_for(&name);
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(sql)?;
    let taken: bool = tx
        .query_row(
            "SELECT COUNT(*) > 0 FROM users WHERE username = ?1 COLLATE NOCASE",
            params![username],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if taken {
        return Err(Error::invalid(format!(
            "username already taken: {username}"
        )));
    }
    ensure_person_name_available(&tx, &name, None, false)?;
    let id = AccountId::new();
    insert_account(&tx, id, &name, &username, now_ms())?;
    // Someone exists now: an unused first-owner token must not make a
    // second, unrelated owner.
    tx.execute(
        "DELETE FROM setup_tokens WHERE purpose = 'first-owner' AND used_at IS NULL",
        [],
    )
    .map_err(sql)?;
    // Enrolling a first passkey for an existing person is exactly what a
    // recovery token does, so it is one (no new kind of token to secure).
    let token = new_setup_token()?;
    insert_token(&tx, &token, "recover", Some(&id.to_string()), ttl)?;
    crate::spaces::commit(tx)?;
    Ok(Created {
        id,
        name,
        username,
        token,
    })
}

/// Insert a new account and its personal space in an open authority transaction.
/// Cost: three row inserts. Callers commit through the protected recovery barrier.
pub(crate) fn insert_account(
    tx: &rusqlite::Transaction<'_>,
    id: AccountId,
    name: &str,
    username: &str,
    created: i64,
) -> Result<crate::SpaceId> {
    tx.execute(
        "INSERT INTO users (id, name, username, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![id.to_string(), name, username, created],
    )
    .map_err(sql)?;
    Ok(crate::spaces::insert_owned(tx, id, "Personal", created, true)?.id)
}

fn insert_token(
    tx: &rusqlite::Transaction<'_>,
    token: &str,
    purpose: &str,
    user: Option<&str>,
    ttl: i64,
) -> Result<i64> {
    let canon = normalize_setup_token(token).ok_or_else(|| Error::internal("bad new token"))?;
    let now = now_ms();
    tx.execute(
        "INSERT INTO setup_tokens (hash, purpose, user_id, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![hash(&canon), purpose, user, now, now + ttl],
    )
    .map_err(sql)?;
    Ok(now + ttl)
}

fn invalid(detail: &str) -> Error {
    Error::new(ErrorCode::SetupTokenInvalid, detail.to_owned())
}

/// Checks a pasted token without using it up. The token is only consumed
/// when the passkey is saved, so a cancelled prompt can be retried.
pub(crate) async fn check(inner: &Arc<Inner>, raw: &str, name: &str) -> Result<Redeemable> {
    let canon = normalize_setup_token(raw);
    let token_hash = hash(canon.as_deref().unwrap_or_default());
    let h = token_hash.clone();
    let invite_hash = inner.invite_key.lookup(raw);
    let now = now_ms();
    let (setup, invite) = inner.db.run(move |c| {
        let setup = c.query_row(
            "SELECT purpose,user_id FROM setup_tokens
             WHERE hash=?1 AND used_at IS NULL AND expires_at>?2
               AND (user_id IS NULL OR EXISTS(SELECT 1 FROM users WHERE id=user_id AND disabled_at IS NULL))",
            params![h,now], |r| Ok((r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?)),
        ).optional().map_err(sql)?;
        let invite = crate::invites::find(c, invite_hash.as_bytes(), now)?;
        Ok((setup, invite))
    }).await?;
    // Check both stores even for malformed input; one generic refusal covers both.
    if invite.is_some() {
        let name = clean_person_name(name).ok_or_else(|| Error::invalid("a name is needed"))?;
        return Ok(Redeemable {
            token_hash: invite_hash.as_bytes().to_vec(),
            user: AccountId::new(),
            name,
            first_owner: false,
            invite: true,
            signup: None,
            replacement: false,
        });
    }
    let (purpose, user) = setup
        .filter(|_| canon.is_some())
        .ok_or_else(|| invalid("token invalid or expired"))?;
    if purpose == "first-owner" {
        let name = clean_person_name(name).ok_or_else(|| Error::invalid("a name is needed"))?;
        return Ok(Redeemable {
            token_hash,
            user: AccountId::new(),
            name,
            first_owner: true,
            invite: false,
            signup: None,
            replacement: false,
        });
    }
    let user = user
        .as_deref()
        .and_then(|u| AccountId::parse(u).ok())
        .ok_or_else(|| Error::internal("recover token without a user"))?;
    let u = user.to_string();
    let name: String = inner
        .db
        .run(move |c| {
            c.query_row("SELECT name FROM users WHERE id = ?1", params![u], |r| {
                r.get(0)
            })
            .map_err(sql)
        })
        .await?;
    Ok(Redeemable {
        token_hash,
        user,
        name,
        first_owner: false,
        invite: false,
        signup: None,
        replacement: false,
    })
}

/// Uses up the token and saves the passkey and a new Installation, all in
/// one transaction. Exactly one caller can ever win a given token.
/// Configured recovery publication completes before enrollment is acknowledged.
pub(crate) async fn redeem(
    inner: &Arc<Inner>,
    r: Redeemable,
    cred_id: Vec<u8>,
    passkey_json: String,
    device: String,
) -> Result<Enrollment> {
    let ttl = inner.config.session_ttl.as_millis() as i64;
    let inner = Arc::clone(inner);
    let out = inner.db.clone()
        .run(move |c| {
            let tx = c
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(sql)?;
            let now = now_ms();
            if r.invite {
                let grant = crate::invites::find(&tx, r.token_hash.as_slice().try_into().map_err(Error::internal)?, now)?
                    .ok_or_else(crate::invites::invalid)?;
                ensure_person_name_available(&tx, &r.name, None, false)?;
                let username = format!("{}-{}", username_for(&r.name), r.user.as_uuid().simple());
                insert_signup(&tx, &r, &username, now)?;
                crate::invites::consume(&tx, grant, r.user, now)?;
            } else {
                let used = tx.execute(
                   "UPDATE setup_tokens SET used_at=?2
                    WHERE hash=?1 AND used_at IS NULL AND expires_at>?2",
                    params![r.token_hash, now],
                ).map_err(sql)?;
                if used != 1 {
                    return Err(invalid("token used or expired meanwhile"));
                }
                if !r.first_owner {
                    let active: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM users WHERE id=?1 AND disabled_at IS NULL)",
                        [r.user.to_string()], |row| row.get(0)).map_err(sql)?;
                    if !active { return Err(invalid("account unavailable")); }
                }
                if r.first_owner {
                    let users: i64 = tx.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0)).map_err(sql)?;
                    if users > 0 {
                        return Err(invalid("the server already has an owner"));
                    }
                    let username = username_for(&r.name);
                    insert_signup(&tx, &r, &username, now)?;
                }
            }
            let added = tx
                .execute(
                    "INSERT OR IGNORE INTO passkeys (cred_id, user_id, passkey, created_at, name)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        cred_id,
                        r.user.to_string(),
                        passkey_json,
                        now,
                        crate::passkeys::clean_label(&device, "Passkey")
                    ],
                )
                .map_err(sql)?;
            if added != 1 {
                return Err(Error::new(
                    ErrorCode::PasskeyFailed,
                    "passkey already registered",
                ));
            }
            let out = session::create(&tx, r.user, InstallationKind::Browser, &device, ttl, false)?;
            tx.execute(
                "UPDATE installations SET assertion_at = ?2, registration_session = 1 WHERE id = ?1",
                params![out.0.to_string(), now],
            )
            .map_err(sql)?;
            let codes = if r.first_owner || r.invite {
                crate::account_details::rotate_codes(&tx, &inner.invite_key, r.user)?
            } else {
                Vec::new()
            };
            crate::spaces::commit(tx)?;
            Ok((out.0,out.1,codes))
        })
        .await?;
    Ok(out)
}

pub(crate) fn insert_signup(
    tx: &rusqlite::Transaction<'_>,
    r: &Redeemable,
    default_username: &str,
    now: i64,
) -> Result<()> {
    let username = r.signup.as_ref().map_or(default_username, |s| &s.username);
    crate::account_details::check_username(tx, username, r.user)?;
    insert_account(tx, r.user, &r.name, username, now)?;
    if let Some(signup) = &r.signup
        && let Some(email) = &signup.email
    {
        if let Some(id) = &signup.verification {
            crate::account_email::claim_email(tx, &r.token_hash, id, email, now)?;
        }
        crate::account_email::save_email(
            tx,
            r.user,
            email,
            signup.verification.as_ref().map(|_| now),
        )?;
    }
    Ok(())
}

/// Names are NFC-normalized and must not hide controls or formatting marks.
pub(crate) fn clean_person_name(name: &str) -> Option<String> {
    use unicode_general_category::{GeneralCategory, get_general_category};
    if name.chars().any(|c| {
        let category = get_general_category(c);
        let formatting = matches!(
            category,
            GeneralCategory::Control
                | GeneralCategory::Format
                | GeneralCategory::LineSeparator
                | GeneralCategory::ParagraphSeparator
        );
        // Unicode Default_Ignorable_Code_Point, including non-format fillers,
        // grapheme joiners and variation selectors. These can hide name changes.
        let invisible = matches!(
            c as u32,
            0x034f
                | 0x115f..=0x1160
                | 0x17b4..=0x17b5
                | 0x180b..=0x180f
                | 0x200b..=0x200f
                | 0x202a..=0x202e
                | 0x2060..=0x206f
                | 0x2800
                | 0x3164
                | 0xfe00..=0xfe0f
                | 0xfeff
                | 0xffa0
                | 0xfff0..=0xfff8
                | 0x1bca0..=0x1bca3
                | 0x1d173..=0x1d17a
                | 0xe0000..=0xe0fff
        );
        formatting || invisible
    }) {
        return None;
    }
    let s: String = name.trim().nfc().collect();
    (!s.is_empty() && s.chars().count() <= 80).then_some(s)
}

fn name_skeletons(name: &str) -> [String; 2] {
    let normalized: String = name.trim().nfkc().collect();
    let folded: String = normalized.case_fold().collect();
    // Preserve glyph confusables erased by folding first (I/l), and also
    // compare folded spellings (Straße/STRASSE). Fold the skeleton itself
    // because UTS 39 can introduce uppercase letters, such as 0 -> O.
    [&normalized, &folded].map(|text| unicode_security::skeleton(text).case_fold().nfd().collect())
}

/// Guard inside the writing transaction, including after a passkey prompt.
/// On promotion, check every other person so no existing name becomes an
/// administrator impersonation. Profile updates exclude the updating account.
pub(crate) fn ensure_person_name_available(
    c: &Connection,
    name: &str,
    exclude: Option<&str>,
    becoming_admin: bool,
) -> Result<()> {
    let skeletons = name_skeletons(name);
    let mut st = c
        .prepare(
            "SELECT u.id,u.name FROM users u WHERE (?1 IS NULL OR u.id<>?1)
         AND (?2 OR EXISTS (SELECT 1 FROM role_capabilities r
              WHERE r.role=u.role AND r.capability='people.manage'))",
        )
        .map_err(sql)?;
    let rows = st
        .query_map(params![exclude, becoming_admin], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(sql)?;
    for row in rows {
        let (_, existing) = row.map_err(sql)?;
        let existing_skeletons = name_skeletons(&existing);
        if skeletons.iter().any(|s| existing_skeletons.contains(s)) {
            return Err(Error::invalid(
                "choose a different name: it resembles an administrator's name",
            ));
        }
    }
    Ok(())
}

/// A username derived from a name: lowercase ASCII letters, digits and
/// dashes. People can change it later; it's never shown before sign-in.
pub(crate) fn username_for(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out
        .trim_end_matches('-')
        .chars()
        .take(32)
        .collect::<String>();
    if out.is_empty() { "owner".into() } else { out }
}

#[cfg(test)]
#[path = "setup/validation_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "setup/owner_code_tests.rs"]
mod owner_code_tests;
