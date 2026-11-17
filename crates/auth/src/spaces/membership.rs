//! Direct grants and group grants, checked against the same role catalog.
use super::*;
use crate::people::{can_see_group, can_see_person};

/// The account or group that receives a direct grant to a space.
#[derive(Debug, Clone, Copy)]
pub enum MemberTarget {
    /// A person's account.
    Account(AccountId),
    /// A server-managed group.
    Group(GroupId),
}

/// A group with a direct space grant; its members' identities stay private.
#[derive(Debug, Clone, Serialize)]
pub struct GroupMember {
    /// Stable group UUID.
    pub group_id: GroupId,
    /// Display name, never an authority key.
    pub name: String,
    /// Role granted to current members of the group.
    pub role: String,
    /// Number of active accounts that inherit this grant.
    pub member_count: i64,
}

pub(super) enum MemberEdit<'a> {
    Add(&'a str),
    SetRole(&'a str),
    Remove,
}

pub(crate) fn privileged(c: &Connection, name: &str) -> Result<bool> {
    // Compare to the Editor set, so new Editor capabilities do not also require
    // maintaining a separate list of structural authorization exceptions.
    c.query_row(
        "SELECT EXISTS(
           SELECT capability FROM space_role_capabilities WHERE role=?1
           EXCEPT SELECT capability FROM space_role_capabilities WHERE role='editor'
         )",
        [name],
        |r| r.get(0),
    )
    .map_err(sql)
}

fn protect_role(c: &Connection, actor: AccountId, space: SpaceId, name: &str) -> Result<()> {
    if privileged(c, name)? {
        require(c, actor, space, Capability::ManagersManage.name())?;
        if !is_owner(c, actor, space)? {
            return Err(denied());
        }
    }
    Ok(())
}

pub(crate) fn assignable_role(
    c: &Connection,
    actor: AccountId,
    space: SpaceId,
    name: &str,
) -> Result<()> {
    let known: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM space_roles WHERE name=?1 AND name<>'owner')",
            [name],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if !known {
        return Err(Error::invalid("unknown or nonassignable space role"));
    }
    protect_role(c, actor, space, name)
}

pub(super) fn change_member(
    c: &Connection,
    actor: AccountId,
    space: SpaceId,
    target: MemberTarget,
    edit: MemberEdit<'_>,
) -> Result<()> {
    let (table, column, target_id) = match target {
        MemberTarget::Account(id) => {
            if is_owner(c, id, space)? {
                return Err(denied());
            }
            ("space_memberships", "account_id", id.to_string())
        }
        MemberTarget::Group(id) => ("space_group_memberships", "group_id", id.to_string()),
    };
    let scope = space.to_string();
    let current: Option<String> = c
        .query_row(
            &format!("SELECT role FROM {table} WHERE space_id=?1 AND {column}=?2"),
            params![scope, target_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    match edit {
        MemberEdit::Add(role) => {
            let visible = match target {
                MemberTarget::Account(id) => can_see_person(c, actor, id)?,
                MemberTarget::Group(id) => can_see_group(c, actor, id)?,
            };
            if !visible {
                return Err(Error::new(ErrorCode::NotFound, "member target missing"));
            }
            assignable_role(c, actor, space, role)?;
            if current.is_some() {
                return Err(Error::new(
                    ErrorCode::Conflict,
                    "direct grant already exists",
                ));
            }
            c.execute(
                &format!("INSERT INTO {table}(space_id,{column},role) VALUES(?1,?2,?3)"),
                params![scope, target_id, role],
            )
            .map_err(sql)?;
        }
        MemberEdit::SetRole(role) => {
            protect_role(c, actor, space, &current.ok_or_else(denied)?)?;
            assignable_role(c, actor, space, role)?;
            c.execute(
                &format!("UPDATE {table} SET role=?3 WHERE space_id=?1 AND {column}=?2"),
                params![scope, target_id, role],
            )
            .map_err(sql)?;
        }
        MemberEdit::Remove => {
            protect_role(c, actor, space, &current.ok_or_else(denied)?)?;
            c.execute(
                &format!("DELETE FROM {table} WHERE space_id=?1 AND {column}=?2"),
                params![scope, target_id],
            )
            .map_err(sql)?;
        }
    }
    Ok(())
}

impl Spaces {
    /// Returns direct account and group grants in one read snapshot after checking
    /// space access. Cost: one authorization query and two indexed grant scans.
    pub fn members(
        &self,
        actor: AccountId,
        space: SpaceId,
    ) -> Result<(Vec<Member>, Vec<GroupMember>)> {
        self.run(move |c| {
            let tx = c.transaction().map_err(sql)?;
            require(&tx, actor, space, PeopleOperation::ListSpaceMembers.capability().name())?;
            let scope = space.to_string();
            let members = {
                let mut stmt = tx.prepare_cached(
                    "SELECT m.account_id,u.name,CASE WHEN s.owner_account=m.account_id THEN 'owner' ELSE m.role END
                     FROM space_memberships m JOIN spaces s ON s.id=m.space_id JOIN users u ON u.id=m.account_id
                     WHERE s.id=?1 ORDER BY m.account_id",
                ).map_err(sql)?;
                stmt.query_map([&scope], |r| Ok((r.get::<_,String>(0)?,r.get(1)?,r.get(2)?)))
                    .map_err(sql)?.map(|row| {
                        let (id, name, role) = row.map_err(sql)?;
                        Ok(Member { account_id: account(id)?, name, role })
                    }).collect::<Result<Vec<_>>>()?
            };
            let groups = {
                let mut stmt = tx.prepare_cached(
                    "SELECT m.group_id,g.name,m.role,
                      (SELECT count(*) FROM group_members p JOIN users u ON u.id=p.user_id
                       WHERE p.group_id=g.id AND u.disabled_at IS NULL)
                     FROM space_group_memberships m JOIN groups g ON g.id=m.group_id
                     WHERE m.space_id=?1 ORDER BY m.group_id",
                ).map_err(sql)?;
                stmt.query_map([&scope], |r| Ok((r.get::<_,String>(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))
                    .map_err(sql)?.map(|row| {
                        let (id, name, role, member_count) = row.map_err(sql)?;
                        let group_id = GroupId::parse(&id).map_err(sql)?;
                        Ok(GroupMember { group_id, name, role, member_count })
                    }).collect::<Result<Vec<_>>>()?
            };
            tx.commit().map_err(sql)?;
            Ok((members, groups))
        })
    }
}
