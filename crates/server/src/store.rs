//! Notes adapter over generic, physically per-space content storage.
//!
//! `state.sqlite3` retains plugin enablement and unfinished control publication intents only;
//! it has no item, content or history tables. `system/content-index.db` holds
//! identifier-only item/request routes. The content crate owns each UUID space's
//! `content.db`, inherited `index.db` journal/receipts and SSD-tree files.
//! `open` selects separate `userdata-ssd`/`userdata-hdd` subdirectories for disposable preview data;
//! production callers must supply validated and locked roots through `open_roots`.
//! No old shared-content schema is migrated or adopted.
use crate::fault::{self, FaultPoint};
use calternal_types::Error;
use calternal_content::{Content, Item, ReadPool, Request};
use calternal_types::{AccountId, SpaceId};
use calternal_core::{Note, NoteSummary, NoteVersion};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};
use uuid::Uuid;

/// Host adapter over the generic per-space content service, shared by every request
/// thread without an outer lock: its methods take `&self`.
///
/// Locks: content work takes only the target space's lock (see
/// [`calternal_content`]). `state.sqlite3` (plugin activation and the control
/// outbox) has one writer connection, locked only for an enable/disable transaction
/// or an outbox delete; every read goes through a [`ReadPool`] and never waits for it.
/// Enable/disable are rare manager actions, so this installation-wide writer (held
/// across that transaction's fsync) is not on any content read or write path.
pub struct Store {
    writer: Mutex<Connection>,
    readers: ReadPool,
    content: Content,
    secrets: std::sync::Arc<calternal_secrets::LocalStore>,
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
fn db_err(e: rusqlite::Error) -> Error {
    tracing::error!(error=%e,"sqlite error");
    Error::internal("storage error")
}
fn io_err(context: &'static str, e: std::io::Error) -> Error {
    tracing::error!(error=%e,context,"io error");
    Error::internal("storage io error")
}
fn content_fault(point: calternal_content::FaultPoint) {
    fault::hit(match point {
        calternal_content::FaultPoint::AfterCommit => FaultPoint::AfterCommit,
        calternal_content::FaultPoint::BeforePlacement => FaultPoint::BeforePlacement,
        calternal_content::FaultPoint::AfterPlacement => FaultPoint::AfterPlacement,
    });
}
/// A typed ID read back from the plugin outbox, written by this adapter.
fn stored_id<T: std::str::FromStr>(text: &str) -> Result<T, Error> {
    text.parse()
        .map_err(|_| Error::internal("corrupt plugin publication"))
}
/// One space's stored activation row for `plugin_id`. Cost: one indexed read.
fn plugin_row(
    c: &Connection,
    space_id: SpaceId,
    plugin_id: &str,
) -> rusqlite::Result<Option<PluginRow>> {
    c.query_row(
        "SELECT enabled, generation FROM plugin_state WHERE space_id = ?1 AND plugin_id = ?2",
        params![space_id.to_string(), plugin_id],
        |r| Ok(PluginRow { enabled: r.get::<_, i64>(0)? != 0, generation: r.get(1)? }),
    )
    .optional()
}
fn item_id(id: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(id).map_err(|_| Error::not_found("item not found"))
}
fn note(item: &Item) -> Note {
    Note {
        id: item.id.to_string(),
        space_id: item.space.to_string(),
        title: item.metadata["title"].as_str().unwrap_or("").into(),
        markdown: String::from_utf8_lossy(&item.bytes).into_owned(),
        revision: item.revision.clone(),
        updated_at: item.updated_at.clone(),
    }
}
impl Store {
    /// The `state.sqlite3` writer. Held for one short transaction; never across
    /// content or journal work.
    fn writer(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.writer.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// Shared secret store under this appdata root. Cost: one reference-count
    /// increment.
    pub fn secrets(&self) -> std::sync::Arc<calternal_secrets::LocalStore> {
        self.secrets.clone()
    }
    /// Opens disposable preview roots under one data directory and takes its lock.
    /// Cost: directory/lock/plugin SQLite I/O, generic catalog startup and replay;
    /// recovered writes fsync their database, journal, current file and directory.
    /// This helper is not production three-root validation or witness provisioning.
    pub fn open(root: &Path) -> Result<Self, Error> {
        Self::open_roots(
            calternal_roots::Roots {
                appdata: root.into(),
                userdata_ssd: root.join("userdata-ssd"),
                userdata_hdd: root.join("userdata-hdd"),
            },
            root.join("secrets"),
        )
    }
    /// Opens caller-validated, externally locked three-root storage plus a separate
    /// journal witness root. The host must retain the roots guards for this lifetime.
    /// Cost: one installation lock, plugin/catalog database opens and schema fsyncs,
    /// then startup-only pending publication/materialization recovery per space.
    pub fn open_roots(roots: calternal_roots::Roots, secrets: PathBuf) -> Result<Self, Error> {
        let root = roots.appdata.clone();
        calternal_fsio::private::directory(&root).map_err(|e| io_err("create data dir", e))?;
        let lock = calternal_fsio::private::file(&root.join(".calternal.lock"))
            .map_err(|e| io_err("open lock", e))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => {
                return Err(Error::unavailable(
                    "data directory is locked by another calternal process",
                ));
            }
            Err(fs::TryLockError::Error(e)) => return Err(io_err("lock data dir", e)),
        }
        let path = root.join("state.sqlite3");
        calternal_fsio::private::file(&path)
            .map_err(|e| io_err("prepare plugin database", e))?;
        let conn = Connection::open(&path).map_err(db_err)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(db_err)?;
        conn.pragma_update(None, "synchronous", "FULL")
            .map_err(db_err)?;
        conn.busy_timeout(Duration::from_secs(5)).map_err(db_err)?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(db_err)?;
        if version != 0 && version != 3 {
            return Err(Error::unavailable(
                "unsupported preview schema; use fresh disposable data",
            ));
        }
        conn.execute_batch("BEGIN;
            CREATE TABLE IF NOT EXISTS plugin_state(space_id TEXT NOT NULL,plugin_id TEXT NOT NULL,enabled INTEGER NOT NULL,generation INTEGER NOT NULL,PRIMARY KEY(space_id,plugin_id));
            CREATE TABLE IF NOT EXISTS plugin_outbox(principal TEXT NOT NULL,request_id TEXT NOT NULL,operation TEXT NOT NULL,fingerprint TEXT NOT NULL,result_json TEXT NOT NULL,space_id TEXT NOT NULL,PRIMARY KEY(principal,space_id,request_id));
            PRAGMA user_version=3;COMMIT;").map_err(db_err)?;
        let content = Content::open(roots, secrets, Duration::from_secs(600), content_fault)
            ?;
        // Fails closed: a missing or wrong secret key stops startup instead of
        // minting a key that cannot read the stored secrets.
        let secrets = calternal_secrets::LocalStore::open(&root)
            .map_err(|e| Error::unavailable(e.to_string()))?;
        let store = Self {
            writer: Mutex::new(conn),
            readers: ReadPool::new(path),
            content,
            secrets: std::sync::Arc::new(secrets),
            _lock: lock,
        };
        let pending = store
            .readers
            .read(|c| {
                c.prepare("SELECT principal,space_id,request_id FROM plugin_outbox ORDER BY rowid")?
                    .query_map([], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(db_err)?;
        for (principal, space, request) in pending {
            store.publish_plugin_pending(stored_id(&principal)?, stored_id(&space)?, &request)?;
        }
        Ok(store)
    }
    /// Returns the generic item service for trusted host integrations such as
    /// plugin kind/standard-collection provisioning. Every caller must enforce
    /// current authority before opening a selected account or space.
    /// Cost: borrowed reference only; no allocation, locks, SQLite or filesystem I/O.
    pub fn content(&self) -> &Content {
        &self.content
    }
    /// Resolves identifier-only catalog routing for any stored kind. No content is read
    /// and no authority is granted. A malformed ID is simply absent.
    /// Cost: catalog mutex and one indexed read; no membership scan or fsync.
    pub fn locate(&self, kind: &str, id: &str) -> Result<Option<SpaceId>, Error> {
        let Ok(id) = Uuid::parse_str(id) else {
            return Ok(None);
        };
        self.content.locate(kind, id)
    }
    /// Lists bounded Notes metadata in one authorized space. Cost: generic cache/space
    /// locks and O(log N + page) indexed read, allocating at most 1,000 rows; no file I/O.
    pub fn list(&self, space_id: SpaceId, limit: u32) -> Result<Vec<NoteSummary>, Error> {
        self.content
            .list(space_id, "notes", limit)
            
            .map(|rows| {
                rows.into_iter()
                    .map(|i| NoteSummary {
                        id: i.id.to_string(),
                        space_id: space_id.to_string(),
                        title: i.metadata["title"].as_str().unwrap_or("").into(),
                        revision: i.revision,
                        updated_at: i.updated_at,
                    })
                    .collect()
            })
    }
    /// Gets one Notes item from its selected physical database. A foreign ID is absent.
    /// Cost: generic cache/space locks, indexed head join and body allocation; no fsync.
    pub fn get(&self, space_id: SpaceId, id: &str) -> Result<Note, Error> {
        let item = self
            .content
            .get(space_id, item_id(id)?)
            ?;
        if item.kind != "notes" {
            return Err(Error::not_found("item not found"));
        }
        Ok(note(&item))
    }
    /// Reads materialization state by trusted internal ID. Cost: one global indexed
    /// route and one space metadata lookup; no membership scan, content file read or fsync.
    pub fn mat_state(&self, id: &str) -> Result<Option<MatState>, Error> {
        let Some(space) = self.locate("notes", id)? else {
            return Ok(None);
        };
        self.content
            .materialization(space, item_id(id)?)
            
    }
    /// Reads newest immutable versions using an item-bound opaque cursor. Cost:
    /// cache/space locks, bounded indexed reads and at most 1,001 body allocations;
    /// no other-space data, filesystem scans or fsyncs.
    pub fn history(
        &self,
        space_id: SpaceId,
        id: &str,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<(Vec<NoteVersion>, Option<String>), Error> {
        self.get(space_id, id)?;
        let (versions, next) = self
            .content
            .history(space_id, item_id(id)?, limit, cursor)
            ?;
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
    /// Reads `principal`'s receipt for `request_id` in `space`, after the caller has
    /// checked current grants there. Request IDs are scoped to (account, space), so
    /// another account or space never sees it. A pending control intent for the same
    /// key finishes publication first. Cost: one indexed outbox read and one journal
    /// receipt read; journal fsyncs only during pending replay.
    pub fn receipt(
        &self,
        principal: AccountId,
        space: SpaceId,
        request_id: &str,
    ) -> Result<Option<Receipt>, Error> {
        self.publish_plugin_pending(principal, space, request_id)?;
        self.content
            .receipt(space, principal, request_id)
            .map(|r| {
                r.map(|r| Receipt {
                    operation: r.operation,
                    fingerprint: r.fingerprint,
                    result: r.result,
                    note_id: r.item.map(|id| id.to_string()),
                })
            })
    }
    fn publish_plugins_for_space(&self, space_id: SpaceId) -> Result<(), Error> {
        loop {
            let first=self.readers.read(|c| c.query_row("SELECT principal,request_id FROM plugin_outbox WHERE space_id=?1 ORDER BY rowid LIMIT 1",[space_id.to_string()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()).map_err(db_err)?;
            let Some((principal, request)) = first else {
                return Ok(());
            };
            self.publish_plugin_pending(stored_id(&principal)?, space_id, &request)?;
        }
    }
    fn publish_plugin_pending(
        &self,
        principal: AccountId,
        space: SpaceId,
        request_id: &str,
    ) -> Result<(), Error> {
        let pending=self.readers.read(|c| c.query_row("SELECT operation,fingerprint,result_json FROM plugin_outbox WHERE principal=?1 AND space_id=?2 AND request_id=?3",params![principal.to_string(),space.to_string(),request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).optional()).map_err(db_err)?;
        if let Some((operation, fingerprint, json)) = pending {
            let result = serde_json::from_str(&json)
                .map_err(|_| Error::internal("corrupt plugin publication"))?;
            self.content
                .record_receipt(
                    space,
                    Request {
                        account: principal,
                        id: request_id,
                        operation: &operation,
                        fingerprint: &fingerprint,
                    },
                    result,
                )
                ?;
            self.writer()
                .execute(
                    "DELETE FROM plugin_outbox WHERE principal=?1 AND space_id=?2 AND request_id=?3",
                    params![principal.to_string(), space.to_string(), request_id],
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
        &self,
        principal: AccountId,
        request_id: &str,
        operation: &str,
        fingerprint: &str,
        space_id: SpaceId,
        title: &str,
        markdown: &str,
    ) -> Result<Note, Error> {
        self.content
            .register_kind(
                space_id,
                "notes",
                calternal_content::Placement::Visible {
                    folder: "Notes".into(),
                },
                calternal_content::FileFormat::Markdown,
            )
            ?;
        let request = Request {
            account: principal,
            id: request_id,
            operation,
            fingerprint,
        };
        let item = self
            .content
            .write(
                space_id,
                None,
                "notes",
                None,
                serde_json::json!({"title":title}),
                markdown.as_bytes(),
                request,
                |i| serde_json::json!({"note":note(i)}),
            )
            ?;
        Ok(note(&item))
    }
    /// Updates only the current base revision, preserving external edits/deletes.
    /// Title changes journal a fixed exclusive rename with the same mutation.
    /// Cost: indexed head read plus generic write costs, one old-file hash, optional
    /// exclusive rename and parent fsync; no directory scan on warm Linux name caches.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &self,
        principal: AccountId,
        request_id: &str,
        operation: &str,
        fingerprint: &str,
        space_id: SpaceId,
        id: &str,
        base_revision: &str,
        title: Option<&str>,
        markdown: &str,
    ) -> Result<Note, Error> {
        let current = self.get(space_id, id)?;
        let request = Request {
            account: principal,
            id: request_id,
            operation,
            fingerprint,
        };
        let item = self
            .content
            .write(
                space_id,
                Some(item_id(id)?),
                "notes",
                Some(base_revision),
                serde_json::json!({"title":title.unwrap_or(&current.title)}),
                markdown.as_bytes(),
                request,
                |i| serde_json::json!({"note":note(i)}),
            )
            ?;
        Ok(note(&item))
    }
    /// Finishes checked placement before retry replay. Cost: catalog lookup and
    /// generic materialization locks/reads, with file+directory fsync only if pending.
    pub fn materialize(&self, id: &str) -> Result<(), Error> {
        let space = self
            .locate("notes", id)?
            .ok_or_else(|| Error::not_found("item not found"))?;
        self.content
            .materialize(space, item_id(id)?)
            
    }
    /// Records the result of a mutating plugin operation under the caller's request ID,
    /// so a retry replays it through [`Self::receipt`] instead of running the plugin
    /// again. The receipt lives in the space's journal like every other write receipt.
    /// Cost: one per-space journal append with FULL durability.
    pub fn record_plugin_receipt(
        &self,
        principal: AccountId,
        request_id: &str,
        operation: &str,
        fingerprint: &str,
        space_id: SpaceId,
        result: serde_json::Value,
    ) -> Result<(), Error> {
        let request = Request {
            account: principal,
            id: request_id,
            operation,
            fingerprint,
        };
        self.content
            .record_receipt(space_id, request, result)
            
    }
    /// Reads one item of `kind` for a plugin broker. The global route must place it in
    /// `space_id`; a foreign, absent or wrong-kind ID is the same not-found.
    /// Cost: catalog route lookup plus one indexed item read and body allocation.
    pub fn read_item(&self, space_id: SpaceId, kind: &str, id: &str) -> Result<Item, Error> {
        let missing = || Error::not_found("item not found");
        let (space, id) = (space_id, item_id(id)?);
        if self.content.locate(kind, id)? != Some(space) {
            return Err(missing());
        }
        let item = self.content.get(space, id)?;
        if item.kind != kind {
            return Err(missing());
        }
        Ok(item)
    }
    /// Creates (no `id`) or updates (against `base`) one plugin-kind item with a
    /// per-account receipt, so a replayed receipt returns the original item.
    /// The kind must already be registered in the space (plugin enable does that).
    /// Cost: one generic content write: journal and FULL SQL commits, file and
    /// directory fsync.
    #[allow(clippy::too_many_arguments)]
    pub fn write_item(
        &self,
        principal: AccountId,
        request_id: &str,
        fingerprint: &str,
        space_id: SpaceId,
        kind: &str,
        id: Option<Uuid>,
        base: Option<&str>,
        metadata: serde_json::Value,
        text: &str,
    ) -> Result<Item, Error> {
        if let Some(id) = id {
            self.read_item(space_id, kind, &id.to_string())?;
        }
        let request = Request {
            account: principal,
            id: request_id,
            operation: "plugin.write_item",
            fingerprint,
        };
        self.content
            .write(
                space_id,
                id,
                kind,
                base,
                metadata,
                text.as_bytes(),
                request,
                |i| serde_json::json!({"id": i.id, "revision": i.revision}),
            )
            
    }
    /// Provisions a plugin in one space: registers each declared kind's placement
    /// and format, then creates (or returns) each standard collection with its
    /// stable ID. Idempotent; a changed placement for an existing kind is refused.
    /// Cost: one indexed read per kind (plus one FULL insert when new) and, per
    /// absent collection, one durable content write.
    pub fn provision_plugin(
        &self,
        space_id: SpaceId,
        principal: AccountId,
        kinds: &[(String, calternal_content::Placement, calternal_content::FileFormat)],
        collections: &[(String, String, String)],
    ) -> Result<(), Error> {
        let space = space_id;
        for (kind, placement, format) in kinds {
            self.content
                .register_kind(space, kind, placement.clone(), format.clone())
                ?;
        }
        if collections.is_empty() {
            return Ok(());
        }
        // Collections are host-owned items with one fixed hidden placement.
        self.content
            .register_kind(
                space,
                "collections",
                calternal_content::Placement::Hidden {
                    plugin_id: "collections".into(),
                },
                calternal_content::FileFormat::Opaque {
                    extension: "json".into(),
                },
            )
            ?;
        for (kind, key, title) in collections {
            self.content
                .ensure_standard_collection(space, principal, kind, key, title)
                ?;
        }
        Ok(())
    }
    /// Read one space's plugin state without inheriting another space's enablement.
    /// Cost: one indexed read on a pooled read connection; no lock wait, content I/O
    /// or fsync.
    pub fn plugin_row(
        &self,
        space_id: SpaceId,
        plugin_id: &str,
    ) -> Result<Option<PluginRow>, Error> {
        self.readers
            .read(|c| plugin_row(c, space_id, plugin_id))
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
        &self,
        principal: AccountId,
        request_id: &str,
        operation: &str,
        fingerprint: &str,
        space_id: SpaceId,
        plugin_id: &str,
        default: PluginRow,
        enabled: bool,
        result: impl FnOnce(&PluginRow) -> serde_json::Value,
    ) -> Result<(PluginRow, serde_json::Value), Error> {
        self.publish_plugins_for_space(space_id)?;
        let mut writer = self.writer();
        let tx = writer
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_err)?;
        // Read inside the write transaction so concurrent toggles see each other.
        let current = plugin_row(&tx, space_id, plugin_id)
            .map_err(db_err)?
            .unwrap_or(default);
        let next = if current.enabled == enabled {
            current
        } else {
            PluginRow {
                enabled,
                generation: current.generation + 1,
            }
        };
        let value = result(&next);
        tx.execute(
            "INSERT INTO plugin_state (space_id, plugin_id, enabled, generation) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(space_id, plugin_id) DO UPDATE SET enabled = excluded.enabled, generation = excluded.generation",
            params![space_id.to_string(), plugin_id, next.enabled as i64, next.generation],
        )
        .map_err(db_err)?;
        tx.execute(
            "INSERT INTO plugin_outbox VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                principal.to_string(),
                request_id,
                operation,
                fingerprint,
                value.to_string(),
                space_id.to_string()
            ],
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)?;
        drop(writer);
        self.publish_plugin_pending(principal, space_id, request_id)?;
        Ok((next, value))
    }
}
/// External bytes prevented checked placement. Cost: short error/JSON allocations;
/// no I/O, locks or fsync. The generic content materializer uses the same reason.
pub fn blocked_error() -> Error {
    Error::conflict("note file was changed outside Calternal; writes are blocked")
        .with_details(serde_json::json!({"reason":"external_change"}))
}
