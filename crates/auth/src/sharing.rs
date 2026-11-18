//! Item grants share the directory's protected transactions and publication barrier.

use crate::db::sql;

/// Maximum live account and group grants per item. This policy is provisional.
pub const AUDIENCE_LIMIT: u32 = 100;

/// Maximum live public links per item. This policy is provisional.
pub const LINK_LIMIT: u32 = 32;

mod links;
mod pages;
use crate::people::{can_see_group, can_see_person};
use crate::spaces::{Spaces, has, require};
use crate::{Error, ErrorCode, Result};
use calternal_types::{AccountId, Capability, CapabilitySet, GroupId, SpaceId};
pub use links::{LinkEdit, ResolvedLink};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use uuid::Uuid;

/// A directory identity receiving a named item grant.
#[derive(Clone, Copy)]
pub enum Audience {
    /// An active account on this installation.
    Account(AccountId),
    /// A group whose current members inherit the grant.
    Group(GroupId),
}

impl Audience {
    fn columns(self) -> (Option<String>, Option<String>) {
        match self {
            Self::Account(id) => (Some(id.to_string()), None),
            Self::Group(id) => (None, Some(id.to_string())),
        }
    }

    fn visible(self, c: &Connection, actor: AccountId) -> Result<bool> {
        match self {
            Self::Account(id) => can_see_person(c, actor, id),
            Self::Group(id) => can_see_group(c, actor, id),
        }
    }
}

pub(super) fn missing() -> Error {
    Error::new(ErrorCode::NotFound, "item or grant not found")
}

fn require_editor(c: &Connection, actor: AccountId, space: SpaceId) -> Result<()> {
    if !has(c, actor, space, Capability::SpaceRead.name())? {
        return Err(missing());
    }
    require(c, actor, space, Capability::ContentWrite.name())
}

fn link_admin(c: &Connection, actor: AccountId, space: SpaceId) -> Result<bool> {
    Ok(has(c, actor, space, Capability::SpaceSettings.name())?
        || (has(c, actor, space, Capability::ContentWrite.name())?
            && c.query_row(
                "SELECT EXISTS(SELECT 1 FROM item_link_policies WHERE space_id=?1)",
                [space.to_string()],
                |r| r.get::<_, bool>(0),
            )
            .map_err(sql)?))
}

impl Spaces {
    /// Read current space capabilities and the strongest named permission in one
    /// cached statement. Collection IDs must come from the host's live reverse
    /// membership index in this space. The result grants no space or plugin
    /// authority by itself. Disabled identities produce no capabilities.
    pub async fn item_authority(
        &self,
        actor: AccountId,
        space: SpaceId,
        id: Uuid,
        collections: Vec<Uuid>,
    ) -> Result<(Option<CapabilitySet>, u8)> {
        self.db
            .read(move |c| item_authority_in(c, actor, space, id, &collections))
            .await
    }

    /// Check the write capability under the caller's item lock immediately
    /// before staging a received edit. Reuses the one protected authority query.
    pub fn can_edit_shared_item(
        &self,
        actor: AccountId,
        space: SpaceId,
        id: Uuid,
        collections: Vec<Uuid>,
    ) -> Result<bool> {
        self.db.read_blocking(move |c| {
            let (caps, permission) = item_authority_in(c, actor, space, id, &collections)?;
            Ok(permission == 2 || caps.is_some_and(|caps| caps.contains(Capability::ContentWrite)))
        })
    }

    /// Assign a visible account or group one stable item grant. A protected
    /// transaction rechecks management authority before replay and enforces the
    /// combined bound on named grants before inserting.
    pub fn grant_item(
        &self,
        actor: AccountId,
        space: SpaceId,
        id: Uuid,
        kind: String,
        audience: Audience,
        edit: bool,
        receipt: (&str, &str, &str),
    ) -> Result<Value> {
        self.mutate(actor, Some(space), Some(receipt), move |c| {
            require_editor(c, actor, space)?;
            if !audience.visible(c, actor)? {
                return Err(missing());
            }
            let (account, group) = audience.columns();
            Ok(move |c: &rusqlite::Transaction<'_>| {
                let item = id.to_string();
                let scope = space.to_string();
                let existing: Option<String> = c.query_row(
                    "SELECT id FROM item_grants WHERE item_id=?1 AND space_id=?2 AND account_id=?3
                     UNION ALL SELECT id FROM item_grants WHERE item_id=?1 AND space_id=?2 AND group_id=?4",
                    params![item, scope, account, group], |r| r.get(0),
                ).optional().map_err(sql)?;
                let permission = if edit { 2 } else { 1 };
                let grant = if let Some(grant) = existing {
                    c.execute("UPDATE item_grants SET permission=?2 WHERE id=?1", params![grant, permission]).map_err(sql)?;
                    grant
                } else {
                    let count: u32 = c.query_row(
                        "SELECT count(*) FROM item_grants WHERE item_id=?1 AND link_hash IS NULL",
                        [&item], |r| r.get(0),
                    ).map_err(sql)?;
                    if count >= AUDIENCE_LIMIT {
                        return Err(Error::invalid("this item already has 100 named grants"));
                    }
                    let grant = Uuid::now_v7().to_string();
                    c.execute(
                        "INSERT INTO item_grants(id,item_id,kind,space_id,created_by,account_id,group_id,permission)
                         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                        params![grant, item, kind, scope, actor.to_string(), account, group, permission],
                    ).map_err(sql)?;
                    grant
                };
                grant_row(c, &grant)
            })
        })
    }

    /// Mark an existing received share as seen. The mark is private to the
    /// recipient, and a current direct or group audience is required.
    pub fn mark_shared_item_seen(
        &self,
        actor: AccountId,
        space: SpaceId,
        id: Uuid,
        receipt: (&str, &str, &str),
    ) -> Result<Value> {
        self.mutate(actor, Some(space), Some(receipt), move |c| {
            let existing: bool = c.query_row(
                "SELECT EXISTS(SELECT 1 FROM item_grants g JOIN users u ON u.id=?1 AND u.disabled_at IS NULL
                   WHERE g.item_id=?2 AND g.space_id=?3
                     AND (g.account_id=?1 OR EXISTS(SELECT 1 FROM group_members m WHERE m.user_id=?1 AND m.group_id=g.group_id)))",
                params![actor.to_string(),id.to_string(),space.to_string()], |r| r.get(0),
            ).map_err(sql)?;
            if !existing {
                return Err(missing());
            }
            Ok(move |c: &rusqlite::Transaction<'_>| {
                c.execute(
                    "INSERT OR IGNORE INTO item_share_seen VALUES(?1,?2,?3)",
                    params![actor.to_string(),space.to_string(),id.to_string()],
                ).map_err(sql)?;
                Ok(json!({"seen":true}))
            })
        })
    }

    /// Remove grant authority and private seen markers for an irreversibly
    /// purged item. The caller establishes the durable item tombstone first.
    /// One protected transaction and no content I/O or cross-space access.
    pub async fn purge_item_grants(&self, space: SpaceId, id: Uuid) -> Result<()> {
        self.db
            .run(move |c| {
                let tx = c
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .map_err(sql)?;
                tx.execute(
                    "DELETE FROM item_grants WHERE space_id=?1 AND item_id=?2",
                    params![space.to_string(), id.to_string()],
                )
                .map_err(sql)?;
                tx.execute(
                    "DELETE FROM item_share_seen WHERE space_id=?1 AND item_id=?2",
                    params![space.to_string(), id.to_string()],
                )
                .map_err(sql)?;
                crate::spaces::commit(tx)
            })
            .await
    }

    /// Remove one item grant in a protected transaction. A public-link revoke
    /// additionally checks the current link policy before a first application.
    pub fn revoke_item(
        &self,
        actor: AccountId,
        space: SpaceId,
        id: Uuid,
        grant: Uuid,
        receipt: (&str, &str, &str),
    ) -> Result<Value> {
        self.mutate(actor, Some(space), Some(receipt), move |c| {
            require_editor(c, actor, space)?;
            let link = link_admin(c, actor, space)?;
            Ok(move |c: &rusqlite::Transaction<'_>| {
                let removed = c
                    .execute(
                        "DELETE FROM item_grants WHERE id=?1 AND item_id=?2 AND space_id=?3
                     AND (?4 OR link_hash IS NULL)",
                        params![grant.to_string(), id.to_string(), space.to_string(), link],
                    )
                    .map_err(sql)?;
                if removed == 0 {
                    return Err(missing());
                }
                Ok(json!({"grant_id": grant}))
            })
        })
    }
}

fn grant_row(c: &Connection, grant: &str) -> Result<Value> {
    c.query_row(
        "SELECT g.permission,g.account_id,g.group_id,coalesce(u.name,m.name),g.label,g.expires_at
         FROM item_grants g LEFT JOIN users u ON u.id=g.account_id LEFT JOIN groups m ON m.id=g.group_id
         WHERE g.id=?1",
        [grant],
        |r| {
            let permission = if r.get::<_, u8>(0)? == 2 { "edit" } else { "view" };
            let account: Option<String> = r.get(1)?;
            let group: Option<String> = r.get(2)?;
            let name: Option<String> = r.get(3)?;
            Ok(if let Some(account) = account {
                json!({"grant_id":grant,"permission":permission,"audience":{"account_id":account},"name":name})
            } else if let Some(group) = group {
                json!({"grant_id":grant,"permission":permission,"audience":{"group_id":group},"name":name})
            } else {
                json!({"grant_id":grant,"permission":permission,"label":r.get::<_, String>(4)?,"expires_at":r.get::<_, Option<i64>>(5)?})
            })
        },
    ).optional().map_err(sql)?.ok_or_else(missing)
}

fn item_authority_in(
    c: &Connection,
    actor: AccountId,
    space: SpaceId,
    id: Uuid,
    collections: &[Uuid],
) -> Result<(Option<CapabilitySet>, u8)> {
    let ids = serde_json::to_string(collections).map_err(Error::internal)?;
    let mut statement = c
        .prepare_cached(
            "WITH targets(id) AS (SELECT ?3 UNION ALL SELECT value FROM json_each(?4))
                 SELECT DISTINCT rc.capability, 0 FROM space_account_roles m
                 LEFT JOIN space_role_capabilities rc ON rc.role=m.role
                 WHERE m.account_id=?1 AND m.space_id=?2
                 UNION ALL
                 SELECT NULL, max(permission) FROM (
                   SELECT g.permission FROM targets t CROSS JOIN item_grants g
                   WHERE g.item_id=t.id AND g.space_id=?2 AND g.account_id=?1
                   UNION ALL
                   SELECT g.permission FROM group_members m CROSS JOIN targets t
                   CROSS JOIN item_grants g
                   WHERE m.user_id=?1 AND g.item_id=t.id AND g.space_id=?2 AND g.group_id=m.group_id
                 ) WHERE EXISTS(SELECT 1 FROM users WHERE id=?1 AND disabled_at IS NULL)",
        )
        .map_err(sql)?;
    let mut rows = statement
        .query(params![
            actor.to_string(),
            space.to_string(),
            id.to_string(),
            ids
        ])
        .map_err(sql)?;
    let mut set = None;
    let mut permission = 0;
    while let Some(row) = rows.next().map_err(sql)? {
        match row.get::<_, Option<u8>>(1).map_err(sql)? {
            Some(0) => {
                let caps = set.get_or_insert_with(CapabilitySet::empty);
                if let Some(name) = row.get::<_, Option<String>>(0).map_err(sql)?
                    && let Some(cap) = Capability::from_name(&name)
                {
                    *caps = caps.with(cap);
                }
            }
            Some(value) => permission = value,
            None => (),
        }
    }
    Ok((set, permission))
}
