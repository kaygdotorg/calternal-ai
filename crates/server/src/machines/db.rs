//! `system/machines.db`: enrolment codes, machines, pool grants and leases.
//!
//! SQLite through `rusqlite`, WAL, `synchronous=FULL`: a lease's fence must never
//! go backwards after a crash, so every commit is durable (one fsync, about
//! 1–5 ms; commits happen on enrolment, placement, revocation and lease changes,
//! never on heartbeats). The operator CLI writes the same file while the server
//! runs; the server notices through `PRAGMA data_version` within a sweep tick.
//! Exec records and output are in memory only.

use std::path::Path;
use std::time::Duration;

// Moves to calternal-fsio with the shared-types work; this line is the only change then.
use calternal_auth::private_fs;
use calternal_auth::token::{hash32, new_setup_token, normalize_setup_token};
use calternal_auth::{AccountId, SpaceId};
use calternal_machine::protocol::Owner;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::error::HostError;

/// Current schema version (`PRAGMA user_version`).
pub const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS enrol_codes (
    hash BLOB PRIMARY KEY,
    owner TEXT NOT NULL,
    created_ms INTEGER NOT NULL,
    expires_ms INTEGER NOT NULL,
    used_ms INTEGER
);
CREATE TABLE IF NOT EXISTS machines (
    id TEXT PRIMARY KEY,
    owner TEXT NOT NULL,
    public_key BLOB NOT NULL,
    name TEXT NOT NULL,
    os TEXT NOT NULL,
    arch TEXT NOT NULL,
    enrolled_ms INTEGER NOT NULL,
    revoked_ms INTEGER,
    direct INTEGER NOT NULL DEFAULT 0,
    draining INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS pool_grants (
    account TEXT PRIMARY KEY,
    granted_ms INTEGER NOT NULL
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
CREATE INDEX IF NOT EXISTS leases_thread_history ON leases(account,thread,created_ms DESC);
CREATE UNIQUE INDEX IF NOT EXISTS leases_active_thread ON leases(thread) WHERE state != 'ended';
";

/// Milliseconds since the Unix epoch. (auth and store keep private copies; a
/// shared time helper belongs in the coming shared-types crate.)
pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// Open (creating 0600 if missing) and migrate the database.
pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(dir) = path.parent() {
        private_fs::directory(dir)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    }
    private_fs::file(path).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let c = Connection::open(path)?;
    c.busy_timeout(std::time::Duration::from_secs(5))?;
    c.pragma_update(None, "journal_mode", "WAL")?;
    c.pragma_update(None, "synchronous", "FULL")?;
    c.pragma_update(None, "foreign_keys", "ON")?;
    let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if v > SCHEMA_VERSION {
        return Err(rusqlite::Error::InvalidQuery);
    }
    c.execute_batch(SCHEMA)?;
    c.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(c)
}

/// Counter that changes whenever another connection commits to the file.
pub fn data_version(c: &Connection) -> rusqlite::Result<i64> {
    c.query_row("PRAGMA data_version", [], |r| r.get(0))
}

/// Stored form of an owner: `pool` or `account:<uuid>`.
pub fn owner_text(owner: Owner) -> String {
    match owner {
        Owner::Pool => "pool".into(),
        Owner::Account { id } => format!("account:{id}"),
    }
}

/// Parse the stored form of an owner.
pub fn parse_owner(s: &str) -> Option<Owner> {
    if s == "pool" {
        return Some(Owner::Pool);
    }
    Uuid::parse_str(s.strip_prefix("account:")?)
        .ok()
        .map(|id| Owner::Account { id })
}

/// Make a one-time enrolment code for `owner`, valid for `ttl`, and store only
/// its hash; codes expired for a day are deleted at the same time (retention).
/// The code uses the setup-token format of `calternal-auth` (52 characters in
/// groups of four; dashes and case ignored). Returns (code, expiry in ms).
pub fn new_code(c: &Connection, owner: Owner, ttl: Duration) -> Result<(String, i64), HostError> {
    let code = new_setup_token().map_err(|_| HostError::internal("no randomness"))?;
    let canon = normalize_setup_token(&code).ok_or_else(|| HostError::internal("bad new code"))?;
    let now = now_ms();
    let expires = now + ttl.as_millis() as i64;
    let store = || -> rusqlite::Result<()> {
        c.execute(
            "DELETE FROM enrol_codes WHERE expires_ms < ?1",
            params![now - 86_400_000],
        )?;
        c.execute(
            "INSERT INTO enrol_codes (hash, owner, created_ms, expires_ms) VALUES (?1, ?2, ?3, ?4)",
            params![&hash32(&canon)[..], owner_text(owner), now, expires],
        )?;
        Ok(())
    };
    store().map_err(super::sql)?;
    Ok((code, expires))
}

/// A machine row.
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
    /// Account owner explicitly permitted direct processes; never true in the pool.
    pub direct: bool,
}

/// Use a code and create the machine in one transaction. `None` when the code
/// is unknown, used or expired; a code works exactly once even under races.
pub fn redeem_code(
    c: &mut Connection,
    hash: &[u8; 32],
    public_key: &[u8],
    name: &str,
    os: &str,
    arch: &str,
    direct: bool,
) -> rusqlite::Result<Option<MachineRow>> {
    let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let now = now_ms();
    let owner: Option<String> = tx
        .query_row(
            "UPDATE enrol_codes SET used_ms = ?2
             WHERE hash = ?1 AND used_ms IS NULL AND expires_ms > ?2 RETURNING owner",
            params![&hash[..], now],
            |r| r.get(0),
        )
        .optional()?;
    let Some(owner) = owner.as_deref().and_then(parse_owner) else {
        return Ok(None);
    };
    if direct && owner == calternal_machine::protocol::Owner::Pool {
        return Ok(None);
    }
    let row = MachineRow {
        id: Uuid::new_v4(),
        owner,
        public_key: public_key.to_vec(),
        name: name.to_owned(),
        os: os.to_owned(),
        arch: arch.to_owned(),
        revoked: false,
        draining: false,
        direct,
    };
    tx.execute(
        "INSERT INTO machines (id, owner, public_key, name, os, arch, enrolled_ms, direct) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![row.id.to_string(), owner_text(owner), row.public_key, row.name, row.os, row.arch, now, direct],
    )?;
    tx.commit()?;
    Ok(Some(row))
}

/// Every machine, revoked ones included.
pub fn machines(c: &Connection) -> rusqlite::Result<Vec<MachineRow>> {
    let mut st = c.prepare(
        "SELECT id, owner, public_key, name, os, arch, revoked_ms IS NOT NULL, draining, direct FROM machines",
    )?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
            r.get(6)?,
            r.get::<_, i64>(7)? != 0,
            r.get::<_, i64>(8)? != 0,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, owner, public_key, name, os, arch, revoked, draining, direct) = row?;
        if let (Ok(id), Some(owner)) = (Uuid::parse_str(&id), parse_owner(&owner)) {
            out.push(MachineRow {
                id,
                owner,
                public_key,
                name,
                os,
                arch,
                revoked,
                draining,
                direct,
            });
        }
    }
    Ok(out)
}

/// Mark a machine revoked. Returns false when no such machine exists.
pub fn revoke(c: &Connection, id: Uuid) -> rusqlite::Result<bool> {
    Ok(c.execute(
        "UPDATE machines SET revoked_ms = coalesce(revoked_ms, ?2) WHERE id = ?1",
        params![id.to_string(), now_ms()],
    )? == 1)
}

/// Set or clear draining. Returns false when no such machine exists.
pub fn set_draining(c: &Connection, id: Uuid, draining: bool) -> rusqlite::Result<bool> {
    Ok(c.execute(
        "UPDATE machines SET draining = ?2 WHERE id = ?1",
        params![id.to_string(), draining as i64],
    )? == 1)
}

/// Accounts allowed to use pool machines.
pub fn pool_grants(c: &Connection) -> rusqlite::Result<Vec<AccountId>> {
    let mut st = c.prepare("SELECT account FROM pool_grants")?;
    let rows = st.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for r in rows {
        out.extend(AccountId::parse(&r?));
    }
    Ok(out)
}

/// Grant or withdraw pool use for an account.
pub fn set_pool_grant(c: &Connection, account: AccountId, granted: bool) -> rusqlite::Result<()> {
    if granted {
        c.execute(
            "INSERT OR IGNORE INTO pool_grants (account, granted_ms) VALUES (?1, ?2)",
            params![account.to_string(), now_ms()],
        )?;
    } else {
        c.execute(
            "DELETE FROM pool_grants WHERE account = ?1",
            params![account.to_string()],
        )?;
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

fn lease_from(r: &rusqlite::Row<'_>) -> rusqlite::Result<LeaseRow> {
    let uuid = |s: Option<String>| s.and_then(|s| Uuid::parse_str(&s).ok());
    Ok(LeaseRow {
        id: uuid(r.get(0)?).unwrap_or_default(),
        space: SpaceId(uuid(r.get(1)?).unwrap_or_default()),
        account: AccountId(uuid(r.get(2)?).unwrap_or_default()),
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

/// Last machine for this account's thread, used only as a placement preference
/// after explicit resume or a later turn. Cost: one indexed history read.
pub(crate) fn last_thread_machine(
    c: &Connection,
    account: AccountId,
    thread: Uuid,
) -> rusqlite::Result<Option<Uuid>> {
    let id:Option<String>=c.query_row("SELECT last_machine FROM leases WHERE account=?1 AND thread=?2 AND last_machine IS NOT NULL ORDER BY created_ms DESC LIMIT 1",params![account.to_string(),thread.to_string()],|r|r.get(0)).optional()?;
    Ok(id.and_then(|s| Uuid::parse_str(&s).ok()))
}
