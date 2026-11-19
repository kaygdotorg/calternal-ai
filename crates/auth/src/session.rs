//! Installations: one per signed-in client.
//!
//! Each Installation has a random token. Browsers keep it in an HttpOnly
//! cookie; the CLI, agents and native apps will send it as a bearer token.
//! Only the token's hash is stored. Checking a session is a memory lookup
//! after the first request, so it costs nothing on the hot path. A session
//! that isn't used for `session_ttl` expires; each use pushes that out.

use crate::Inner;
use crate::db::{now_ms, sql};
use crate::token::{hash, new_session_token, plausible_session_token};
use crate::{AccountId, Error, ErrorCode, InstallationId, Result};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::RwLock;

/// How often a session's "last seen" time is written back, at most.
const TOUCH_EVERY_MS: i64 = 60 * 60 * 1000;

/// What kind of client an Installation is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallationKind {
    /// A web browser or the installed web app.
    Browser,
    /// The macOS or iOS app.
    Native,
    /// The command line tool.
    Cli,
    /// An AI agent, through MCP or the API.
    Agent,
}

impl InstallationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Browser => "browser",
            Self::Native => "native",
            Self::Cli => "cli",
            Self::Agent => "agent",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "browser" => Self::Browser,
            "native" => Self::Native,
            "cli" => Self::Cli,
            "agent" => Self::Agent,
            _ => return None,
        })
    }
}

/// One signed-in client, as shown in the list of Installations.
#[derive(Debug, Clone, Serialize)]
pub struct Installation {
    /// Its id.
    pub id: InstallationId,
    /// What kind of client it is.
    pub kind: InstallationKind,
    /// A name people recognise, like "Safari on iPhone".
    pub name: String,
    /// When it signed in, in Unix milliseconds.
    pub created_at: i64,
    /// When it was last used, in Unix milliseconds (updated hourly).
    pub last_seen_at: i64,
}

#[derive(Clone, Copy)]
struct Live {
    user: AccountId,
    installation: InstallationId,
    expires_at: i64,
    touched_at: i64,
}

/// Most sessions kept in the cache; a full cache first drops expired entries,
/// then starts empty again (it is rebuilt from `auth.db` on demand).
const CACHE_MAX: usize = 4096;
/// How long a revocation is remembered in memory. It only has to outlive a
/// database lookup that started before the revoke committed; afterwards the
/// database itself no longer finds the session.
const REVOKED_KEEP_MS: i64 = 10 * 60 * 1000;

/// Sessions seen recently, by token hash. Bounded: at most [`CACHE_MAX`] cached
/// sessions, and revocations for [`REVOKED_KEEP_MS`].
#[derive(Default)]
pub(crate) struct Sessions {
    cache: RwLock<HashMap<Vec<u8>, Live>>,
    /// Installations revoked recently, with when. Checked on every request, so a
    /// lookup racing a revoke can never bring a session back.
    revoked: RwLock<HashMap<InstallationId, i64>>,
}

impl Sessions {
    fn forget_installation(&self, id: InstallationId) {
        self.forget_installations(vec![id]);
    }

    pub(crate) fn forget_installations(&self, ids: Vec<InstallationId>) {
        if ids.is_empty() {
            return;
        }
        let now = now_ms();
        let mut revoked = self.revoked.write().unwrap_or_else(|p| p.into_inner());
        revoked.retain(|_, at| now - *at < REVOKED_KEEP_MS);
        revoked.extend(ids.into_iter().map(|id| (id, now)));
        let mut cache = self.cache.write().unwrap_or_else(|p| p.into_inner());
        cache.retain(|_, live| !revoked.contains_key(&live.installation));
    }

    pub(crate) fn is_revoked(&self, id: InstallationId) -> bool {
        self.revoked
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .contains_key(&id)
    }

    /// Caches a looked-up session, keeping the cache within [`CACHE_MAX`].
    /// Cost: O(1), or O(cache) when full.
    fn remember(&self, key: Vec<u8>, live: Live, now: i64) {
        let mut cache = self.cache.write().unwrap_or_else(|p| p.into_inner());
        if cache.len() >= CACHE_MAX {
            cache.retain(|_, l| l.expires_at > now);
            if cache.len() >= CACHE_MAX {
                cache.clear();
            }
        }
        cache.insert(key, live);
    }
}

/// Creates an installation for an active account in the caller’s transaction.
/// Cost: one account status read and one session insert before protected commit.
pub(crate) fn create(
    tx: &Transaction<'_>,
    user: AccountId,
    kind: InstallationKind,
    name: &str,
    ttl_ms: i64,
    asserted: bool,
) -> Result<(InstallationId, String)> {
    crate::people::require_active(tx, user)?;
    let id = InstallationId::new();
    let token = new_session_token()?;
    let now = now_ms();
    tx.execute(
        "INSERT INTO installations (id, user_id, kind, name, token_hash, created_at, last_seen_at, expires_at, assertion_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7, ?8)",
        params![
            id.to_string(),
            user.to_string(),
            kind.as_str(),
            clean_name(name),
            hash(&token),
            now,
            now + ttl_ms,
            asserted.then_some(now)
        ],
    )
    .map_err(sql)?;
    Ok((id, token))
}

/// Who a token belongs to, if it's a live session. Expiry is judged against
/// [`AuthConfig::clock`](crate::AuthConfig::clock); a cache miss costs one
/// database read on the auth worker.
pub(crate) async fn identify(
    inner: &std::sync::Arc<Inner>,
    token: &str,
) -> Option<(AccountId, InstallationId)> {
    if !plausible_session_token(token) {
        return None;
    }
    let key = hash(token);
    // Expiry is judged by the configured clock, so tests can advance time.
    let now = inner.config.clock.now();
    let cached = {
        let c = inner
            .sessions
            .cache
            .read()
            .unwrap_or_else(|p| p.into_inner());
        c.get(&key).copied()
    };
    let live = match cached {
        Some(l) => l,
        None => {
            let k = key.clone();
            let found = inner.db.read(move |c| lookup(c, &k)).await.ok().flatten()?;
            inner.sessions.remember(key.clone(), found, now);
            found
        }
    };
    if live.expires_at <= now || inner.sessions.is_revoked(live.installation) {
        let mut cache = inner
            .sessions
            .cache
            .write()
            .unwrap_or_else(|p| p.into_inner());
        cache.remove(&key);
        return None;
    }
    if now - live.touched_at > TOUCH_EVERY_MS {
        touch(inner, key, live, now);
    }
    Some((live.user, live.installation))
}

/// Extends a session and records use in the background. Last-seen/rolling expiry
/// touches are intentionally excluded from recovery publication: losing a touch
/// can shorten a recovered session, while explicit revocations use the barrier.
fn touch(inner: &std::sync::Arc<Inner>, key: Vec<u8>, mut live: Live, now: i64) {
    let ttl = inner.config.session_ttl.as_millis() as i64;
    live.touched_at = now;
    live.expires_at = now + ttl;
    {
        let mut cache = inner
            .sessions
            .cache
            .write()
            .unwrap_or_else(|p| p.into_inner());
        cache.insert(key, live);
    }
    let db = inner.db.clone();
    let id = live.installation.to_string();
    tokio::spawn(async move {
        let r = db
            .run(move |c| {
                c.execute(
                    "UPDATE installations SET last_seen_at = ?2, expires_at = ?3
                     WHERE id = ?1 AND revoked_at IS NULL",
                    params![id, now, now + ttl],
                )
                .map_err(sql)
            })
            .await;
        if let Err(e) = r {
            tracing::warn!(detail = %e, "could not record session use");
        }
    });
}

fn lookup(c: &rusqlite::Connection, key: &[u8]) -> Result<Option<Live>> {
    c.query_row(
        "SELECT user_id, id, expires_at, last_seen_at FROM installations
         WHERE token_hash = ?1 AND revoked_at IS NULL AND kind <> 'dav'",
        params![key],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, i64>(3)?,
            ))
        },
    )
    .optional()
    .map_err(sql)?
    .map(|(u, i, exp, seen)| {
        Ok(Live {
            user: AccountId::parse(&u).map_err(|_| Error::internal("bad user id"))?,
            installation: InstallationId::parse(&i)
                .map_err(|_| Error::internal("bad installation id"))?,
            expires_at: exp.unwrap_or(i64::MAX),
            touched_at: seen,
        })
    })
    .transpose()
}

/// Every live Installation of a user, newest first.
pub(crate) async fn list(inner: &Inner, user: AccountId) -> Result<Vec<Installation>> {
    let now = now_ms();
    // Room authorization samples the authority epoch around this session
    // read. Only changes belong on the auth worker's write queue.
    inner
        .db
        .read(move |c| {
            let mut st = c
                .prepare(
                    "SELECT i.id, i.kind, i.name, i.created_at, i.last_seen_at
                     FROM installations i
                     WHERE i.user_id = ?1 AND i.revoked_at IS NULL
                       AND (i.expires_at IS NULL OR i.expires_at > ?2)
                     ORDER BY i.created_at DESC",
                )
                .map_err(sql)?;
            let rows = st
                .query_map(params![user.to_string(), now], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                    ))
                })
                .map_err(sql)?;
            let mut out = Vec::new();
            for row in rows {
                let (id, kind, name, created_at, last_seen_at) = row.map_err(sql)?;
                out.push(Installation {
                    id: InstallationId::parse(&id)
                        .map_err(|_| Error::internal("bad installation id"))?,
                    kind: InstallationKind::parse(&kind)
                        .ok_or_else(|| Error::internal("bad installation kind"))?,
                    name,
                    created_at,
                    last_seen_at,
                });
            }
            Ok(out)
        })
        .await
}

/// Signs an Installation out. Only its owner may do this. Takes effect
/// immediately for every request after this one.
pub(crate) async fn revoke(
    inner: &std::sync::Arc<Inner>,
    user: AccountId,
    id: InstallationId,
) -> Result<()> {
    revoke_matching(inner, user, id, false).await
}

/// Revokes under an IMMEDIATE transaction and waits for configured recovery.
async fn revoke_matching(
    inner: &std::sync::Arc<Inner>,
    user: AccountId,
    id: InstallationId,
    only_dav: bool,
) -> Result<()> {
    let now = now_ms();
    let fence = inner.clone();
    let changed = inner
        .db
        .run(move |c| {
            let tx = c
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(sql)?;
            let changed = tx
                .execute(
                    "UPDATE installations SET revoked_at = ?3
                 WHERE id = ?1 AND user_id = ?2 AND revoked_at IS NULL
                   AND (?4 = 0 OR kind = 'dav')",
                    params![id.to_string(), user.to_string(), now, only_dav as i64],
                )
                .map_err(sql)?;
            crate::spaces::commit_with(tx, || {
                if changed != 0 {
                    fence.sessions.forget_installation(id);
                }
            })?;
            Ok(changed)
        })
        .await?;
    if changed == 0 {
        // Not this person's (or already gone). Touch nothing: marking the
        // id revoked here would sign out whoever it does belong to (#409).
        return Err(Error::new(ErrorCode::NotFound, "no such installation"));
    }
    Ok(())
}

/// Recent passkey proof from a browser/native session. Registration counts
/// for managing passkeys; app-password minting still needs a sign-in assertion.
pub(crate) async fn require_fresh(
    inner: &std::sync::Arc<Inner>,
    user: AccountId,
    installation: Option<InstallationId>,
    registration_ok: bool,
) -> Result<()> {
    let Some(installation) = installation else {
        return Err(Error::new(ErrorCode::Forbidden, "sign in again first"));
    };
    if inner.sessions.is_revoked(installation) {
        return Err(Error::new(ErrorCode::Forbidden, "session revoked"));
    }
    let config = inner.config.clone();
    inner
        .db
        .run(move |connection| {
            check_fresh(connection, &config, user, installation, registration_ok)
        })
        .await
}

/// Check the live installation and its passkey timestamp within the caller's
/// existing authority transaction. One indexed row read and no flush.
pub(crate) fn check_fresh(
    connection: &Connection,
    config: &crate::AuthConfig,
    user: AccountId,
    installation: InstallationId,
    registration_ok: bool,
) -> Result<()> {
    let (kind, asserted, registration): (String, Option<i64>, bool) = connection.query_row(
        "SELECT kind, assertion_at, registration_session FROM installations WHERE id=?1 AND user_id=?2 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>?3)",
        params![installation.to_string(), user.to_string(), now_ms()],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
    ).optional().map_err(sql)?.ok_or_else(|| Error::new(ErrorCode::Forbidden, "session revoked"))?;
    if !matches!(kind.as_str(), "browser" | "native") {
        return Err(Error::new(
            ErrorCode::PasskeyReverificationRequired,
            "this needs a signed-in browser or app",
        ));
    }
    let window = i64::try_from(config.app_password_fresh.as_millis()).unwrap_or(i64::MAX);
    match asserted {
        Some(at)
            if (registration_ok || !registration)
                && crate::db::now_ms().saturating_sub(at) <= window =>
        {
            Ok(())
        }
        _ => Err(Error::new(
            ErrorCode::PasskeyReverificationRequired,
            "sign in again with your passkey first",
        )),
    }
}

/// Characters that draw nothing or reorder text: controls, bidi controls
/// (including U+061C), zero-width and other default-ignorable characters,
/// variation selectors and tags. They let one name look like another.
pub(crate) fn hidden_char(c: char) -> bool {
    c.is_control()
        || matches!(c,
            '\u{00ad}' | '\u{034f}' | '\u{061c}' | '\u{115f}'..='\u{1160}'
            | '\u{17b4}'..='\u{17b5}' | '\u{180b}'..='\u{180f}'
            | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}' | '\u{3164}' | '\u{fe00}'..='\u{fe0f}'
            | '\u{feff}' | '\u{ffa0}' | '\u{fff0}'..='\u{fff8}'
            | '\u{e0000}'..='\u{e0fff}')
}

/// Installation names come from the client. Keep them short and printable.
pub(crate) fn clean_name(name: &str) -> String {
    let s: String = name.chars().filter(|c| !hidden_char(*c)).take(80).collect();
    let s = s.trim();
    if s.is_empty() {
        "Browser".into()
    } else {
        s.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name(""), "Browser");
        assert_eq!(clean_name("  Safari\n on iPhone\u{7} "), "Safari on iPhone");
        assert_eq!(clean_name(&"x".repeat(500)).len(), 80);
        assert_eq!(clean_name("Mac\u{202e}book\u{200b}\u{feff}"), "Macbook");
        for c in [
            '\u{061c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}',
            '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
        ] {
            assert_eq!(clean_name(&format!("a{c}b")), "ab");
        }
        assert_eq!(clean_name("a\u{061c}b\u{00ad}c\u{e0041}d\u{fe0f}"), "abcd");
    }
}

#[cfg(test)]
mod port_tests {
    use super::*;
    #[tokio::test]
    async fn hourly_touch_extends_expiry_without_storing_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let auth = crate::Auth::open(
            dir.path(),
            crate::AuthConfig::new("http://localhost:8080").unwrap(),
        )
        .unwrap();
        let made = auth.create("Alice").await.unwrap();
        let (installation, token) = auth
            .inner
            .db
            .run(move |c| {
                let tx = c.transaction().map_err(sql)?;
                let out = create(
                    &tx,
                    made.id,
                    InstallationKind::Browser,
                    "Mac",
                    2 * TOUCH_EVERY_MS,
                    true,
                )?;
                tx.execute(
                    "UPDATE installations SET last_seen_at=?2 WHERE id=?1",
                    params![out.0.to_string(), now_ms() - TOUCH_EVERY_MS - 1],
                )
                .map_err(sql)?;
                tx.commit().map_err(sql)?;
                Ok(out)
            })
            .await
            .unwrap();
        let before = now_ms();
        assert_eq!(auth.identify(&token).await, Some((made.id, installation)));
        let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let (seen, expiry, stored): (i64, i64, Vec<u8>) = auth
                .inner
                .db
                .run(move |c| {
                    c.query_row(
                        "SELECT last_seen_at,expires_at,token_hash FROM installations WHERE id=?1",
                        [installation.to_string()],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )
                    .map_err(sql)
                })
                .await
                .unwrap();
            assert_eq!(stored, hash(&token));
            if seen >= before {
                assert!(expiry >= before + auth.inner.config.session_ttl.as_millis() as i64);
                break;
            }
            assert!(
                std::time::Instant::now() < until,
                "touch did not reach database"
            );
            tokio::task::yield_now().await;
        }
    }
}
