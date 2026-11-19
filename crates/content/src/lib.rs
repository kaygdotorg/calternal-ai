//! Generic per-space item storage shared by Notes and other plugins.
//!
//! Metadata, immutable history and publication intents live exclusively in
//! `<appdata>/spaces/<uuid>/content.db`; current bytes live in the selected SSD
//! space tree. The global catalog contains routing identifiers only. The
//! inherited store owns each space's journal and account-qualified receipts.
//!
//! A FULL content transaction records a head and outbox intent. Publication
//! durably records its journal event/receipt, before acknowledgement. The existing lifecycle worker then uses checked fsio
//! placement within ADR 0062's one-second maximum; overdue placement blocks new
//! writes. First use replays unfinished intents before exposing that space. A changed/deleted/foreign file blocks writes and is preserved.
//! This preserves the preview materialization contract; complete file-only
//! recovery, external-editor reconciliation and tier moves remain separate work.
//!
//! Callers must retain installation root locks and enforce current authority.
//! SQL and file work belong on blocking host threads, never on async executor workers.
//!
//! Locks: work in one space never waits for another space's work, except that a
//! new item's first catalog route takes the one catalog writer (below). The space
//! cache map lock is held only to find, insert or remove an entry (no I/O: an
//! evicted space is closed after the map lock is released). Each space has its own
//! lock, held for that space's whole operation, including its SQLite, journal and
//! file fsyncs; it serializes work within the space only. Catalog reads use a
//! [`ReadPool`]; the one catalog writer connection is locked only when an item is
//! first routed (a create), for one insert. The cache keeps at most 64 open spaces
//! and closes idle ones after a configurable deadline; a background thread sweeps
//! every 30 seconds even with no requests.
#![deny(missing_docs, rustdoc::broken_intra_doc_links)]
use crate::types::{internal, missing};
mod collections;
mod db;
mod discovery;
pub use discovery::ItemOrder;
mod files;
mod instructions;
mod settings;
pub use settings::{SettingEdit, SettingRecord};
mod journal;
mod lifecycle;
mod linked_write;
mod mutations;
mod pool;
mod publication;
mod recovery;
pub use recovery::ReceiptPruningIssue;
mod rename_job;
mod search;
pub use search::{
    IndexedHit, IndexedPage, MessageDecoder, MessageRecord, SearchConfig, SearchConfigSource,
    SearchKey, SearchKind, SearchMatch, SearchProbe, fold_title, message_terms, typo_expression,
};
mod revision_slice;
mod rooms;
use lifecycle::{Cache, Slot, spawn_sweeper, sweep};
pub use rooms::{LiveEdit, LiveTextEdit, RoomSnapshot};
mod thread_append;
mod trash;
mod trash_job;
mod types;
use calternal_roots::Roots;
use calternal_types::{AccountId, Error, ErrorCode, SpaceId};
pub use collections::StoredCollection;
use db::Space;
use journal::{Journals, lock};
pub use pool::ReadPool;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
pub use types::{
    Collection, CollectionOwner, FaultPoint, FileFormat, Item, Placement, Receipt, Request, Result,
    Summary,
};
use uuid::Uuid;

/// Installation content service with bounded per-space connections and idle release.
/// The host retains root locks; opening two services for the same roots is unsupported.
pub struct Content {
    // Drop the placement worker before the stores and their root guards.
    sweeper: lifecycle::Sweeper,
    roots: Roots,
    search_readers: search::Readers,
    search_writers: search::Writers,
    search_worker: std::sync::OnceLock<std::sync::mpsc::SyncSender<SpaceId>>,
    lifecycle: calternal_types::OwnerLifecycle,
    index: Mutex<Connection>,
    catalog: ReadPool,
    // Declared before `journals`: open stores drain before the driver stops.
    spaces: Cache,
    root_guard: lifecycle::RootGuard,
    idle_after: Duration,
    journals: Journals,
    fault: fn(FaultPoint),
}
/// BLAKE3 of an item file's bytes, as hex. Cost: O(bytes), one allocation.
pub(crate) fn file_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
/// Maximum submitted document size in bytes, before file placement adds identity.
pub const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;

/// Maximum physical size of one saved Markdown conversation, including identity.
pub const MAX_THREAD_BYTES: usize = 16 * 1024 * 1024;

/// Maximum serialized metadata bytes per content revision.
pub const MAX_METADATA_BYTES: usize = 256 * 1024;
/// How long request receipts are kept by default: seven days, the store's default.
/// A retry after that is treated as a new request.
pub const DEFAULT_RECEIPT_RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// Current UTC timestamp used by item versions and server adapters.
/// Cost: one clock read and a short string allocation; no locks, SQLite or fsync.
pub fn now_rfc3339() -> String {
    format_rfc3339(time::OffsetDateTime::now_utc())
}

/// Format an already captured UTC instant with the same millisecond sort key.
/// Cost: one string allocation, no clock read or I/O.
pub fn format_rfc3339(instant: time::OffsetDateTime) -> String {
    instant
        .to_offset(time::UtcOffset::UTC)
        .format(time::macros::format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        ))
        .expect("timestamp format")
}
/// Descriptor for the installation-wide UUID edge catalogue, retained on upgrade.
pub const ROUTE_SCHEMA: calternal_store::migrations::Schema<'_> =
    calternal_store::migrations::Schema::new("content-index.db", 1, &["CREATE TABLE IF NOT EXISTS items(id TEXT PRIMARY KEY,space TEXT NOT NULL,kind TEXT NOT NULL) STRICT;
            CREATE INDEX IF NOT EXISTS items_space ON items(space);
            CREATE TABLE IF NOT EXISTS account_items(id TEXT PRIMARY KEY,account TEXT NOT NULL,kind TEXT NOT NULL) STRICT;
            CREATE INDEX IF NOT EXISTS account_items_owner ON account_items(account);"], &[]);

/// Descriptor for the authoritative per-space item and revision database.
pub const SPACE_SCHEMA: calternal_store::migrations::Schema<'_> = db::SPACE_SCHEMA;

/// Descriptor for protected account collections.
pub const ACCOUNT_SCHEMA: calternal_store::migrations::Schema<'_> = collections::ACCOUNT_SCHEMA;

impl Content {
    /// Retain the host's validated root locks during background placement. The
    /// lifecycle worker pins them only while doing I/O, so it cannot outlive the
    /// installation locks if the host drops its last service handle. Call once
    /// before exposing this service. Cost: a short mutex and an Arc move; no I/O.
    pub fn retain_root_guard(&self, guard: Arc<calternal_roots::RootLocks>) {
        *lock(&self.root_guard) = Some(guard);
    }

    /// Return the validated SSD userdata root for descriptor-relative, authorized
    /// metadata reads. The caller must retain root guards and check space access.
    /// Cost: one path clone, no I/O or content/space locks.
    pub fn userdata_root(&self) -> PathBuf {
        self.roots.userdata_ssd.clone()
    }

    /// Opens the identifier-only catalog; spaces recover lazily before first use.
    /// `receipt_retention` bounds how long each space keeps request receipts
    /// ([`DEFAULT_RECEIPT_RETENTION`] unless the operator sets another); a retry
    /// after that runs as a new request.
    /// Roots must have been validated/locked by the host. `secrets` is the journal
    /// witness root, outside production state backups. Fault callbacks are trusted
    /// test hooks; production callers pass a no-op.
    /// Cost: directory/catalog I/O and FULL schema syncs, two journal-driver threads
    /// and one sweeper thread;
    /// startup never scans or opens spaces. First use recovers that space's
    /// outbox and pending files before exposing it, at [`Self::write`] costs.
    pub fn open(
        roots: Roots,
        secrets: PathBuf,
        idle_after: Duration,
        receipt_retention: Duration,
        fault: fn(FaultPoint),
    ) -> Result<Self> {
        for path in [
            &roots.appdata,
            &roots.userdata_ssd,
            &roots.userdata_hdd,
            &secrets,
        ] {
            calternal_fsio::private::directory(path).map_err(internal)?;
        }
        let catalog_path = roots.appdata.join("system/content-index.db");
        let index = db::database(&catalog_path, ROUTE_SCHEMA)?;
        let spaces = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let root_guard = Arc::new(Mutex::new(None));
        let lifecycle = calternal_types::OwnerLifecycle::default();
        let sweeper = spawn_sweeper(&spaces, &root_guard, lifecycle.clone(), idle_after, fault)?;
        let journals = Journals::open(roots.appdata.clone(), secrets, receipt_retention)?;
        let catalog = ReadPool::new(catalog_path);
        let content = Self {
            search_readers: search::Readers::default(),
            search_writers: search::Writers::default(),
            search_worker: std::sync::OnceLock::new(),
            roots,
            lifecycle,
            index: Mutex::new(index),
            catalog,
            spaces,
            root_guard,
            sweeper,
            idle_after,
            journals,
            fault,
        };
        Ok(content)
    }
    /// Runs `f` on space `id` under that space's lock, opening the space first if it
    /// is not cached. Cost: map lock for a lookup or insert (no I/O); a cold space
    /// opens SQLite and its journal under its own lock only. Refuses with
    /// unavailable when 64 spaces are open and all are in use.
    fn with_space<T>(&self, id: SpaceId, f: impl FnOnce(&mut Space) -> Result<T>) -> Result<T> {
        self.with_space_access(id, None, f)
    }

    fn with_space_access<T>(
        &self,
        id: SpaceId,
        _append_to: Option<Uuid>,
        f: impl FnOnce(&mut Space) -> Result<T>,
    ) -> Result<T> {
        let _step = self.lifecycle.step(id.as_uuid())?;
        // A capacity-evicted space is dropped (draining its journal writer) only
        // after the map lock is released, so no other space waits on that drain.
        let mut evicted = None;
        let slot = {
            let mut map = lock(&self.spaces);
            match map.get(&id) {
                Some(slot) => slot.clone(),
                None => {
                    if map.len() >= 64 {
                        let idle = map
                            .iter()
                            .filter(|(_, s)| Arc::strong_count(s) == 1 && !s.in_use())
                            .min_by_key(|(_, s)| *lock(&s.last_used))
                            .map(|(k, _)| *k)
                            .ok_or_else(|| Error::unavailable("content cache is busy"))?;
                        evicted = map.remove(&idle);
                    }
                    let slot = Arc::new(Slot {
                        space: Mutex::new(None),
                        visibility: Default::default(),
                        last_used: Mutex::new(Instant::now()),
                    });
                    map.insert(id, slot.clone());
                    slot
                }
            }
        };
        drop(evicted);
        #[cfg(feature = "perf-timings")]
        let slot_wait = calternal_store::timings::SaveStage::SlotWait.start();
        let mut held = lock(&slot.space);
        #[cfg(feature = "perf-timings")]
        slot_wait.finish();
        if held.is_none() {
            let journal = self.journals.open_store(id, self.lifecycle.clone())?;
            let mut opened =
                Space::open(&self.roots.appdata, &self.roots.userdata_ssd, id, journal)?;
            self.recover_space(&mut opened)?;
            *held = Some(opened);
            slot.visibility
                .get_or_init(|| ReadPool::new(db::space_path(&self.roots.appdata, id)));
        }
        // A failed NORMAL bookkeeping reset must never weaken a later original
        // mutation. Like the journal writer, restore FULL before each admission;
        // this PRAGMA performs no flush when no transaction is active.
        held.as_ref()
            .expect("opened above")
            .conn
            .pragma_update(None, "synchronous", "FULL")
            .map_err(internal)?;
        let opened = held.as_mut().expect("opened above");
        let result = (|| {
            #[cfg(target_os = "linux")]
            if opened.append_recovering
                && !opened
                    .append_group
                    .as_ref()
                    .is_some_and(|group| group.accepts(_append_to))
            {
                let cold_recovery = opened.append_group.is_none();
                self.publish_pending(opened)?;
                if cold_recovery {
                    opened.materialize_pending(self.fault)?;
                }
                opened.append_recovering = false;
            }
            f(opened)
        })();
        *lock(&slot.last_used) = Instant::now();
        let pending = held.as_ref().is_some_and(|s| {
            #[cfg(target_os = "linux")]
            if s.append_task.is_some() {
                return true;
            }
            s.pending_since.is_some() || s.published_appends.is_some()
        });
        drop(held);
        if pending {
            self.sweeper.unpark();
        }
        result
    }
    /// Share the space lifecycle with jobs, account input, and secrets. Cost:
    /// an Arc clone. The host closes it from its protected authority commit.
    pub fn lifecycle(&self) -> calternal_types::OwnerLifecycle {
        self.lifecycle.clone()
    }

    /// Close and drain a space, then detach its cached stores before erasure.
    /// The map lock covers removal only. Dropping a closed journal skips
    /// witness publication. Call from a blocking worker.
    pub fn retire_space(&self, space: SpaceId) {
        self.lifecycle.drain(space.as_uuid());
        self.search_readers.forget(space);
        self.search_writers.forget(space);
        let slot = lock(&self.spaces).remove(&space);
        drop(slot);
    }

    /// Unroute an owner whose lifecycle has drained. One indexed deletion
    /// from the rebuildable identifier catalog. Repeating it is safe.
    pub fn unroute_owner(&self, id: Uuid, is_space: bool) -> Result<()> {
        let _revision = calternal_types::begin_item_access_change();
        let sql = if is_space {
            "DELETE FROM items WHERE space=?1"
        } else {
            "DELETE FROM account_items WHERE account=?1"
        };
        lock(&self.index)
            .execute(sql, [id.to_string()])
            .map_err(internal)?;
        Ok(())
    }

    /// Releases idle unpinned spaces, also performed by the background sweeper.
    /// Cost: O(open spaces), at most 64, under the cache map lock; closing happens
    /// after it is released and may checkpoint WAL and drain the journal writer.
    pub fn sweep(&self) -> usize {
        sweep(&self.spaces, self.idle_after)
    }
    /// Registers a kind's userdata placement and encoding in one space. Plugins
    /// choose a visible top-level folder or hidden `.calternal/<plugin-id>` storage.
    /// Registration is idempotent; changing a persisted placement/format is refused
    /// until the host provides an explicit relocation operation. No files are moved.
    /// Cost: cache/space locks and one indexed read; first registration additionally
    /// performs one FULL SQL insertion. No other-space scans or file byte reads.
    pub fn register_kind(
        &self,
        space: SpaceId,
        kind: &str,
        placement: Placement,
        format: FileFormat,
    ) -> Result<()> {
        files::validate_kind(kind)?;
        files::validate_registration(&placement, &format)?;
        let definition = serde_json::to_string(&(placement, format)).map_err(internal)?;
        self.with_space(space, |s| {
            let old = s
                .conn
                .query_row("SELECT definition FROM kinds WHERE kind=?1", [kind], |r| {
                    r.get::<_, String>(0)
                })
                .optional()
                .map_err(internal)?;
            if let Some(old) = old {
                if old != definition {
                    return Err(Error::new(
                        ErrorCode::Conflict,
                        "kind placement change requires relocation",
                    ));
                }
            } else {
                s.conn
                    .execute("INSERT INTO kinds VALUES(?1,?2)", params![kind, definition])
                    .map_err(internal)?;
            }
            Ok(())
        })
    }
    /// Looks up a global item route without reading content. Wrong kind is absent.
    /// The returned space grants no authority; the host must check current membership.
    /// Cost: catalog mutex and one indexed lookup, allocating one UUID string; no fsync.
    pub fn locate(&self, kind: &str, id: Uuid) -> Result<Option<SpaceId>> {
        self.catalog
            .read(|c| {
                c.query_row(
                    "SELECT space FROM items WHERE id=?1 AND kind=?2",
                    params![id.to_string(), kind],
                    |r| r.get::<_, String>(0),
                )
                .optional()
            })
            .map_err(internal)?
            .map(|s| s.parse().map_err(internal))
            .transpose()
    }
    /// Reads one current item after the caller selects an authorized space.
    /// A foreign ID is absent structurally, without a space predicate.
    /// Cost: cache mutex, one space mutex, indexed metadata/head join and byte allocation;
    /// a cold space additionally opens/validates SQLite. No fsync or other-space reads.
    pub fn get(&self, space: SpaceId, id: Uuid) -> Result<Item> {
        self.with_space(space, |s| {
            let item = s.get(id)?;
            #[cfg(target_os = "linux")]
            if item.kind == "threads" {
                s.schedule_thread_proof(id, item.bytes.len());
            }
            Ok(item)
        })
    }
    /// Lists at most 1,000 metadata rows of one kind; never loads content bytes.
    /// Cost: cache/space locks, O(log N + page) indexed read and page allocations;
    /// a cold space pays SQLite open cost. No fsync or per-row filesystem stats.
    pub fn list(&self, space: SpaceId, kind: &str, limit: u32) -> Result<Vec<Summary>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        self.item_page(
            space,
            kind,
            ItemOrder::Updated { label: None },
            None,
            limit as usize,
        )
        .map(|p| p.rows)
    }
    /// Reads a bounded immutable history page; cursors bind to this exact item.
    /// Cost: cache/space locks, two or three indexed reads and at most 1,001
    /// version allocations; no fsync, directory scan or other-space query.
    pub fn history(
        &self,
        space: SpaceId,
        id: Uuid,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<(Vec<Item>, Option<String>)> {
        self.with_space(space, |s| s.history(id, limit, cursor, None))
    }
    /// Reads an account-qualified receipt from the selected space's inherited store.
    /// Current authority must be checked before replay. Cost: the space's lock,
    /// a lazy journal open, indexed receipt read and payload allocation;
    /// unfinished local outbox intents are republished before replay, with the
    /// write path's journal/catalog fsync costs. No other-space content is read.
    pub fn receipt(
        &self,
        space: SpaceId,
        account: AccountId,
        request: &str,
    ) -> Result<Option<Receipt>> {
        self.with_space(space, |s| {
            self.publish_pending(s)?;
            self.journals.receipt(&s.journal, account, request)
        })
    }
    /// Publishes a receipt for a host-owned control mutation, such as plugin
    /// enablement. The host must commit its own mutation plus a replayable outbox
    /// before calling, and retire that outbox only after success. No item bytes or
    /// metadata are stored in the global catalog. Repeated publication is idempotent.
    /// Cost: the space's lock, one receipt read and, when new, one FULL receipt/event
    /// transaction on the space's journal writer.
    pub fn record_receipt(
        &self,
        space: SpaceId,
        request: Request<'_>,
        result: Value,
    ) -> Result<()> {
        let _actor = self.lifecycle.step(request.account.as_uuid())?;
        self.with_space(space, |s| {
            self.journals.publish(
                &s.journal,
                request.account,
                request.via,
                request.id,
                request.operation,
                request.fingerprint,
                None,
                "control",
                result,
            )
        })
    }
    /// Age at which a request receipt becomes eligible for pruning. A durable
    /// pending outbox intent pins its receipt. Cost: a copy.
    pub fn receipt_retention(&self) -> Duration {
        self.journals.retention()
    }
    /// Checks one item's current committed kind and existence in an authorized
    /// space. A live journal pins an already recovered slot. Its indexed WAL
    /// read uses the existing bounded read pool, bypassing the writer's lock,
    /// with an owner permit to exclude erasure. A cold call first recovers the
    /// space. A database error fails the visibility check.
    pub fn contains(&self, space: SpaceId, kind: &str, item: Uuid) -> Result<bool> {
        let exists = |conn: &rusqlite::Connection| {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM items WHERE id=?1 AND kind=?2)",
                params![item.to_string(), kind],
                |r| r.get(0),
            )
        };
        let cached = lock(&self.spaces).get(&space).cloned();
        if let Some(slot) = cached
            && let Some(visibility) = slot.visibility.get()
        {
            let _step = self.lifecycle.step(space.as_uuid())?;
            let visible = visibility.read(exists).map_err(internal)?;
            *lock(&slot.last_used) = Instant::now();
            return Ok(visible);
        }
        self.with_space(space, |s| exists(&s.conn).map_err(internal))
    }

    /// The space's journal store, for asynchronous following. The host must
    /// authorize the space first and call this on a blocking thread. While the
    /// returned handle (or a subscription on it) lives, the space stays open, so a
    /// reopen never starts a second writer on the same journal.
    /// Cost: the space's lock (opening the space if cold) and one Arc clone.
    pub fn live_journal(&self, space: SpaceId) -> Result<calternal_store::SpaceStore> {
        self.with_space(space, |s| Ok(s.journal.clone()))
    }

    /// Returns a bounded journal page for host diagnostics and conformance tests.
    /// Cost: the space's lock and one indexed read allocating at most 10,000 events;
    /// a cold journal opens its database/witness. No content reads or normal fsync.
    pub fn journal(&self, space: SpaceId) -> Result<Vec<calternal_store::JournalEntry>> {
        self.with_space(space, |s| self.journals.entries(&s.journal))
    }
    /// Returns the stored ID-to-path mapping in the selected SSD tree.
    /// `kind` must match; callers must authorize the selected space first.
    /// Cost: cache/space locks, one indexed kind/path query,
    /// allocating a path; no tree probe, file read or fsync.
    pub fn file_path(&self, space: SpaceId, kind: &str, id: Uuid) -> Result<PathBuf> {
        self.with_space(space, |s| {
            let path = s
                .conn
                .query_row(
                    "SELECT path FROM items WHERE id=?1 AND kind=?2",
                    params![id.to_string(), kind],
                    |r| r.get::<_, String>(0),
                )
                .optional()
                .map_err(internal)?
                .ok_or_else(missing)?;
            Ok(s.files.join(path))
        })
    }
    /// Verify the current file of one live item before using it as executable
    /// configuration. Cost: one per-space lock, indexed head lookup, and file
    /// hash. A pending write is placed first using the ordinary recovery path.
    /// External changes fail closed and are preserved by the normal file guard.
    pub fn verify_file(&self, space: SpaceId, kind: &str, id: Uuid, revision: &str) -> Result<()> {
        self.with_space(space, |s| {
            let mut head = s.header(id)?;
            if head.kind != kind || head.path.starts_with(".trash/") {
                return Err(missing());
            }
            if head.revision != revision {
                return Err(Error::conflict("item has a newer revision"));
            }
            if head.state == "pending" {
                s.place_header(id, head, None, self.fault)?;
                head = s.header(id)?;
            }
            s.check_header(id, &head).map(|_| ())
        })
    }

    /// Finishes pending file placement without changing the committed revision.
    /// Cost: cache/space locks, head bytes/hash allocation; pending placement reads
    /// at most the old/new file, then fsio fsyncs file and parent plus one NORMAL replayable SQL
    /// bookkeeping update. Materialized items need only metadata reads.
    pub fn materialize(&self, space: SpaceId, id: Uuid) -> Result<()> {
        self.with_space(space, |s| {
            self.publish_pending(s)?;
            s.place(id, self.fault)
        })
    }
    /// Reads one per-space control value, such as a plugin's enablement, from the
    /// space's own database. Callers authorize the space first.
    /// Cost: the space's lock and one indexed read; no other space is touched.
    pub fn control(&self, space: SpaceId, key: &str) -> Result<Option<Value>> {
        self.with_space(space, |s| {
            s.conn
                .query_row("SELECT value FROM controls WHERE key=?1", [key], |r| {
                    r.get::<_, String>(0)
                })
                .optional()
                .map_err(internal)?
                .map(|v| serde_json::from_str(&v).map_err(internal))
                .transpose()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calternal_types::AccountId;
    use std::sync::mpsc;

    #[test]
    fn live_visibility_can_read_a_committed_item_while_the_space_writer_is_busy() {
        let dir = tempfile::tempdir().unwrap();
        let content = Content::open(
            calternal_roots::Roots {
                appdata: dir.path().join("appdata"),
                userdata_ssd: dir.path().join("ssd"),
                userdata_hdd: dir.path().join("hdd"),
            },
            dir.path().join("secrets"),
            Duration::from_secs(60),
            DEFAULT_RECEIPT_RETENTION,
            |_| {},
        )
        .unwrap();
        let space = SpaceId::new();
        content
            .register_kind(
                space,
                "notes",
                Placement::Visible {
                    folder: "Notes".into(),
                },
                FileFormat::Markdown,
            )
            .unwrap();
        let item = content
            .write(
                space,
                None,
                "notes",
                None,
                serde_json::json!({"title": "Visible"}),
                b"body",
                Request {
                    account: AccountId::new(),
                    via: None,
                    id: "create",
                    operation: "notes.create",
                    fingerprint: "create",
                },
                |item| serde_json::json!({"id": item.id}),
            )
            .unwrap();
        let _journal = content.live_journal(space).unwrap();

        let observed = std::thread::scope(|scope| {
            let (locked, entered) = mpsc::channel();
            let (release, resume) = mpsc::channel();
            let (answer, observed) = mpsc::channel();
            let content = &content;
            scope.spawn(move || {
                content
                    .with_space(space, |_| {
                        locked.send(()).unwrap();
                        resume.recv().unwrap();
                        Ok(())
                    })
                    .unwrap();
            });
            entered.recv().unwrap();
            scope.spawn(move || {
                let _ = answer.send(content.contains(space, "notes", item.id));
            });
            let observed = observed.recv_timeout(Duration::from_secs(2));
            release.send(()).unwrap();
            observed
        });
        assert!(observed.expect("visibility waited for the writer").unwrap());
    }
}
