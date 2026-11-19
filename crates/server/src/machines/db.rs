//! `system/machines.db`: machine placement, reservations and erasure delivery.
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
    worker INTEGER NOT NULL,
    machine TEXT,
    last_machine TEXT,
    pinned_machine TEXT,
    fence INTEGER NOT NULL,
    state TEXT NOT NULL,
    reason TEXT,
    created_ms INTEGER NOT NULL,
    updated_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS erased_worker_stops (
    lease TEXT PRIMARY KEY,
    machine TEXT NOT NULL,
    slots INTEGER NOT NULL CHECK (slots = 1)
) STRICT, WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS workspace_discards(
 lease TEXT NOT NULL,machine TEXT NOT NULL,request_id TEXT NOT NULL UNIQUE,account TEXT NOT NULL,thread TEXT NOT NULL,
 state TEXT NOT NULL,updated_ms INTEGER NOT NULL,PRIMARY KEY(lease,machine)
) STRICT;
-- Placement provenance lives beyond lease retention. Space erase consumes
-- workspace locations, while account homes survive until account erase.
CREATE TABLE IF NOT EXISTS workspace_locations(
 space TEXT NOT NULL, account TEXT NOT NULL, thread TEXT NOT NULL, machine TEXT NOT NULL,
 PRIMARY KEY(space,account,thread,machine)
) STRICT, WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS workspace_locations_account ON workspace_locations(account);
CREATE TABLE IF NOT EXISTS account_home_locations(
 account TEXT NOT NULL, machine TEXT NOT NULL, PRIMARY KEY(account,machine)
) STRICT, WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS machine_erasures(
 machine TEXT NOT NULL, account TEXT NOT NULL, thread TEXT NOT NULL,
 request_id TEXT NOT NULL UNIQUE, retry_after_ms INTEGER NOT NULL DEFAULT 0,
 PRIMARY KEY(machine,account,thread)
) STRICT, WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS machine_erasure_delivery ON machine_erasures(machine,retry_after_ms,request_id);
CREATE INDEX IF NOT EXISTS workspace_discards_pending ON workspace_discards(machine,state);
CREATE INDEX IF NOT EXISTS workspace_discards_account ON workspace_discards(account);
CREATE INDEX IF NOT EXISTS leases_space ON leases(space);
CREATE INDEX IF NOT EXISTS leases_thread_history ON leases(account,thread,created_ms DESC);
CREATE UNIQUE INDEX IF NOT EXISTS leases_active_thread ON leases(thread) WHERE state != 'ended';
";

const TITLE_LEASES: &str = "
CREATE TABLE title_leases(
 lease TEXT PRIMARY KEY REFERENCES leases(id) ON DELETE CASCADE
) STRICT, WITHOUT ROWID;
";

pub(crate) use calternal_types::now_ms;

/// Open the private placement database and apply any supported migrations.
/// One version read on a current database. Never accepts a newer schema.
pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(dir) = path.parent() {
        calternal_fsio::private::directory(dir)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    }
    calternal_fsio::private::file(path)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let c = Connection::open(path)?;
    c.busy_timeout(std::time::Duration::from_secs(5))?;
    c.pragma_update(None, "synchronous", "FULL")?;
    c.pragma_update(None, "foreign_keys", "ON")?;
    calternal_store::migrations::migrate(&c, upgrade_schema())?;
    c.pragma_update(None, "journal_mode", "WAL")?;
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
    /// Owner permitted direct execution, stored in protected authority.
    pub direct: bool,
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
            direct: record.direct,
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
    /// Ephemeral plugin worker; ended on server restart rather than adopted as an agent.
    pub worker: bool,
    /// Host-owned scratch lease fenced on restart.
    pub background_title: bool,
    /// Requested base image.
    pub image: Option<String>,
    /// Provider profile and confinement selected at admission. It is immutable
    /// across lease reassignments and server restarts.
    pub harness: Option<calternal_machine::protocol::HarnessSandbox>,
    /// Current machine, while assigned.
    pub machine: Option<Uuid>,
    /// Machine that last held it (placement affinity).
    pub last_machine: Option<Uuid>,
    /// Mandatory placement for a turn returning from a person request.
    pub pinned_machine: Option<Uuid>,
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
    let harness = l
        .harness
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    c.execute(
        "INSERT INTO leases (id, space, account, thread, image, machine, last_machine, fence, state, reason, created_ms, updated_ms, worker, pinned_machine, harness)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
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
            now,
            // Older releases already fence worker rows on restart.
            l.worker || l.background_title,
            l.pinned_machine.map(|machine| machine.to_string()),
            harness,
        ],
    )?;
    Ok(())
}

/// Insert a private title lease and its classification in one FULL commit.
pub(super) fn put_title(c: &mut Connection, row: &LeaseRow) -> rusqlite::Result<()> {
    let tx = c.transaction()?;
    put_lease(&tx, row)?;
    tx.execute("INSERT INTO title_leases VALUES(?1)", [row.id.to_string()])?;
    tx.commit()
}

/// Commit placement and both erasure locations before sending an assignment.
/// The location rows outlive timed lease pruning. One FULL transaction.
pub(super) fn put_assignment(c: &mut Connection, row: &LeaseRow) -> rusqlite::Result<()> {
    let tx = c.transaction()?;
    put_lease(&tx, row)?;
    if let Some(machine) = row.machine.filter(|_| !row.worker && !row.background_title) {
        let machine = machine.to_string();
        let account = row.account.to_string();
        tx.execute(
            "INSERT OR IGNORE INTO workspace_locations VALUES(?1,?2,?3,?4)",
            params![
                row.space.to_string(),
                account,
                row.thread.to_string(),
                machine
            ],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO account_home_locations VALUES(?1,?2)",
            params![account, machine],
        )?;
    }
    tx.commit()
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
    let background_title: bool = r.get(13)?;
    Ok(LeaseRow {
        id: uuid(r.get(0)?).unwrap_or_default(),
        space: typed(1, r.get(1)?)?,
        account: typed(2, r.get(2)?)?,
        thread: uuid(r.get(3)?).unwrap_or_default(),
        image: r.get(4)?,
        worker: r.get::<_, bool>(11)? && !background_title,
        machine: uuid(r.get(5)?),
        last_machine: uuid(r.get(6)?),
        fence: r.get::<_, i64>(7)? as u64,
        state: r.get(8)?,
        reason: r.get(9)?,
        created_ms: r.get(10)?,
        pinned_machine: uuid(r.get(12)?),
        background_title,
        harness: r
            .get::<_, Option<String>>(14)?
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    14,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
    })
}

const LEASE_COLUMNS: &str = "id, space, account, thread, image, machine, last_machine, fence, state, reason, created_ms, worker, pinned_machine, EXISTS(SELECT 1 FROM title_leases WHERE lease=leases.id), harness";

/// Leases that have not ended, oldest first.
pub fn active_leases(c: &Connection) -> rusqlite::Result<Vec<LeaseRow>> {
    let mut st = c.prepare(&format!(
        "SELECT {LEASE_COLUMNS} FROM leases WHERE state != 'ended' ORDER BY created_ms"
    ))?;
    st.query_map([], lease_from)?.collect()
}

/// Preserve one machine slot in the transaction that deletes the worker lease.
/// Writes one row with the machine and lease IDs as its only identifiers.
pub(super) fn put_erased_worker_stop(
    tx: &rusqlite::Transaction<'_>,
    lease: Uuid,
    machine: Uuid,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO erased_worker_stops (lease, machine, slots) VALUES (?1, ?2, 1)",
        params![lease.to_string(), machine.to_string()],
    )?;
    Ok(())
}

/// Read pending stops at startup. The schema guarantees one slot per row.
pub(super) fn erased_worker_stops(c: &Connection) -> rusqlite::Result<Vec<(Uuid, Uuid)>> {
    let mut query = c.prepare("SELECT lease, machine FROM erased_worker_stops")?;
    query
        .query_map([], |row| {
            let id = |column| {
                row.get::<_, String>(column)?.parse().map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        column,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            };
            Ok((id(0)?, id(1)?))
        })?
        .collect()
}

/// Retire one machine-only reservation by lease ID. One indexed delete.
pub(super) fn remove_erased_worker_stop(c: &Connection, lease: Uuid) -> rusqlite::Result<()> {
    c.execute(
        "DELETE FROM erased_worker_stops WHERE lease=?1",
        [lease.to_string()],
    )?;
    Ok(())
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

/// after explicit resume or a later turn. Cost: one indexed history read.
pub(crate) fn last_thread_machine(
    c: &Connection,
    account: AccountId,
    thread: Uuid,
) -> rusqlite::Result<Option<Uuid>> {
    let id:Option<String>=c.query_row("SELECT last_machine FROM leases WHERE account=?1 AND thread=?2 AND last_machine IS NOT NULL ORDER BY created_ms DESC LIMIT 1",params![account.to_string(),thread.to_string()],|r|r.get(0)).optional()?;
    Ok(id.and_then(|s| Uuid::parse_str(&s).ok()))
}

const ADD_HARNESS: &str =
    "ALTER TABLE leases ADD COLUMN harness TEXT CHECK (harness IS NULL OR json_valid(harness));";

/// Return the machine registry schema shared by normal startup and upgrade admission.
/// Baseline 6 is the first production schema, including durable erased-worker stops.
pub(crate) fn upgrade_schema() -> calternal_store::migrations::Schema<'static> {
    use calternal_store::migrations::Schema;

    const TITLES: (i64, &str) = (7, TITLE_LEASES);
    const HARNESS: (i64, &str) = (8, ADD_HARNESS);
    const CURRENT: Schema<'static> = Schema::new("machines.db", 6, &[SCHEMA], &[TITLES, HARNESS]);
    #[cfg(feature = "test-plugins")]
    if option_env!("CALTERNAL_RELEASE_TEST_MIGRATION") == Some("v7") {
        const FIXTURE: Schema<'static> = Schema::new(
            "machines.db",
            6,
            &[SCHEMA],
            &[
                TITLES,
                HARNESS,
                (
                    9,
                    "CREATE TABLE upgrade_fixture (id INTEGER PRIMARY KEY) STRICT;",
                ),
            ],
        )
        .with_compatible_creation(8);
        return FIXTURE;
    }
    CURRENT
}

#[cfg(test)]
mod tests {
    use super::*;
    use calternal_store::migrations::{Schema, migrate_release};

    #[test]
    fn harness_lease_migration_preserves_existing_fences_and_writes_scoped_profiles() {
        use calternal_machine::protocol::{HarnessSandbox, ProviderLoginScope};

        let conn = Connection::open_in_memory().unwrap();
        let previous = Schema::new("machines.db", 6, &[SCHEMA], &[(7, TITLE_LEASES)]);
        migrate_release(&conn, previous, false).unwrap();
        let (id, space, account, thread) = (
            Uuid::new_v4(),
            SpaceId::new(),
            AccountId::new(),
            Uuid::new_v4(),
        );
        conn.execute(
            "INSERT INTO leases
             (id, space, account, thread, image, worker, fence, state, created_ms, updated_ms)
             VALUES (?1, ?2, ?3, ?4, 'fixture-image', 0, 11, 'running', 1, 1)",
            params![
                id.to_string(),
                space.to_string(),
                account.to_string(),
                thread.to_string(),
            ],
        )
        .unwrap();

        migrate_release(&conn, upgrade_schema(), true).unwrap();
        let prior = lease(&conn, id).unwrap().unwrap();
        assert_eq!(prior.fence, 11);
        assert_eq!(prior.image.as_deref(), Some("fixture-image"));
        assert_eq!(prior.harness, None);
        let harness = HarnessSandbox {
            provider_id: "fixture-harness".into(),
            login_scope: ProviderLoginScope::Space(space),
            workspace_read_only: true,
            disposable_profile: false,
            legacy_profile: false,
            guest_image_digest: None,
        };
        let managed = LeaseRow {
            id: Uuid::new_v4(),
            thread: Uuid::new_v4(),
            harness: Some(harness.clone()),
            ..prior
        };
        put_lease(&conn, &managed).unwrap();
        assert_eq!(
            lease(&conn, managed.id).unwrap().unwrap().harness,
            Some(harness)
        );
    }

    #[test]
    fn title_leases_migrate_with_durable_classification_for_both_writers() {
        let mut conn = Connection::open_in_memory().unwrap();
        let previous = Schema::new("machines.db", 6, &[SCHEMA], &[]);
        migrate_release(&conn, previous, false).unwrap();
        let existing = LeaseRow {
            id: Uuid::new_v4(),
            space: SpaceId::new(),
            account: AccountId::new(),
            thread: Uuid::new_v4(),
            worker: false,
            background_title: false,
            image: None,
            harness: None,
            machine: None,
            last_machine: None,
            pinned_machine: None,
            fence: 0,
            state: "waiting".into(),
            reason: None,
            created_ms: 1,
        };
        conn.execute(
            "INSERT INTO leases
             (id, space, account, thread, worker, fence, state, created_ms, updated_ms)
             VALUES (?1, ?2, ?3, ?4, 0, 0, 'waiting', 1, 1)",
            params![
                existing.id.to_string(),
                existing.space.to_string(),
                existing.account.to_string(),
                existing.thread.to_string()
            ],
        )
        .unwrap();

        let schema = upgrade_schema();
        migrate_release(&conn, schema, true).unwrap();
        assert_eq!(
            calternal_store::migrations::inspect(&conn, previous).unwrap(),
            schema.version()
        );
        assert!(!lease(&conn, existing.id).unwrap().unwrap().background_title);

        let title = LeaseRow {
            id: Uuid::new_v4(),
            thread: Uuid::new_v4(),
            background_title: true,
            ..existing
        };
        put_title(&mut conn, &title).unwrap();
        let reread = lease(&conn, title.id).unwrap().unwrap();
        assert!(reread.background_title);
        assert!(!reread.worker);
        let legacy_worker: bool = conn
            .query_row(
                "SELECT worker FROM leases WHERE id=?1",
                [title.id.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert!(legacy_worker);
    }
}
