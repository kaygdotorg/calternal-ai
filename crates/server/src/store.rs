//! Notes adapter over generic, physically per-space content storage.
//!
//! `state.sqlite3` retains plugin enablement and unfinished control publication intents only;
//! it has no item, content or history tables. `system/content-index.db` holds
//! identifier-only item/request routes. The content crate owns each UUID space's
//! `content.db`, inherited `index.db` journal/receipts and SSD-tree files.
//! `open` selects separate `userdata-ssd`/`userdata-hdd` subdirectories for disposable preview data;
//! production callers must supply validated and locked roots through `open_roots`.
//! No old shared-content schema is migrated or adopted.
use crate::{
    error::HostError,
    fault::{self, FaultPoint},
};
use calternal_content::{AccountId, Content, Item, Request, SpaceId};
use calternal_core::{Note, NoteSummary, NoteVersion};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

/// Locked host adapter; Notes content is owned by the generic per-space service.
pub struct Store {
    conn: Connection,
    root: PathBuf,
    content: Content,
    _lock: File,
}
/// Replayable account-qualified result; note responses live in their space journal.
pub struct Receipt {
    /// Operation that committed the response.
    pub operation: String,
    /// Canonical operation/input digest.
    pub fingerprint: String,
    /// Previously committed response, private to its acting account.
    pub result: serde_json::Value,
    /// Item whose file placement must finish before replay.
    pub note_id: Option<String>,
}
/// Placement bookkeeping shared with the generic content layer.
pub use calternal_content::Materialization as MatState;
/// Persisted plugin enablement and asynchronous-result invalidation generation.
#[derive(Debug, Clone)]
pub struct PluginRow {
    /// Whether the plugin is enabled in its owning space.
    pub enabled: bool,
    /// Monotonic generation checked before asynchronous delivery.
    pub generation: u32,
}
/// Current UTC commit timestamp. Cost: clock read and string allocation; no I/O or locks.
pub fn now_rfc3339() -> String {
    calternal_content::now_rfc3339()
}
/// SHA-256 request/byte digest. Cost: O(bytes) CPU and one hex allocation; no locks or I/O.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn db_err(e: rusqlite::Error) -> HostError {
    tracing::error!(error=%e,"sqlite error");
    HostError::internal("storage error")
}
fn io_err(context: &'static str, e: std::io::Error) -> HostError {
    tracing::error!(error=%e,context,"io error");
    HostError::internal("storage io error")
}
fn content_error(e: calternal_content::Error) -> HostError {
    HostError {
        code: e.code,
        message: e.message,
        details: e.details,
    }
}
fn content_fault(point: calternal_content::FaultPoint) {
    fault::hit(match point {
        calternal_content::FaultPoint::AfterCommit => FaultPoint::AfterCommit,
        calternal_content::FaultPoint::BeforePlacement => FaultPoint::BeforePlacement,
        calternal_content::FaultPoint::AfterPlacement => FaultPoint::AfterPlacement,
    });
}
fn item_id(id: &str) -> Result<Uuid, HostError> {
    Uuid::parse_str(id).map_err(|_| HostError::not_found("item not found"))
}
fn note(item: &Item, space: &str) -> Note {
    Note {
        id: item.id.to_string(),
        space_id: space.into(),
        title: item.metadata["title"].as_str().unwrap_or("").into(),
        markdown: String::from_utf8_lossy(&item.bytes).into_owned(),
        revision: item.revision.clone(),
        updated_at: item.updated_at.clone(),
    }
}
impl Store {
    /// Opens disposable preview roots under one data directory and takes its lock.
    /// Cost: directory/lock/plugin SQLite I/O, generic catalog startup and replay;
    /// recovered writes fsync their database, journal, current file and directory.
    /// This helper is not production three-root validation or witness provisioning.
    pub fn open(root: &Path) -> Result<Self, HostError> {
        Self::open_roots(
            calternal_roots::Roots {
                state: root.into(),
                ssd_space: root.join("userdata-ssd"),
                hdd_space: root.join("userdata-hdd"),
            },
            root.join("secrets"),
        )
    }
    /// Opens caller-validated, externally locked three-root storage plus a separate
    /// journal witness root. The host must retain the roots guards for this lifetime.
    /// Cost: one installation lock, plugin/catalog database opens and schema fsyncs,
    /// then startup-only pending publication/materialization recovery per space.
    pub fn open_roots(roots: calternal_roots::Roots, secrets: PathBuf) -> Result<Self, HostError> {
        let root = roots.state.clone();
        calternal_auth::private_fs::directory(&root).map_err(|e| io_err("create data dir", e))?;
        let lock = calternal_auth::private_fs::file(&root.join(".calternal.lock"))
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
        let path = root.join("state.sqlite3");
        calternal_auth::private_fs::file(&path)
            .map_err(|e| io_err("prepare plugin database", e))?;
        let conn = Connection::open(path).map_err(db_err)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(db_err)?;
        conn.pragma_update(None, "synchronous", "FULL")
            .map_err(db_err)?;
        conn.busy_timeout(Duration::from_secs(5)).map_err(db_err)?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(db_err)?;
        if version != 0 && version != 2 {
            return Err(HostError::unavailable(
                "unsupported preview schema; use fresh disposable data",
            ));
        }
        conn.execute_batch("BEGIN;
            CREATE TABLE IF NOT EXISTS plugin_state(space_id TEXT NOT NULL,plugin_id TEXT NOT NULL,enabled INTEGER NOT NULL,generation INTEGER NOT NULL,PRIMARY KEY(space_id,plugin_id));
            CREATE TABLE IF NOT EXISTS plugin_outbox(principal TEXT NOT NULL,request_id TEXT NOT NULL,operation TEXT NOT NULL,fingerprint TEXT NOT NULL,result_json TEXT NOT NULL,space_id TEXT NOT NULL,PRIMARY KEY(principal,request_id));
            CREATE INDEX IF NOT EXISTS plugin_outbox_space ON plugin_outbox(space_id);
            CREATE TABLE IF NOT EXISTS fixture_bindings(domain TEXT NOT NULL,name TEXT NOT NULL,id TEXT NOT NULL UNIQUE,PRIMARY KEY(domain,name));
            PRAGMA user_version=2;COMMIT;").map_err(db_err)?;
        let content = Content::open(roots, secrets, Duration::from_secs(600), content_fault)
            .map_err(content_error)?;
        let store = Self {
            conn,
            root,
            content,
            _lock: lock,
        };
        let mut statement = store
            .conn
            .prepare("SELECT principal,request_id FROM plugin_outbox ORDER BY rowid")
            .map_err(db_err)?;
        let pending = statement
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(db_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_err)?;
        drop(statement);
        for (principal, request) in pending {
            store.publish_plugin_pending(&principal, &request)?;
        }
        Ok(store)
    }
    // Explicit preview/synthetic-fixture bindings preserve existing test directories'
    // authority vocabulary. All real databases and trees use independently allocated
    // UUIDv7 identifiers. These mappings never grant authority.
    fn binding(&self, domain: &str, name: &str) -> Result<String, HostError> {
        if let Ok(id) = Uuid::parse_str(name) {
            return Ok(id.to_string());
        }
        if domain == "space" || domain == "account" {
            let path = self.root.join("system/auth.db");
            if path.exists() {
                let auth =
                    Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                        .map_err(db_err)?;
                if let Ok(Some(id)) = auth
                    .query_row(
                        "SELECT id FROM development_directory WHERE name=?1",
                        [name],
                        |r| r.get::<_, String>(0),
                    )
                    .optional()
                {
                    return Ok(id);
                }
            }
        }
        if let Some(id) = self
            .conn
            .query_row(
                "SELECT id FROM fixture_bindings WHERE domain=?1 AND name=?2",
                params![domain, name],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(db_err)?
        {
            return Ok(id);
        }
        let id = Uuid::now_v7().to_string();
        self.conn
            .execute(
                "INSERT INTO fixture_bindings VALUES(?1,?2,?3)",
                params![domain, name, id],
            )
            .map_err(db_err)?;
        Ok(id)
    }
    fn space(&self, name: &str) -> Result<SpaceId, HostError> {
        self.binding("space", name)?
            .parse()
            .map_err(|_| HostError::invalid_input("invalid space identifier"))
    }
    fn account(&self, name: &str) -> Result<AccountId, HostError> {
        self.binding("account", name)?
            .parse()
            .map_err(|_| HostError::invalid_input("invalid account identifier"))
    }
    /// Returns the generic item service for trusted host integrations such as
    /// plugin kind/standard-collection provisioning. Every caller must enforce
    /// current authority before opening a selected account or space.
    /// Cost: borrowed reference only; no allocation, locks, SQLite or filesystem I/O.
    pub fn content(&self) -> &Content {
        &self.content
    }
    /// Resolves identifier-only catalog routing for any stored kind. No content is read
    /// and no authority is granted. Cost: catalog mutex/indexed read plus at most one
    /// indexed fixture binding lookup; no membership scan or materialization/fsync.
    pub fn locate(&self, kind: &str, id: &str) -> Result<Option<String>, HostError> {
        let Ok(id) = Uuid::parse_str(id) else {
            return Ok(None);
        };
        let Some(space) = self.content.locate(kind, id).map_err(content_error)? else {
            return Ok(None);
        };
        let alias = self
            .conn
            .query_row(
                "SELECT name FROM fixture_bindings WHERE domain='space' AND id=?1",
                [space.to_string()],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(db_err)?;
        Ok(Some(alias.unwrap_or_else(|| space.to_string())))
    }
    /// Lists bounded Notes metadata in one authorized space. Cost: generic cache/space
    /// locks and O(log N + page) indexed read, allocating at most 1,000 rows; no file I/O.
    pub fn list(&self, space_id: &str, limit: u32) -> Result<Vec<NoteSummary>, HostError> {
        self.content
            .list(self.space(space_id)?, "notes", limit)
            .map_err(content_error)
            .map(|rows| {
                rows.into_iter()
                    .map(|i| NoteSummary {
                        id: i.id.to_string(),
                        space_id: space_id.into(),
                        title: i.metadata["title"].as_str().unwrap_or("").into(),
                        revision: i.revision,
                        updated_at: i.updated_at,
                    })
                    .collect()
            })
    }
    /// Gets one Notes item from its selected physical database. A foreign ID is absent.
    /// Cost: generic cache/space locks, indexed head join and body allocation; no fsync.
    pub fn get(&self, space_id: &str, id: &str) -> Result<Note, HostError> {
        let item = self
            .content
            .get(self.space(space_id)?, item_id(id)?)
            .map_err(content_error)?;
        if item.kind != "notes" {
            return Err(HostError::not_found("item not found"));
        }
        Ok(note(&item, space_id))
    }
    /// Reads materialization state by trusted internal ID. Cost: one global indexed
    /// route and one space metadata lookup; no membership scan, content file read or fsync.
    pub fn mat_state(&self, id: &str) -> Result<Option<MatState>, HostError> {
        let Some(space) = self.locate("notes", id)? else {
            return Ok(None);
        };
        self.content
            .materialization(self.space(&space)?, item_id(id)?)
            .map_err(content_error)
    }
    /// Reads newest immutable versions using an item-bound opaque cursor. Cost:
    /// cache/space locks, bounded indexed reads and at most 1,001 body allocations;
    /// no other-space data, filesystem scans or fsyncs.
    pub fn history(
        &self,
        space_id: &str,
        id: &str,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<(Vec<NoteVersion>, Option<String>), HostError> {
        self.get(space_id, id)?;
        let (versions, next) = self
            .content
            .history(self.space(space_id)?, item_id(id)?, limit, cursor)
            .map_err(content_error)?;
        Ok((
            versions
                .into_iter()
                .map(|i| NoteVersion {
                    title: i.metadata["title"].as_str().unwrap_or("").into(),
                    markdown: String::from_utf8_lossy(&i.bytes).into_owned(),
                    revision: i.revision,
                    updated_at: i.updated_at,
                })
                .collect(),
            next,
        ))
    }
    /// Reads an account-qualified receipt after the caller checks current grants.
    /// All operation payloads are in per-space store receipts; this adapter uses identifier-only
    /// routing for the existing account-wide request namespace. Pending control
    /// intents finish publication first. Cost: indexed control/catalog reads and one
    /// bounded journal bridge/read, with journal fsyncs only during pending replay.
    pub fn receipt(&self, principal: &str, request_id: &str) -> Result<Option<Receipt>, HostError> {
        self.publish_plugin_pending(principal, request_id)?;
        let account = self.account(principal)?;
        let Some(space) = self
            .content
            .receipt_space(account, request_id)
            .map_err(content_error)?
        else {
            return Ok(None);
        };
        self.content
            .receipt(space, account, request_id)
            .map_err(content_error)
            .map(|r| {
                r.map(|r| Receipt {
                    operation: r.operation,
                    fingerprint: r.fingerprint,
                    result: r.result,
                    note_id: r.item.map(|id| id.to_string()),
                })
            })
    }
    fn publish_plugins_for_space(&self, space_id: &str) -> Result<(), HostError> {
        loop {
            let first=self.conn.query_row("SELECT principal,request_id FROM plugin_outbox WHERE space_id=?1 ORDER BY rowid LIMIT 1",[space_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional().map_err(db_err)?;
            let Some((principal, request)) = first else {
                return Ok(());
            };
            self.publish_plugin_pending(&principal, &request)?;
        }
    }
    fn publish_plugin_pending(&self, principal: &str, request_id: &str) -> Result<(), HostError> {
        let pending=self.conn.query_row("SELECT operation,fingerprint,result_json,space_id FROM plugin_outbox WHERE principal=?1 AND request_id=?2",params![principal,request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?))).optional().map_err(db_err)?;
        if let Some((operation, fingerprint, json, space)) = pending {
            let result = serde_json::from_str(&json)
                .map_err(|_| HostError::internal("corrupt plugin publication"))?;
            self.content
                .record_receipt(
                    self.space(&space)?,
                    Request {
                        account: self.account(principal)?,
                        id: request_id,
                        operation: &operation,
                        fingerprint: &fingerprint,
                    },
                    result,
                )
                .map_err(content_error)?;
            self.conn
                .execute(
                    "DELETE FROM plugin_outbox WHERE principal=?1 AND request_id=?2",
                    params![principal, request_id],
                )
                .map_err(db_err)?;
        }
        Ok(())
    }
    /// Creates a globally UUIDv7-identified Notes item with a human filename and
    /// frontmatter identity. Cost: generic write allocations/locks, FULL content and
    /// journal commits, fsio file+directory fsync, placement/outbox SQL commits.
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
        self.content
            .register_kind(
                self.space(space_id)?,
                "notes",
                calternal_content::Placement::Visible {
                    folder: "Notes".into(),
                },
                calternal_content::FileFormat::Markdown,
            )
            .map_err(content_error)?;
        let request = Request {
            account: self.account(principal)?,
            id: request_id,
            operation,
            fingerprint,
        };
        let item = self
            .content
            .write(
                self.space(space_id)?,
                None,
                "notes",
                None,
                serde_json::json!({"title":title}),
                markdown.as_bytes(),
                request,
                |i| serde_json::json!({"note":note(i,space_id)}),
            )
            .map_err(content_error)?;
        Ok(note(&item, space_id))
    }
    /// Updates only the current base revision, preserving external edits/deletes.
    /// Title changes journal a fixed exclusive rename with the same mutation.
    /// Cost: indexed head read plus generic write costs, one old-file hash, optional
    /// exclusive rename and parent fsync; no directory scan on warm Linux name caches.
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
        let request = Request {
            account: self.account(principal)?,
            id: request_id,
            operation,
            fingerprint,
        };
        let item = self
            .content
            .write(
                self.space(space_id)?,
                Some(item_id(id)?),
                "notes",
                Some(base_revision),
                serde_json::json!({"title":title.unwrap_or(&current.title)}),
                markdown.as_bytes(),
                request,
                |i| serde_json::json!({"note":note(i,space_id)}),
            )
            .map_err(content_error)?;
        Ok(note(&item, space_id))
    }
    /// Finishes checked placement before retry replay. Cost: catalog lookup and
    /// generic materialization locks/reads, with file+directory fsync only if pending.
    pub fn materialize(&mut self, id: &str) -> Result<(), HostError> {
        let space = self
            .locate("notes", id)?
            .ok_or_else(|| HostError::not_found("item not found"))?;
        self.content
            .materialize(self.space(&space)?, item_id(id)?)
            .map_err(content_error)
    }
    /// Read one space's plugin state without inheriting another space's enablement.
    /// Cost: one indexed read on the plugin connection; no content I/O or fsync.
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
    /// publication intent in the same transaction, then durably publish its receipt
    /// through the owning space store, so a retried old enable can never
    /// undo a newer disable.
    /// Cost: plugin connection reads and one FULL SQLite transaction, with result
    /// serialization and WAL fsync, followed by per-space receipt publication and
    /// FULL journal durability, then outbox retirement. No user file access.
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
        self.publish_plugins_for_space(space_id)?;
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
        tx.execute(
            "INSERT INTO plugin_outbox VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                principal,
                request_id,
                operation,
                fingerprint,
                value.to_string(),
                space_id
            ],
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)?;
        self.publish_plugin_pending(principal, request_id)?;
        Ok((next, value))
    }
}
/// External bytes prevented checked placement. Cost: short error/JSON allocations;
/// no I/O, locks or fsync. The generic content materializer uses the same reason.
pub fn blocked_error() -> HostError {
    HostError::conflict("note file was changed outside Calternal; writes are blocked")
        .with_details(serde_json::json!({"reason":"external_change"}))
}
