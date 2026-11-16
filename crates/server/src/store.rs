//! Development-preview persistence.
//!
//! SQLite is the single authoritative store for committed revisions, request
//! receipts and plugin state. Each note's head revision is also *materialized*
//! as a portable Markdown file at `spaces/<space>/Notes/<uuid>.md`. The file is
//! a checked copy, never parsed back as truth:
//!
//! 1. One SQLite transaction (WAL, `synchronous=FULL`) commits the full new
//!    revision, the canonical request fingerprint + result receipt and
//!    `mat_state='pending'`.
//! 2. Materialization compares the file on disk with the SHA-256 recorded at
//!    the last placement. A file that is missing or differs (an external edit
//!    or delete) is never overwritten; the note becomes `blocked`.
//! 3. Otherwise the new bytes go to a temp file in the same directory, are
//!    fsynced, renamed over the target and the directory is fsynced.
//! 4. The note is marked `materialized` with the new hash and only then is
//!    the write acknowledged.
//!
//! On open the data directory is exclusively locked and every `pending` note
//! is replayed through step 2–4 before serving.
//!
//! Limits: the SHA-256 check detects external edits/deletes/symlinks that
//! exist *before* a write. It does not lock against an external writer racing
//! the hash check and rename; concurrent external editing is out of scope for
//! the preview. Detected bytes are preserved and the note is reported as
//! `conflict` with `details.reason = "external_change"`; there is no import or
//! automatic unblock tonight.
//!
//! This is NOT the inherited file-only recovery / tiering guarantee. It is a
//! disposable-data preview profile.

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use calternal_core::{Note, NoteSummary, NoteVersion};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::HostError;
use crate::fault::{self, FaultPoint};

const SCHEMA_VERSION: i64 = 1;

pub struct Store {
    conn: Connection,
    root: PathBuf,
    _lock: File,
}

/// A request receipt as stored at commit time.
pub struct Receipt {
    pub operation: String,
    pub fingerprint: String,
    pub result: serde_json::Value,
    pub note_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatState {
    Pending,
    Materialized,
    Blocked,
}

impl MatState {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Materialized => "materialized",
            Self::Blocked => "blocked",
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "pending" => Self::Pending,
            "materialized" => Self::Materialized,
            _ => Self::Blocked,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PluginRow {
    pub enabled: bool,
    pub generation: u32,
}

pub fn now_rfc3339() -> String {
    let now = time::OffsetDateTime::now_utc();
    let fmt = time::macros::format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
    );
    now.format(&fmt).expect("timestamp format")
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn random_hex(n_bytes: usize) -> String {
    let mut buf = vec![0u8; n_bytes];
    getrandom::fill(&mut buf).expect("os randomness");
    hex::encode(buf)
}

fn db_err(e: rusqlite::Error) -> HostError {
    tracing::error!(error = %e, "sqlite error");
    HostError::internal("storage error")
}

fn io_err(context: &'static str, e: std::io::Error) -> HostError {
    tracing::error!(error = %e, context, "io error");
    HostError::internal("storage io error")
}

impl Store {
    /// Open (creating if needed) the data directory, take the exclusive lock,
    /// migrate and replay pending materializations.
    pub fn open(root: &Path) -> Result<Self, HostError> {
        fs::create_dir_all(root).map_err(|e| io_err("create data dir", e))?;
        let lock_path = root.join(".calternal.lock");
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .map_err(|e| io_err("open lock", e))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => {
                return Err(HostError::unavailable(
                    "data directory is locked by another calternal process",
                ));
            }
            Err(fs::TryLockError::Error(e)) => return Err(io_err("lock data dir", e)),
        }
        let conn = Connection::open(root.join("state.sqlite3")).map_err(db_err)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(db_err)?;
        conn.pragma_update(None, "synchronous", "FULL")
            .map_err(db_err)?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(db_err)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(db_err)?;
        let mut store = Store {
            conn,
            root: root.to_path_buf(),
            _lock: lock,
        };
        store.migrate()?;
        store.cleanup_temp_files();
        store.replay_pending()?;
        Ok(store)
    }

    fn migrate(&mut self) -> Result<(), HostError> {
        let version: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(db_err)?;
        if version == SCHEMA_VERSION {
            return Ok(());
        }
        if version != 0 {
            return Err(HostError::unavailable(
                "unsupported preview data schema version; use a fresh disposable data dir",
            ));
        }
        self.conn
            .execute_batch(
                "BEGIN;
                CREATE TABLE notes (
                    id TEXT PRIMARY KEY,
                    space_id TEXT NOT NULL,
                    title TEXT NOT NULL,
                    head_revision TEXT NOT NULL,
                    head_seq INTEGER NOT NULL,
                    updated_at TEXT NOT NULL,
                    mat_state TEXT NOT NULL,
                    file_sha256 TEXT
                );
                CREATE INDEX notes_space ON notes(space_id, updated_at);
                CREATE TABLE versions (
                    note_id TEXT NOT NULL REFERENCES notes(id),
                    seq INTEGER NOT NULL,
                    revision TEXT NOT NULL UNIQUE,
                    title TEXT NOT NULL,
                    markdown TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    author TEXT NOT NULL,
                    PRIMARY KEY (note_id, seq)
                );
                CREATE TABLE receipts (
                    principal TEXT NOT NULL,
                    request_id TEXT NOT NULL,
                    operation TEXT NOT NULL,
                    fingerprint TEXT NOT NULL,
                    result_json TEXT NOT NULL,
                    note_id TEXT,
                    created_at TEXT NOT NULL,
                    PRIMARY KEY (principal, request_id)
                );
                CREATE TABLE plugin_state (
                    space_id TEXT NOT NULL,
                    plugin_id TEXT NOT NULL,
                    enabled INTEGER NOT NULL,
                    generation INTEGER NOT NULL,
                    PRIMARY KEY (space_id, plugin_id)
                );
                PRAGMA user_version = 1;
                COMMIT;",
            )
            .map_err(db_err)
    }

    fn notes_dir(&self, space_id: &str) -> PathBuf {
        self.root.join("spaces").join(space_id).join("Notes")
    }

    fn note_path(&self, space_id: &str, id: &str) -> PathBuf {
        self.notes_dir(space_id).join(format!("{id}.md"))
    }

    /// Remove our own orphaned temp files left by a crash before rename.
    /// Only exact `.<uuid>.<12 hex>.tmp` names inside real
    /// `spaces/<known space>/Notes` directories are removed.
    fn cleanup_temp_files(&self) {
        for space in crate::auth::SPACES {
            let dir = self.notes_dir(space);
            match fs::symlink_metadata(&dir) {
                Ok(m) if m.file_type().is_dir() => {}
                _ => continue,
            }
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name();
                let Some(name) = name.to_str() else { continue };
                if is_owned_temp_name(name)
                    && entry.file_type().map(|t| t.is_file()).unwrap_or(false)
                {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }

    fn replay_pending(&mut self) -> Result<(), HostError> {
        let ids: Vec<String> = {
            let mut stmt = self
                .conn
                .prepare("SELECT id FROM notes WHERE mat_state = 'pending'")
                .map_err(db_err)?;
            stmt.query_map([], |r| r.get(0))
                .map_err(db_err)?
                .collect::<Result<_, _>>()
                .map_err(db_err)?
        };
        for id in ids {
            match self.materialize(&id) {
                Ok(()) => tracing::info!(note = %id, "replayed pending materialization"),
                Err(e) => {
                    tracing::warn!(note = %id, error = %e.message, "pending materialization not completed")
                }
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------- reads

    pub fn list(&self, space_id: &str, limit: u32) -> Result<Vec<NoteSummary>, HostError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, space_id, title, head_revision, updated_at FROM notes
                 WHERE space_id = ?1 ORDER BY updated_at DESC, id LIMIT ?2",
            )
            .map_err(db_err)?;
        stmt.query_map(params![space_id, limit], |r| {
            Ok(NoteSummary {
                id: r.get(0)?,
                space_id: r.get(1)?,
                title: r.get(2)?,
                revision: r.get(3)?,
                updated_at: r.get(4)?,
            })
        })
        .map_err(db_err)?
        .collect::<Result<_, _>>()
        .map_err(db_err)
    }

    pub fn get(&self, space_id: &str, id: &str) -> Result<Note, HostError> {
        self.conn
            .query_row(
                "SELECT n.id, n.space_id, n.title, v.markdown, n.head_revision, n.updated_at
                 FROM notes n JOIN versions v ON v.note_id = n.id AND v.seq = n.head_seq
                 WHERE n.id = ?1 AND n.space_id = ?2",
                params![id, space_id],
                |r| {
                    Ok(Note {
                        id: r.get(0)?,
                        space_id: r.get(1)?,
                        title: r.get(2)?,
                        markdown: r.get(3)?,
                        revision: r.get(4)?,
                        updated_at: r.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(db_err)?
            .ok_or_else(|| HostError::not_found("note not found"))
    }

    pub fn mat_state(&self, id: &str) -> Result<Option<MatState>, HostError> {
        self.conn
            .query_row("SELECT mat_state FROM notes WHERE id = ?1", [id], |r| {
                r.get::<_, String>(0)
            })
            .optional()
            .map_err(db_err)
            .map(|o| o.map(|s| MatState::parse(&s)))
    }

    /// Newest first. Cursor is an opaque `h<seq>` meaning "versions older than seq".
    pub fn history(
        &self,
        space_id: &str,
        id: &str,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<(Vec<NoteVersion>, Option<String>), HostError> {
        // Ensures existence + space binding.
        self.get(space_id, id)?;
        let before: i64 = match cursor {
            None => i64::MAX,
            Some(c) => c
                .strip_prefix('h')
                .and_then(|s| s.parse::<i64>().ok())
                .filter(|n| *n > 0)
                .ok_or_else(|| HostError::invalid_input("invalid history cursor"))?,
        };
        let mut stmt = self
            .conn
            .prepare(
                "SELECT seq, title, markdown, revision, updated_at FROM versions
                 WHERE note_id = ?1 AND seq < ?2 ORDER BY seq DESC LIMIT ?3",
            )
            .map_err(db_err)?;
        let rows: Vec<(i64, NoteVersion)> = stmt
            .query_map(params![id, before, limit + 1], |r| {
                Ok((
                    r.get(0)?,
                    NoteVersion {
                        title: r.get(1)?,
                        markdown: r.get(2)?,
                        revision: r.get(3)?,
                        updated_at: r.get(4)?,
                    },
                ))
            })
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;
        let more = rows.len() > limit as usize;
        let mut versions: Vec<(i64, NoteVersion)> = rows;
        versions.truncate(limit as usize);
        let next = if more {
            versions.last().map(|(s, _)| format!("h{s}"))
        } else {
            None
        };
        Ok((versions.into_iter().map(|(_, v)| v).collect(), next))
    }

    pub fn receipt(&self, principal: &str, request_id: &str) -> Result<Option<Receipt>, HostError> {
        self.conn
            .query_row(
                "SELECT operation, fingerprint, result_json, note_id FROM receipts
                 WHERE principal = ?1 AND request_id = ?2",
                params![principal, request_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(db_err)?
            .map(|(operation, fingerprint, json, note_id)| {
                let result = serde_json::from_str(&json)
                    .map_err(|_| HostError::internal("corrupt receipt"))?;
                Ok(Receipt {
                    operation,
                    fingerprint,
                    result,
                    note_id,
                })
            })
            .transpose()
    }

    // --------------------------------------------------------------- writes

    /// Commit a new note and materialize it. Caller has verified authorization,
    /// input validity and that no receipt exists for (principal, request_id).
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &mut self,
        principal: &str,
        request_id: &str,
        operation: &str,
        fingerprint: &str,
        space_id: &str,
        title: &str,
        markdown: &str,
    ) -> Result<Note, HostError> {
        let id = Uuid::new_v4().to_string();
        let revision = format!("v1.{}", random_hex(8));
        let now = now_rfc3339();
        let note = Note {
            id: id.clone(),
            space_id: space_id.to_string(),
            title: title.to_string(),
            markdown: markdown.to_string(),
            revision: revision.clone(),
            updated_at: now.clone(),
        };
        let result = serde_json::json!({ "note": &note });
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_err)?;
        tx.execute(
            "INSERT INTO notes (id, space_id, title, head_revision, head_seq, updated_at, mat_state, file_sha256)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, 'pending', NULL)",
            params![id, space_id, title, revision, now],
        )
        .map_err(db_err)?;
        tx.execute(
            "INSERT INTO versions (note_id, seq, revision, title, markdown, updated_at, author)
             VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6)",
            params![id, revision, title, markdown, now, principal],
        )
        .map_err(db_err)?;
        insert_receipt(
            &tx,
            principal,
            request_id,
            operation,
            fingerprint,
            &result,
            Some(&id),
        )?;
        tx.commit().map_err(db_err)?;
        fault::hit(FaultPoint::AfterCommit);
        self.materialize(&id)?;
        Ok(note)
    }

    /// Commit a new revision if `base_revision` is the head and the materialized
    /// file has not been changed externally.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        principal: &str,
        request_id: &str,
        operation: &str,
        fingerprint: &str,
        space_id: &str,
        id: &str,
        base_revision: &str,
        title: Option<&str>,
        markdown: &str,
    ) -> Result<Note, HostError> {
        let current = self.get(space_id, id)?;
        // Finish any interrupted placement first; never stack writes on an
        // unknown file state.
        self.materialize(id)?;
        self.check_file_unchanged(space_id, id)?;
        if current.revision != base_revision {
            return Err(
                HostError::conflict("note has a newer revision").with_details(serde_json::json!({
                    "reason": "stale_revision",
                    "current_revision": current.revision,
                })),
            );
        }
        let (head_seq,): (i64,) = self
            .conn
            .query_row("SELECT head_seq FROM notes WHERE id = ?1", [id], |r| {
                Ok((r.get(0)?,))
            })
            .map_err(db_err)?;
        let seq = head_seq + 1;
        let revision = format!("v{seq}.{}", random_hex(8));
        let now = now_rfc3339();
        let title = title.unwrap_or(&current.title).to_string();
        let note = Note {
            id: id.to_string(),
            space_id: space_id.to_string(),
            title: title.clone(),
            markdown: markdown.to_string(),
            revision: revision.clone(),
            updated_at: now.clone(),
        };
        let result = serde_json::json!({ "note": &note });
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_err)?;
        tx.execute(
            "INSERT INTO versions (note_id, seq, revision, title, markdown, updated_at, author)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, seq, revision, title, markdown, now, principal],
        )
        .map_err(db_err)?;
        let changed = tx
            .execute(
                "UPDATE notes SET title = ?1, head_revision = ?2, head_seq = ?3, updated_at = ?4,
                 mat_state = 'pending' WHERE id = ?5 AND head_seq = ?6",
                params![title, revision, seq, now, id, head_seq],
            )
            .map_err(db_err)?;
        if changed != 1 {
            return Err(HostError::conflict("note changed concurrently"));
        }
        insert_receipt(
            &tx,
            principal,
            request_id,
            operation,
            fingerprint,
            &result,
            Some(id),
        )?;
        tx.commit().map_err(db_err)?;
        fault::hit(FaultPoint::AfterCommit);
        self.materialize(id)?;
        Ok(note)
    }

    /// Detect an external change to an already-materialized file before
    /// accepting a new revision. Marks the note blocked if changed.
    fn check_file_unchanged(&mut self, space_id: &str, id: &str) -> Result<(), HostError> {
        let (state, sha): (String, Option<String>) = self
            .conn
            .query_row(
                "SELECT mat_state, file_sha256 FROM notes WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(db_err)?;
        if MatState::parse(&state) == MatState::Blocked {
            return Err(blocked_error());
        }
        let path = self.note_path(space_id, id);
        let on_disk = match file_hash(&path)? {
            FileState::Missing => None,
            FileState::Regular(h) => Some(h),
            FileState::NotRegular => {
                self.set_state(id, MatState::Blocked)?;
                return Err(blocked_error());
            }
        };
        if on_disk != sha {
            self.set_state(id, MatState::Blocked)?;
            tracing::warn!(note = %id, "external change to materialized note; writes blocked");
            return Err(blocked_error());
        }
        Ok(())
    }

    fn set_state(&self, id: &str, state: MatState) -> Result<(), HostError> {
        self.conn
            .execute(
                "UPDATE notes SET mat_state = ?1 WHERE id = ?2",
                params![state.as_str(), id],
            )
            .map_err(db_err)?;
        Ok(())
    }

    /// Place the head revision on disk if pending. Idempotent.
    pub fn materialize(&mut self, id: &str) -> Result<(), HostError> {
        let (space_id, state, file_sha, markdown): (String, String, Option<String>, String) = self
            .conn
            .query_row(
                "SELECT n.space_id, n.mat_state, n.file_sha256, v.markdown
                 FROM notes n JOIN versions v ON v.note_id = n.id AND v.seq = n.head_seq
                 WHERE n.id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(db_err)?
            .ok_or_else(|| HostError::not_found("note not found"))?;
        match MatState::parse(&state) {
            MatState::Materialized => return Ok(()),
            MatState::Blocked => return Err(blocked_error()),
            MatState::Pending => {}
        }
        let new_sha = sha256_hex(markdown.as_bytes());
        let dir = self.notes_dir(&space_id);
        if !dir.is_dir() {
            fs::create_dir_all(&dir).map_err(|e| io_err("create notes dir", e))?;
            // Make the new directory entries themselves durable.
            for d in [
                self.root.join("spaces").join(&space_id),
                self.root.join("spaces"),
                self.root.clone(),
            ] {
                File::open(&d)
                    .and_then(|f| f.sync_all())
                    .map_err(|e| io_err("fsync parent dir", e))?;
            }
        }
        // Every path component we create must be a real directory, not a symlink.
        for d in [
            self.root.join("spaces"),
            self.root.join("spaces").join(&space_id),
            dir.clone(),
        ] {
            let m = fs::symlink_metadata(&d).map_err(|e| io_err("stat notes dir", e))?;
            if !m.file_type().is_dir() {
                tracing::error!(dir = %d.display(), "notes directory is not a real directory");
                return Err(HostError::unavailable(
                    "note storage directory is not a regular directory",
                ));
            }
        }
        let path = self.note_path(&space_id, id);
        let on_disk = match file_hash(&path)? {
            FileState::Missing => None,
            FileState::Regular(h) => Some(h),
            FileState::NotRegular => {
                self.set_state(id, MatState::Blocked)?;
                tracing::warn!(note = %id, "note path is not a regular file; blocked");
                return Err(blocked_error());
            }
        };
        if on_disk.as_deref() == Some(new_sha.as_str()) {
            // Bytes were placed before a crash, but the rename / directory
            // entry may not be durable yet: sync file and directory before
            // marking, exactly as a fresh placement would.
            File::open(&path)
                .and_then(|f| f.sync_all())
                .map_err(|e| io_err("fsync placed file", e))?;
            File::open(&dir)
                .and_then(|d| d.sync_all())
                .map_err(|e| io_err("fsync dir", e))?;
        } else if on_disk == file_sha {
            fault::hit(FaultPoint::BeforePlacement);
            atomic_write(&dir, &path, id, markdown.as_bytes())?;
            fault::hit(FaultPoint::AfterPlacement);
        } else {
            // Missing-after-placed, or bytes we did not write: preserve them.
            self.set_state(id, MatState::Blocked)?;
            tracing::warn!(note = %id, "external change before placement; preserved file, blocked");
            return Err(blocked_error());
        }
        self.conn
            .execute(
                "UPDATE notes SET mat_state = 'materialized', file_sha256 = ?1 WHERE id = ?2",
                params![new_sha, id],
            )
            .map_err(db_err)?;
        Ok(())
    }

    // -------------------------------------------------------------- plugins

    pub fn plugin_row(
        &self,
        space_id: &str,
        plugin_id: &str,
    ) -> Result<Option<PluginRow>, HostError> {
        self.conn
            .query_row(
                "SELECT enabled, generation FROM plugin_state WHERE space_id = ?1 AND plugin_id = ?2",
                params![space_id, plugin_id],
                |r| Ok(PluginRow { enabled: r.get::<_, i64>(0)? != 0, generation: r.get(1)? }),
            )
            .optional()
            .map_err(db_err)
    }

    /// Set enabled (bump generation on change) and record the request
    /// receipt in the same transaction, so a retried old enable can never
    /// undo a newer disable.
    #[allow(clippy::too_many_arguments)]
    pub fn set_plugin_enabled(
        &mut self,
        principal: &str,
        request_id: &str,
        operation: &str,
        fingerprint: &str,
        space_id: &str,
        plugin_id: &str,
        default: PluginRow,
        enabled: bool,
        result: impl FnOnce(&PluginRow) -> serde_json::Value,
    ) -> Result<(PluginRow, serde_json::Value), HostError> {
        let current = self.plugin_row(space_id, plugin_id)?.unwrap_or(default);
        let next = if current.enabled == enabled {
            current
        } else {
            PluginRow {
                enabled,
                generation: current.generation + 1,
            }
        };
        let value = result(&next);
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_err)?;
        tx.execute(
            "INSERT INTO plugin_state (space_id, plugin_id, enabled, generation) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(space_id, plugin_id) DO UPDATE SET enabled = excluded.enabled, generation = excluded.generation",
            params![space_id, plugin_id, next.enabled as i64, next.generation],
        )
        .map_err(db_err)?;
        insert_receipt(
            &tx,
            principal,
            request_id,
            operation,
            fingerprint,
            &value,
            None,
        )?;
        tx.commit().map_err(db_err)?;
        Ok((next, value))
    }
}

enum FileState {
    Missing,
    Regular(String),
    /// Symlink, directory, device...: never followed or replaced.
    NotRegular,
}

fn file_hash(path: &Path) -> Result<FileState, HostError> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(FileState::Missing),
        Err(e) => return Err(io_err("stat note file", e)),
        Ok(m) if !m.file_type().is_file() => return Ok(FileState::NotRegular),
        Ok(_) => {}
    }
    let bytes = fs::read(path).map_err(|e| io_err("read note file", e))?;
    Ok(FileState::Regular(sha256_hex(&bytes)))
}

/// `.{uuid}.{12 lowercase hex}.tmp` exactly, as produced by `atomic_write`.
fn is_owned_temp_name(name: &str) -> bool {
    let Some(inner) = name.strip_prefix('.').and_then(|n| n.strip_suffix(".tmp")) else {
        return false;
    };
    let Some((id, suffix)) = inner.rsplit_once('.') else {
        return false;
    };
    suffix.len() == 12
        && suffix
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && Uuid::parse_str(id)
            .map(|u| u.to_string() == id)
            .unwrap_or(false)
}

pub fn blocked_error() -> HostError {
    HostError::conflict("note file was changed outside Calternal; writes are blocked")
        .with_details(serde_json::json!({ "reason": "external_change" }))
}

fn insert_receipt(
    tx: &rusqlite::Transaction<'_>,
    principal: &str,
    request_id: &str,
    operation: &str,
    fingerprint: &str,
    result: &serde_json::Value,
    note_id: Option<&str>,
) -> Result<(), HostError> {
    tx.execute(
        "INSERT INTO receipts (principal, request_id, operation, fingerprint, result_json, note_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![principal, request_id, operation, fingerprint, result.to_string(), note_id, now_rfc3339()],
    )
    .map_err(db_err)?;
    Ok(())
}

fn atomic_write(dir: &Path, target: &Path, id: &str, bytes: &[u8]) -> Result<(), HostError> {
    let tmp = dir.join(format!(".{id}.{}.tmp", random_hex(6)));
    {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| io_err("create temp", e))?;
        f.write_all(bytes).map_err(|e| io_err("write temp", e))?;
        f.sync_all().map_err(|e| io_err("fsync temp", e))?;
    }
    if let Err(e) = fs::rename(&tmp, target) {
        let _ = fs::remove_file(&tmp);
        return Err(io_err("rename", e));
    }
    File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(|e| io_err("fsync dir", e))?;
    Ok(())
}
