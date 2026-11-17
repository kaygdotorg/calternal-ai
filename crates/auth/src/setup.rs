//! Setup tokens: how the first owner gets in, and how someone who lost
//! every device gets back in.
//!
//! - On first start, while the server has no owner, it prints a long
//!   first-owner token in its log. Pasting it on the first screen, with a
//!   name, creates the owner and their first passkey.
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
}

/// Issues a fresh first-owner token while nobody owns the server.
/// The optional protected recovery barrier precedes acknowledgement.
pub(crate) async fn first_owner_token(inner: &Arc<Inner>) -> Result<Option<String>> {
    let ttl = inner.config.setup_ttl.as_millis() as i64;
    inner
        .db
        .run(move |c| {
            let tx = c.transaction().map_err(sql)?;
            let users: i64 = tx
                .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
                .map_err(sql)?;
            if users > 0 {
                return Ok(None);
            }
            tx.execute(
                "DELETE FROM setup_tokens WHERE purpose = 'first-owner' AND used_at IS NULL",
                [],
            )
            .map_err(sql)?;
            let token = new_setup_token()?;
            insert_token(&tx, &token, "first-owner", None, ttl)?;
            crate::spaces::commit(tx)?;
            Ok(Some(token))
        })
        .await
}

/// Who `who` (an exact username or UUID) is: their id and name.
pub(crate) fn find_in(c: &Connection, who: &str) -> Result<(String, String)> {
    // Display names are never identity selectors, even when currently unique.
    // BINARY overrides the username column's case-insensitive uniqueness collation.
    let id = AccountId::parse(who)
        .filter(|id| who.eq_ignore_ascii_case(&id.to_string()))
        .map(|id| id.to_string());
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
    let id = AccountId::new().to_string();
    tx.execute(
        "INSERT INTO users (id, name, username, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, name, username, now_ms()],
    )
    .map_err(sql)?;
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
    insert_token(&tx, &token, "recover", Some(&id), ttl)?;
    crate::spaces::commit(tx)?;
    Ok(Created {
        id: AccountId::parse(&id).ok_or_else(|| Error::internal("bad account id"))?,
        name,
        username,
        token,
    })
}

fn insert_token(
    tx: &rusqlite::Transaction<'_>,
    token: &str,
    purpose: &str,
    user: Option<&str>,
    ttl: i64,
) -> Result<()> {
    let canon = normalize_setup_token(token).ok_or_else(|| Error::internal("bad new token"))?;
    let now = now_ms();
    tx.execute(
        "INSERT INTO setup_tokens (hash, purpose, user_id, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![hash(&canon), purpose, user, now, now + ttl],
    )
    .map_err(sql)?;
    Ok(())
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
    let now = now_ms();
    let row: Option<(String, Option<String>)> = inner
        .db
        .run(move |c| {
            c.query_row(
                "SELECT purpose, user_id FROM setup_tokens
                 WHERE hash = ?1 AND used_at IS NULL AND expires_at > ?2",
                params![h, now],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(sql)
        })
        .await?;
    let (purpose, user) = row
        .filter(|_| canon.is_some())
        .ok_or_else(|| invalid("token invalid or expired"))?;
    if purpose == "first-owner" {
        let name = clean_person_name(name).ok_or_else(|| Error::invalid("a name is needed"))?;
        return Ok(Redeemable {
            token_hash,
            user: AccountId::new(),
            name,
            first_owner: true,
        });
    }
    let user = user
        .as_deref()
        .and_then(AccountId::parse)
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
) -> Result<(InstallationId, String)> {
    let ttl = inner.config.session_ttl.as_millis() as i64;
    let out = inner
        .db
        .run(move |c| {
            let tx = c
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(sql)?;
            let now = now_ms();
                let used = tx
                    .execute(
                        "UPDATE setup_tokens SET used_at = ?2
                     WHERE hash = ?1 AND used_at IS NULL AND expires_at > ?2",
                        params![r.token_hash, now],
                    )
                    .map_err(sql)?;
                if used != 1 {
                    return Err(invalid("token used or expired meanwhile"));
                }
                if r.first_owner {
                    let users: i64 = tx
                        .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
                        .map_err(sql)?;
                    if users > 0 {
                        return Err(invalid("the server already has an owner"));
                    }
                    let username = username_for(&r.name);
                    tx.execute(
                    "INSERT INTO users (id, name, username, created_at) VALUES (?1, ?2, ?3, ?4)",
                    params![r.user.to_string(), r.name, username, now],
                )
                .map_err(sql)?;
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
            crate::spaces::commit(tx)?;
            Ok(out)
        })
        .await?;
    Ok(out)
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
/// administrator impersonation. No person-name update API currently exists;
/// future updates must use this same guard and exclude their own UUID.
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
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::test_support::TempDir;

    #[test]
    fn usernames() {
        assert_eq!(username_for("K Gopal Krishna"), "k-gopal-krishna");
        assert_eq!(username_for("  kayg!! "), "kayg");
        assert_eq!(username_for("स्वेता"), "owner");
        assert_eq!(username_for(&"a".repeat(100)).len(), 32);
    }

    #[test]
    fn names() {
        assert_eq!(clean_person_name("  kayg  "), Some("kayg".into()));
        assert_eq!(clean_person_name("   "), None);
        assert_eq!(clean_person_name(&"x".repeat(81)), None);
    }

    #[test]
    fn names_reject_invisible_and_normalize_nfc() {
        for name in [
            "ka\u{202e}yg",
            "ka\u{200d}yg",
            "ka\u{200b}yg",
            "ka\u{00ad}yg",
            "ka\n yg",
            "ka\u{034f}yg",
            "ka\u{fe0f}yg",
        ] {
            assert_eq!(clean_person_name(name), None, "{name:?}");
        }
        assert_eq!(clean_person_name("  Cafe\u{301} "), Some("Café".into()));
    }

    #[test]
    fn recovery_requires_exact_username_or_uuid_never_display_name() {
        let dir = TempDir::new();
        let path = dir.path().join("system/auth.db");
        drop(Db::open(&path).unwrap());
        let mut c = open_conn(&path).unwrap();
        let owner = AccountId::new().to_string();
        let other = AccountId::new().to_string();
        for (id, username) in [(&owner, "owner-login"), (&other, "other-login")] {
            c.execute(
                "INSERT INTO users (id,name,username,created_at) VALUES (?1,'Kay',?2,0)",
                params![id, username],
            )
            .unwrap();
        }
        for selector in ["Kay", "OWNER-LOGIN", " owner-login "] {
            assert_eq!(
                recover_in(&mut c, selector, 1000).unwrap_err().code(),
                ErrorCode::NotFound
            );
        }
        let tokens: i64 = c
            .query_row("SELECT count(*) FROM setup_tokens", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tokens, 0, "refused selectors must not issue any token");
        assert_eq!(find_in(&c, "owner-login").unwrap().0, owner);
        assert_eq!(find_in(&c, &other).unwrap().0, other);
        recover_in(&mut c, &owner, 1000).unwrap();
        // A 32-character hexadecimal username is also valid UUID syntax.
        let hex_username = AccountId::new().0.simple().to_string();
        c.execute(
            "UPDATE users SET username=?2 WHERE id=?1",
            params![other, hex_username],
        )
        .unwrap();
        assert_eq!(find_in(&c, &hex_username).unwrap().0, other);
        // Refuse legacy data where two accepted identifiers collide.
        c.execute(
            "UPDATE users SET username=?2 WHERE id=?1",
            params![other, owner],
        )
        .unwrap();
        assert_eq!(
            recover_in(&mut c, &owner, 1000).err().unwrap().code(),
            ErrorCode::InvalidInput
        );
        let tokens: i64 = c
            .query_row("SELECT count(*) FROM setup_tokens", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            tokens, 1,
            "ambiguous selectors must not replace or issue tokens"
        );
    }

    #[test]
    fn cli_creation_refuses_admin_name_confusables() {
        let cases: &[(&str, &[&str])] = &[
            ("Kay", &["KAY", "Ｋａｙ", "Kаy", "Kay"]),
            ("Owner", &["0wner"]),
            ("Ivy", &["lvy"]),
            ("Straße", &["STRASSE"]),
            ("Café", &["Cafe\u{301}"]),
        ];
        let mut failures = Vec::new();
        for (admin, impostors) in cases {
            let dir = TempDir::new();
            let path = dir.path().join("system/auth.db");
            drop(Db::open(&path).unwrap());
            let mut c = open_conn(&path).unwrap();
            create_in(&mut c, admin, 1000).unwrap();
            for name in *impostors {
                if create_in(&mut c, name, 1000).is_ok() {
                    failures.push(format!("{name:?} impersonates {admin:?}"));
                }
            }
            let count: i64 = c
                .query_row("SELECT count(*) FROM users", [], |r| r.get(0))
                .unwrap();
            if count != 1 {
                failures.push(format!("{admin:?}: expected one user, found {count}"));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("; "));
    }

    #[test]
    fn recover_needs_a_known_person() {
        let dir = TempDir::new();
        let path = dir.path().join("system/auth.db");
        drop(Db::open(&path).unwrap());
        let mut c = open_conn(&path).unwrap();
        let err = recover_in(&mut c, "nobody", 1000).unwrap_err();
        assert_eq!(err.code(), ErrorCode::NotFound);
        c.execute(
            "INSERT INTO users (id, name, username, created_at) VALUES ('u1', 'Kayg', 'kayg', 0)",
            [],
        )
        .unwrap();
        let (name, t1) = recover_in(&mut c, "kayg", 1000).unwrap();
        assert_eq!(name, "Kayg");
        let (_, t2) = recover_in(&mut c, "kayg", 1000).unwrap();
        assert_ne!(t1, t2);
        let live: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM setup_tokens WHERE used_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(live, 1, "a new recovery token replaces the old one");
    }
}
