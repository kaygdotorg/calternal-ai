//! Protected server people and local groups, ported from ADRs 0058 and 0060.
use crate::db::{now_ms, sql};
use crate::spaces::Spaces;
use crate::{AccountId, Auth, Error, ErrorCode, InstallationId, Result};
use calternal_core::PeopleOperation;
use calternal_types::{Capability, GroupId};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

fn missing() -> Error {
    Error::new(ErrorCode::NotFound, "directory entry not found")
}

pub(crate) fn require_active(c: &Connection, account: AccountId) -> Result<()> {
    if c.query_row(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id=?1 AND disabled_at IS NULL)",
        [account.to_string()],
        |r| r.get(0),
    )
    .map_err(sql)?
    {
        Ok(())
    } else {
        Err(missing())
    }
}

fn has(c: &Connection, actor: AccountId, capability: Capability) -> Result<bool> {
    c.query_row(
        "SELECT EXISTS(SELECT 1 FROM users u JOIN role_capabilities r ON u.role=r.role
          WHERE u.id=?1 AND u.disabled_at IS NULL AND r.capability=?2)",
        params![actor.to_string(), capability.name()],
        |r| r.get(0),
    )
    .map_err(sql)
}

fn require(c: &Connection, actor: AccountId, capability: Capability) -> Result<()> {
    if has(c, actor, capability)? {
        Ok(())
    } else {
        Err(missing())
    }
}

/// The inherited directory visibility rule: self, shared groups, or people manager.
pub(crate) fn can_see_person(c: &Connection, viewer: AccountId, person: AccountId) -> Result<bool> {
    let allowed = viewer == person || has(c, viewer, Capability::PeopleManage)?;
    c.query_row(
        "SELECT EXISTS(SELECT 1 FROM users u WHERE u.id=?2 AND u.disabled_at IS NULL
           AND (?3 OR EXISTS(SELECT 1 FROM group_members a JOIN group_members b USING(group_id)
                 WHERE a.user_id=?1 AND b.user_id=u.id)))",
        params![viewer.to_string(), person.to_string(), allowed],
        |r| r.get(0),
    )
    .map_err(sql)
}

pub(crate) fn can_see_group(c: &Connection, viewer: AccountId, group: GroupId) -> Result<bool> {
    let admin = has(c, viewer, Capability::GroupsManage)?;
    c.query_row(
        "SELECT EXISTS(SELECT 1 FROM groups g WHERE g.id=?2
          AND (?3 OR EXISTS(SELECT 1 FROM group_members m WHERE m.group_id=g.id AND m.user_id=?1)))",
        params![viewer.to_string(), group.to_string(), admin], |r| r.get(0),
    ).map_err(sql)
}

/// A local group change authorized against server roles in its transaction.
pub enum GroupChange {
    /// Create a local group.
    Create {
        /// Printable display name.
        name: String,
    },
    /// Rename a local group.
    Rename {
        /// Stable group UUID.
        group: GroupId,
        /// New display name.
        name: String,
    },
    /// Remove a local group and all its grants, preserving accounts.
    Delete(GroupId),
    /// Add a visible account to a local group.
    AddMember {
        /// Destination group.
        group: GroupId,
        /// Account receiving group access.
        account: AccountId,
    },
    /// Remove an account from a local group.
    RemoveMember {
        /// Destination group.
        group: GroupId,
        /// Account losing group access.
        account: AccountId,
    },
}

fn group_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err(Error::invalid("a group needs a short, printable name"));
    }
    Ok(name)
}

fn editable(c: &Connection, group: GroupId) -> Result<String> {
    let group = group.to_string();
    if c.query_row(
        "SELECT EXISTS(SELECT 1 FROM groups WHERE id=?1 AND source='local')",
        [&group],
        |r| r.get(0),
    )
    .map_err(sql)?
    {
        Ok(group)
    } else {
        Err(missing())
    }
}

impl Spaces {
    /// Return the canonical stored space role sets in one indexed catalog scan.
    /// Requires an active account and returns no individual grant information.
    pub fn roles_list(&self, actor: AccountId) -> Result<Value> {
        self.run(move |c| {
            require_active(c, actor)?;
            let mut stmt = c
                .prepare_cached(
                    "SELECT r.name, rc.capability FROM space_roles r
                 LEFT JOIN space_role_capabilities rc ON rc.role=r.name
                 ORDER BY r.name,rc.capability",
                )
                .map_err(sql)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
                })
                .map_err(sql)?;
            let mut roles = std::collections::BTreeMap::<String, Vec<String>>::new();
            for row in rows {
                let (role, capability) = row.map_err(sql)?;
                roles.entry(role).or_default().extend(capability);
            }
            Ok(json!({"roles": roles.into_iter().map(|(name, capabilities)|
                json!({"name":name,"capabilities":capabilities})).collect::<Vec<_>>() }))
        })
    }

    /// List server accounts for a current people manager. No credentials, space
    /// grants or private quota data are returned. Cost: one check and one scan.
    pub fn people_list(&self, actor: AccountId) -> Result<Value> {
        self.run(move |c| {
            require(c, actor, PeopleOperation::ListPeople.capability())?;
            let mut stmt = c.prepare_cached(
                "SELECT id,name,role,disabled_at IS NOT NULL FROM users ORDER BY name,id",
            ).map_err(sql)?;
            let people = stmt.query_map([], |r| Ok(json!({"account_id":r.get::<_,String>(0)?,
                "name":r.get::<_,String>(1)?,"role":r.get::<_,String>(2)?,"disabled":r.get::<_,bool>(3)?})))
                .map_err(sql)?.collect::<rusqlite::Result<Vec<_>>>().map_err(sql)?;
            Ok(json!({"people":people}))
        })
    }

    /// List group metadata and current active member IDs for a group manager.
    /// Cost: one authority check and one ordered group/member scan.
    pub fn groups_list(&self, actor: AccountId) -> Result<Value> {
        self.run(move |c| {
            require(c, actor, PeopleOperation::ListGroups.capability())?;
            let mut stmt = c.prepare_cached(
                "SELECT g.id,g.name,g.source,u.id FROM groups g
                 LEFT JOIN (group_members m JOIN users u ON u.id=m.user_id AND u.disabled_at IS NULL)
                 ON m.group_id=g.id ORDER BY g.name,g.id,u.id",
            ).map_err(sql)?;
            let mut rows = stmt.query([]).map_err(sql)?;
            let mut groups = Vec::new();
            let mut previous = String::new();
            while let Some(r) = rows.next().map_err(sql)? {
                let id: String = r.get(0).map_err(sql)?;
                if id != previous {
                    groups.push(json!({"group_id": id, "name": r.get::<_,String>(1).map_err(sql)?,
                        "source": r.get::<_,String>(2).map_err(sql)?, "members": []}));
                    previous = id;
                }
                if let Some(member) = r.get::<_,Option<String>>(3).map_err(sql)? {
                    groups.last_mut().unwrap()["members"].as_array_mut().unwrap().push(json!(member));
                }
            }
            Ok(json!({"groups":groups}))
        })
    }

    /// Change a local group and publish the resulting authority before replying.
    /// One immediate transaction checks current server authority before receipt replay.
    pub fn change_group(
        &self,
        actor: AccountId,
        change: GroupChange,
        receipt: (&str, &str, &str),
    ) -> Result<Value> {
        let operation = match &change {
            GroupChange::Create { .. } => PeopleOperation::CreateGroup,
            GroupChange::Rename { .. } => PeopleOperation::RenameGroup,
            GroupChange::Delete(_) => PeopleOperation::DeleteGroup,
            GroupChange::AddMember { .. } => PeopleOperation::AddGroupMember,
            GroupChange::RemoveMember { .. } => PeopleOperation::RemoveGroupMember,
        };
        self.mutate(actor, None, Some(receipt), move |tx| {
            require(tx, actor, operation.capability())?;
            Ok(move |tx: &rusqlite::Transaction<'_>| apply_group(tx, actor, change))
        })
    }

    /// Edit one's own normalized display name, preserving the login identity.
    /// Reuses enrollment's confusable-name check and protected receipt commit.
    pub fn set_profile(
        &self,
        actor: AccountId,
        name: String,
        receipt: (&str, &str, &str),
    ) -> Result<Value> {
        self.mutate(actor, None, Some(receipt), move |tx| {
            require_active(tx, actor)?;
            Ok(move |tx: &rusqlite::Transaction<'_>| {
                let name = crate::setup::clean_person_name(&name)
                    .ok_or_else(|| Error::invalid("choose a printable account name"))?;
                let id = actor.to_string();
                crate::setup::ensure_person_name_available(
                    tx,
                    &name,
                    Some(&id),
                    has(tx, actor, Capability::PeopleManage)?,
                )?;
                tx.execute("UPDATE users SET name=?2 WHERE id=?1", params![id, name])
                    .map_err(sql)?;
                Ok(json!({"account_id":actor,"name":name}))
            })
        })
    }
}

fn apply_group(c: &Connection, actor: AccountId, change: GroupChange) -> Result<Value> {
    let group = match change {
        GroupChange::Create { name } => {
            let id = GroupId::new().to_string();
            c.execute(
                "INSERT INTO groups(id,name) VALUES(?1,?2)",
                params![id, group_name(&name)?],
            )
            .map_err(sql)?;
            id
        }
        GroupChange::Rename { group, name } => {
            let id = editable(c, group)?;
            c.execute(
                "UPDATE groups SET name=?2 WHERE id=?1",
                params![id, group_name(&name)?],
            )
            .map_err(sql)?;
            id
        }
        GroupChange::Delete(group) => {
            let id = editable(c, group)?;
            c.execute("DELETE FROM groups WHERE id=?1", [&id])
                .map_err(sql)?;
            id
        }
        GroupChange::AddMember { group, account } => {
            let id = editable(c, group)?;
            if !can_see_person(c, actor, account)? {
                return Err(missing());
            }
            c.execute(
                "INSERT INTO group_members(group_id,user_id) VALUES(?1,?2) ON CONFLICT DO NOTHING",
                params![id, account.to_string()],
            )
            .map_err(sql)?;
            id
        }
        GroupChange::RemoveMember { group, account } => {
            let id = editable(c, group)?;
            if c.execute(
                "DELETE FROM group_members WHERE group_id=?1 AND user_id=?2",
                params![id, account.to_string()],
            )
            .map_err(sql)?
                == 0
            {
                return Err(missing());
            }
            id
        }
    };
    Ok(json!({"group_id":group}))
}

impl Auth {
    /// Check whether an account can authenticate using an explicit development token.
    /// Cost: one indexed lookup on the bounded auth worker.
    pub async fn development_account_active(&self, account: AccountId) -> bool {
        self.inner
            .db
            .run(move |c| require_active(c, account))
            .await
            .is_ok()
    }

    /// Disable an account without erasing its content, revoking its browser sessions
    /// in the same protected commit. The last active people manager is protected.
    pub fn disable_person(
        &self,
        actor: AccountId,
        target: AccountId,
        receipt: (&str, &str, &str),
    ) -> Result<Value> {
        let inner = self.inner.clone();
        self.spaces().mutate_with(actor, None, Some(receipt), move |tx| {
            require(tx, actor, PeopleOperation::DisablePerson.capability())?;
            Ok(move |tx: &rusqlite::Transaction<'_>| {
                require_active(tx, target)?;
                if has(tx, target, Capability::PeopleManage)? {
                    let another: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM users u JOIN role_capabilities r ON u.role=r.role
                         WHERE u.id<>?1 AND u.disabled_at IS NULL AND r.capability=?2)",
                        params![target.to_string(), Capability::PeopleManage.name()], |r| r.get(0),
                    ).map_err(sql)?;
                    if !another {
                        return Err(Error::new(ErrorCode::Forbidden, "the last people manager cannot be disabled"));
                    }
                }
                let now = now_ms();
                let id = target.to_string();
                tx.execute("UPDATE users SET disabled_at=?2 WHERE id=?1", params![id,now]).map_err(sql)?;
                tx.execute("UPDATE agent_access SET hash=NULL,expires_ms=0,updated_ms=?2
                            WHERE account=?1 AND hash IS NOT NULL", params![id,now]).map_err(sql)?;
                let mut stmt = tx.prepare_cached(
                    "UPDATE installations SET revoked_at=?2 WHERE user_id=?1 AND revoked_at IS NULL RETURNING id",
                ).map_err(sql)?;
                let rows = stmt.query_map(params![id,now], |r| r.get::<_,String>(0)).map_err(sql)?;
                let installations = rows.map(|row| InstallationId::parse(&row.map_err(sql)?).map_err(Error::internal))
                    .collect::<Result<Vec<_>>>()?;
                Ok((json!({"account_id":target,"disabled":true}), installations))
            })
        }, move |revoked| inner.sessions.forget_installations(revoked))
    }
}
