//! On-disk backend: one SQLite database per owner, values sealed under the
//! master key.
//!
//! Layout (all files 0600, directories 0700):
//! - `appdata/system/secret.key`: 32-byte master key.
//! - `appdata/system/secrets.db`: key fingerprint and the owners that have secrets.
//! - `appdata/accounts/<account-uuid>/secrets.db` and
//!   `appdata/spaces/<space-uuid>/secrets.db`: that owner's sealed values, pending
//!   requests and audit trail. Deleting an owner's directory deletes all three.
//!
//! Every call opens only its owner's database (one `open` and a `user_version`
//! read; the schema is created once per file) and closes it again; secret calls
//! are rare next to content calls, so no connection cache is kept, and no lock or
//! connection is shared between owners. Writes run in one IMMEDIATE transaction
//! with `synchronous=FULL`, so a crash leaves either the old or the new state.
//!
//! Hot path counts: `use_value` = 1 open, 1 indexed read, 1 AEAD open, 1 commit
//! (1 WAL fsync). `put`/`rotate`/`provide`/`request` = 1 open, 1–2 indexed reads,
//! 1 commit (1 WAL fsync); an owner's first write adds the registry insert and
//! directory creation (3 more fsyncs). Reads (`list`, `pending`, `audit`,
//! `handle`, `resolve`) = 1 open, 1 query, no fsync.
//! `secure_delete` overwrites freed pages, so a rotated or deleted ciphertext does
//! not linger in the main file; WAL frames holding it are overwritten at the next
//! checkpoint.

use crate::LIMITS;
use crate::key::MasterKey;
use crate::types::text;
use crate::{
    Action, Actor, AuditEntry, Consumer, Error, Handle, HandleToken, Metadata, Owner,
    PendingRequest, RequestId, Result, SecretName, SecretStore, SecretValue,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use std::path::{Path, PathBuf};
use std::time::Duration;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS secrets(
  name TEXT PRIMARY KEY, label TEXT NOT NULL, version INTEGER NOT NULL,
  handle TEXT NOT NULL UNIQUE, sealed BLOB NOT NULL,
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS requests(
  id TEXT PRIMARY KEY, name TEXT NOT NULL, label TEXT NOT NULL, reason TEXT NOT NULL,
  requested_by TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS audit(
  id INTEGER PRIMARY KEY, at INTEGER NOT NULL, actor TEXT NOT NULL, action TEXT NOT NULL,
  name TEXT NOT NULL, version INTEGER, consumer TEXT);";

/// Local secret store rooted at an installation's appdata directory.
/// One instance per appdata root; it is `Sync` and cheap to share in an `Arc`.
pub struct LocalStore {
    appdata: PathBuf,
    key: MasterKey,
}

impl LocalStore {
    /// Open the store, creating the master key on first use.
    /// The host must have validated and locked `appdata` first.
    /// Fails with [`Error::KeyMissing`] or [`Error::KeyUnusable`] instead of
    /// starting with a key that cannot read existing values.
    pub fn open(appdata: &Path) -> Result<Self> {
        let key = MasterKey::open(appdata, &crate::key::registry(appdata)?)?;
        Ok(Self {
            appdata: appdata.to_owned(),
            key,
        })
    }

    fn path(&self, owner: &Owner) -> PathBuf {
        let dir = match owner {
            Owner::Account(_) => "accounts",
            Owner::Space(_) => "spaces",
        };
        self.appdata
            .join(dir)
            .join(owner.uuid().as_hyphenated().to_string())
            .join("secrets.db")
    }

    /// Open for reading; `None` if this owner never stored anything.
    fn reader(&self, owner: &Owner) -> Result<Option<Connection>> {
        let path = self.path(owner);
        if !path.try_exists().map_err(|_| Error::Internal)? {
            return Ok(None);
        }
        open_db(&path, SCHEMA).map(Some)
    }

    /// Open for writing. An owner without a database is first recorded in the
    /// registry (so a lost key is detected even if a crash follows), then its
    /// database is created; later writes skip the registry entirely.
    fn writer(&self, owner: &Owner) -> Result<Connection> {
        if let Some(c) = self.reader(owner)? {
            return Ok(c);
        }
        crate::key::registry(&self.appdata)?
            .execute(
                "INSERT OR IGNORE INTO owners(kind,id) VALUES(?1,?2)",
                params![owner.kind(), owner.uuid().to_string()],
            )
            .map_err(|_| Error::Internal)?;
        let path = self.path(owner);
        crate::private_fs::directory(path.parent().ok_or(Error::Internal)?)
            .map_err(|_| Error::Internal)?;
        open_db(&path, SCHEMA)
    }

    fn store(
        &self,
        tx: &Transaction<'_>,
        owner: &Owner,
        name: &SecretName,
        label: &str,
        version: u64,
        value: &SecretValue,
    ) -> Result<Handle> {
        let handle = Handle {
            owner: *owner,
            name: name.clone(),
            version,
        };
        let sealed = self
            .key
            .seal(owner, name.as_str(), version, value.expose())?;
        let now = now_ms();
        tx.execute(
            "INSERT INTO secrets(name,label,version,handle,sealed,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?6)
             ON CONFLICT(name) DO UPDATE SET version=excluded.version, handle=excluded.handle,
               sealed=excluded.sealed, updated_at=excluded.updated_at",
            params![
                name.as_str(),
                label,
                version as i64,
                self.key.token(&handle).0,
                sealed,
                now
            ],
        )
        .map_err(|_| Error::Internal)?;
        Ok(handle)
    }
}

/// Open (creating with mode 0600 if needed) a database with FULL sync and
/// overwrite-on-delete. The schema and WAL mode are applied once, when
/// `user_version` is still 0; later opens only set per-connection pragmas.
pub(crate) fn open_db(path: &Path, schema: &str) -> Result<Connection> {
    crate::private_fs::file(path).map_err(|_| Error::Internal)?;
    let c = Connection::open(path).map_err(|_| Error::Internal)?;
    c.busy_timeout(Duration::from_secs(5))
        .map_err(|_| Error::Internal)?;
    c.execute_batch("PRAGMA synchronous=FULL; PRAGMA secure_delete=ON;")
        .map_err(|_| Error::Internal)?;
    let version: i64 = c
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|_| Error::Internal)?;
    if version == 0 {
        c.execute_batch(&format!(
            "PRAGMA journal_mode=WAL; BEGIN IMMEDIATE; {schema} PRAGMA user_version=1; COMMIT;"
        ))
        .map_err(|_| Error::Internal)?;
    }
    Ok(c)
}

fn begin(c: &mut Connection) -> Result<Transaction<'_>> {
    c.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| Error::Internal)
}

fn commit(tx: Transaction<'_>, point: &str) -> Result<()> {
    crash_point(point);
    tx.commit().map_err(|_| Error::Internal)
}

fn audit(
    tx: &Transaction<'_>,
    actor: &Actor,
    action: Action,
    name: &str,
    version: Option<u64>,
    consumer: Option<String>,
) -> Result<()> {
    tx.execute(
        "INSERT INTO audit(at,actor,action,name,version,consumer) VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            now_ms(),
            actor.describe(),
            action.as_str(),
            name,
            version.map(|v| v as i64),
            consumer
        ],
    )
    .map_err(|_| Error::Internal)?;
    // Keep the newest LIMITS.audit rows; usually deletes zero or one row.
    tx.execute(
        "DELETE FROM audit WHERE id <= last_insert_rowid() - ?1",
        [LIMITS.audit],
    )
    .map(drop)
    .map_err(|_| Error::Internal)
}

fn count(tx: &Connection, table: &str) -> Result<u32> {
    tx.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .map_err(|_| Error::Internal)
}

fn current(tx: &Connection, name: &str) -> Result<Option<(u64, String)>> {
    tx.query_row(
        "SELECT version,label FROM secrets WHERE name=?1",
        [name],
        |r| Ok((r.get::<_, i64>(0)? as u64, r.get(1)?)),
    )
    .optional()
    .map_err(|_| Error::Internal)
}

use calternal_types::now_ms;

/// Test-only process abort just before a commit, to prove crash atomicity.
#[cfg(test)]
fn crash_point(point: &str) {
    if std::env::var("CALTERNAL_SECRETS_CRASH").is_ok_and(|p| p == point) {
        std::process::abort();
    }
}
#[cfg(not(test))]
fn crash_point(_: &str) {}

impl SecretStore for LocalStore {
    /// Cost: one AEAD seal and one fsync'd transaction (see the module's counts).
    fn put(
        &self,
        owner: &Owner,
        name: &SecretName,
        label: &str,
        value: SecretValue,
        actor: &Actor,
    ) -> Result<Handle> {
        let label = text(label, 120)?;
        let mut c = self.writer(owner)?;
        let tx = begin(&mut c)?;
        if current(&tx, name.as_str())?.is_some() {
            return Err(Error::Conflict);
        }
        if count(&tx, "secrets")? >= LIMITS.secrets {
            return Err(Error::Limit);
        }
        let handle = self.store(&tx, owner, name, label, 1, &value)?;
        audit(&tx, actor, Action::Put, name.as_str(), Some(1), None)?;
        commit(tx, "put")?;
        Ok(handle)
    }

    /// Cost: as [`put`](Self::put); the old ciphertext is overwritten in place.
    fn rotate(
        &self,
        owner: &Owner,
        name: &SecretName,
        value: SecretValue,
        actor: &Actor,
    ) -> Result<Handle> {
        let mut c = self.reader(owner)?.ok_or(Error::NotFound)?;
        let tx = begin(&mut c)?;
        let (version, label) = current(&tx, name.as_str())?.ok_or(Error::NotFound)?;
        let handle = self.store(&tx, owner, name, &label, version + 1, &value)?;
        audit(
            &tx,
            actor,
            Action::Rotate,
            name.as_str(),
            Some(version + 1),
            None,
        )?;
        commit(tx, "rotate")?;
        Ok(handle)
    }

    /// Cost: one indexed read.
    fn handle(&self, owner: &Owner, name: &SecretName) -> Result<Handle> {
        let c = self.reader(owner)?.ok_or(Error::NotFound)?;
        let (version, _) = current(&c, name.as_str())?.ok_or(Error::NotFound)?;
        Ok(Handle {
            owner: *owner,
            name: name.clone(),
            version,
        })
    }

    /// Cost: one keyed BLAKE3 hash.
    fn token(&self, handle: &Handle) -> HandleToken {
        self.key.token(handle)
    }

    /// Cost: one unique-index read plus a MAC recheck.
    fn resolve(&self, owner: &Owner, token: &HandleToken) -> Result<Handle> {
        let c = self.reader(owner)?.ok_or(Error::NotFound)?;
        let (name, version): (String, i64) = c
            .query_row(
                "SELECT name,version FROM secrets WHERE handle=?1",
                [token.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|_| Error::Internal)?
            .ok_or(Error::NotFound)?;
        let handle = Handle {
            owner: *owner,
            name: SecretName::new(&name)?,
            version: version as u64,
        };
        // A row copied in from another owner carries that owner's MAC.
        if self.key.token(&handle) != *token {
            return Err(Error::NotFound);
        }
        Ok(handle)
    }

    /// Cost: one read, one AEAD open, one fsync'd audit insert. The audit row
    /// commits before the value is returned.
    fn use_value(
        &self,
        handle: &Handle,
        actor: &Actor,
        consumer: &Consumer,
    ) -> Result<SecretValue> {
        consumer.validate()?;
        let mut c = self.reader(&handle.owner)?.ok_or(Error::NotFound)?;
        let tx = begin(&mut c)?;
        let sealed: Vec<u8> = tx
            .query_row(
                "SELECT sealed FROM secrets WHERE name=?1 AND version=?2",
                params![handle.name.as_str(), handle.version as i64],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| Error::Internal)?
            .ok_or(Error::NotFound)?;
        let plain =
            self.key
                .open_value(&handle.owner, handle.name.as_str(), handle.version, &sealed)?;
        audit(
            &tx,
            actor,
            Action::Use,
            handle.name.as_str(),
            Some(handle.version),
            Some(consumer.describe()),
        )?;
        commit(tx, "use")?;
        Ok(SecretValue::from_plain(plain))
    }

    /// Cost: one table scan of this owner's secrets.
    fn list(&self, owner: &Owner) -> Result<Vec<Metadata>> {
        let Some(c) = self.reader(owner)? else {
            return Ok(Vec::new());
        };
        let mut s = c
            .prepare("SELECT name,label,version,created_at,updated_at FROM secrets ORDER BY name")
            .map_err(|_| Error::Internal)?;
        s.query_map([], |r| {
            Ok(Metadata {
                name: r.get(0)?,
                label: r.get(1)?,
                version: r.get::<_, i64>(2)? as u64,
                created_at: r.get(3)?,
                updated_at: r.get(4)?,
            })
        })
        .and_then(|rows| rows.collect())
        .map_err(|_| Error::Internal)
    }

    /// Cost: one fsync'd transaction; freed pages are overwritten.
    fn delete(&self, owner: &Owner, name: &SecretName, actor: &Actor) -> Result<()> {
        let mut c = self.reader(owner)?.ok_or(Error::NotFound)?;
        let tx = begin(&mut c)?;
        let (version, _) = current(&tx, name.as_str())?.ok_or(Error::NotFound)?;
        tx.execute("DELETE FROM secrets WHERE name=?1", [name.as_str()])
            .map_err(|_| Error::Internal)?;
        audit(
            &tx,
            actor,
            Action::Delete,
            name.as_str(),
            Some(version),
            None,
        )?;
        commit(tx, "delete")
    }

    /// Cost: one fsync'd insert.
    fn request(
        &self,
        owner: &Owner,
        name: &SecretName,
        label: &str,
        reason: &str,
        actor: &Actor,
    ) -> Result<RequestId> {
        let (label, reason) = (text(label, 120)?, text(reason, 500)?);
        let id = RequestId(uuid::Uuid::now_v7());
        let mut c = self.writer(owner)?;
        let tx = begin(&mut c)?;
        if count(&tx, "requests")? >= LIMITS.pending {
            return Err(Error::Limit);
        }
        tx.execute(
            "INSERT INTO requests(id,name,label,reason,requested_by,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
            params![id.0.to_string(), name.as_str(), label, reason, actor.describe(), now_ms()],
        )
        .map_err(|_| Error::Internal)?;
        audit(&tx, actor, Action::Request, name.as_str(), None, None)?;
        commit(tx, "request")?;
        Ok(id)
    }

    /// Cost: one scan of this owner's pending requests.
    fn pending(&self, owner: &Owner) -> Result<Vec<PendingRequest>> {
        let Some(c) = self.reader(owner)? else {
            return Ok(Vec::new());
        };
        let mut s = c
            .prepare(
                "SELECT id,name,label,reason,requested_by,created_at FROM requests ORDER BY id",
            )
            .map_err(|_| Error::Internal)?;
        s.query_map([], |r| {
            Ok(PendingRequest {
                id: uuid::Uuid::parse_str(&r.get::<_, String>(0)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                name: r.get(1)?,
                label: r.get(2)?,
                reason: r.get(3)?,
                requested_by: r.get(4)?,
                created_at: r.get(5)?,
            })
        })
        .and_then(|rows| rows.collect())
        .map_err(|_| Error::Internal)
    }

    /// Cost: as [`put`](Self::put) plus one delete, in one transaction.
    fn provide(
        &self,
        owner: &Owner,
        request: &RequestId,
        value: SecretValue,
        actor: &Actor,
    ) -> Result<Handle> {
        let mut c = self.reader(owner)?.ok_or(Error::NotFound)?;
        let tx = begin(&mut c)?;
        let (name, label): (String, String) = tx
            .query_row(
                "SELECT name,label FROM requests WHERE id=?1",
                [request.0.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|_| Error::Internal)?
            .ok_or(Error::NotFound)?;
        let name = SecretName::new(&name)?;
        let version = match current(&tx, name.as_str())? {
            Some((v, _)) => v + 1,
            None if count(&tx, "secrets")? >= LIMITS.secrets => return Err(Error::Limit),
            None => 1,
        };
        let handle = self.store(&tx, owner, &name, &label, version, &value)?;
        tx.execute("DELETE FROM requests WHERE id=?1", [request.0.to_string()])
            .map_err(|_| Error::Internal)?;
        audit(
            &tx,
            actor,
            Action::Provide,
            name.as_str(),
            Some(version),
            None,
        )?;
        commit(tx, "provide")?;
        Ok(handle)
    }

    /// Cost: one fsync'd delete.
    fn cancel(&self, owner: &Owner, request: &RequestId, actor: &Actor) -> Result<()> {
        let mut c = self.reader(owner)?.ok_or(Error::NotFound)?;
        let tx = begin(&mut c)?;
        let name: String = tx
            .query_row(
                "DELETE FROM requests WHERE id=?1 RETURNING name",
                [request.0.to_string()],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| Error::Internal)?
            .ok_or(Error::NotFound)?;
        audit(&tx, actor, Action::Cancel, &name, None, None)?;
        commit(tx, "cancel")
    }

    /// Cost: one reverse primary-key scan of at most `limit` rows.
    fn audit(&self, owner: &Owner, limit: u32) -> Result<Vec<AuditEntry>> {
        let Some(c) = self.reader(owner)? else {
            return Ok(Vec::new());
        };
        let mut s = c
            .prepare(
                "SELECT at,actor,action,name,version,consumer FROM audit ORDER BY id DESC LIMIT ?1",
            )
            .map_err(|_| Error::Internal)?;
        s.query_map([limit], |r| {
            Ok(AuditEntry {
                at: r.get(0)?,
                actor: r.get(1)?,
                action: Action::parse(&r.get::<_, String>(2)?)
                    .ok_or(rusqlite::Error::InvalidQuery)?,
                name: r.get(3)?,
                version: r.get::<_, Option<i64>>(4)?.map(|v| v as u64),
                consumer: r.get(5)?,
            })
        })
        .and_then(|rows| rows.collect())
        .map_err(|_| Error::Internal)
    }
}
