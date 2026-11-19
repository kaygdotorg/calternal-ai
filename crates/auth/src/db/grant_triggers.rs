//! Connection-local triggers fence affected assignments before SQLite changes a row.
use rusqlite::Connection;

enum Scope {
    Space,
    Account,
    Group,
    Machine,
    AccountStatus,
    SpaceRole,
}

impl Scope {
    fn sql(&self, row: &str, key: &str) -> String {
        match self {
            Self::Space => format!("SELECT fence_space_grant('space',{row}.{key});"),
            Self::Machine => format!("SELECT fence_space_grant('machine',{row}.{key});"),
            Self::Account | Self::AccountStatus => {
                let mut sql = format!(
                    "SELECT fence_space_grant('space',space_id) FROM (
                        SELECT space_id FROM space_memberships WHERE account_id={row}.{key}
                        UNION SELECT s.space_id FROM group_members g
                            JOIN space_group_memberships s USING(group_id)
                            WHERE g.user_id={row}.{key}
                        UNION SELECT space_id FROM item_grants WHERE account_id={row}.{key}
                        UNION SELECT s.space_id FROM group_members g
                            JOIN item_grants s USING(group_id) WHERE g.user_id={row}.{key}
                        UNION SELECT space FROM agent_access WHERE account={row}.{key}
                    );"
                );
                if matches!(self, Self::AccountStatus) {
                    sql.push_str(&format!(
                        "SELECT fence_space_grant('machine',id)
                         FROM machines WHERE owner_account={row}.{key};"
                    ));
                }
                sql
            }
            Self::SpaceRole => format!(
                "SELECT fence_space_grant('space',space_id) FROM (
                    SELECT space_id FROM space_memberships WHERE role={row}.{key}
                    UNION SELECT space_id FROM space_group_memberships WHERE role={row}.{key}
                    UNION SELECT id FROM spaces WHERE {row}.{key}='owner'
                );"
            ),
            Self::Group => format!(
                "SELECT fence_space_grant('space',space_id) FROM (
                    SELECT space_id FROM space_group_memberships WHERE group_id={row}.{key}
                    UNION SELECT space_id FROM item_grants WHERE group_id={row}.{key}
                );"
            ),
        }
    }
}

pub(super) fn install(connection: &Connection) -> rusqlite::Result<()> {
    use Scope::*;
    let tables = [
        ("space_memberships", "space_id", Space, ""),
        ("space_role_capabilities", "role", SpaceRole, ""),
        ("space_group_memberships", "space_id", Space, ""),
        ("spaces", "id", Space, " OF owner_account,personal_for"),
        ("item_grants", "space_id", Space, ""),
        ("item_link_policies", "space_id", Space, ""),
        (
            "agent_access",
            "space",
            Space,
            " OF hash,scope,expires_ms,run",
        ),
        ("agent_approvals", "space", Space, ""),
        ("groups", "id", Group, " OF source"),
        ("group_members", "group_id", Group, " OF group_id,user_id"),
        (
            "installations",
            "user_id",
            Account,
            " OF revoked_at,expires_at,token_hash,user_id",
        ),
        ("users", "id", AccountStatus, " OF disabled_at"),
        ("machine_pool_grants", "account", Account, ""),
        (
            "machines",
            "id",
            Machine,
            " OF revoked_ms,owner_account,direct,public_key",
        ),
    ];
    let mut sql = String::new();
    for (table, key, scope, update_columns) in tables {
        let old = scope.sql("OLD", key);
        let new = scope.sql("NEW", key);
        sql.push_str(&format!(
            "CREATE TEMP TRIGGER grant_{table}_insert BEFORE INSERT ON main.{table}
                BEGIN {new} END;
            CREATE TEMP TRIGGER grant_{table}_delete BEFORE DELETE ON main.{table}
                BEGIN {old} END;
            CREATE TEMP TRIGGER grant_{table}_update BEFORE UPDATE{update_columns} ON main.{table}
                BEGIN {old} {new} END;"
        ));
    }
    connection.execute_batch(&sql)
}
