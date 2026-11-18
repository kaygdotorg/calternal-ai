//! Resource attribution uses an explicit owner group, independent of sharing grants.

use super::{Machines, commit, sql};
use crate::{Error, ErrorCode, Result};
use calternal_types::{AccountId, GroupId, ResourceOwner, SpaceId};
use rusqlite::{OptionalExtension, TransactionBehavior, params};

impl Machines {
    /// Look up a space's accounting owner from protected authority.
    /// Unknown spaces and unassigned shared spaces have no accounting owner.
    /// Cost: one indexed auth-worker read. Call off the async executor.
    pub fn resource_owner(&self, space: SpaceId) -> Result<Option<ResourceOwner>> {
        self.db.read_blocking(move |conn| {
            let row: Option<(Option<String>, Option<String>)> = conn
                .query_row(
                    "SELECT s.personal_for,r.group_id FROM spaces s
                     LEFT JOIN space_resource_groups r ON r.space_id=s.id WHERE s.id=?1",
                    [space.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(sql)?;
            match row {
                Some((Some(id), _)) => AccountId::parse(&id)
                    .map(ResourceOwner::Account)
                    .map(Some)
                    .map_err(sql),
                Some((None, Some(id))) => GroupId::parse(&id)
                    .map(ResourceOwner::Group)
                    .map(Some)
                    .map_err(sql),
                _ => Ok(None),
            }
        })
    }

    /// Read the account's existing invite-assigned or administrator-managed
    /// groups for an explicitly granted resource policy. Up to 64 stable group
    /// UUIDs. A larger catalogue is refused rather than silently truncated.
    pub fn resource_groups(&self, account: AccountId) -> Result<Vec<GroupId>> {
        self.db.read_blocking(move |conn| {
            let mut stmt = conn.prepare_cached(
                "SELECT group_id FROM group_members WHERE user_id=?1 ORDER BY group_id LIMIT 65"
            ).map_err(sql)?;
            let rows = stmt
                .query_map([account.to_string()], |row| row.get::<_, String>(0))
                .map_err(sql)?;
            let groups = rows
                .map(|row| GroupId::parse(&row.map_err(sql)?).map_err(sql))
                .collect::<Result<Vec<_>>>()?;
            if groups.len() > 64 {
                return Err(Error::invalid(
                    "resource group catalogue exceeds 64 entries",
                ));
            }
            Ok(groups)
        })
    }

    /// Verify a space exists before a local operator opens its storage.
    /// Cost: one indexed read on the auth worker.
    pub fn require_resource_space(&self, space: SpaceId) -> Result<()> {
        self.db.read_blocking(move |conn| {
            let exists = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM spaces WHERE id=?1)",
                    [space.to_string()],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(sql)?;
            if exists {
                Ok(())
            } else {
                Err(Error::new(ErrorCode::NotFound, "space not found"))
            }
        })
    }

    /// Check that a manually selected resource scope exists in authority.
    /// Cost: one indexed auth-worker read, outside the placement lock.
    pub fn require_resource_owner(&self, owner: ResourceOwner) -> Result<()> {
        self.db.read_blocking(move |conn| {
            let (query, id) = match owner {
                ResourceOwner::Account(id) => (
                    "SELECT EXISTS(SELECT 1 FROM users WHERE id=?1 AND disabled_at IS NULL)",
                    id.to_string(),
                ),
                ResourceOwner::Group(id) => (
                    "SELECT EXISTS(SELECT 1 FROM groups WHERE id=?1)",
                    id.to_string(),
                ),
            };
            let exists = conn
                .query_row(query, [id], |row| row.get::<_, bool>(0))
                .map_err(sql)?;
            if exists {
                Ok(())
            } else {
                Err(Error::new(ErrorCode::NotFound, "resource owner not found"))
            }
        })
    }

    /// Bind an existing shared space to an existing group, or clear the binding.
    /// This does not grant access to the space, machine, or provider login.
    /// Cost: two indexed validation reads and one protected authority commit.
    /// The caller must have verified operator authority.
    pub fn set_resource_group(&self, space: SpaceId, group: Option<GroupId>) -> Result<()> {
        self.db.run_blocking(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql)?;
            let id = space.to_string();
            let personal = tx
                .query_row(
                    "SELECT personal_for IS NOT NULL FROM spaces WHERE id=?1",
                    [&id],
                    |row| row.get::<_, bool>(0),
                )
                .optional()
                .map_err(sql)?
                .ok_or_else(|| Error::new(ErrorCode::NotFound, "space not found"))?;
            if personal {
                return Err(Error::invalid(
                    "personal space resources belong to the account",
                ));
            }
            if let Some(group) = group {
                let group = group.to_string();
                let exists = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM groups WHERE id=?1)",
                        [&group],
                        |row| row.get::<_, bool>(0),
                    )
                    .map_err(sql)?;
                if !exists {
                    return Err(Error::new(ErrorCode::NotFound, "group not found"));
                }
                tx.execute(
                    "INSERT INTO space_resource_groups VALUES(?1,?2)
                     ON CONFLICT(space_id) DO UPDATE SET group_id=excluded.group_id
                     WHERE group_id<>excluded.group_id",
                    params![id, group],
                )
                .map_err(sql)?;
            } else {
                tx.execute("DELETE FROM space_resource_groups WHERE space_id=?1", [&id])
                    .map_err(sql)?;
            }
            commit(tx)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_shared_group_is_independent_from_membership_and_personal_attribution() {
        let root = tempfile::tempdir().unwrap();
        let db = crate::db::Db::open(&root.path().join("auth.db")).unwrap();
        let owner = AccountId::new();
        let group = GroupId::new();
        let (personal, shared) = db
            .run_blocking(move |conn| {
                let tx = conn.transaction().map_err(sql)?;
                tx.execute(
                    "INSERT INTO users(id,name,username,created_at) VALUES(?1,'Owner','owner',0)",
                    [owner.to_string()],
                )
                .map_err(sql)?;
                tx.execute(
                    "INSERT INTO groups(id,name) VALUES(?1,'Group')",
                    [group.to_string()],
                )
                .map_err(sql)?;
                let personal = crate::spaces::insert_owned(&tx, owner, "Personal", 0, true)?.id;
                let shared = crate::spaces::insert_owned(&tx, owner, "Shared", 0, false)?.id;
                commit(tx)?;
                Ok((personal, shared))
            })
            .unwrap();
        let resources = Machines::new(db.clone());
        assert_eq!(
            resources.resource_owner(personal).unwrap(),
            Some(ResourceOwner::Account(owner))
        );
        assert_eq!(resources.resource_owner(shared).unwrap(), None);
        assert!(resources.set_resource_group(personal, Some(group)).is_err());
        assert!(
            resources
                .set_resource_group(shared, Some(GroupId::new()))
                .is_err()
        );
        resources.set_resource_group(shared, Some(group)).unwrap();
        assert_eq!(
            resources.resource_owner(shared).unwrap(),
            Some(ResourceOwner::Group(group))
        );
        db.run_blocking(move |conn| {
            conn.execute("DELETE FROM groups WHERE id=?1", [group.to_string()])
                .map_err(sql)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(resources.resource_owner(shared).unwrap(), None);
    }
}
