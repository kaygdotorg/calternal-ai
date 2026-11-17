//! `auth.db`: users, passkeys, setup tokens and Installations.
//!
//! One global SQLite file in WAL mode with `synchronous=FULL`: a signed-in
//! session or a used-up setup token must survive a power cut. Sign-in work
//! is tiny and rare, so one connection is enough. Every call runs on a
//! blocking thread, never on the async workers, so a slow disk never stalls
//! anyone else's requests.

use crate::{Error, Result};
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Bump when the schema changes, and add a step to [`migrate`].
const VERSION: i64 = 16;

const SCHEMA_V1: &str = "
CREATE TABLE users (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    username    TEXT NOT NULL UNIQUE COLLATE NOCASE,
    created_at  INTEGER NOT NULL
);
CREATE TABLE passkeys (
    cred_id      BLOB PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id),
    passkey      TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER
);
CREATE INDEX passkeys_user ON passkeys(user_id);
CREATE TABLE setup_tokens (
    hash        BLOB PRIMARY KEY,
    purpose     TEXT NOT NULL CHECK (purpose IN ('first-owner', 'recover')),
    user_id     TEXT REFERENCES users(id),
    created_at  INTEGER NOT NULL,
    expires_at  INTEGER NOT NULL,
    used_at     INTEGER
);
CREATE TABLE installations (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id),
    kind         TEXT NOT NULL CHECK (kind IN ('browser', 'native', 'cli', 'agent', 'dav')),
    name         TEXT NOT NULL,
    token_hash   BLOB NOT NULL UNIQUE,
    created_at   INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    expires_at   INTEGER,
    revoked_at   INTEGER
);
CREATE INDEX installations_user ON installations(user_id);
";

/// v2: passkeys get a name people can change.
const SCHEMA_V2: &str = "
ALTER TABLE passkeys ADD COLUMN name TEXT NOT NULL DEFAULT 'Passkey';
";

/// v3: capability sets keep role checks independent of role names.
const SCHEMA_V3: &str = "
CREATE TABLE roles (name TEXT PRIMARY KEY);
CREATE TABLE role_capabilities (
    role TEXT NOT NULL REFERENCES roles(name),
    capability TEXT NOT NULL,
    PRIMARY KEY (role, capability)
);
INSERT INTO roles VALUES ('admin'), ('member');
INSERT INTO role_capabilities VALUES
    ('admin','people.invite'), ('admin','people.manage'),
    ('admin','groups.manage'), ('admin','server.settings');
-- SQLite cannot add a REFERENCES column with a default to a populated table.
-- Keep users in place and enforce that reference with triggers instead.
ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'member';
CREATE TRIGGER user_role_insert BEFORE INSERT ON users
WHEN NOT EXISTS (SELECT 1 FROM roles WHERE name=NEW.role)
BEGIN SELECT RAISE(ABORT,'unknown role'); END;
CREATE TRIGGER user_role_update BEFORE UPDATE OF role ON users
WHEN NOT EXISTS (SELECT 1 FROM roles WHERE name=NEW.role)
BEGIN SELECT RAISE(ABORT,'unknown role'); END;
CREATE TRIGGER role_in_use BEFORE DELETE ON roles
WHEN EXISTS (SELECT 1 FROM users WHERE role=OLD.name)
BEGIN SELECT RAISE(ABORT,'role in use'); END;
UPDATE users SET role='admin' WHERE id=(SELECT id FROM users ORDER BY created_at,rowid LIMIT 1);
CREATE TRIGGER first_person_admin AFTER INSERT ON users
WHEN (SELECT count(*) FROM users)=1
BEGIN UPDATE users SET role='admin' WHERE id=NEW.id; END;
";

/// v4: invite settings and the memberships they grant on redemption.
const SCHEMA_V4: &str = "
ALTER TABLE users ADD COLUMN quota_bytes INTEGER CHECK (quota_bytes IS NULL OR quota_bytes >= 0);
CREATE TABLE groups (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'local' CHECK (source IN ('local','oidc'))
);
CREATE TABLE group_members (
    group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id),
    manager INTEGER NOT NULL DEFAULT 0 CHECK (manager IN (0,1)),
    PRIMARY KEY (group_id,user_id)
);
CREATE INDEX group_people ON group_members(user_id,group_id);
CREATE TABLE invites (
    id TEXT PRIMARY KEY,
    created_by TEXT NOT NULL REFERENCES users(id),
    hash BLOB NOT NULL UNIQUE,
    secret BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    used_at INTEGER,
    revoked_at INTEGER,
    quota_bytes INTEGER CHECK (quota_bytes IS NULL OR quota_bytes >= 0),
    plan TEXT
);
CREATE TABLE invite_groups (
    invite_id TEXT NOT NULL REFERENCES invites(id) ON DELETE CASCADE,
    group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    PRIMARY KEY (invite_id,group_id)
);
";

/// v5: old invite indexes are upgraded with the server key before serving.
const SCHEMA_V5: &str = "
ALTER TABLE invites ADD COLUMN lookup_version INTEGER NOT NULL DEFAULT 0;
";

/// v6: obsolete cookie state, removed by v7.
const SCHEMA_V6: &str = "
ALTER TABLE installations ADD COLUMN host_cookie INTEGER NOT NULL DEFAULT 0;
";

/// v7: sessions authenticate only through the host cookie.
const SCHEMA_V7: &str = "ALTER TABLE installations DROP COLUMN host_cookie;";

/// v8: DAV app passwords. `scopes` lists the protocols a `dav` Installation
/// may speak (comma separated); every other kind keeps it empty. Triggers
/// keep that pairing true even if code is wrong.
const SCHEMA_V8: &str = "
ALTER TABLE installations ADD COLUMN scopes TEXT NOT NULL DEFAULT '';
CREATE TRIGGER installation_scopes_insert BEFORE INSERT ON installations
WHEN (NEW.kind = 'dav') <> (NEW.scopes <> '')
BEGIN SELECT RAISE(ABORT,'dav installations need scopes, others must have none'); END;
CREATE TRIGGER installation_scopes_update BEFORE UPDATE OF kind, scopes ON installations
WHEN (NEW.kind = 'dav') <> (NEW.scopes <> '')
BEGIN SELECT RAISE(ABORT,'dav installations need scopes, others must have none'); END;
";

/// v9: `assertion_at` is when the session was made by a passkey assertion
/// (sign-in). Setup, recovery and invite sessions leave it empty, so they
/// never count as a fresh sign-in. Also tightens the v8 scope triggers to
/// known scope names (databases that ran an earlier v8 get them too).
const SCHEMA_V9: &str = "
ALTER TABLE installations ADD COLUMN assertion_at INTEGER;
DROP TRIGGER IF EXISTS installation_scopes_insert;
DROP TRIGGER IF EXISTS installation_scopes_update;
CREATE TRIGGER installation_scopes_insert BEFORE INSERT ON installations
WHEN (NEW.kind = 'dav') <> (NEW.scopes <> '')
  OR NEW.scopes NOT IN ('', 'contacts', 'calendar', 'calendar,contacts')
BEGIN SELECT RAISE(ABORT,'dav installations need known scopes, others must have none'); END;
CREATE TRIGGER installation_scopes_update BEFORE UPDATE OF kind, scopes ON installations
WHEN (NEW.kind = 'dav') <> (NEW.scopes <> '')
  OR NEW.scopes NOT IN ('', 'contacts', 'calendar', 'calendar,contacts')
BEGIN SELECT RAISE(ABORT,'dav installations need known scopes, others must have none'); END;
";

/// Registration proves possession for passkey management, while minting
/// app passwords continues to require a sign-in assertion.
const SCHEMA_V10: &str = "
ALTER TABLE installations ADD COLUMN registration_session INTEGER NOT NULL DEFAULT 0;
";

/// v11: independent Notes app-password scopes and the named-share source
/// journal. Each admission is sequenced with its one durable fan-out intent.
const SCHEMA_V11: &str = "
DROP TRIGGER IF EXISTS installation_scopes_insert;
DROP TRIGGER IF EXISTS installation_scopes_update;
CREATE TRIGGER installation_scopes_insert BEFORE INSERT ON installations
WHEN (NEW.kind <> 'dav' AND NEW.scopes <> '')
  OR NEW.scopes NOT IN ('', 'contacts', 'calendar', 'calendar,contacts')
BEGIN SELECT RAISE(ABORT,'dav installations need known scopes, others must have none'); END;
CREATE TRIGGER installation_scopes_update BEFORE UPDATE OF kind, scopes ON installations
WHEN (NEW.kind <> 'dav' AND NEW.scopes <> '')
  OR NEW.scopes NOT IN ('', 'contacts', 'calendar', 'calendar,contacts')
BEGIN SELECT RAISE(ABORT,'dav installations need known scopes, others must have none'); END;

CREATE TABLE installation_notes_scopes (
    installation_id TEXT NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
    scope TEXT NOT NULL CHECK (scope IN ('notes.read','notes.write','notes.share')),
    PRIMARY KEY (installation_id, scope)
) STRICT;
CREATE TRIGGER installation_notes_scopes_insert BEFORE INSERT ON installation_notes_scopes
WHEN (SELECT kind FROM installations WHERE id=NEW.installation_id) <> 'dav'
BEGIN SELECT RAISE(ABORT,'only app passwords may have Notes scopes'); END;

CREATE TABLE note_shares (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    note_id TEXT NOT NULL,
    last_note_seq INTEGER NOT NULL DEFAULT 0 CHECK (last_note_seq >= 0),
    created_at INTEGER NOT NULL,
    creator_installation TEXT REFERENCES installations(id),
    UNIQUE (owner_id, note_id)
) STRICT;
CREATE TABLE share_grants (
    id TEXT PRIMARY KEY,
    share_id TEXT NOT NULL REFERENCES note_shares(id) ON DELETE CASCADE,
    person_id TEXT NOT NULL REFERENCES users(id),
    permission TEXT NOT NULL CHECK (permission IN ('read','edit')),
    grant_start_seq INTEGER NOT NULL CHECK (grant_start_seq >= 0),
    revoked_at_seq INTEGER CHECK (revoked_at_seq IS NULL OR revoked_at_seq > grant_start_seq),
    created_by TEXT NOT NULL REFERENCES users(id),
    revoked_by TEXT REFERENCES users(id)
) STRICT;
CREATE UNIQUE INDEX share_grants_active_person
    ON share_grants(share_id, person_id) WHERE revoked_at_seq IS NULL;
CREATE INDEX share_grants_person ON share_grants(person_id, share_id);

CREATE TABLE note_journal (
    owner_id TEXT NOT NULL,
    note_id TEXT NOT NULL,
    note_seq INTEGER NOT NULL CHECK (note_seq > 0),
    event_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    item TEXT NOT NULL,
    data TEXT NOT NULL,
    actor_id TEXT NOT NULL REFERENCES users(id),
    installation_id TEXT REFERENCES installations(id),
    request_id TEXT,
    at_ms INTEGER NOT NULL,
    PRIMARY KEY (owner_id, note_id, note_seq),
    UNIQUE (owner_id, event_id),
    FOREIGN KEY (owner_id, note_id) REFERENCES note_shares(owner_id, note_id)
) STRICT;
CREATE TABLE share_fanout (
    owner_id TEXT NOT NULL,
    note_id TEXT NOT NULL,
    note_seq INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (owner_id, note_id, note_seq),
    FOREIGN KEY (owner_id, note_id, note_seq)
        REFERENCES note_journal(owner_id, note_id, note_seq) ON DELETE CASCADE
) STRICT;
CREATE INDEX share_fanout_pending ON share_fanout(created_at, owner_id, note_id, note_seq);

CREATE TABLE recipient_control (
    id TEXT PRIMARY KEY,
    recipient_id TEXT NOT NULL REFERENCES users(id),
    share_id TEXT NOT NULL REFERENCES note_shares(id) ON DELETE CASCADE,
    note_seq INTEGER NOT NULL CHECK (note_seq >= 0),
    kind TEXT NOT NULL CHECK (kind IN ('purge','delete','baseline','loss-of-edit')),
    created_at INTEGER NOT NULL,
    UNIQUE (recipient_id, share_id, note_seq, kind)
) STRICT;
CREATE TABLE recipient_control_acks (
    control_id TEXT NOT NULL REFERENCES recipient_control(id) ON DELETE CASCADE,
    installation_id TEXT NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
    acknowledged_at INTEGER NOT NULL,
    PRIMARY KEY (control_id, installation_id)
) STRICT;
CREATE TABLE recipient_share_state (
    recipient_id TEXT NOT NULL REFERENCES users(id),
    share_id TEXT NOT NULL REFERENCES note_shares(id) ON DELETE CASCADE,
    hidden INTEGER NOT NULL DEFAULT 0 CHECK (hidden IN (0,1)),
    PRIMARY KEY (recipient_id, share_id)
) STRICT;
";

/// v12: acknowledgements bound fan-out retention; dormant Installations can
/// resume through a full authorized reset after leaving the known set.
const SCHEMA_V12: &str = "
CREATE TABLE share_fanout_acks (
    owner_id TEXT NOT NULL,
    note_id TEXT NOT NULL,
    note_seq INTEGER NOT NULL,
    installation_id TEXT NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
    acknowledged_at INTEGER NOT NULL,
    PRIMARY KEY (owner_id, note_id, note_seq, installation_id),
    FOREIGN KEY (owner_id, note_id, note_seq)
        REFERENCES share_fanout(owner_id, note_id, note_seq) ON DELETE CASCADE
) STRICT;
CREATE INDEX share_fanout_acks_installation
    ON share_fanout_acks(installation_id, owner_id, note_id, note_seq);
CREATE TABLE recipient_share_fences (
    recipient_id TEXT NOT NULL REFERENCES users(id),
    share_id TEXT NOT NULL REFERENCES note_shares(id) ON DELETE CASCADE,
    note_seq INTEGER NOT NULL CHECK (note_seq >= 0),
    PRIMARY KEY (recipient_id, share_id)
) STRICT;
CREATE TABLE sharing_installation_resets (
    installation_id TEXT PRIMARY KEY REFERENCES installations(id) ON DELETE CASCADE,
    required_at INTEGER NOT NULL
) STRICT;
CREATE INDEX recipient_control_note_order
    ON recipient_control(recipient_id, share_id, note_seq, id);
";

/// v13: bind full reset acknowledgements to their per-note sequence snapshot.
const SCHEMA_V13: &str = "
CREATE TABLE sharing_reset_cursors (
    installation_id TEXT NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
    owner_id TEXT NOT NULL REFERENCES users(id),
    note_id TEXT NOT NULL,
    note_seq INTEGER NOT NULL CHECK (note_seq >= 0),
    PRIMARY KEY (installation_id, owner_id, note_id)
) STRICT;
CREATE INDEX sharing_reset_cursors_note
    ON sharing_reset_cursors(owner_id, note_id, note_seq);
";

/// v14: a shared note stays unavailable while it is in the owner's Trash.
const SCHEMA_V14: &str = "
ALTER TABLE note_shares ADD COLUMN deleted_at_seq INTEGER
    CHECK (deleted_at_seq IS NULL OR deleted_at_seq > 0);
";

/// v15: durable publication receipts for recipient journal fan-out.
const SCHEMA_V15: &str = "
CREATE TABLE share_fanout_deliveries (
    owner_id TEXT NOT NULL,
    note_id TEXT NOT NULL,
    note_seq INTEGER NOT NULL,
    recipient_id TEXT NOT NULL REFERENCES users(id),
    delivered_at INTEGER NOT NULL,
    PRIMARY KEY (owner_id, note_id, note_seq, recipient_id),
    FOREIGN KEY (owner_id, note_id, note_seq)
        REFERENCES share_fanout(owner_id, note_id, note_seq) ON DELETE CASCADE
) STRICT;
CREATE INDEX share_fanout_deliveries_recipient
    ON share_fanout_deliveries(recipient_id, delivered_at);
";

/// A handle to `auth.db`. Cheap to clone.
#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
    worker: Arc<OnceLock<tokio::sync::mpsc::Sender<Job>>>,
}

type Job = Box<dyn FnOnce(&mut Connection) + Send>;

impl Db {
    /// Opens or creates the database and brings its schema up to date.
    ///
    /// Blocking startup operation: creates parent directories, configures
    /// WAL/FULL/foreign keys and a 5 s busy timeout, then applies migrations
    /// atomically. Call outside async request workers. Returns [`Error::Internal`]
    /// on filesystem, connection or migration failure. Creates the requested
    /// parent as 0700 and database as 0600 before SQLite can create sidecars.
    /// Does not validate root pairing or recover lost installation state;
    /// callers must apply those guards before allowing database creation.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            create_parent(dir)?;
        }
        crate::private_fs::file(path).map_err(Error::internal)?;
        let conn = Connection::open(path).map_err(Error::internal)?;
        configure(&conn).map_err(Error::internal)?;
        migrate(&conn).map_err(Error::internal)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            worker: Arc::new(OnceLock::new()),
        })
    }

    /// Runs `f` with the connection on a dedicated blocking thread.
    ///
    /// One call allocates a boxed closure and a oneshot result channel, sends
    /// once through a bounded MPSC queue, and receives once through the oneshot.
    /// Admission is O(1); latency includes waiting for capacity, earlier jobs,
    /// connection locking and `f` itself (including SQLite's 5 s busy timeout).
    /// The first call also starts one OS thread shared by all handle clones.
    /// At most 64 jobs are queued, plus one executing; waiting caller futures
    /// retain their own closures outside the queue. No general Tokio blocking
    /// pool slot is used. Use [`Self::try_run`] for immediate overload refusal.
    /// Dropping the future after admission does not cancel `f`.
    ///
    /// Returns an internal error if the worker or result channel stops.
    /// Like the source, panics if the first worker thread cannot be started.
    pub async fn run<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    {
        let worker = self.worker();
        let (tx, rx) = tokio::sync::oneshot::channel();
        worker
            .send(Box::new(move |c| {
                let _ = tx.send(f(c));
            }))
            .await
            .map_err(|_| Error::internal("auth database worker stopped"))?;
        rx.await.map_err(Error::internal)?
    }

    /// Runs `f` on the same worker, refusing admission immediately if all 64
    /// queued slots are occupied. The currently executing job is separate.
    /// Returns [`Error::QueueFull`] without executing `f` on saturation, or
    /// [`Error::Internal`] if the worker stops. Once admitted, cancellation
    /// does not cancel the database operation. Costs match [`Self::run`],
    /// except there is no queue-capacity wait.
    pub async fn try_run<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.worker()
            .try_send(Box::new(move |c| {
                let _ = tx.send(f(c));
            }))
            .map_err(|e| match e {
                tokio::sync::mpsc::error::TrySendError::Full(_) => Error::QueueFull,
                tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                    Error::internal("auth database worker stopped")
                }
            })?;
        rx.await.map_err(Error::internal)?
    }

    /// Runs a job synchronously on the same bounded dedicated worker.
    /// Host adapters with synchronous directory traits must call this only from
    /// blocking threads, never a Tokio async worker. Admission waits for capacity
    /// and preserves the source worker's 64 queued-job bound and ordering.
    /// Cancellation after admission does not cancel the job.
    pub fn run_blocking<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.worker()
            .blocking_send(Box::new(move |c| {
                let _ = tx.send(f(c));
            }))
            .map_err(|_| Error::internal("auth database worker stopped"))?;
        rx.blocking_recv().map_err(Error::internal)?
    }

    fn worker(&self) -> &tokio::sync::mpsc::Sender<Job> {
        // One dedicated worker owns the serialized database workload. Its bounded
        // queue never occupies Tokio's general blocking pool while waiting.
        self.worker.get_or_init(|| {
            let (tx, mut rx) = tokio::sync::mpsc::channel::<Job>(64);
            let conn = self.conn.clone();
            std::thread::Builder::new()
                .name("auth-db".into())
                .spawn(move || {
                    while let Some(job) = rx.blocking_recv() {
                        let mut conn = conn.lock().unwrap_or_else(|p| p.into_inner());
                        job(&mut conn);
                    }
                })
                .expect("could not start auth database worker");
            tx
        })
    }
}

/// Opens a configured, migrated connection for synchronous host/CLI work.
///
/// Uses the same blocking creation/migration behavior and errors as [`Db::open`],
/// without starting a dedicated worker. Callers own serialization and must keep
/// SQLite work off async request workers. Root validation remains the caller's
/// responsibility, including refusal to recreate unexpectedly missing auth state.
pub fn open_conn(path: &Path) -> Result<Connection> {
    let db = Db::open(path)?;
    let conn = Arc::try_unwrap(db.conn)
        .map_err(|_| Error::internal("auth.db still shared"))?
        .into_inner()
        .unwrap_or_else(|p| p.into_inner());
    Ok(conn)
}

fn configure(conn: &Connection) -> rusqlite::Result<()> {
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "FULL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    // Read the version after taking the write lock: a CLI command may open
    // the same database while the server is starting.
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
    let have: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (version, schema) in [
        (1, SCHEMA_V1),
        (2, SCHEMA_V2),
        (3, SCHEMA_V3),
        (4, SCHEMA_V4),
        (5, SCHEMA_V5),
        (6, SCHEMA_V6),
        (7, SCHEMA_V7),
        (8, SCHEMA_V8),
        (9, SCHEMA_V9),
        (10, SCHEMA_V10),
        (11, SCHEMA_V11),
        (12, SCHEMA_V12),
        (13, SCHEMA_V13),
        (14, SCHEMA_V14),
        (15, SCHEMA_V15),
        (16, include_str!("spaces_schema.sql")),
    ] {
        if have < version {
            tx.execute_batch(schema)?;
            tx.pragma_update(None, "user_version", version)?;
        }
    }
    debug_assert!(
        have >= VERSION
            || tx.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))? == VERSION
    );
    tx.commit()
}

// Owner-only creation and fsio directory durability; not a root guard.
fn create_parent(dir: &Path) -> Result<()> {
    crate::private_fs::directory(dir).map_err(Error::internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn migration_keeps_the_first_owner_when_creation_times_tie() {
        let dir = TempDir::new();
        let path = dir.path().join("auth.db");
        let old = Connection::open(&path).unwrap();
        old.execute_batch(&format!("{SCHEMA_V1} {SCHEMA_V2}
          PRAGMA user_version=2;
          INSERT INTO users VALUES ('z-owner','Owner','owner',10),('a-member','Member','member',10);" )).unwrap();
        drop(old);
        let migrated = open_conn(&path).unwrap();
        let admin: String = migrated
            .query_row("SELECT id FROM users WHERE role='admin'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(admin, "z-owner");
    }

    #[test]
    fn existing_database_keeps_people_credentials_and_sessions() {
        let dir = TempDir::new();
        let source = dir.path().join("old.db");
        let old = Connection::open(&source).unwrap();
        old.execute_batch(&format!("{SCHEMA_V1} {SCHEMA_V2}
          PRAGMA user_version=2;
          INSERT INTO users VALUES ('first','Owner','owner',10),('second','Member','member',20);
          INSERT INTO passkeys (cred_id,user_id,passkey,created_at,name) VALUES (x'01','first','{{}}',10,'Phone');
          INSERT INTO setup_tokens VALUES (x'02','recover','second',20,9999999999999,NULL);
          INSERT INTO installations VALUES ('device','first','browser','Phone',x'03',10,10,NULL,NULL);")).unwrap();
        drop(old);
        let path = dir.path().join("copy.db");
        std::fs::copy(source, &path).unwrap();
        let c = open_conn(&path).unwrap();
        let rows: Vec<(String, String)> = c
            .prepare("SELECT id,role FROM users ORDER BY created_at")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            rows,
            vec![
                ("first".into(), "admin".into()),
                ("second".into(), "member".into())
            ]
        );
        for table in ["passkeys", "setup_tokens", "installations"] {
            let count: i64 = c
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 1);
        }
        drop(c);
        drop(open_conn(&path).unwrap());
    }

    #[test]
    fn version_7_gets_scopes_and_scope_rules_are_enforced() {
        let dir = TempDir::new();
        let path = dir.path().join("old.db");
        let old = Connection::open(&path).unwrap();
        old.execute_batch(&format!(
            "{SCHEMA_V1} {SCHEMA_V2} {SCHEMA_V3} {SCHEMA_V4} {SCHEMA_V5} {SCHEMA_V6} {SCHEMA_V7}
             PRAGMA user_version = 7;
             INSERT INTO users (id,name,username,created_at) VALUES ('u','U','u',1);
             INSERT INTO installations VALUES ('b','u','browser','Safari',x'01',1,1,NULL,NULL);
             INSERT INTO installations VALUES ('d','u','dav','Old',x'02',1,1,NULL,NULL);"
        ))
        .unwrap();
        drop(old);
        let c = open_conn(&path).unwrap();
        let scopes: Vec<String> = c
            .prepare("SELECT scopes FROM installations ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        // Rows from before keep working as they were; an old dav row has no
        // scope, so it can reach nothing.
        assert_eq!(scopes, vec!["", ""]);
        let bad = |kind: &str, scopes: &str, n: u8| {
            let token_hash = [0xff, n];
            c.execute(
                "INSERT INTO installations (id,user_id,kind,name,token_hash,created_at,last_seen_at,scopes)
                 VALUES (?1,'u',?2,'x',?4,1,1,?3)",
                rusqlite::params![format!("n{n}"), kind, scopes, token_hash.as_slice()],
            )
        };
        assert!(
            bad("dav", "", 1).is_ok(),
            "Notes-only app passwords have no DAV scopes"
        );
        assert!(bad("browser", "contacts", 2).is_err(), "browsers get none");
        assert!(bad("dav", "bogus", 4).is_err(), "only known scope names");
        assert!(bad("dav", "contacts,files", 5).is_err());
        assert!(bad("dav", "contacts", 3).is_ok());
        c.execute(
            "INSERT INTO installation_notes_scopes (installation_id, scope) VALUES ('n3','notes.read')",
            [],
        )
        .unwrap();
        assert!(
            c.execute("UPDATE installations SET scopes='' WHERE id='n3'", [])
                .is_ok(),
            "a Notes-only app password has no DAV scopes"
        );
    }

    #[test]
    fn version_9_preserves_signin_proof_without_marking_registration() {
        let dir = TempDir::new();
        let path = dir.path().join("v9.db");
        let old = Connection::open(&path).unwrap();
        old.execute_batch(&format!(
            "{SCHEMA_V1} {SCHEMA_V2} {SCHEMA_V3} {SCHEMA_V4} {SCHEMA_V5} {SCHEMA_V6} {SCHEMA_V7} {SCHEMA_V8} {SCHEMA_V9}
             PRAGMA user_version=9;
             INSERT INTO users (id,name,username,created_at) VALUES ('u','U','u',1);
             INSERT INTO installations (id,user_id,kind,name,token_hash,created_at,last_seen_at,assertion_at)
             VALUES ('login','u','browser','Browser',x'01',1,1,123),
                    ('legacy','u','browser','Browser',x'02',1,1,NULL);"
        )).unwrap();
        drop(old);
        let c = open_conn(&path).unwrap();
        for (id, expected) in [("login", Some(123)), ("legacy", None)] {
            let (proof, registration): (Option<i64>, bool) = c
                .query_row(
                    "SELECT assertion_at, registration_session FROM installations WHERE id=?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(proof, expected);
            assert!(!registration);
        }
    }

    #[test]
    fn version_10_migrates_notes_scopes_and_sharing_tables() {
        let dir = TempDir::new();
        let path = dir.path().join("v10.db");
        let old = Connection::open(&path).unwrap();
        old.execute_batch(&format!(
            "{SCHEMA_V1} {SCHEMA_V2} {SCHEMA_V3} {SCHEMA_V4} {SCHEMA_V5} {SCHEMA_V6} {SCHEMA_V7} {SCHEMA_V8} {SCHEMA_V9} {SCHEMA_V10}
             PRAGMA user_version=10;
             INSERT INTO users (id,name,username,created_at) VALUES ('u','U','u',1);
             INSERT INTO installations (id,user_id,kind,name,token_hash,created_at,last_seen_at,scopes)
             VALUES ('dav','u','dav','DAV',x'01',1,1,'contacts');"
        )).unwrap();
        drop(old);

        let c = open_conn(&path).unwrap();
        let version: i64 = c
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, VERSION);
        let reset_cursors: i64 = c
            .query_row(
                "SELECT count(*) FROM sqlite_master
                 WHERE type='table' AND name='sharing_reset_cursors'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(reset_cursors, 1);
        let dav_scopes: String = c
            .query_row("SELECT scopes FROM installations WHERE id='dav'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(dav_scopes, "contacts");

        // A Notes-only password has no DAV protocols in the legacy column.
        c.execute(
            "INSERT INTO installations (id,user_id,kind,name,token_hash,created_at,last_seen_at,scopes)
             VALUES ('notes-only','u','dav','Notes',x'02',1,1,'')",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO installation_notes_scopes (installation_id, scope) VALUES ('notes-only','notes.read')",
            [],
        )
        .unwrap();

        for scope in ["notes.read", "notes.write", "notes.share"] {
            c.execute(
                "INSERT INTO installation_notes_scopes (installation_id, scope) VALUES ('dav', ?1)",
                [scope],
            )
            .unwrap();
        }
        assert!(c.execute(
            "INSERT INTO installation_notes_scopes (installation_id, scope) VALUES ('dav', 'notes.admin')",
            [],
        ).is_err());

        for table in [
            "note_shares",
            "share_grants",
            "share_fanout",
            "share_fanout_acks",
            "recipient_control",
            "recipient_share_fences",
            "sharing_installation_resets",
        ] {
            c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        }
    }

    #[test]
    fn opens_twice_and_keeps_schema() {
        let dir = TempDir::new();
        let path = dir.path().join("system/auth.db");
        drop(Db::open(&path).unwrap());
        let conn = open_conn(&path).unwrap();
        let v: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, VERSION);
        let sync: i64 = conn
            .pragma_query_value(None, "synchronous", |r| r.get(0))
            .unwrap();
        assert_eq!(sync, 2, "auth.db must use synchronous=FULL");
    }

    #[test]
    fn a_version_1_database_gets_passkey_names() {
        let dir = TempDir::new();
        let path = dir.path().join("system/auth.db");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let old = Connection::open(&path).unwrap();
        old.execute_batch(&format!(
            "{SCHEMA_V1} PRAGMA user_version = 1;
             INSERT INTO users VALUES ('u', 'Kay', 'kay', 1);
             INSERT INTO passkeys (cred_id, user_id, passkey, created_at) VALUES (x'01', 'u', '{{}}', 1);"
        ))
        .unwrap();
        drop(old);
        let conn = open_conn(&path).unwrap();
        let name: String = conn
            .query_row("SELECT name FROM passkeys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "Passkey");
    }
}

#[cfg(test)]
mod port_tests {
    use super::*;
    use crate::test_support::TempDir;
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    const MIGRATIONS: [&str; 16] = [
        SCHEMA_V1,
        SCHEMA_V2,
        SCHEMA_V3,
        SCHEMA_V4,
        SCHEMA_V5,
        SCHEMA_V6,
        SCHEMA_V7,
        SCHEMA_V8,
        SCHEMA_V9,
        SCHEMA_V10,
        SCHEMA_V11,
        SCHEMA_V12,
        SCHEMA_V13,
        SCHEMA_V14,
        SCHEMA_V15,
        include_str!("spaces_schema.sql"),
    ];

    // Physical rootpage numbers are allocation details, not schema. Include all
    // tables, indexes (including autoindexes) and triggers with their exact SQL.
    fn schema(c: &Connection) -> Vec<(String, String, String, Option<String>)> {
        c.prepare("SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY type,name")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn fresh_and_each_individually_committed_version_have_identical_schema() {
        let dir = TempDir::new();
        let fresh = open_conn(&dir.path().join("fresh.db")).unwrap();
        let expected = schema(&fresh);
        assert_eq!(expected.iter().filter(|r| r.0 == "table").count(), 29);
        // Every historical version is independently a supported opening point.
        for have in 0..=16 {
            let path = dir.path().join(format!("version-{have}.db"));
            let mut old = Connection::open(&path).unwrap();
            for (index, sql) in MIGRATIONS.iter().take(have).enumerate() {
                let tx = old.transaction().unwrap();
                tx.execute_batch(sql).unwrap();
                tx.pragma_update(None, "user_version", (index + 1) as i64)
                    .unwrap();
                tx.commit().unwrap();
            }
            drop(old);
            let upgraded = open_conn(&path).unwrap();
            assert_eq!(schema(&upgraded), expected, "starting version {have}");
            assert_eq!(
                upgraded
                    .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                    .unwrap(),
                16
            );
            drop(upgraded);
            let reopened = open_conn(&path).unwrap();
            assert_eq!(schema(&reopened), expected, "reopen version {have}");
        }
    }

    // Database portion of security_regressions.rs:424–507. The full real
    // credential/login test remains ignored in tests/migration.rs.
    #[test]
    fn migration_failure_rolls_back_schema_version_and_identity_then_retries() {
        let dir = TempDir::new();
        let path = dir.path().join("auth.db");
        let c = Connection::open(&path).unwrap();
        c.execute_batch(include_str!("../tests/fixtures/auth-v2.sql"))
            .unwrap();
        c.execute_batch(
            "INSERT INTO users VALUES ('u','Existing','existing',1),('v','Second','second',2);
            INSERT INTO passkeys VALUES (x'01','u','opaque original credential',1,NULL,'Phone');
            INSERT INTO installations VALUES ('device','u','browser','Phone',x'03',1,1,NULL,NULL);
            INSERT INTO setup_tokens VALUES (x'02','recover','u',1,9999999999999,NULL);
            CREATE TABLE groups (collision TEXT);",
        )
        .unwrap();
        let before = schema(&c);
        assert!(Db::open(&path).is_err());
        assert_eq!(
            schema(&c),
            before,
            "all successful DDL before collision rolls back too"
        );
        assert_eq!(
            c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            c.query_row("SELECT passkey FROM passkeys", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "opaque original credential"
        );
        // The previous schema remains writable after failed migration.
        c.execute("UPDATE users SET name='Still usable' WHERE id='v'", [])
            .unwrap();
        c.execute("DROP TABLE groups", []).unwrap();
        let upgraded = open_conn(&path).unwrap();
        let after = schema(&upgraded);
        drop(upgraded);
        let reopened = open_conn(&path).unwrap();
        assert_eq!(schema(&reopened), after);
        assert_eq!(
            reopened
                .query_row("SELECT role FROM users WHERE id='u'", [], |r| r
                    .get::<_, String>(0))
                .unwrap(),
            "admin"
        );
        assert_eq!(
            reopened
                .query_row("SELECT name FROM users WHERE id='v'", [], |r| r
                    .get::<_, String>(0))
                .unwrap(),
            "Still usable"
        );
        assert_eq!(
            reopened
                .query_row("SELECT passkey FROM passkeys", [], |r| r
                    .get::<_, String>(0))
                .unwrap(),
            "opaque original credential"
        );
        assert_eq!(
            reopened
                .query_row("SELECT token_hash FROM installations", [], |r| r
                    .get::<_, Vec<u8>>(0))
                .unwrap(),
            vec![3]
        );
        assert_eq!(
            reopened
                .query_row(
                    "SELECT hash FROM setup_tokens WHERE used_at IS NULL",
                    [],
                    |r| r.get::<_, Vec<u8>>(0)
                )
                .unwrap(),
            vec![2]
        );
        assert_eq!(
            reopened
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            16
        );
    }

    #[test]
    fn connection_preserves_all_source_pragmas() {
        let dir = TempDir::new();
        let c = open_conn(&dir.path().join("system/auth.db")).unwrap();
        assert_eq!(
            c.pragma_query_value(None, "journal_mode", |r| r.get::<_, String>(0))
                .unwrap(),
            "wal"
        );
        for (pragma, expected) in [
            ("synchronous", 2),
            ("foreign_keys", 1),
            ("busy_timeout", 5000),
        ] {
            assert_eq!(
                c.pragma_query_value(None, pragma, |r| r.get::<_, i64>(0))
                    .unwrap(),
                expected
            );
        }
    }

    #[tokio::test]
    async fn worker_refuses_65th_queued_job_and_run_retains_waiting_backpressure() {
        let dir = TempDir::new();
        let db = Db::open(&dir.path().join("auth.db")).unwrap();
        let (release, blocked) = std::sync::mpsc::channel();
        let (started_tx, started) = tokio::sync::oneshot::channel();
        let first_db = db.clone();
        let first = tokio::spawn(async move {
            first_db
                .run(move |_| {
                    started_tx.send(()).unwrap();
                    blocked.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(())
                })
                .await
        });
        started.await.unwrap();
        let mut cx = Context::from_waker(Waker::noop());
        let mut admitted = Vec::new();
        for n in 0..64 {
            let mut future = Box::pin(db.try_run(move |_| Ok(n)));
            assert!(future.as_mut().poll(&mut cx).is_pending());
            admitted.push(future);
        }
        assert_eq!(db.worker().capacity(), 0);
        let mut refused =
            Box::pin(db.try_run(|_| -> Result<()> { panic!("refused job executed") }));
        assert!(matches!(
            refused.as_mut().poll(&mut cx),
            Poll::Ready(Err(Error::QueueFull))
        ));
        // Source run waits without growing the queue, while try_run refuses.
        let mut waiting = Box::pin(db.run(|_| Ok(65)));
        assert!(waiting.as_mut().poll(&mut cx).is_pending());
        assert_eq!(db.worker().capacity(), 0);
        release.send(()).unwrap();
        first.await.unwrap().unwrap();
        for (n, future) in admitted.into_iter().enumerate() {
            assert_eq!(future.await.unwrap(), n);
        }
        assert_eq!(waiting.await.unwrap(), 65);
        assert_eq!(db.worker().capacity(), 64);
    }

    // Ported from reliability_review.rs:805–846; probe the database directly
    // instead of the excluded HTTP /auth/status adapter.
    #[test]
    fn authentication_database_does_not_wait_for_general_blocking_workers() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(2)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let dir = TempDir::new();
            let db = Db::open(&dir.path().join("auth.db")).unwrap();
            db.run(|c| {
                c.query_row("SELECT count(*) FROM users", [], |r| r.get::<_, i64>(0))
                    .map_err(Error::internal)
            })
            .await
            .unwrap();
            let mut release = Vec::new();
            let mut workers = Vec::new();
            for _ in 0..2 {
                let (tx, rx) = std::sync::mpsc::channel();
                let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
                release.push(tx);
                workers.push(tokio::task::spawn_blocking(move || {
                    ready_tx.send(()).unwrap();
                    rx.recv_timeout(Duration::from_secs(5)).unwrap();
                }));
                ready_rx.await.unwrap();
            }
            let result = tokio::time::timeout(
                Duration::from_millis(100),
                db.run(|c| {
                    c.query_row("SELECT count(*) FROM users", [], |r| r.get::<_, i64>(0))
                        .map_err(Error::internal)
                }),
            )
            .await;
            for tx in release {
                tx.send(()).unwrap();
            }
            for worker in workers {
                worker.await.unwrap();
            }
            assert_eq!(
                result
                    .expect("auth waited for the general blocking pool")
                    .unwrap(),
                0
            );
        });
    }
}

pub(crate) use calternal_types::now_ms;

/// Converts a SQLite failure into a private host diagnostic.
pub(crate) fn sql(e: rusqlite::Error) -> crate::Error {
    crate::Error::internal(e)
}
