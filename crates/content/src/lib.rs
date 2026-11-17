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
//! The cache is capped at 64 active spaces with a configurable idle deadline;
//! a background sweep runs every 30 seconds even with no requests. The journal
//! bridge has a bounded 32-command queue. SQL and file work belong on blocking
//! host threads, never on async executor workers.
#![deny(missing_docs, rustdoc::broken_intra_doc_links)]
use crate::types::{internal, missing};
mod collections;
mod db;
mod files;
mod journal;
mod types;
use calternal_roots::Roots;
use calternal_types::{AccountId, Error, ErrorCode, SpaceId};
pub use collections::StoredCollection;
use db::Space;
use journal::{Journals, Shared, lock};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
pub use types::{
    Collection, CollectionOwner, FaultPoint, FileFormat, Item, Materialization, Placement,
    Receipt, Request, Result, Summary,
};
use uuid::Uuid;

struct Entry {
    space: Shared<Space>,
    last_used: Instant,
}
type Cache = Arc<Mutex<HashMap<SpaceId, Entry>>>;
/// Installation content service with bounded per-space connections and idle release.
/// The host retains root locks; opening two services for the same roots is unsupported.
pub struct Content {
    roots: Roots,
    index: Mutex<Connection>,
    spaces: Cache,
    idle_after: Duration,
    journals: Journals,
    fault: fn(FaultPoint),
    writes: Vec<Mutex<()>>,
    admission: Mutex<()>,
}
fn sweep(spaces: &Cache, idle: Duration) -> usize {
    let mut spaces = lock(spaces);
    let before = spaces.len();
    spaces.retain(|_, e| Arc::strong_count(&e.space) > 1 || e.last_used.elapsed() < idle);
    before - spaces.len()
}
pub(crate) fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
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
    /// Cost: directory/catalog I/O and FULL schema syncs, one journal-runtime thread;
    /// startup visits existing space databases, then reads only pending intents.
    /// Each recovered revision has [`Self::write`] publication/placement costs.
    pub fn open(
        roots: Roots,
        secrets: PathBuf,
        idle_after: Duration,
        fault: fn(FaultPoint),
    ) -> Result<Self> {
        for path in [&roots.appdata, &roots.userdata_ssd, &roots.userdata_hdd, &secrets] {
            calternal_auth::private_fs::directory(path).map_err(internal)?;
        }
        let index = db::database(&roots.appdata.join("system/content-index.db"))?;
        index.execute_batch("CREATE TABLE IF NOT EXISTS items(id TEXT PRIMARY KEY,space TEXT NOT NULL,kind TEXT NOT NULL) STRICT;
            CREATE TABLE IF NOT EXISTS account_items(id TEXT PRIMARY KEY,account TEXT NOT NULL,kind TEXT NOT NULL) STRICT;
            CREATE TABLE IF NOT EXISTS requests(account TEXT NOT NULL,request TEXT NOT NULL,space TEXT NOT NULL,PRIMARY KEY(account,request)) STRICT;").map_err(internal)?;
        let spaces = Arc::new(Mutex::new(HashMap::new()));
        let cache = spaces.clone();
        let journals = Journals::open(roots.appdata.clone(), secrets, move || {
            sweep(&cache, idle_after);
        })?;
        let content = Self {
            roots,
            index: Mutex::new(index),
            spaces,
            idle_after,
            journals,
            fault,
            writes: (0..64).map(|_| Mutex::new(())).collect(),
            admission: Mutex::new(()),
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
                                && e.code != ErrorCode::Conflict {
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
    fn with_space<T>(&self, id: SpaceId, f: impl FnOnce(&mut Space) -> Result<T>) -> Result<T> {
        let cached = {
            let mut map = lock(&self.spaces);
            map.get_mut(&id).map(|e| {
                e.last_used = Instant::now();
                e.space.clone()
            })
        };
        let held = if let Some(held) = cached {
            held
        } else {
            // Bound cold admissions before opening SQLite/writer handles. Never
            // hold the map mutex across journal round trips or database I/O.
            let _admission=lock(&self.admission);
            let again={lock(&self.spaces).get(&id).map(|e|e.space.clone())};
            if let Some(held)=again {held} else {
                {
                    let mut map=lock(&self.spaces);
                    if map.len()>=64 {
                        if let Some(old)=map.iter().filter(|(_,e)|Arc::strong_count(&e.space)==1).min_by_key(|(_,e)|e.last_used).map(|(k,_)|*k){map.remove(&old);}
                        else {return Err(Error::new(ErrorCode::Unavailable,"content cache is busy"));}
                    }
                }
                let opened=Arc::new(Mutex::new(Space::open(&self.roots.appdata,&self.roots.userdata_ssd,id,self.journals.handle(id)?)?));
                lock(&self.spaces).insert(id,Entry {space:opened.clone(),last_used:Instant::now()});
                opened
            }
        };
        let result = f(&mut lock(&held));
        if let Some(entry) = lock(&self.spaces).get_mut(&id) {
            entry.last_used = Instant::now();
        }
        result
    }
    /// Releases idle unpinned connections, also performed by the background worker.
    /// Cost: O(active spaces), at most 64, under the cache mutex; closing SQLite may
    /// checkpoint WAL. No directory scan or unrelated item/history read.
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
        lock(&self.index)
            .query_row(
                "SELECT space FROM items WHERE id=?1 AND kind=?2",
                params![id.to_string(), kind],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(internal)?
            .map(|s| s.parse().map_err(internal))
            .transpose()
    }
    /// Selects a receipt's space by identifiers only, for the host's account-wide
    /// request namespace check. Never returns a payload or grants content access.
    /// Cost: catalog mutex, one indexed lookup and UUID allocation; no space scans.
    pub fn receipt_space(&self, account: AccountId, request: &str) -> Result<Option<SpaceId>> {
        lock(&self.index)
            .query_row(
                "SELECT space FROM requests WHERE account=?1 AND request=?2",
                params![account.to_string(), request],
                |r| r.get::<_, String>(0),
            )
            .optional()
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
    /// Current authority must be checked before replay. Cost: one bounded bridge
    /// round trip, lazy journal open, indexed receipt read and payload allocation;
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
            self.journals.receipt(space, account, request)
        })
    }
    /// Publishes a receipt for a host-owned control mutation, such as plugin
    /// enablement. The host must commit its own mutation plus a replayable outbox
    /// before calling, and retire that outbox only after success. No item bytes or
    /// metadata are stored in the global catalog. Repeated publication is idempotent.
    /// Cost: cache/space locks, identifier-only catalog write and one journal bridge
    /// round trip with receipt/event transaction followed by a FULL durability barrier.
    pub fn record_receipt(
        &self,
        space: SpaceId,
        request: Request<'_>,
        result: Value,
    ) -> Result<()> {
        self.with_space(space,|_| {
            {
                let index=lock(&self.index);
                index.execute("INSERT INTO requests VALUES(?1,?2,?3) ON CONFLICT(account,request) DO NOTHING",params![request.account.to_string(),request.id,space.to_string()]).map_err(internal)?;
                let bound:String=index.query_row("SELECT space FROM requests WHERE account=?1 AND request=?2",params![request.account.to_string(),request.id],|r|r.get(0)).map_err(internal)?;
                if bound!=space.to_string(){return Err(Error::new(ErrorCode::Conflict,"request_id was already used with different input"));}
            }
            self.journals.publish(space,request.account,request.id.into(),request.operation.into(),request.fingerprint.into(),None,"control".into(),result)
        })
    }
    /// Returns a bounded journal page for host diagnostics and conformance tests.
    /// Cost: one bridge round trip and indexed read allocating at most 10,000 events;
    /// a cold journal opens its database/witness. No content reads or normal fsync.
    pub fn journal(&self, space: SpaceId) -> Result<Vec<calternal_store::JournalEntry>> {
        self.with_space(space, |_| self.journals.entries(space))
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
    /// Reads placement bookkeeping without opening a file.
    /// Cost: cache/space locks and one indexed SQLite lookup, no fsync or byte allocation.
    pub fn materialization(&self, space: SpaceId, id: Uuid) -> Result<Option<Materialization>> {
        self.with_space(space, |s| s.materialization(id))
    }
    /// Finishes pending file placement without changing the committed revision.
    /// Cost: cache/space locks, head bytes/hash allocation; pending placement reads
    /// at most the old/new file, then fsio fsyncs file and parent plus one FULL SQL
    /// bookkeeping update. Materialized items need only metadata reads.
    pub fn materialize(&self, space: SpaceId, id: Uuid) -> Result<()> {
        self.with_space(space, |s| s.place(id, self.fault))
    }
    fn publish_pending(&self, space: &mut Space) -> Result<()> {
        loop {
            let row=space.conn.query_row("SELECT account,request,operation,fingerprint,item,revision,result FROM outbox ORDER BY rowid LIMIT 1",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?))).optional().map_err(internal)?;
            let Some((account, request, op, fp, id, revision, result)) = row else {
                return Ok(());
            };
            let item = space.get(Uuid::parse_str(&id).map_err(internal)?)?;
            {
                let index = lock(&self.index);
                index
                    .execute(
                        "INSERT INTO items VALUES(?1,?2,?3) ON CONFLICT(id) DO NOTHING",
                        params![id, space.id.to_string(), item.kind],
                    )
                    .map_err(internal)?;
                let existing: (String, String) = index
                    .query_row("SELECT space,kind FROM items WHERE id=?1", [&id], |r| {
                        Ok((r.get(0)?, r.get(1)?))
                    })
                    .map_err(internal)?;
                if existing != (space.id.to_string(), item.kind.clone()) {
                    return Err(internal("global item collision"));
                }
                index.execute("INSERT INTO requests VALUES(?1,?2,?3) ON CONFLICT(account,request) DO NOTHING",params![account,request,space.id.to_string()]).map_err(internal)?;
                let bound: String = index
                    .query_row(
                        "SELECT space FROM requests WHERE account=?1 AND request=?2",
                        params![account, request],
                        |r| r.get(0),
                    )
                    .map_err(internal)?;
                if bound != space.id.to_string() {
                    return Err(Error::new(
                        ErrorCode::Conflict,
                        "request_id was already used with different input",
                    ));
                }
            }
            self.journals.publish(
                space.id,
                account.parse().map_err(internal)?,
                request.clone(),
                op,
                fp,
                Some(item.id),
                revision,
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
    /// Same-account request IDs are unique across this service's spaces.
    /// Cost: one of 64 request-stripe locks, cache/space locks, bounded metadata/byte
    /// allocations, indexed head/name checks, one FULL content transaction, catalog
    /// routing writes, two journal queue round trips (receipt/event then FULL barrier),
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
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        request.account.hash(&mut hash);
        request.id.hash(&mut hash);
        let _stripe = lock(&self.writes[hash.finish() as usize % self.writes.len()]);
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
            if self.receipt_space(request.account,request.id)?.is_some(){return Err(Error::new(ErrorCode::Conflict,"request_id was already used with different input"));}
            let (id,seq,old_path,expected)=if let Some(id)=id {
                let current=s.get(id)?;if current.kind!=kind{return Err(missing());}
                s.place(id,self.fault)?;s.check_file(id)?;
                if base!=Some(current.revision.as_str()) {return Err(Error {code:ErrorCode::Conflict,message:"item has a newer revision".into(),details:Some(serde_json::json!({"reason":"stale_revision","current_revision":current.revision}))});}
                let (seq,path,sha):(i64,String,Option<String>)=s.conn.query_row("SELECT seq,path,file_sha256 FROM items WHERE id=?1",[id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(internal)?;
                (id,seq+1,Some(path),sha)
            }else{(Uuid::now_v7(),1,None,None)};
            let title=metadata.get("title").and_then(Value::as_str).unwrap_or(kind);
            let path=s.choose_path(kind,title,old_path.as_deref())?;
            let item=Item {id,space,kind:kind.into(),metadata,bytes:bytes.to_vec(),revision:format!("v{seq}.{}",Uuid::now_v7().simple()),updated_at:now_rfc3339()};
            let response=result(&item);
            let tx=s.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(internal)?;
            tx.execute("INSERT INTO items(id,kind,metadata,revision,seq,updated_at,path,folded_path,old_path,mat_state,file_sha256) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'pending',?10) ON CONFLICT(id) DO UPDATE SET metadata=excluded.metadata,revision=excluded.revision,seq=excluded.seq,updated_at=excluded.updated_at,path=excluded.path,folded_path=excluded.folded_path,old_path=excluded.old_path,mat_state='pending'",params![id.to_string(),kind,item.metadata.to_string(),item.revision,seq,item.updated_at,path,calternal_fsio::names::fold(&path),old_path.filter(|old|old!=&path),expected]).map_err(internal)?;
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
