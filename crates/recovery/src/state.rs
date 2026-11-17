use crate::{Cell, Result, Snapshot, Table, err, invalid};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Canonical key for schema inventory, fixed policy, unbound enrollment tokens
/// and pool machine authority (account-owned tables with no account).
pub(crate) const GLOBAL: &str = "000000000000000000000000000000000";
// A closed inventory of every auth.db table: adding a table requires an explicit
// owner mapping here, or recovery configuration refuses the database.
const SPECS: &[(&str, char, &str)] = &[
    ("users", '1', "id"),
    ("passkeys", '1', "user_id"),
    ("setup_tokens", '1', "user_id"),
    ("installations", '1', "user_id"),
    ("group_members", '1', "user_id"),
    ("space_receipts", '1', "actor"),
    ("machine_codes", '1', "owner_account"),
    ("machines", '1', "owner_account"),
    ("machine_pool_grants", '1', "account"),
    ("spaces", '2', "id"),
    ("space_memberships", '2', "space_id"),
    ("groups", '3', "id"),
    ("invites", '4', "id"),
    ("invite_groups", '4', "invite_id"),
    ("owner_secret_keys", '0', ""),
    ("roles", '0', ""),
    ("role_capabilities", '0', ""),
    ("space_roles", '0', ""),
    ("space_role_capabilities", '0', ""),
    ("development_directory", '0', ""),
];
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
    let names = tx
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'")
        .map_err(err)?
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(err)?;
    if names
        .iter()
        .any(|n| !n.starts_with("security_recovery_") && !SPECS.iter().any(|(s, _, _)| s == n))
    {
        return Err(err(
            "unclassified auth table; recovery owner mapping required",
        ));
    }
    tx.execute_batch("CREATE TABLE IF NOT EXISTS security_recovery_commits(singleton INTEGER PRIMARY KEY CHECK(singleton=1),sequence INTEGER NOT NULL,digest TEXT NOT NULL,proposal_sequence INTEGER NOT NULL,proposal_digest TEXT NOT NULL) STRICT;
        CREATE TABLE IF NOT EXISTS security_recovery_dirty(key TEXT PRIMARY KEY) STRICT;
        CREATE INDEX IF NOT EXISTS security_recovery_setup_user ON setup_tokens(user_id);").map_err(err)?;
    for (table, kind, owner) in SPECS {
        for event in ["INSERT", "UPDATE", "DELETE"] {
            let mut exprs = Vec::new();
            if event != "INSERT" {
                exprs.push(key_expr(*kind, owner, "OLD"));
            }
            if event != "DELETE" {
                exprs.push(key_expr(*kind, owner, "NEW"));
            }
            let when = if event == "UPDATE" && ["passkeys", "installations"].contains(table) {
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
                .map(|e| format!("INSERT OR IGNORE INTO security_recovery_dirty VALUES({e});"))
                .collect::<String>();
            tx.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS security_recovery_{table}_{event} AFTER {event} ON {} {when} BEGIN {body} END;",quote(table))).map_err(err)?;
        }
    }
    if !initialized {
        tx.execute(
            "INSERT OR IGNORE INTO security_recovery_dirty VALUES(?1)",
            [GLOBAL],
        )
        .map_err(err)?;
        for (table, kind) in [
            ("users", '1'),
            ("spaces", '2'),
            ("groups", '3'),
            ("invites", '4'),
        ] {
            tx.execute(&format!("INSERT OR IGNORE INTO security_recovery_dirty SELECT '{kind}'||replace(lower(id),'-','') FROM {}",quote(table)),[]).map_err(err)?;
        }
    }
    tx.commit().map_err(err)
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
) -> Result<Option<Record>> {
    if !crate::key_ok(key) {
        return Err(invalid());
    }
    let kind = key.as_bytes()[0] as char;
    let id = Uuid::parse_str(&key[1..]).map_err(err)?.to_string();
    if kind != '0' {
        let table = match kind {
            '1' => "users",
            '2' => "spaces",
            '3' => "groups",
            '4' => "invites",
            _ => return Err(invalid()),
        };
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
    for (name, owner_kind, owner) in SPECS {
        if kind != '0' && kind != *owner_kind {
            continue;
        }
        let (schema, columns) = schema(c, name)?;
        let filter = if *owner_kind == '0' {
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
            if *name == "passkeys"
                && let Some(i) = columns.iter().position(|n| n == "last_used_at")
            {
                cells[i] = Cell::Null
            }
            if *name == "installations" {
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
    for (table, kind) in [
        ("users", '1'),
        ("spaces", '2'),
        ("groups", '3'),
        ("invites", '4'),
    ] {
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
    let records = keys
        .iter()
        .map(|k| capture_record(c, installation, k)?.ok_or_else(invalid))
        .collect::<Result<Vec<_>>>()?;
    assemble(installation, Uuid::now_v7(), sequence, records)
}
