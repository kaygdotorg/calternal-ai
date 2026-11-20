//! Where a space lives on disk, its host state and its index, opened once.
//!
//! Layout below the server root (one place, so the choice is easy to change):
//!
//! | Path | Class | Holds |
//! | --- | --- | --- |
//! | `spaces/<space>/` | userdata | the Markdown files people keep |
//! | `state/<space>/host.db` | host state (not rebuildable) | kinds and IDs of files, stamps, record positions and hashes, receipts, members, plugins, audit |
//! | `appdata/<space>/index.db` | appdata (rebuildable) | the field index for questions |
//!
//! Owner question A1/D5, recommended option: host state sits beside the files
//! and keeps **hashes, offsets and stamps only**, never a copy of a record's
//! text. Every consequence of that choice is in `records.rs` and marked `D5`.
use crate::decl::{self, Declaration};
use calternal_fsio::beneath::Folder;
use calternal_types::{AccountId, Error, SpaceId};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Host-state schema. Eight tables; see the layout table above.
const HOST_SCHEMA: &str = "
PRAGMA journal_mode=WAL;
PRAGMA synchronous=FULL;
PRAGMA secure_delete=ON;
CREATE TABLE IF NOT EXISTS space(key BLOB NOT NULL);
CREATE TABLE IF NOT EXISTS members(person TEXT PRIMARY KEY, name TEXT NOT NULL, role TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS plugins(id TEXT PRIMARY KEY, declaration TEXT NOT NULL, digest TEXT NOT NULL,
  installed_at INTEGER NOT NULL, installed_by TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS files(id TEXT PRIMARY KEY, kind TEXT NOT NULL, plugin TEXT NOT NULL,
  path TEXT NOT NULL UNIQUE, rev INTEGER NOT NULL, len INTEGER NOT NULL, prefix_len INTEGER NOT NULL,
  hash BLOB NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS stamps(seq INTEGER PRIMARY KEY, file TEXT NOT NULL, rev INTEGER NOT NULL,
  anchor TEXT, op TEXT NOT NULL, person TEXT NOT NULL, plugin TEXT NOT NULL, request TEXT NOT NULL,
  at INTEGER NOT NULL, hash BLOB);
CREATE TABLE IF NOT EXISTS records(anchor TEXT PRIMARY KEY, file TEXT NOT NULL, off INTEGER NOT NULL,
  len INTEGER NOT NULL, person TEXT NOT NULL, at INTEGER NOT NULL, hash BLOB NOT NULL, state TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS records_by_file ON records(file, off);
CREATE TABLE IF NOT EXISTS receipts(request TEXT PRIMARY KEY, name TEXT NOT NULL, result TEXT NOT NULL,
  at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS audit(seq INTEGER PRIMARY KEY, at INTEGER NOT NULL, person TEXT NOT NULL,
  plugin TEXT NOT NULL, name TEXT NOT NULL, request TEXT NOT NULL, allowed INTEGER NOT NULL, reason TEXT);
";

/// Appdata schema: one table, rebuilt from the files and host state.
const INDEX_SCHEMA: &str = "
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
CREATE TABLE IF NOT EXISTS fields(file TEXT NOT NULL, kind TEXT NOT NULL, key TEXT NOT NULL,
  value TEXT NOT NULL, PRIMARY KEY(file, key));
CREATE INDEX IF NOT EXISTS fields_by_value ON fields(kind, key, value);
";

/// Receipts and audit rows older than this are dropped at open (owner decision 2026-11-17 for receipts;
/// the audit bound is the spike's own stub, see the report).
pub const RETENTION_MS: i64 = 7 * 24 * 3600 * 1000;

/// Every space the server has opened. The map's lock is held only to find or
/// insert an entry, never across I/O; each space has its own lock (its single writer).
pub struct Host {
    root: PathBuf,
    spaces: Mutex<HashMap<SpaceId, Arc<Mutex<Space>>>>,
}

/// One open space: its folder, host state, index and installed plugins.
pub struct Space {
    /// The userdata folder; every path is resolved beneath it.
    pub folder: Folder,
    /// The userdata folder's path, for the few writes `Folder` cannot do (see `records.rs`).
    pub root: PathBuf,
    /// Host state.
    pub db: Connection,
    /// Appdata.
    pub index: Connection,
    key: [u8; 32],
    /// Installed declarations by plugin ID.
    pub plugins: BTreeMap<String, Declaration>,
    /// What the host last saw of each file it wrote: (length, mtime ns, inode).
    /// Not persisted: after a restart the first touch of a file verifies it.
    pub seen: HashMap<String, (u64, i128, u64)>,
}

impl Host {
    /// A host serving spaces below `root`. Cost: no I/O until a space is opened.
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_owned(),
            spaces: Mutex::new(HashMap::new()),
        }
    }

    /// The open space, opening it on first use.
    ///
    /// # Errors
    /// `not_found` when the space was never created (`host-spike init`).
    pub fn space(&self, id: SpaceId) -> Result<Arc<Mutex<Space>>, Error> {
        if let Some(s) = self.spaces.lock().unwrap().get(&id) {
            return Ok(s.clone());
        }
        if !self
            .root
            .join("state")
            .join(id.to_string())
            .join("host.db")
            .exists()
        {
            return Err(Error::not_found("no such space"));
        }
        let space = Arc::new(Mutex::new(Space::open(&self.root, id)?));
        Ok(self
            .spaces
            .lock()
            .unwrap()
            .entry(id)
            .or_insert(space)
            .clone())
    }
}

fn internal(e: impl std::fmt::Display) -> Error {
    eprintln!("internal: {e}");
    Error::internal("the host failed; see its log")
}

/// Map any lower-level failure to the caller-safe internal error, logging the detail.
pub fn fail<E: std::fmt::Display>(e: E) -> Error {
    internal(e)
}

impl Space {
    /// Create a space with its members (`init`). Idempotent for the same members.
    ///
    /// # Errors
    /// `internal` on any I/O or SQL failure.
    pub fn create(
        root: &Path,
        id: SpaceId,
        members: &[(AccountId, String, String)],
    ) -> Result<(), Error> {
        let space = Self::open(root, id)?;
        for (person, name, role) in members {
            space
                .db
                .execute(
                    "INSERT OR REPLACE INTO members(person, name, role) VALUES (?1, ?2, ?3)",
                    params![person.to_string(), name, role],
                )
                .map_err(fail)?;
        }
        Ok(())
    }

    fn open(root: &Path, id: SpaceId) -> Result<Self, Error> {
        let state = root.join("state").join(id.to_string());
        let app = root.join("appdata").join(id.to_string());
        let user = root.join("spaces").join(id.to_string());
        for dir in [&state, &app, &user] {
            calternal_fsio::create_dir_all_durable(dir).map_err(fail)?;
        }
        let db = Connection::open(state.join("host.db")).map_err(fail)?;
        db.execute_batch(HOST_SCHEMA).map_err(fail)?;
        let key: Option<Vec<u8>> = db
            .query_row("SELECT key FROM space", [], |r| r.get(0))
            .optional()
            .map_err(fail)?;
        let key = match key {
            Some(k) => k,
            None => {
                let k: Vec<u8> = [
                    uuid::Uuid::new_v4().into_bytes(),
                    uuid::Uuid::new_v4().into_bytes(),
                ]
                .concat();
                db.execute("INSERT INTO space(key) VALUES (?1)", [&k])
                    .map_err(fail)?;
                k
            }
        };
        let cutoff = calternal_types::now_ms() - RETENTION_MS;
        db.execute("DELETE FROM receipts WHERE at < ?1", [cutoff])
            .map_err(fail)?;
        db.execute("DELETE FROM audit WHERE at < ?1", [cutoff])
            .map_err(fail)?;
        let index = Connection::open(app.join("index.db")).map_err(fail)?;
        index.execute_batch(INDEX_SCHEMA).map_err(fail)?;
        let mut plugins = BTreeMap::new();
        {
            let mut stmt = db
                .prepare("SELECT declaration FROM plugins")
                .map_err(fail)?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(fail)?;
            for raw in rows {
                let value: serde_json::Value =
                    serde_json::from_str(&raw.map_err(fail)?).map_err(fail)?;
                let d = decl::validate(&value)?;
                plugins.insert(d.id.clone(), d);
            }
        }
        let mut space = Self {
            folder: Folder::open(&user).map_err(fail)?,
            root: user,
            db,
            index,
            key: key
                .try_into()
                .map_err(|_| internal("space key is not 32 bytes"))?,
            plugins,
            seen: HashMap::new(),
        };
        crate::query::rebuild(&mut space)?;
        Ok(space)
    }

    /// Keyed hash of bytes. Keyed by a per-space secret so a short record's
    /// hash ("ok") cannot be reversed by guessing (consequence of D5).
    pub fn hash(&self, bytes: &[u8]) -> Vec<u8> {
        blake3::keyed_hash(&self.key, bytes).as_bytes().to_vec()
    }

    /// The role of a member, or `None`.
    ///
    /// # Errors
    /// `internal` on SQL failure. Cost: one indexed read.
    pub fn role(&self, person: AccountId) -> Result<Option<(String, String)>, Error> {
        self.db
            .query_row(
                "SELECT role, name FROM members WHERE person = ?1",
                [person.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(fail)
    }

    /// The display name for a stamp's person (`outside` for disk edits).
    pub fn display_name(&self, person: &str) -> String {
        self.db
            .query_row(
                "SELECT name FROM members WHERE person = ?1",
                [person],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| person.to_owned())
    }

    /// The plugin that owns a kind, and the kind.
    pub fn kind(&self, kind: &str) -> Option<(&Declaration, &decl::Kind)> {
        self.plugins
            .values()
            .find_map(|d| d.kinds.get(kind).map(|k| (d, k)))
    }

    /// A file's host-state facts: (kind, path, rev, len, prefix_len, hash).
    ///
    /// # Errors
    /// `not_found` for an unknown ID. Cost: one indexed read.
    pub fn file(&self, id: &str) -> Result<FileRow, Error> {
        self.db
            .query_row(
                "SELECT kind, path, rev, len, prefix_len, hash FROM files WHERE id = ?1",
                [id],
                |r| {
                    Ok(FileRow {
                        id: id.to_owned(),
                        kind: r.get(0)?,
                        path: r.get(1)?,
                        rev: r.get(2)?,
                        len: r.get::<_, i64>(3)? as u64,
                        prefix_len: r.get::<_, i64>(4)? as u64,
                        hash: r.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(fail)?
            .ok_or_else(|| Error::not_found("no such file"))
    }

    /// (length, mtime ns, inode) of a file below the space root, without following symlinks.
    pub fn stat(&self, path: &str) -> Result<(u64, i128, u64), Error> {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::symlink_metadata(self.root.join(path)).map_err(fail)?;
        Ok((
            m.len(),
            m.mtime() as i128 * 1_000_000_000 + m.mtime_nsec() as i128,
            m.ino(),
        ))
    }
}

/// A file as host state knows it.
#[derive(Debug, Clone)]
pub struct FileRow {
    /// The file's ID (UUIDv7), fixed at create.
    pub id: String,
    /// The file's kind, fixed at create.
    pub kind: String,
    /// The path below the space root.
    pub path: String,
    /// The file's revision; every host write adds one.
    pub rev: i64,
    /// The length the host last wrote.
    pub len: u64,
    /// For record files, the length of the frontmatter part before the first record.
    pub prefix_len: u64,
    /// Keyed hash of the whole file (plain files) or of the frontmatter part (record files).
    pub hash: Vec<u8>,
}
