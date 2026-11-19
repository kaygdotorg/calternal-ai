use crate::{Cell, Result, Snapshot, Table, err, invalid};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Canonical key for schema inventory, fixed policy, unbound enrollment tokens
/// and pool machine authority (account-owned tables with no account).
pub(crate) const GLOBAL: &str = "000000000000000000000000000000000";
// A live principal and its erasure record have different recovery keys.
pub(crate) const OWNER_TABLES: &[(char, &str)] = &[
    ('1', "users"),
    ('2', "spaces"),
    ('3', "groups"),
    ('4', "invites"),
    ('5', "owner_erasures"),
];
// The shipped owner mappings. Later additive SQL registers extra mappings in
// the database so even today's binary captures them after a rollback.
const SPECS: &[(&str, char, &str)] = &[
    ("security_recovery_inventory", '0', ""),
    ("users", '1', "id"),
    ("oidc_providers", '0', ""),
    ("oidc_links", '1', "user_id"),
    ("oidc_sessions", '1', "user_id"),
    ("account_details", '1', "user_id"),
    ("account_recovery_codes", '1', "user_id"),
    ("account_email_challenges", '1', "user_id"),
    ("account_recovery_flows", '1', "user_id"),
    ("account_recovery_grants", '1', "user_id"),
    ("account_security_events", '1', "user_id"),
    ("passkeys", '1', "user_id"),
    ("setup_tokens", '1', "user_id"),
    ("installations", '1', "user_id"),
    ("group_members", '1', "user_id"),
    ("space_receipts", '1', "actor"),
    ("agent_access", '1', "account"),
    ("agent_approvals", '1', "account"),
    ("agent_discards", '1', "account"),
    ("machine_codes", '1', "owner_account"),
    ("machines", '1', "owner_account"),
    ("machine_pool_grants", '1', "account"),
    ("spaces", '2', "id"),
    ("item_grants", '2', "space_id"),
    ("item_link_policies", '2', "space_id"),
    ("item_share_seen", '1', "account_id"),
    ("space_group_memberships", '2', "space_id"),
    ("space_memberships", '2', "space_id"),
    ("groups", '3', "id"),
    ("invites", '4', "id"),
    ("invite_groups", '4', "invite_id"),
    ("owner_secret_keys", '0', ""),
    ("owner_erasures", '5', "id"),
    ("roles", '0', ""),
    ("role_capabilities", '0', ""),
    ("space_roles", '0', ""),
    ("space_role_capabilities", '0', ""),
    ("development_directory", '0', ""),
];
const INTERNAL: &[&str] = &[
    "_calternal_schema_compatibility",
    "security_recovery_commits",
    "security_recovery_dirty",
];

/// The extension mappings read once per startup or dirty transaction.
pub(crate) struct Inventory(Vec<(String, char, String)>);

impl Inventory {
    /// Read the transaction's protected table declarations.
    pub(crate) fn read(c: &Connection) -> Result<Self> {
        let mut stmt = c
            .prepare("SELECT table_name,owner_kind,owner_column FROM security_recovery_inventory ORDER BY table_name")
            .map_err(err)?;
        let specifications = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(err)?;
        let mut extensions = Vec::new();
        for specification in specifications {
            let (table, kind, column) = specification.map_err(err)?;
            let kind = match kind.as_bytes() {
                [b'0'] if column.is_empty() => '0',
                [kind @ b'1'..=b'5'] if !column.is_empty() => *kind as char,
                _ => return Err(err("invalid auth recovery owner namespace")),
            };
            if table.starts_with("sqlite_")
                || INTERNAL.contains(&table.as_str())
                || SPECS.iter().any(|&(name, _, _)| name == table)
            {
                return Err(err("auth recovery mapping uses a reserved table name"));
            }
            extensions.push((table, kind, column));
        }
        Ok(Self(extensions))
    }

    fn check_owner_links(&self, c: &Connection) -> Result<()> {
        for (table, kind, column) in &self.0 {
            let Some((_, parent)) = OWNER_TABLES.iter().find(|(key, _)| key == kind) else {
                continue; // An installation-wide table has no owner foreign key.
            };
            let cascade = c
                .query_row(
                    r#"SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_list(?1)
                       WHERE "table"=?2 AND "from"=?3 AND "to"='id' AND on_delete='CASCADE')"#,
                    (table, parent, column),
                    |row| row.get::<_, bool>(0),
                )
                .map_err(err)?;
            if !cascade {
                return Err(err(format!(
                    "auth recovery table {table} requires an owner deletion cascade"
                )));
            }
        }
        Ok(())
    }

    fn tables(&self) -> impl Iterator<Item = (&str, char, &str)> {
        SPECS.iter().copied().chain(
            self.0
                .iter()
                .map(|(table, kind, column)| (table.as_str(), *kind, column.as_str())),
        )
    }
}
/// Comparable format-three owner record for delta capture; never exposed to tenants.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Record {
    /// Recovery record format (three), independent of SQLite migration numbers.
    pub version: u32,
    /// Externally pinned installation UUID.
    pub installation: Uuid,
    /// Canonical owner namespace and UUID, or the reserved global-policy key.
    pub key: String,
    /// Only tables and rows owned by this record; global policy includes schemas.
    pub tables: BTreeMap<String, Table>,
}
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
fn key_expr(kind: char, owner: &str, prefix: &str) -> String {
    if kind == '0' {
        return format!("'{GLOBAL}'");
    }
    let value = format!("{prefix}.{}", quote(owner));
    format!(
        "CASE WHEN {value} IS NULL THEN '{GLOBAL}' ELSE '{kind}'||replace(lower({value}),'-','') END"
    )
}
/// Installs format-three chain witnesses and dirty-owner triggers; seeds initial log publication.
pub(crate) fn initialize(c: &Connection) -> Result<()> {
    let tx = rusqlite::Transaction::new_unchecked(c, rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let initialized: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='security_recovery_dirty')",
            [],
            |r| r.get(0),
        )
        .map_err(err)?;
    let inventory = Inventory::read(&tx)?;
    inventory.check_owner_links(&tx)?;
    let names = tx
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT GLOB 'sqlite_*'")
        .map_err(err)?
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(err)?;
    if names.iter().any(|n| {
        !INTERNAL.contains(&n.as_str()) && !inventory.tables().any(|(table, _, _)| table == n)
    }) {
        return Err(err(
            "unclassified auth table; recovery owner mapping required",
        ));
    }
    for (table, kind, owner) in inventory.tables() {
        let (_, columns) = schema(&tx, table)?;
        if kind != '0' && !columns.iter().any(|column| column == owner) {
            return Err(err("auth recovery owner column is missing"));
        }
    }
    tx.execute_batch("CREATE TABLE IF NOT EXISTS security_recovery_commits(singleton INTEGER PRIMARY KEY CHECK(singleton=1),sequence INTEGER NOT NULL,digest TEXT NOT NULL,proposal_sequence INTEGER NOT NULL,proposal_digest TEXT NOT NULL) STRICT;
        CREATE TABLE IF NOT EXISTS security_recovery_dirty(key TEXT PRIMARY KEY) STRICT;
        CREATE INDEX IF NOT EXISTS security_recovery_setup_user ON setup_tokens(user_id);").map_err(err)?;
    for (table, kind, owner) in inventory.tables() {
        for event in ["INSERT", "UPDATE", "DELETE"] {
            let mut exprs = Vec::new();
            if event != "INSERT" {
                exprs.push(key_expr(kind, owner, "OLD"));
            }
            if event != "DELETE" {
                exprs.push(key_expr(kind, owner, "NEW"));
            }
            let when = if event == "UPDATE" && ["passkeys", "installations"].contains(&table) {
                let (_, columns) = schema(&tx, table)?;
                let stable = columns
                    .iter()
                    .filter(|n| {
                        !["last_used_at", "last_seen_at", "expires_at"].contains(&n.as_str())
                    })
                    .map(|n| format!("OLD.{} IS NOT NEW.{}", quote(n), quote(n)))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                format!("WHEN {stable}")
            } else {
                String::new()
            };
            let body = exprs
                .iter()
                .map(|e| format!("INSERT INTO security_recovery_dirty(key) VALUES({e}) ON CONFLICT(key) DO NOTHING;"))
                .collect::<String>();
            let trigger = quote(&format!("security_recovery_{table}_{event}"));
            tx.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS {trigger} AFTER {event} ON {} {when} BEGIN {body} END;",quote(table))).map_err(err)?;
        }
    }
    if !initialized {
        mark_all(&tx)?;
    }
    tx.commit().map_err(err)
}
/// Remove capture triggers so a locked schema upgrade rebuilds their column checks.
pub(crate) fn reset_capture_triggers(conn: &Connection) -> Result<()> {
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    for (table, _, _) in Inventory::read(&tx)?.tables() {
        for event in ["INSERT", "UPDATE", "DELETE"] {
            tx.execute_batch(&format!(
                "DROP TRIGGER IF EXISTS {};",
                quote(&format!("security_recovery_{table}_{event}"))
            ))
            .map_err(err)?;
        }
    }
    tx.commit().map_err(err)
}

/// Queue the global schema and every current owner for an upgraded snapshot.
pub(crate) fn mark_all(conn: &Connection) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO security_recovery_dirty VALUES(?1)",
        [GLOBAL],
    )
    .map_err(err)?;
    for &(kind, table) in OWNER_TABLES {
        conn.execute(&format!("INSERT OR IGNORE INTO security_recovery_dirty SELECT '{kind}'||replace(lower(id),'-','') FROM {}",quote(table)),[]).map_err(err)?;
    }
    Ok(())
}
fn schema(c: &Connection, table: &str) -> Result<(String, Vec<String>)> {
    let schema = c
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name=?1 AND type='table'",
            [table],
            |r| r.get(0),
        )
        .map_err(err)?;
    let stmt = c
        .prepare(&format!("SELECT * FROM {} LIMIT 0", quote(table)))
        .map_err(err)?;
    Ok((schema, stmt.column_names().map(str::to_owned).collect()))
}
/// Captures one format-three delta record with indexed predicates and normalized telemetry.
pub(crate) fn capture_record(
    c: &Connection,
    installation: Uuid,
    key: &str,
    inventory: &Inventory,
) -> Result<Option<Record>> {
    if !crate::key_ok(key) {
        return Err(invalid());
    }
    let kind = key.as_bytes()[0] as char;
    let id = Uuid::parse_str(&key[1..]).map_err(err)?.to_string();
    if kind != '0' {
        let table = OWNER_TABLES
            .iter()
            .find_map(|&(key, table)| (key == kind).then_some(table))
            .ok_or_else(invalid)?;
        if !c
            .query_row(
                &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?1)"),
                [&id],
                |r| r.get::<_, bool>(0),
            )
            .map_err(err)?
        {
            return Ok(None);
        }
    } else if key != GLOBAL {
        return Err(invalid());
    }
    let mut tables = BTreeMap::new();
    for (name, owner_kind, owner) in inventory.tables() {
        if kind != '0' && kind != owner_kind {
            continue;
        }
        let (schema, columns) = schema(c, name)?;
        let filter = if owner_kind == '0' {
            if kind == '0' { "1".into() } else { "0".into() }
        } else if kind == '0' {
            // Account-owned rows with no account (unbound setup tokens, pool
            // machine codes and machines) belong to the global record.
            format!("{} IS NULL", quote(owner))
        } else {
            format!("{}=?1", quote(owner))
        };
        let mut stmt = c
            .prepare(&format!("SELECT * FROM {} WHERE {filter}", quote(name)))
            .map_err(err)?;
        let mut rows = if filter.contains("?1") {
            stmt.query([&id]).map_err(err)?
        } else {
            stmt.query([]).map_err(err)?
        };
        let mut data = Vec::new();
        while let Some(row) = rows.next().map_err(err)? {
            let mut cells = Vec::new();
            for i in 0..columns.len() {
                use rusqlite::types::ValueRef;
                cells.push(match row.get_ref(i).map_err(err)? {
                    ValueRef::Null => Cell::Null,
                    ValueRef::Integer(v) => Cell::Integer(v),
                    ValueRef::Real(v) => Cell::Real(v.to_bits()),
                    ValueRef::Text(v) => {
                        Cell::Text(std::str::from_utf8(v).map_err(err)?.to_owned())
                    }
                    ValueRef::Blob(v) => Cell::Blob(v.to_vec()),
                });
            }
            if name == "passkeys"
                && let Some(i) = columns.iter().position(|n| n == "last_used_at")
            {
                cells[i] = Cell::Null
            }
            if name == "installations" {
                let created = columns
                    .iter()
                    .position(|n| n == "created_at")
                    .ok_or_else(invalid)?;
                let seen = columns
                    .iter()
                    .position(|n| n == "last_seen_at")
                    .ok_or_else(invalid)?;
                cells[seen] = cells[created].clone();
            }
            data.push(cells);
        }
        tables.insert(
            name.to_string(),
            Table {
                schema,
                columns,
                rows: data,
            },
        );
    }
    let mut record = Record {
        version: 3,
        installation,
        key: key.into(),
        tables,
    };
    sort_tables(&mut record.tables);
    Ok(Some(record))
}
/// Reads the transaction-local dirty-key inventory in canonical order.
pub(crate) fn dirty(c: &Connection) -> Result<Vec<String>> {
    c.prepare("SELECT key FROM security_recovery_dirty ORDER BY key")
        .map_err(err)?
        .query_map([], |r| r.get(0))
        .map_err(err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(err)
}
fn sort_tables(tables: &mut BTreeMap<String, Table>) {
    for t in tables.values_mut() {
        t.rows
            .sort_by_cached_key(|r| serde_json::to_vec(r).expect("cells serialize"));
    }
}
/// Joins current snapshot/log records into format-three rebuild input with global schemas.
pub(crate) fn assemble(
    installation: Uuid,
    transaction: Uuid,
    sequence: u64,
    records: Vec<Record>,
) -> Result<Snapshot> {
    let global = records
        .iter()
        .find(|r| r.key == GLOBAL)
        .ok_or_else(invalid)?;
    let mut tables = global.tables.clone();
    for record in records.iter().filter(|r| r.key != GLOBAL) {
        if record.version != 3 || record.installation != installation {
            return Err(invalid());
        }
        for (name, table) in &record.tables {
            let dst = tables.get_mut(name).ok_or_else(invalid)?;
            if dst.schema != table.schema || dst.columns != table.columns {
                return Err(invalid());
            }
            dst.rows.extend(table.rows.clone());
        }
    }
    sort_tables(&mut tables);
    let snapshot = Snapshot {
        version: 3,
        installation,
        transaction,
        sequence,
        tables,
    };
    validate(&snapshot, installation)?;
    Ok(snapshot)
}
/// Extracts and parses a protected UUID column for cross-record identity checks.
pub(crate) fn ids(snapshot: &Snapshot, table: &str, column: &str) -> Result<Vec<Uuid>> {
    let table = snapshot.tables.get(table).ok_or_else(invalid)?;
    let index = table
        .columns
        .iter()
        .position(|c| c == column)
        .ok_or_else(invalid)?;
    table
        .rows
        .iter()
        .map(|r| match r.get(index) {
            Some(Cell::Text(s)) => Uuid::parse_str(s).map_err(err),
            _ => Err(invalid()),
        })
        .collect()
}
/// Checks format three, protected inventory, cell widths and account/space identity separation.
pub(crate) fn validate(snapshot: &Snapshot, installation: Uuid) -> Result<()> {
    if snapshot.version != 3
        || snapshot.installation != installation
        || snapshot.sequence == 0
        || SPECS
            .iter()
            .any(|(n, _, _)| !snapshot.tables.contains_key(*n))
    {
        return Err(invalid());
    }
    let users = ids(snapshot, "users", "id")?;
    if ids(snapshot, "spaces", "id")?
        .iter()
        .any(|s| users.contains(s))
    {
        return Err(invalid());
    }
    if snapshot
        .tables
        .values()
        .any(|t| t.rows.iter().any(|r| r.len() != t.columns.len()))
    {
        return Err(invalid());
    }
    Ok(())
}
/// Captures all records only for independent test comparisons, never on the mutation path.
#[cfg(test)]
pub(crate) fn capture(c: &Connection, installation: Uuid, sequence: u64) -> Result<Snapshot> {
    let mut keys = vec![GLOBAL.into()];
    for &(kind, table) in OWNER_TABLES {
        keys.extend(
            c.prepare(&format!(
                "SELECT '{kind}'||replace(lower(id),'-','') FROM {table}"
            ))
            .map_err(err)?
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?,
        );
    }
    let inventory = Inventory::read(c)?;
    let records = keys
        .iter()
        .map(|k| capture_record(c, installation, k, &inventory)?.ok_or_else(invalid))
        .collect::<Result<Vec<_>>>()?;
    assemble(installation, Uuid::now_v7(), sequence, records)
}
