//! Single-use space invitations, adapted from the source invite actions and setup.
use crate::db::{now_ms, sql};
use crate::people::{page_value, take_one_more};
use crate::spaces::{assignable_role, is_owner, privileged, require};
use crate::{AccountId, Auth, Error, ErrorCode, Result, SpaceId};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use calternal_core::PeopleOperation;
use calternal_types::InviteId;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

const PREFIX: &str = "invite_";
const DEFAULT_TTL: i64 = 7 * 24 * 60 * 60 * 1000;
const PURGE_BATCH: usize = 256;

/// Default days to keep an invitation after its first terminal event.
pub const DEFAULT_INVITE_RETENTION_DAYS: u32 = 7;
/// Maximum usable invitations each space may hold concurrently.
pub const MAX_LIVE_SPACE_INVITES: usize = 100;

pub(crate) fn invalid() -> Error {
    Error::new(ErrorCode::SetupTokenInvalid, "invite invalid or expired")
}
fn missing() -> Error {
    Error::new(ErrorCode::NotFound, "invite not found")
}

pub(crate) struct Grant {
    pub id: String,
    space: String,
    role: String,
}

/// Looks up the entire candidate's keyed hash and checks it in constant time.
pub(crate) fn find(c: &Connection, hash: &[u8; 32], now: i64) -> Result<Option<Grant>> {
    let row = c
        .query_row(
            "SELECT id,space_id,space_role,hash,expires_at,used_at IS NULL,revoked_at IS NULL
         FROM invites WHERE hash=?1",
            [hash.as_slice()],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Vec<u8>>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, bool>(5)?,
                    r.get::<_, bool>(6)?,
                ))
            },
        )
        .optional()
        .map_err(sql)?;
    let stored = row.as_ref().map_or(&[0; 32][..], |row| row.3.as_slice());
    let matches = crate::token::same_hash(hash, stored);
    Ok(
        row.and_then(|(id, space, role, _, expiry, unused, active)| {
            if matches & (expiry > now) & unused & active {
                Some(Grant {
                    id,
                    space: space?,
                    role: role?,
                })
            } else {
                None
            }
        }),
    )
}

pub(crate) fn consume(
    c: &Connection,
    grant: Grant,
    account: AccountId,
    now: i64,
) -> Result<SpaceId> {
    let account = account.to_string();
    let changed = c
        .execute(
            "UPDATE space_memberships SET role=?3 WHERE space_id=?1 AND account_id=?2
         AND role<>?3 AND NOT EXISTS(SELECT 1 FROM spaces WHERE id=?1 AND owner_account=?2)
         AND NOT EXISTS(
           SELECT capability FROM space_role_capabilities WHERE role=space_memberships.role
           EXCEPT SELECT capability FROM space_role_capabilities WHERE role=?3
         )",
            params![grant.space, account, grant.role],
        )
        .map_err(sql)?;
    if changed == 0 {
        c.execute(
            "INSERT OR IGNORE INTO space_memberships(space_id,account_id,role) VALUES(?1,?2,?3)",
            params![grant.space, account, grant.role],
        )
        .map_err(sql)?;
    }
    c.execute(
        "UPDATE invites SET used_at=?2 WHERE id=?1",
        params![grant.id, now],
    )
    .map_err(sql)?;
    SpaceId::parse(&grant.space).map_err(Error::internal)
}

impl Auth {
    /// Creates an encrypted single-use invitation. Its receipt holds only the
    /// public invitation ID and expiry. An authorized list can return the link.
    /// Cost: one immediate transaction and the protected publication barrier.
    pub fn create_invite(
        &self,
        actor: AccountId,
        space: SpaceId,
        role: String,
        expires_at: Option<i64>,
        receipt: (&str, &str, &str),
    ) -> Result<Value> {
        let inner = self.inner.clone();
        self.spaces().mutate(actor, Some(space), Some(receipt), move |tx| {
            require(tx, actor, space, PeopleOperation::CreateInvite.capability().name())?;
            assignable_role(tx, actor, space, &role)?;
            Ok(move |tx: &rusqlite::Transaction<'_>| {
                let now = now_ms();
                let expires_at = expires_at.unwrap_or_else(|| now.saturating_add(DEFAULT_TTL));
                if expires_at <= now {
                    return Err(Error::invalid("invite expiry must be in the future"));
                }
                let live: i64 = tx.query_row(
                    "SELECT count(*) FROM (
                       SELECT 1 FROM invites WHERE space_id=?1 AND expires_at>?2
                        AND used_at IS NULL AND revoked_at IS NULL LIMIT ?3
                     )",
                    params![space.to_string(), now, MAX_LIVE_SPACE_INVITES as i64],
                    |row| row.get(0),
                ).map_err(sql)?;
                if live >= MAX_LIVE_SPACE_INVITES as i64 {
                    return Err(Error::new(ErrorCode::InviteLimit, "live invitation limit reached"));
                }
                let id = InviteId::new().to_string();
                let secret = format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(crate::token::random32()?));
                let hash = inner.invite_key.lookup(&secret);
                let encrypted = inner.invite_key.encrypt(&secret, &id)?;
                tx.execute(
                    "INSERT INTO invites(id,created_by,hash,secret,created_at,expires_at,space_id,space_role)
                     VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![id,actor.to_string(),hash.as_bytes().as_slice(),encrypted,now,expires_at,space.to_string(),role],
                ).map_err(sql)?;
                Ok(json!({"invite_id":id,"expires_at":expires_at}))
            })
        })
    }

    /// Lists one UUID-ordered invitation page in an authorized snapshot.
    /// Managers see their own ordinary grants. The Owner can see all grants.
    /// The server decrypts only usable links on the returned page.
    pub fn list_invites(
        &self,
        actor: AccountId,
        space: SpaceId,
        after: Option<InviteId>,
    ) -> Result<Value> {
        let inner = self.inner.clone();
        self.spaces().run(move |c| {
            let tx = c.transaction().map_err(sql)?;
            require(&tx, actor, space, PeopleOperation::ListInvites.capability().name())?;
            let owner = is_owner(&tx, actor, space)?;
            let now = now_ms();
            let mut stmt = tx.prepare_cached(
                "WITH privileged(role) AS (
                   SELECT DISTINCT r.role FROM space_role_capabilities r
                   WHERE NOT EXISTS(SELECT 1 FROM space_role_capabilities editor
                                    WHERE editor.role='editor' AND editor.capability=r.capability)
                 )
                 SELECT id,space_role,created_at,expires_at,used_at,revoked_at,secret
                 FROM invites WHERE space_id=?1 AND id>?5 AND expires_at>?2
                   AND (?3 OR (created_by=?4 AND space_role NOT IN (SELECT role FROM privileged)))
                 ORDER BY id LIMIT ?6",
            ).map_err(sql)?;
            let after = after.map_or_else(String::new, |id| id.to_string());
            let mut rows = stmt.query(params![space.to_string(), now, owner, actor.to_string(), after, take_one_more() as i64]).map_err(sql)?;
            let mut invites = Vec::new();
            while let Some(row) = rows.next().map_err(sql)? {
                let id: String = row.get(0).map_err(sql)?;
                let role: String = row.get(1).map_err(sql)?;
                let used: Option<i64> = row.get(4).map_err(sql)?;
                let revoked: Option<i64> = row.get(5).map_err(sql)?;
                let link = if invites.len() < calternal_core::DIRECTORY_PAGE_SIZE
                    && used.is_none() && revoked.is_none() {
                    let blob: Vec<u8> = row.get(6).map_err(sql)?;
                    Some(format!("{}/invites/{}", inner.config.origin.as_str().trim_end_matches('/'),
                        inner.invite_key.decrypt(&blob, &id)?))
                } else { None };
                invites.push(json!({"invite_id":id,"role":role,"created_at":row.get::<_,i64>(2).map_err(sql)?,
                    "expires_at":row.get::<_,i64>(3).map_err(sql)?,"used_at":used,"revoked_at":revoked,"link":link}));
            }
            Ok(page_value("invites", invites, "invite_id"))
        })
    }

    /// Remove one indexed batch of up to 256 invitation rows older than
    /// retention, measured from the first of expiry, redemption and revocation.
    /// Publishes deletions through the protected authority commit. Returns true
    /// if a full batch was deleted and another batch may be due.
    pub async fn purge_invites(&self) -> Result<bool> {
        let age = i64::try_from(self.inner.config.invite_retention.as_millis()).unwrap_or(i64::MAX);
        let cutoff = now_ms().saturating_sub(age);
        self.inner.db.run(move |c| {
            let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(sql)?;
            let deleted = tx.execute(
                "DELETE FROM invites WHERE id IN (
                   SELECT id FROM invites
                   WHERE min(expires_at, coalesce(used_at, expires_at), coalesce(revoked_at, expires_at))<=?1
                   ORDER BY min(expires_at, coalesce(used_at, expires_at), coalesce(revoked_at, expires_at))
                   LIMIT ?2
                 )",
                params![cutoff, PURGE_BATCH as i64],
            ).map_err(sql)?;
            if deleted > 0 {
                crate::spaces::commit(tx)?;
            }
            Ok(deleted == PURGE_BATCH)
        }).await
    }

    /// Revokes an unused link in its own space. Rechecks the grant before replay
    /// and publishes the revocation through the protected recovery barrier.
    pub fn revoke_invite(
        &self,
        actor: AccountId,
        space: SpaceId,
        id: InviteId,
        receipt: (&str, &str, &str),
    ) -> Result<Value> {
        self.spaces().mutate(actor, Some(space), Some(receipt), move |tx| {
            require(tx, actor, space, PeopleOperation::RevokeInvite.capability().name())?;
            let (creator, role): (String, String) = tx.query_row(
                "SELECT created_by,space_role FROM invites WHERE space_id=?1 AND id=?2",
                params![space.to_string(),id.to_string()], |r| Ok((r.get(0)?,r.get(1)?)),
            ).optional().map_err(sql)?.ok_or_else(missing)?;
            if !is_owner(tx, actor, space)? && (creator != actor.to_string() || privileged(tx, &role)?) {
                return Err(missing());
            }
            Ok(move |tx: &rusqlite::Transaction<'_>| {
                if tx.execute(
                    "UPDATE invites SET revoked_at=coalesce(revoked_at,?3) WHERE space_id=?1 AND id=?2
                      AND used_at IS NULL AND expires_at>?3",
                    params![space.to_string(),id.to_string(),now_ms()],
                ).map_err(sql)? == 0 {
                    return Err(missing());
                }
                Ok(json!({"invite_id":id}))
            })
        })
    }

    /// Redeems an invitation into an already signed-in account. Unknown, expired,
    /// consumed and revoked links share one error; an exact private receipt replay
    /// does not consume it twice. Cost: one protected transaction and one current
    /// space-grant check before returning the space ID.
    pub fn redeem_invite(
        &self,
        actor: AccountId,
        secret: String,
        receipt: (&str, &str, &str),
    ) -> Result<Value> {
        let hash = self.inner.invite_key.lookup(&secret);
        let spaces = self.spaces();
        let value = spaces.mutate(actor, None, Some(receipt), move |tx| {
            crate::people::require_active(tx, actor)?;
            Ok(move |tx: &rusqlite::Transaction<'_>| {
                let now = now_ms();
                let grant = find(tx, hash.as_bytes(), now)?.ok_or_else(invalid)?;
                let space = consume(tx, grant, actor, now)?;
                Ok(json!({"space_id":space}))
            })
        })?;
        let scope: SpaceId =
            serde_json::from_value(value["space_id"].clone()).map_err(Error::internal)?;
        if spaces.capabilities(actor, scope)?.is_none() {
            return Err(invalid());
        }
        Ok(value)
    }
}
