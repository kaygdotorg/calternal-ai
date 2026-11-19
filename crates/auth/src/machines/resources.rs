//! Protected account and group facts for resource policy.

use super::{Machines, sql};
use crate::{Error, ErrorCode, Result};
use calternal_types::{AccountId, GroupId, ResourceOwner, SpaceId};
use rusqlite::OptionalExtension;

impl Machines {
    /// Read the account that owns a personal space. Shared or missing spaces
    /// have no personal account. One indexed read, off the async executor.
    pub fn resource_account(&self, space: SpaceId) -> Result<Option<AccountId>> {
        self.db.read_blocking(move |conn| {
            let account: Option<Option<String>> = conn
                .query_row(
                    "SELECT personal_for FROM spaces WHERE id=?1",
                    [space.to_string()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(sql)?;
            account
                .flatten()
                .as_deref()
                .map(AccountId::parse)
                .transpose()
                .map_err(sql)
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
}
