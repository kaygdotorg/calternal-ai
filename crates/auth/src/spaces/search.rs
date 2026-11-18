//! Current search authority and the space's mentionable direct grants.
use super::*;

impl Spaces {
    /// Read the current UUIDs on which this account holds the supplied capability.
    /// Uses the same indexed direct/group authority view as a single-space check.
    /// A selected UUID never discloses whether inaccessible space metadata exists.
    pub async fn search_spaces(
        &self,
        account: AccountId,
        selected: Option<SpaceId>,
        capability: Capability,
    ) -> Result<Vec<SpaceId>> {
        self.db
            .read(move |c| {
                let mut stmt = c
                    .prepare_cached(
                        "SELECT DISTINCT m.space_id FROM space_account_roles m
                 JOIN space_role_capabilities rc ON rc.role=m.role
                 WHERE m.account_id=?1 AND rc.capability=?2
                   AND (?3 IS NULL OR m.space_id=?3) ORDER BY m.space_id",
                    )
                    .map_err(sql)?;
                stmt.query_map(
                    params![
                        account.to_string(),
                        capability.name(),
                        selected.map(|id| id.to_string())
                    ],
                    |r| r.get::<_, String>(0),
                )
                .map_err(sql)?
                .map(|row| space_id(row.map_err(sql)?))
                .collect()
            })
            .await
    }

    /// Query direct account and group grants by display-name prefix. Uses the
    /// grant indexes for the selected space and one SQL statement, including the
    /// live capability check. Accounts reached through groups stay private.
    pub async fn mentionable(
        &self,
        account: AccountId,
        space: SpaceId,
        prefix: String,
    ) -> Result<Vec<calternal_core::PersonMatch>> {
        self.db
            .read(move |c| {
                let mut stmt = c
                    .prepare_cached(
                        "WITH allowed AS MATERIALIZED (
                     SELECT 1 FROM space_account_roles m JOIN space_role_capabilities rc USING(role)
                     WHERE m.account_id=?1 AND m.space_id=?2 AND rc.capability=?3 LIMIT 1
                 ), subjects(kind,id,name) AS (
                     SELECT 'account', u.id, u.name FROM space_memberships m
                     JOIN users u ON u.id=m.account_id
                     WHERE m.space_id=?2 AND u.disabled_at IS NULL AND EXISTS(SELECT 1 FROM allowed)
                     UNION ALL
                     SELECT 'group', g.id, g.name FROM space_group_memberships m
                     JOIN groups g ON g.id=m.group_id
                     WHERE m.space_id=?2 AND EXISTS(SELECT 1 FROM allowed)
                 )
                 SELECT kind,id,name FROM subjects
                 WHERE substr(name,1,length(?4))=?4 COLLATE NOCASE
                 ORDER BY name COLLATE NOCASE,kind,id LIMIT 50",
                    )
                    .map_err(sql)?;
                stmt.query_map(
                    params![
                        account.to_string(),
                        space.to_string(),
                        calternal_core::SearchOperation::CAPABILITY.name(),
                        prefix
                    ],
                    |r| {
                        Ok(calternal_core::PersonMatch {
                            kind: r.get(0)?,
                            id: r.get(1)?,
                            name: r.get(2)?,
                        })
                    },
                )
                .map_err(sql)?
                .collect::<rusqlite::Result<_>>()
                .map_err(sql)
            })
            .await
    }
}
