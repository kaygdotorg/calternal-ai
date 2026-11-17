//! Generic per-space item storage shared by Notes and other plugins.
//!
//! Metadata, immutable history and publication intents live exclusively in
//! `<appdata>/spaces/<uuid>/content.db`; current bytes live in the selected SSD
//! space tree. The global catalog contains routing identifiers only. The
//! inherited store owns each space's journal and account-qualified receipts.
//!
//! A FULL content transaction records a head and outbox intent. Publication
//! durably records its journal event/receipt, then checked fsio placement makes
//! the current file durable before acknowledgement. Startup replays unfinished
//! intents. A changed/deleted/foreign file blocks writes and is preserved.
//! This preserves the preview materialization contract; complete file-only
//! recovery, external-editor reconciliation and tier moves remain separate work.
//!
//! Callers must retain installation root locks and enforce current authority.
//! SQL and file work belong on blocking host threads, never on async executor workers.
//!
//! Locks: work in one space never waits for another space. The space cache map
//! lock is held only to find or insert an entry (no I/O). Each space has its own
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
mod files;
mod journal;
mod pool;
mod types;
use calternal_roots::Roots;
use calternal_types::{AccountId, Error, ErrorCode, SpaceId};
pub use collections::StoredCollection;
use db::Space;
use journal::{Journals, lock};
pub use pool::ReadPool;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
pub use types::{
    Collection, CollectionOwner, FaultPoint, FileFormat, Item, Placement, Receipt, Request, Result,
    Summary,
};
use uuid::Uuid;

/// One cached space: opened on first use under its own lock, so a cold open in
/// one space never blocks another.
struct Slot {
    space: Mutex<Option<Space>>,
    last_used: Mutex<Instant>,
}
impl Slot {
    /// Whether something outside the cache (a live subscriber) still holds this
    /// space's journal. Cost: a try-lock and two counter reads; a slot being worked
    /// on right now counts as in use.
    fn in_use(&self) -> bool {
        match self.space.try_lock() {
            Ok(space) => space.as_ref().is_some_and(|s| !s.journal.is_unused()),
            Err(_) => true,
        }
    }
}
type Cache = Arc<Mutex<HashMap<SpaceId, Arc<Slot>>>>;
/// Installation content service with bounded per-space connections and idle release.
/// The host retains root locks; opening two services for the same roots is unsupported.
pub struct Content {
    roots: Roots,
    index: Mutex<Connection>,
    catalog: ReadPool,
    // Declared before `journals`: open stores drain before the driver stops.
    spaces: Cache,
    idle_after: Duration,
    journals: Journals,
    fault: fn(FaultPoint),
}
/// Closes idle spaces nobody holds. Closing drops the space's journal store, which
/// drains its writer, outside the map lock.
fn sweep(spaces: &Cache, idle: Duration) -> usize {
    let mut closed = Vec::new();
    {
        let mut map = lock(spaces);
        map.retain(|_, slot| {
            let keep = Arc::strong_count(slot) > 1
                || lock(&slot.last_used).elapsed() < idle
                || slot.in_use();
            if !keep {
                closed.push(slot.clone());
            }
            keep
        });
    }
    closed.len()
}
/// Sweeps `spaces` every 30 seconds until the cache is dropped.
fn spawn_sweeper(spaces: &Cache, idle: Duration) -> Result<()> {
    let weak = Arc::downgrade(spaces);
    std::thread::Builder::new()
        .name("content-sweep".into())
        .spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(30));
                let Some(spaces) = weak.upgrade() else { return };
                sweep(&spaces, idle);
            }
        })
        .map(drop)
        .map_err(internal)
}
/// BLAKE3 of an item file's bytes, as hex. Cost: O(bytes), one allocation.
pub(crate) fn file_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
/// Current UTC timestamp used by item versions and server adapters.
/// Cost: one clock read and a short string allocation; no locks, SQLite or fsync.
pub fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(time::macros::format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        ))
        .expect("timestamp format")
}
impl Content {
    /// Opens the identifier-only catalog and replays unfinished space intents.
    /// Roots must have been validated/locked by the host. `secrets` is the journal
    /// witness root, outside production state backups. Fault callbacks are trusted
    /// test hooks; production callers pass a no-op.
    /// Cost: directory/catalog I/O and FULL schema syncs, two journal-driver threads
    /// and one sweeper thread;
    /// startup visits existing space databases, then reads only pending intents.
    /// Each recovered revision has [`Self::write`] publication/placement costs.
    pub fn open(
        roots: Roots,
        secrets: PathBuf,
        idle_after: Duration,
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
        let index = db::database(&roots.appdata.join("system/content-index.db"))?;
        index.execute_batch("CREATE TABLE IF NOT EXISTS items(id TEXT PRIMARY KEY,space TEXT NOT NULL,kind TEXT NOT NULL) STRICT;
            CREATE TABLE IF NOT EXISTS account_items(id TEXT PRIMARY KEY,account TEXT NOT NULL,kind TEXT NOT NULL) STRICT;").map_err(internal)?;
        let spaces = Arc::new(Mutex::new(HashMap::new()));
        spawn_sweeper(&spaces, idle_after)?;
        let journals = Journals::open(roots.appdata.clone(), secrets)?;
        let catalog = ReadPool::new(roots.appdata.join("system/content-index.db"));
        let content = Self {
            roots,
            index: Mutex::new(index),
            catalog,
            spaces,
            idle_after,
            journals,
            fault,
        };
        let dir = content.roots.appdata.join("spaces");
        if dir.exists() {
            for entry in fs::read_dir(dir).map_err(internal)? {
                let entry = entry.map_err(internal)?;
                if !entry.file_type().map_err(internal)?.is_dir() {
                    continue;
                }
                let Some(id) = entry
                    .file_name()
                    .to_str()
                    .and_then(|s| s.parse::<SpaceId>().ok())
                else {
                    continue;
                };
                if entry.path().join("content.db").exists() {
                    content.with_space(id, |space| {
                        space.cleanup()?;
                        content.publish_pending(space)?;
                        let mut stmt = space
                            .conn
                            .prepare("SELECT id FROM items WHERE mat_state='pending'")
                            .map_err(internal)?;
                        let ids = stmt
                            .query_map([], |r| r.get::<_, String>(0))
                            .map_err(internal)?
                            .collect::<std::result::Result<Vec<_>, _>>()
                            .map_err(internal)?;
                        drop(stmt);
                        for id in ids {
                            if let Err(e) =
                                space.place(Uuid::parse_str(&id).map_err(internal)?, fault)
                                && e.code != ErrorCode::Conflict
                            {
                                return Err(e);
                            }
                        }
                        Ok(())
                    })?;
                    content.sweep();
                }
            }
        }
        Ok(content)
    }
    /// Runs `f` on space `id` under that space's lock, opening the space first if it
    /// is not cached. Cost: map lock for a lookup or insert (no I/O); a cold space
    /// opens SQLite and its journal under its own lock only. Refuses with
    /// unavailable when 64 spaces are open and all are in use.
    fn with_space<T>(&self, id: SpaceId, f: impl FnOnce(&mut Space) -> Result<T>) -> Result<T> {
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
                        map.remove(&idle);
                    }
                    let slot = Arc::new(Slot {
                        space: Mutex::new(None),
                        last_used: Mutex::new(Instant::now()),
                    });
                    map.insert(id, slot.clone());
                    slot
                }
            }
        };
        let mut held = lock(&slot.space);
        if held.is_none() {
            let journal = self.journals.open_store(id)?;
            *held = Some(Space::open(
                &self.roots.appdata,
                &self.roots.userdata_ssd,
                id,
                journal,
            )?);
        }
        let result = f(held.as_mut().expect("opened above"));
        *lock(&slot.last_used) = Instant::now();
        result
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
        self.with_space(space, |s| s.get(id))
    }
    /// Lists at most 1,000 metadata rows of one kind; never loads content bytes.
    /// Cost: cache/space locks, O(log N + page) indexed read and page allocations;
    /// a cold space pays SQLite open cost. No fsync or per-row filesystem stats.
    pub fn list(&self, space: SpaceId, kind: &str, limit: u32) -> Result<Vec<Summary>> {
        self.with_space(space, |s| s.list(kind, limit))
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
        self.with_space(space, |s| s.history(id, limit, cursor))
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
    /// Cost: the space's lock and one receipt/event transaction followed by a FULL
    /// durability barrier on the space's journal writer.
    pub fn record_receipt(
        &self,
        space: SpaceId,
        request: Request<'_>,
        result: Value,
    ) -> Result<()> {
        self.with_space(space, |s| {
            self.journals.publish(
                &s.journal,
                request.account,
                request.id,
                request.operation,
                request.fingerprint,
                None,
                "control",
                result,
            )
        })
    }
    /// Checks one item's current kind and existence inside an authorized space.
    /// Cost: the space's lock and one indexed read; no catalog access, body
    /// allocation, journal call or fsync.
    pub fn contains(&self, space: SpaceId, kind: &str, item: Uuid) -> Result<bool> {
        self.with_space(space, |s| {
            s.conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM items WHERE id=?1 AND kind=?2)",
                    params![item.to_string(), kind],
                    |r| r.get(0),
                )
                .map_err(internal)
        })
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
    /// Finishes pending file placement without changing the committed revision.
    /// Cost: cache/space locks, head bytes/hash allocation; pending placement reads
    /// at most the old/new file, then fsio fsyncs file and parent plus one FULL SQL
    /// bookkeeping update. Materialized items need only metadata reads.
    pub fn materialize(&self, space: SpaceId, id: Uuid) -> Result<()> {
        self.with_space(space, |s| s.place(id, self.fault))
    }
    /// Records the global route of item `id`, once. An existing route (every update)
    /// is a pooled read only; a new item takes the catalog writer for one FULL insert.
    fn route(&self, space: SpaceId, id: &str, kind: &str) -> Result<()> {
        let read = |c: &Connection| {
            c.query_row("SELECT space,kind FROM items WHERE id=?1", [id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .optional()
        };
        let existing = match self.catalog.read(read).map_err(internal)? {
            Some(existing) => existing,
            None => {
                let index = lock(&self.index);
                index
                    .execute(
                        "INSERT INTO items VALUES(?1,?2,?3) ON CONFLICT(id) DO NOTHING",
                        params![id, space.to_string(), kind],
                    )
                    .map_err(internal)?;
                read(&index)
                    .map_err(internal)?
                    .ok_or_else(|| internal("route missing"))?
            }
        };
        if existing != (space.to_string(), kind.to_owned()) {
            return Err(internal("global item collision"));
        }
        Ok(())
    }
    fn publish_pending(&self, space: &mut Space) -> Result<()> {
        loop {
            let row=space.conn.query_row("SELECT account,request,operation,fingerprint,item,revision,result FROM outbox ORDER BY rowid LIMIT 1",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?))).optional().map_err(internal)?;
            let Some((account, request, op, fp, id, revision, result)) = row else {
                return Ok(());
            };
            let item = space.get(Uuid::parse_str(&id).map_err(internal)?)?;
            self.route(space.id, &id, &item.kind)?;
            self.journals.publish(
                &space.journal,
                account.parse().map_err(internal)?,
                &request,
                &op,
                &fp,
                Some(item.id),
                &revision,
                serde_json::from_str(&result).map_err(internal)?,
            )?;
            space
                .conn
                .execute(
                    "DELETE FROM outbox WHERE account=?1 AND request=?2",
                    params![account, request],
                )
                .map_err(internal)?;
        }
    }
    /// Commits generic bytes/metadata and finishes journal publication and placement.
    /// `id=None` creates a UUIDv7; updates require the current base token. The host
    /// supplies the operation response encoder and checks authority before calling.
    /// Request IDs are unique per (account, space); a reused one is a conflict.
    /// Cost: cache/space locks, bounded metadata/byte
    /// allocations, indexed head/name checks, one FULL content transaction, a catalog
    /// route insert for a new item, two journal commits (receipt/event then FULL barrier),
    /// checked file I/O with file+directory fsync, and FULL placement/outbox updates.
    /// First name allocation loads the directory-name cache; warm Linux allocations
    /// drain inherited name-watch events. Other platforms rescan that one directory.
    /// No unrelated-space history/content reads. This is durability, not a perf gate.
    #[allow(clippy::too_many_arguments)]
    pub fn write(
        &self,
        space: SpaceId,
        id: Option<Uuid>,
        kind: &str,
        base: Option<&str>,
        metadata: Value,
        bytes: &[u8],
        request: Request<'_>,
        result: impl FnOnce(&Item) -> Value,
    ) -> Result<Item> {
        if bytes.len() > 2 * 1024 * 1024
            || metadata.to_string().len() > 256 * 1024
            || request.id.len() > 256
        {
            return Err(Error::invalid_input("content input is too large"));
        }
        files::validate_kind(kind)?;
        self.with_space(space,|s| {
            s.validate_bytes(kind,bytes)?;
            self.publish_pending(s)?;
            // Request IDs are unique per (account, space): the host replays a matching
            // receipt before calling, so any receipt here is a reuse.
            if self.journals.receipt(&s.journal,request.account,request.id)?.is_some(){return Err(Error::request_reused());}
            let (id,seq,old_path,expected)=if let Some(id)=id {
                let current=s.get(id)?;if current.kind!=kind{return Err(missing());}
                s.place(id,self.fault)?;s.check_file(id)?;
                if base!=Some(current.revision.as_str()) {return Err(Error {code:ErrorCode::Conflict,message:"item has a newer revision".into(),details:Some(serde_json::json!({"reason":"stale_revision","current_revision":current.revision}))});}
                let (seq,path,sha):(i64,String,Option<String>)=s.conn.query_row("SELECT seq,path,file_hash FROM items WHERE id=?1",[id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(internal)?;
                (id,seq+1,Some(path),sha)
            }else{(Uuid::now_v7(),1,None,None)};
            let title=metadata.get("title").and_then(Value::as_str).unwrap_or(kind);
            let path=s.choose_path(kind,title,old_path.as_deref())?;
            let item=Item {id,space,kind:kind.into(),metadata,bytes:bytes.to_vec(),revision:format!("v{seq}.{}",Uuid::now_v7().simple()),updated_at:now_rfc3339()};
            let response=result(&item);
            let tx=s.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(internal)?;
            tx.execute("INSERT INTO items(id,kind,metadata,revision,seq,updated_at,path,folded_path,old_path,mat_state,file_hash) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'pending',?10) ON CONFLICT(id) DO UPDATE SET metadata=excluded.metadata,revision=excluded.revision,seq=excluded.seq,updated_at=excluded.updated_at,path=excluded.path,folded_path=excluded.folded_path,old_path=excluded.old_path,mat_state='pending'",params![id.to_string(),kind,item.metadata.to_string(),item.revision,seq,item.updated_at,path,calternal_fsio::names::fold(&path),old_path.filter(|old|old!=&path),expected]).map_err(internal)?;
            tx.execute("INSERT INTO versions VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id.to_string(),seq,item.revision,item.metadata.to_string(),bytes,item.updated_at,request.account.to_string()]).map_err(internal)?;
            tx.execute("INSERT INTO outbox VALUES(?1,?2,?3,?4,?5,?6,?7)",params![request.account.to_string(),request.id,request.operation,request.fingerprint,id.to_string(),item.revision,response.to_string()]).map_err(internal)?;
            tx.commit().map_err(internal)?;
            (self.fault)(FaultPoint::AfterCommit);
            self.publish_pending(s)?;
            s.place(id,self.fault)?;
            Ok(item)
        })
    }
}
