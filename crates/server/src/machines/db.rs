//! `system/machines.db`: machine placement state (leases and draining).
//!
//! Machine authority (enrolment codes, machines, revocation, pool grants) lives
//! in `auth.db` behind the recovery barrier ([`calternal_auth::machines`]); this
//! file holds only what placement needs and is not a security record.
//! SQLite through `rusqlite`, WAL, `synchronous=FULL`: a lease's fence must never
//! go backwards after a crash, so every commit is durable (one fsync, about
//! 1–5 ms; commits happen on placement, draining and lease changes, never on
//! heartbeats). Only the server process opens it, under the root guard.
//! Exec records and output are in memory only.

use std::path::Path;
use std::time::Duration;

use calternal_auth::machines::MachineRecord;
use calternal_machine::protocol::Owner;
use calternal_types::{AccountId, SpaceId};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

/// Current schema version (`PRAGMA user_version`).
pub const SCHEMA_VERSION: i64 = 2;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS draining (
    machine TEXT PRIMARY KEY
);
CREATE TABLE IF NOT EXISTS leases (
    id TEXT PRIMARY KEY,
    space TEXT NOT NULL,
    account TEXT NOT NULL,
    thread TEXT NOT NULL,
    image TEXT,
    machine TEXT,
    last_machine TEXT,
    fence INTEGER NOT NULL,
    state TEXT NOT NULL,
    reason TEXT,
    created_ms INTEGER NOT NULL,
    updated_ms INTEGER NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS leases_active_thread ON leases(thread) WHERE state != 'ended';
";

pub(crate) use calternal_types::now_ms;

/// Open (creating 0600 if missing) the database at this build's schema; a file
/// at another version is refused (development data is disposable).
pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(dir) = path.parent() {
        calternal_fsio::private::directory(dir)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    }
    calternal_fsio::private::file(path)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let c = Connection::open(path)?;
    c.busy_timeout(std::time::Duration::from_secs(5))?;
    c.pragma_update(None, "journal_mode", "WAL")?;
    c.pragma_update(None, "synchronous", "FULL")?;
    c.pragma_update(None, "foreign_keys", "ON")?;
    let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if v != 0 && v != SCHEMA_VERSION {
        return Err(rusqlite::Error::InvalidQuery);
    }
    c.execute_batch(SCHEMA)?;
    c.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(c)
}

/// The protocol owner of a stored authority owner (`None` is the pool).
pub fn owner(account: Option<AccountId>) -> Owner {
    match account {
        None => Owner::Pool,
        Some(id) => Owner::Account { id },
    }
}

/// The authority owner of a protocol owner (`None` is the pool).
pub fn owner_account(owner: Owner) -> Option<AccountId> {
    match owner {
        Owner::Pool => None,
        Owner::Account { id } => Some(id),
    }
}

/// A machine as the registry tracks it: its authority record plus draining.
#[derive(Debug, Clone)]
pub struct MachineRow {
    /// Machine id.
    pub id: Uuid,
    /// Owner.
    pub owner: Owner,
    /// Raw Ed25519 public key.
    pub public_key: Vec<u8>,
    /// Label.
    pub name: String,
    /// Operating system.
    pub os: String,
    /// CPU architecture.
    pub arch: String,
    /// Revoked machines never authenticate again.
    pub revoked: bool,
    /// Draining machines receive no new placements.
    pub draining: bool,
}

impl MachineRow {
    /// Combine an authority record with its draining flag. No I/O.
    pub fn new(record: MachineRecord, draining: bool) -> Self {
        Self {
            id: record.id,
            owner: owner(record.owner),
            public_key: record.public_key,
            name: record.name,
            os: record.os,
            arch: record.arch,
            revoked: record.revoked,
            draining,
        }
    }
}

/// Machines marked draining. Cost: one table scan (bounded by machines).
pub fn draining(c: &Connection) -> rusqlite::Result<std::collections::HashSet<Uuid>> {
    let mut st = c.prepare("SELECT machine FROM draining")?;
    let rows = st.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = std::collections::HashSet::new();
    for r in rows {
        out.extend(Uuid::parse_str(&r?).ok());
    }
    Ok(out)
}

/// Set or clear draining for one machine. Cost: one FULL commit.
pub fn set_draining(c: &Connection, id: Uuid, draining: bool) -> rusqlite::Result<()> {
    if draining {
        c.execute(
            "INSERT OR IGNORE INTO draining(machine) VALUES(?1)",
            [id.to_string()],
        )?;
    } else {
        c.execute("DELETE FROM draining WHERE machine=?1", [id.to_string()])?;
    }
    Ok(())
}

/// Delete ended leases older than `retention`.
pub fn prune_ended(c: &Connection, retention: Duration) -> rusqlite::Result<usize> {
    c.execute(
        "DELETE FROM leases WHERE state = 'ended' AND updated_ms < ?1",
        params![now_ms() - retention.as_millis() as i64],
    )
}

/// A lease row.
#[derive(Debug, Clone)]
pub struct LeaseRow {
    /// Lease id.
    pub id: Uuid,
    /// Space.
    pub space: SpaceId,
    /// Account the work runs for.
    pub account: AccountId,
    /// Thread.
    pub thread: Uuid,
    /// Requested base image.
    pub image: Option<String>,
    /// Current machine, while assigned.
    pub machine: Option<Uuid>,
    /// Machine that last held it (placement affinity).
    pub last_machine: Option<Uuid>,
    /// Fencing token; increases on every (re)assignment and on end.
    pub fence: u64,
    /// `waiting`, `assigned`, `running` or `ended`.
    pub state: String,
    /// Why it ended.
    pub reason: Option<String>,
    /// Creation time.
    pub created_ms: i64,
}

/// Insert or update a lease.
pub fn put_lease(c: &Connection, l: &LeaseRow) -> rusqlite::Result<()> {
    let now = now_ms();
    c.execute(
        "INSERT INTO leases (id, space, account, thread, image, machine, last_machine, fence, state, reason, created_ms, updated_ms)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
         ON CONFLICT(id) DO UPDATE SET machine=excluded.machine, last_machine=excluded.last_machine,
           fence=excluded.fence, state=excluded.state, reason=excluded.reason, updated_ms=excluded.updated_ms",
        params![
            l.id.to_string(),
            l.space.to_string(),
            l.account.to_string(),
            l.thread.to_string(),
            l.image,
            l.machine.map(|m| m.to_string()),
            l.last_machine.map(|m| m.to_string()),
            l.fence as i64,
            l.state,
            l.reason,
            l.created_ms,
            now
        ],
    )?;
    Ok(())
}

/// A stored typed ID; an unparsable one fails the row instead of becoming a default.
fn typed<T: std::str::FromStr<Err = calternal_types::Error>>(
    column: usize,
    text: String,
) -> rusqlite::Result<T> {
    text.parse().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(column, rusqlite::types::Type::Text, Box::new(e))
    })
}

fn lease_from(r: &rusqlite::Row<'_>) -> rusqlite::Result<LeaseRow> {
    let uuid = |s: Option<String>| s.and_then(|s| Uuid::parse_str(&s).ok());
    Ok(LeaseRow {
        id: uuid(r.get(0)?).unwrap_or_default(),
        space: typed(1, r.get(1)?)?,
        account: typed(2, r.get(2)?)?,
        thread: uuid(r.get(3)?).unwrap_or_default(),
        image: r.get(4)?,
        machine: uuid(r.get(5)?),
        last_machine: uuid(r.get(6)?),
        fence: r.get::<_, i64>(7)? as u64,
        state: r.get(8)?,
        reason: r.get(9)?,
        created_ms: r.get(10)?,
    })
}

const LEASE_COLUMNS: &str =
    "id, space, account, thread, image, machine, last_machine, fence, state, reason, created_ms";

/// Leases that have not ended, oldest first.
pub fn active_leases(c: &Connection) -> rusqlite::Result<Vec<LeaseRow>> {
    let mut st = c.prepare(&format!(
        "SELECT {LEASE_COLUMNS} FROM leases WHERE state != 'ended' ORDER BY created_ms"
    ))?;
    st.query_map([], lease_from)?.collect()
}

/// One lease by id, ended or not.
pub fn lease(c: &Connection, id: Uuid) -> rusqlite::Result<Option<LeaseRow>> {
    c.query_row(
        &format!("SELECT {LEASE_COLUMNS} FROM leases WHERE id = ?1"),
        params![id.to_string()],
        lease_from,
    )
    .optional()
}
