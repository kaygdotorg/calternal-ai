//! Bounded management and discovery projections of current sharing authority.

use super::*;
use crate::db::now_ms;

const PAGE: usize = 50;

fn page(mut rows: Vec<(String, Value)>, field: &str) -> Value {
    let more = rows.len() > PAGE;
    rows.truncate(PAGE);
    let next = more.then(|| rows.last().expect("full page").0.clone());
    json!({field:rows.into_iter().map(|(_,v)|v).collect::<Vec<_>>(),"next_cursor":next})
}

impl Spaces {
    /// List grant rows on one item using an indexed grant UUID cursor. A caller
    /// without management capability receives only empty audience data. The host
    /// checks item visibility before invoking and at protected delivery.
    pub async fn item_grants_page(
        &self,
        actor: AccountId,
        space: SpaceId,
        item: Uuid,
        after: Option<String>,
    ) -> Result<Value> {
        self.db
            .read(move |c| {
                let read = has(c, actor, space, Capability::SpaceRead.name())?;
                let edit = has(c, actor, space, Capability::ContentWrite.name())?;
                let links = link_admin(c, actor, space)?;
                let mut rows = Vec::new();
                if edit || links {
                    let mut stmt = c
                        .prepare_cached(
                            "SELECT id FROM item_grants WHERE item_id=?1 AND space_id=?2 AND id>?3
                     AND (expires_at IS NULL OR expires_at>?4)
                     AND ((link_hash IS NULL AND ?5) OR (link_hash IS NOT NULL AND ?6))
                     ORDER BY id LIMIT 51",
                        )
                        .map_err(sql)?;
                    let ids = stmt
                        .query_map(
                            params![
                                item.to_string(),
                                space.to_string(),
                                after.unwrap_or_default(),
                                now_ms(),
                                edit,
                                links
                            ],
                            |r| r.get::<_, String>(0),
                        )
                        .map_err(sql)?;
                    for id in ids {
                        let id = id.map_err(sql)?;
                        rows.push((id.clone(), grant_row(c, &id)?));
                    }
                }
                let mut result = page(rows, "grants");
                result["can_manage_audiences"] = json!(edit);
                result["can_manage_links"] = json!(links);
                result["space_access"] = json!(if edit {
                    Some("edit")
                } else if read {
                    Some("view")
                } else {
                    None
                });
                Ok(result)
            })
            .await
    }

    /// Search a page of shareable accounts and groups using the current directory
    /// visibility rules. One bounded ordered result from the auth worker.
    pub async fn item_audiences_page(
        &self,
        actor: AccountId,
        space: SpaceId,
        query: String,
        after: Option<String>,
    ) -> Result<Value> {
        self.db.read(move |c| {
            require_editor(c, actor, space)?;
            let all_people = crate::people::has(c, actor, Capability::PeopleManage)?;
            let all_groups = crate::people::has(c, actor, Capability::GroupsManage)?;
            let mut stmt = c.prepare_cached(
                "SELECT * FROM (
                   SELECT 'a' || u.id AS key, u.id, u.name FROM users u
                   WHERE u.id<>?1 AND u.disabled_at IS NULL AND ('a'||u.id)>?5
                     AND (?2 OR EXISTS(SELECT 1 FROM group_members a JOIN group_members b USING(group_id)
                         WHERE a.user_id=?1 AND b.user_id=u.id))
                   UNION ALL
                   SELECT 'g' || g.id AS key, g.id, g.name FROM groups g
                   WHERE ('g'||g.id)>?5 AND (?3 OR EXISTS(SELECT 1 FROM group_members m
                         WHERE m.user_id=?1 AND m.group_id=g.id))
                 ) WHERE instr(lower(name),lower(?4))>0 OR ?4=''
                 ORDER BY key LIMIT 51",
            ).map_err(sql)?;
            let rows = stmt.query_map(
                params![actor.to_string(), all_people, all_groups, query, after.unwrap_or_default()],
                |r| {
                    let key: String = r.get(0)?;
                    let id: String = r.get(1)?;
                    let name: String = r.get(2)?;
                    let audience = if key.starts_with('a') {json!({"account_id":id})} else {json!({"group_id":id})};
                    Ok((key,json!({"audience":audience,"name":name})))
                },
            ).map_err(sql)?.collect::<std::result::Result<Vec<_>,_>>().map_err(sql)?;
            Ok(page(rows, "audiences"))
        }).await
    }

    /// List candidate item IDs from indexed grants on the active caller and
    /// their current groups. The host filters lifecycle and binds IDs through
    /// the content catalogue before returning any display data.
    pub async fn received_item_grants(
        &self,
        actor: AccountId,
        kind: String,
        after: Option<String>,
    ) -> Result<Vec<(Uuid, String, bool, bool)>> {
        self.db.read(move |c| {
            let mut stmt = c.prepare_cached(
                "WITH matches AS (
                   SELECT item_id, created_by, permission FROM item_grants
                   WHERE account_id=?1 AND kind=?2 AND item_id>?3
                   UNION ALL
                   SELECT g.item_id,g.created_by,g.permission
                   FROM group_members m CROSS JOIN item_grants g
                   WHERE m.user_id=?1 AND g.group_id=m.group_id AND g.kind=?2 AND g.item_id>?3
                 )
                 SELECT m.item_id,coalesce(min(u.name),'Shared item'),max(permission),s.item_id IS NULL
                 FROM matches m LEFT JOIN users u ON u.id=m.created_by AND u.disabled_at IS NULL
                 LEFT JOIN item_share_seen s ON s.account_id=?1 AND s.item_id=m.item_id
                 WHERE EXISTS(SELECT 1 FROM users WHERE id=?1 AND disabled_at IS NULL)
                 GROUP BY m.item_id ORDER BY m.item_id LIMIT 51",
            ).map_err(sql)?;
            let rows = stmt.query_map(params![actor.to_string(),kind,after.unwrap_or_default()], |r| {
                Ok((r.get::<_, String>(0)?,r.get::<_, String>(1)?,r.get::<_,u8>(2)?==2,r.get::<_,bool>(3)?))
            }).map_err(sql)?;
            rows.map(|r| {
                let (id, sharer, edit, unread) = r.map_err(sql)?;
                Ok((Uuid::parse_str(&id).map_err(Error::internal)?,sharer,edit,unread))
            }).collect()
        }).await
    }

    /// Set whether Editors can manage public item links. Authority is checked
    /// again before replay. The policy and bounded receipt commit together.
    pub fn set_item_link_policy(
        &self,
        actor: AccountId,
        space: SpaceId,
        enabled: bool,
        receipt: (&str, &str, &str),
    ) -> Result<Value> {
        self.mutate(actor, Some(space), Some(receipt), move |c| {
            require_editor(c, actor, space)?;
            require(c, actor, space, Capability::SpaceSettings.name())?;
            Ok(move |c: &rusqlite::Transaction<'_>| {
                let sql_text = if enabled {
                    "INSERT OR IGNORE INTO item_link_policies VALUES(?1)"
                } else {
                    "DELETE FROM item_link_policies WHERE space_id=?1"
                };
                c.execute(sql_text, [space.to_string()]).map_err(sql)?;
                Ok(json!({"editors_can_manage_links":enabled}))
            })
        })
    }
}

impl Spaces {
    /// Resolve an active grant's named audience for post-commit eviction.
    /// One indexed grant lookup; link grants have no named rooms.
    pub async fn eviction_audience(
        &self,
        space: SpaceId,
        item: Uuid,
        grant: Uuid,
    ) -> Result<Option<Audience>> {
        self.db
            .read(move |c| {
                let target: Option<(Option<String>, Option<String>)> = c
                    .query_row(
                        "SELECT account_id,group_id FROM item_grants
                 WHERE id=?1 AND item_id=?2 AND space_id=?3 AND link_hash IS NULL",
                        params![grant.to_string(), item.to_string(), space.to_string()],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()
                    .map_err(sql)?;
                match target {
                    Some((Some(account), _)) => Ok(Some(Audience::Account(
                        AccountId::parse(&account).map_err(Error::internal)?,
                    ))),
                    Some((_, Some(group))) => Ok(Some(Audience::Group(
                        GroupId::parse(&group).map_err(Error::internal)?,
                    ))),
                    _ => Ok(None),
                }
            })
            .await
    }

    /// Read at most 100 current group account IDs for running room eviction
    /// outside the committing transaction. Continuation uses account UUID order.
    pub async fn eviction_group_accounts(
        &self,
        group: GroupId,
        after: Option<AccountId>,
    ) -> Result<Vec<AccountId>> {
        self.db
            .read(move |c| {
                let mut stmt = c
                    .prepare_cached(
                        "SELECT user_id FROM group_members WHERE group_id=?1 AND user_id>?2
                 ORDER BY user_id LIMIT 100",
                    )
                    .map_err(sql)?;
                let rows = stmt
                    .query_map(
                        params![
                            group.to_string(),
                            after.map(|id| id.to_string()).unwrap_or_default()
                        ],
                        |r| r.get::<_, String>(0),
                    )
                    .map_err(sql)?;
                rows.map(|row| AccountId::parse(&row.map_err(sql)?).map_err(Error::internal))
                    .collect()
            })
            .await
    }
}

impl Spaces {
    /// Read the indexed named/group grant roots for the recipient's cross-space
    /// Notes search. Returns at most 501 roots so the host can signal partial
    /// coverage while keeping the derived search query bounded.
    pub async fn search_share_roots(
        &self,
        account: AccountId,
    ) -> Result<Vec<(SpaceId, String, Uuid)>> {
        self.db
            .read(move |c| {
                let mut statement = c
                    .prepare_cached(
                        "SELECT space_id,kind,item_id FROM item_grants WHERE account_id=?1
                 AND EXISTS(SELECT 1 FROM users WHERE id=?1 AND disabled_at IS NULL)
                 UNION
                 SELECT g.space_id,g.kind,g.item_id FROM group_members m CROSS JOIN item_grants g
                 WHERE m.user_id=?1 AND g.group_id=m.group_id
                 AND EXISTS(SELECT 1 FROM users WHERE id=?1 AND disabled_at IS NULL)
                 ORDER BY 1,2,3 LIMIT 501",
                    )
                    .map_err(sql)?;
                let rows = statement
                    .query_map([account.to_string()], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                        ))
                    })
                    .map_err(sql)?;
                rows.map(|row| {
                    let (space, kind, item) = row.map_err(sql)?;
                    Ok((
                        SpaceId::parse(&space).map_err(Error::internal)?,
                        kind,
                        Uuid::parse_str(&item).map_err(Error::internal)?,
                    ))
                })
                .collect()
            })
            .await
    }
}

impl Spaces {
    /// Enumerate a bounded page of protected item grant and seen-marker roots.
    /// Used by maintenance after trash purge and interrupted authority cleanup.
    pub async fn sharing_cleanup_page(
        &self,
        after: Option<(SpaceId, Uuid)>,
    ) -> Result<Vec<(SpaceId, Uuid)>> {
        self.db
            .read(move |c| {
                let (space, item) = after
                    .map(|(space, item)| (space.to_string(), item.to_string()))
                    .unwrap_or_default();
                let mut statement = c
                    .prepare_cached(
                        "SELECT space_id,item_id FROM item_grants WHERE (space_id,item_id)>(?1,?2)
                 UNION
                 SELECT space_id,item_id FROM item_share_seen WHERE (space_id,item_id)>(?1,?2)
                 ORDER BY 1,2 LIMIT 64",
                    )
                    .map_err(sql)?;
                let rows = statement
                    .query_map(params![space, item], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                    })
                    .map_err(sql)?;
                rows.map(|row| {
                    let (space, id) = row.map_err(sql)?;
                    Ok((
                        SpaceId::parse(&space).map_err(Error::internal)?,
                        Uuid::parse_str(&id).map_err(Error::internal)?,
                    ))
                })
                .collect()
            })
            .await
    }
}
