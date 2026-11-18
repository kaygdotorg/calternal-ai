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

/// One page of direct space grants, ordered by subject kind then stable UUID.
#[derive(Debug, Serialize)]
pub struct MemberPage {
    /// Direct grants to accounts.
    pub members: Vec<Member>,
    /// Direct grants to groups.
    pub groups: Vec<GroupMember>,
    /// Continuation after the last displayed grant when more grants exist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

impl Spaces {
    /// Return at most 50 direct grants after checking space access in one read
    /// snapshot. Cost: one authorization query and up to two indexed grant seeks.
    pub fn members(
        &self,
        actor: AccountId,
        space: SpaceId,
        cursor: Option<String>,
    ) -> Result<MemberPage> {
        self.run(move |c| {
            let tx = c.transaction().map_err(sql)?;
            require(&tx, actor, space, PeopleOperation::ListSpaceMembers.capability().name())?;
            let scope = space.to_string();
            let (after_account, after_group) = match cursor.as_deref() {
                None => (Some(""), ""),
                Some(value) if let Some(id) = value.strip_prefix("account:") => {
                    AccountId::parse(id).map_err(Error::invalid)?;
                    (Some(id), "")
                }
                Some(value) if let Some(id) = value.strip_prefix("group:") => {
                    GroupId::parse(id).map_err(Error::invalid)?;
                    (None, id)
                }
                Some(_) => return Err(Error::invalid("invalid member cursor")),
            };
            let limit = calternal_core::DIRECTORY_PAGE_SIZE;
            let mut members = if let Some(after) = after_account {
                let mut stmt = tx.prepare_cached(
                    "SELECT m.account_id,u.name,CASE WHEN s.owner_account=m.account_id THEN 'owner' ELSE m.role END
                     FROM space_memberships m JOIN spaces s ON s.id=m.space_id JOIN users u ON u.id=m.account_id
                     WHERE s.id=?1 AND m.account_id>?2 ORDER BY m.account_id LIMIT ?3",
                ).map_err(sql)?;
                stmt.query_map(params![scope, after, (limit + 1) as i64], |r| Ok((r.get::<_,String>(0)?,r.get(1)?,r.get(2)?)))
                    .map_err(sql)?.map(|row| {
                        let (id, name, role) = row.map_err(sql)?;
                        Ok(Member { account_id: account(id)?, name, role })
                    }).collect::<Result<Vec<_>>>()?
            } else {
                Vec::new()
            };
            let mut groups = Vec::new();
            let mut more = members.len() > limit;
            members.truncate(limit);
            if !more {
                let slots = limit - members.len();
                let mut stmt = tx.prepare_cached(
                    "SELECT m.group_id,g.name,m.role,
                      (SELECT count(*) FROM group_members p JOIN users u ON u.id=p.user_id
                       WHERE p.group_id=g.id AND u.disabled_at IS NULL)
                     FROM space_group_memberships m JOIN groups g ON g.id=m.group_id
                     WHERE m.space_id=?1 AND m.group_id>?2 ORDER BY m.group_id LIMIT ?3",
                ).map_err(sql)?;
                groups = stmt.query_map(params![scope, after_group, (slots + 1) as i64], |r| Ok((r.get::<_,String>(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))
                    .map_err(sql)?.map(|row| {
                        let (id, name, role, member_count) = row.map_err(sql)?;
                        let group_id = GroupId::parse(&id).map_err(sql)?;
                        Ok(GroupMember { group_id, name, role, member_count })
                    }).collect::<Result<Vec<_>>>()?;
                more = groups.len() > slots;
                groups.truncate(slots);
            }
            let next_cursor = if more {
                groups.last().map(|g| format!("group:{}", g.group_id))
                    .or_else(|| members.last().map(|m| format!("account:{}", m.account_id)))
            } else {
                None
            };
            Ok(MemberPage { members, groups, next_cursor })
        })
    }
}

/// Read current grants once on the owning auth connection; shared by async and
/// synchronous adapters. A cached statement does not cache any authority rows.
pub(super) fn current_capabilities(
    c: &Connection,
    actor: AccountId,
    space: SpaceId,
) -> Result<Option<CapabilitySet>> {
    let mut st = c
        .prepare_cached(
            "SELECT DISTINCT rc.capability FROM space_account_roles m
                 LEFT JOIN space_role_capabilities rc ON rc.role=m.role
                 WHERE m.account_id=?1 AND m.space_id=?2",
        )
        .map_err(sql)?;
    let mut rows = st
        .query(params![actor.to_string(), space.to_string()])
        .map_err(sql)?;
    let mut set = None;
    while let Some(row) = rows.next().map_err(sql)? {
        let caps = set.get_or_insert_with(CapabilitySet::empty);
        if let Some(name) = row.get::<_, Option<String>>(0).map_err(sql)?
            && let Some(capability) = Capability::from_name(&name)
        {
            *caps = caps.with(capability);
        }
    }
    Ok(set)
}
