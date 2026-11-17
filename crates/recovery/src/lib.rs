#![doc = include_str!("../PROTOCOL.md")]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

mod index;
mod model;
mod state;
mod storage;
use calternal_auth::spaces::RecoverySink;
use index::{Changes, RecordRef, digest};
pub use model::{Cell, Error, Result, Snapshot, Step, Table};
use model::{decode, encode, err, invalid};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use state::Record;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::{Arc, Mutex},
};
use storage::Storage;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct IntentRef {
    name: String,
    digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Head {
    version: u32,
    installation: Uuid,
    sequence: u64,
    transaction: Uuid,
    root: Option<String>,
    current: Option<IntentRef>,
    pending: Option<IntentRef>,
    collected: bool,
}
#[derive(Serialize, Deserialize)]
struct Intent {
    version: u32,
    installation: Uuid,
    transaction: Uuid,
    sequence: u64,
    base: Head,
    root: Option<String>,
    records: BTreeMap<String, Record>,
    index: Changes,
}

/// Externally retained recovery freshness anchor. Retain this exact value in an
/// independent rollback-resistant location before acknowledging security changes
/// to detect rollback of both the recovery tree and its authoritative database.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Checkpoint {
    /// Externally provisioned installation identity.
    pub installation: Uuid,
    /// Monotonically increasing committed security publication number.
    pub sequence: u64,
    /// BLAKE3 binding of installation, sequence, transaction, root and manifest;
    /// excludes transient pending/collection flags.
    pub digest: String,
}

/// Installation-bound, serialized security publication coordinator. The fixed head
/// authenticates a persistent radix index; mutations capture only transactionally
/// dirtied account/space/policy records. Reclamation touches only replaced paths.
/// Hold installation root locks and configure every auth writer before admission.
/// Filesystem costs and the external rollback-pin requirement are in PROTOCOL.md.
pub struct Coordinator {
    installation: Uuid,
    store: Storage,
    gate: Mutex<()>,
}
impl Coordinator {
    /// Opens a private mode-0700 root without following symlinks. A truly empty
    /// root is explicitly provisioned; partial trees or a missing head are refused.
    /// This Linux descriptor-relative implementation never rebuilds auth state.
    pub fn open(root: &Path, installation: Uuid) -> Result<Self> {
        Self::with_observer(root, installation, Arc::new(|_| Ok(())))
    }
    /// Opens with fault/durability observation at every publication and reclamation
    /// boundary. FileSynced/DirectorySynced count completed fsio durability calls.
    pub fn with_observer(
        root: &Path,
        installation: Uuid,
        observe: Arc<dyn Fn(Step) -> Result<()> + Send + Sync>,
    ) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let (store, empty) = Storage::open(root, observe)?;
            let this = Self {
                installation,
                store,
                gate: Mutex::new(()),
            };
            if empty {
                this.store.write(
                    ".",
                    "head.json",
                    &encode(&Head {
                        version: 2,
                        installation,
                        sequence: 0,
                        transaction: Uuid::nil(),
                        root: None,
                        current: None,
                        pending: None,
                        collected: true,
                    })?,
                )?;
            }
            this.head()?;
            Ok(this)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (root, installation, observe);
            Err(err("protected recovery writer requires Linux"))
        }
    }
    fn head(&self) -> Result<Head> {
        let h: Head = decode(&self.store.read(".", "head.json")?)?;
        if h.version != 2
            || h.installation != self.installation
            || (h.sequence == 0
                && (h.root.is_some() || h.current.is_some() || !h.transaction.is_nil()))
            || (h.sequence > 0
                && (h.root.is_none() || h.current.is_none() || h.transaction.is_nil()))
        {
            return Err(invalid());
        }
        Ok(h)
    }
    fn write_head(&self, h: &Head) -> Result<()> {
        self.store.write(".", "head.json", &encode(h)?)
    }
    fn intent(&self, r: &IntentRef) -> Result<Intent> {
        let bytes = self.store.read("transactions", &r.name)?;
        if digest(&bytes) != r.digest {
            return Err(invalid());
        }
        let intent: Intent = decode(&bytes)?;
        if intent.version != 2
            || intent.installation != self.installation
            || intent.base.installation != self.installation
            || intent.base.pending.is_some()
            || intent.sequence != intent.base.sequence.checked_add(1).ok_or_else(invalid)?
            || r.name != format!("{:020}-{}.intent", intent.sequence, intent.transaction)
        {
            return Err(invalid());
        }
        Ok(intent)
    }
    fn current(&self, h: &Head) -> Result<Option<Intent>> {
        let Some(r) = &h.current else { return Ok(None) };
        let i = self.intent(r)?;
        if i.sequence != h.sequence || i.transaction != h.transaction || i.root != h.root {
            return Err(invalid());
        }
        Ok(Some(i))
    }
    fn witness(c: &Connection) -> Result<Option<(String, String)>> {
        let mut stmt = c
            .prepare("SELECT id,digest FROM security_recovery_commits LIMIT 2")
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        if rows.len() > 1 {
            return Err(invalid());
        }
        Ok(rows.into_iter().next())
    }
    fn matches(w: &Option<(String, String)>, r: Option<&IntentRef>, transaction: Uuid) -> bool {
        match (w, r) {
            (None, None) => true,
            (Some((id, d)), Some(r)) => id == &transaction.to_string() && d == &r.digest,
            _ => false,
        }
    }
    fn reclaim(&self, i: &Intent) -> Result<()> {
        let mut retired = i.index.retired.clone();
        if let Some(r) = &i.base.current {
            retired.push(format!("transactions/{}", r.name));
        }
        self.remove_paths(&retired)
    }
    fn remove_paths(&self, paths: &[String]) -> Result<()> {
        let mut folders = BTreeSet::new();
        for path in paths {
            let (sub, name) = path.split_once('/').ok_or_else(invalid)?;
            if !["accounts", "spaces", "index", "transactions"].contains(&sub) {
                return Err(invalid());
            }
            self.store.remove(sub, name)?;
            folders.insert(sub);
        }
        if !folders.is_empty() {
            for folder in folders {
                self.store.tick(Step::BeforeCompactSync)?;
                self.store.sync(folder)?;
                self.store.tick(Step::AfterCompactSync)?;
            }
        }
        Ok(())
    }
    fn finish(&self, r: &IntentRef, i: &Intent) -> Result<Head> {
        for (key, record) in &i.records {
            if !index::key_ok(key)
                || record.key != *key
                || record.installation != self.installation
                || record.version != 2
            {
                return Err(invalid());
            }
            let bytes = encode(record)?;
            let rr = RecordRef {
                key: key.clone(),
                digest: digest(&bytes),
            };
            self.store.tick(Step::BeforeRecord)?;
            self.store.write(rr.sub(), &rr.name(), &bytes)?;
            self.store.tick(Step::AfterRecord)?;
        }
        for (hash, node) in &i.index.nodes {
            let bytes = encode(node)?;
            if !index::hash_ok(hash) || digest(&bytes) != *hash {
                return Err(invalid());
            }
            self.store.tick(Step::BeforeIndex)?;
            self.store.write("index", &format!("{hash}.json"), &bytes)?;
            self.store.tick(Step::AfterIndex)?;
        }
        let mut head = Head {
            version: 2,
            installation: self.installation,
            sequence: i.sequence,
            transaction: i.transaction,
            root: i.root.clone(),
            current: Some(r.clone()),
            pending: None,
            collected: false,
        };
        self.store.tick(Step::BeforeCompletion)?;
        self.write_head(&head)?;
        self.store.tick(Step::AfterCompletion)?;
        self.reclaim(i)?;
        head.collected = true;
        self.write_head(&head)?;
        Ok(head)
    }
    fn replay_locked(&self, c: &Connection) -> Result<Head> {
        if !c.is_autocommit() {
            return Err(err("replay requires committed database visibility"));
        }
        let mut h = self.head()?;
        let witness = Self::witness(c)?;
        if let Some(r) = h.pending.clone() {
            let i = self.intent(&r)?;
            let mut base = h.clone();
            base.pending = None;
            if i.base != base {
                return Err(invalid());
            }
            self.store.tick(Step::Replay)?;
            if Self::matches(&witness, Some(&r), i.transaction) {
                h = self.finish(&r, &i)?;
            } else if Self::matches(&witness, h.current.as_ref(), h.transaction) {
                self.write_head(&i.base)?;
                self.remove_paths(&[format!("transactions/{}", r.name)])?;
                h = i.base;
            } else {
                return Err(invalid());
            }
        } else if !Self::matches(&witness, h.current.as_ref(), h.transaction) {
            return Err(invalid());
        }
        if let Some(i) = self.current(&h)? {
            if !h.collected {
                self.reclaim(&i)?;
                h.collected = true;
                self.write_head(&h)?;
            }
        }
        // Only one unanchored proposal can exist (failure between intent and
        // pending-head placement). No SQLite commit is allowed in that window.
        let extra = self
            .store
            .names("transactions")?
            .into_iter()
            .filter(|n| h.current.as_ref().is_none_or(|r| r.name != *n))
            .map(|n| format!("transactions/{n}"))
            .collect::<Vec<_>>();
        if !extra.is_empty() {
            self.store.tick(Step::Replay)?;
        }
        self.remove_paths(&extra)?;
        Ok(h)
    }
    /// Reconciles against a surviving authoritative database in autocommit mode.
    /// A pending head is resolved using its committed singleton witness; a clean
    /// head must match that database's commit anchor. Never supply a recreated DB.
    pub fn replay(&self, c: &Connection) -> Result<()> {
        let _guard = self.gate.lock().map_err(err)?;
        self.replay_locked(c).map(|_| ())
    }
    /// Verifies the head, current intent, every index path and exact current record
    /// bytes, returning one coherent logical snapshot. Pending heads or unexpected
    /// transaction files refuse state-loss recovery. No fallback to older versions
    /// occurs. A full-tree rollback needs an external monotonic pin (PROTOCOL.md).
    pub fn validate_and_read(&self) -> Result<Snapshot> {
        let _guard = self.gate.lock().map_err(err)?;
        self.read_locked(None)
    }
    /// Reads one coherent snapshot only if it exactly matches the independently
    /// retained checkpoint. The pin and snapshot are checked under the same gate;
    /// an older valid whole-tree image cannot satisfy a newer external checkpoint.
    pub fn validate_and_read_at(&self, expected: &Checkpoint) -> Result<Snapshot> {
        let _guard = self.gate.lock().map_err(err)?;
        self.read_locked(Some(expected))
    }
    /// Returns the current clean publication's freshness binding. The host owns
    /// independent durable pin storage and acknowledgement ordering; this method
    /// does not implement or certify that external storage.
    pub fn checkpoint(&self) -> Result<Checkpoint> {
        let _guard = self.gate.lock().map_err(err)?;
        let h = self.head()?;
        if h.pending.is_some() || h.sequence == 0 {
            return Err(invalid());
        }
        self.current(&h)?;
        Self::checkpoint_for(&h)
    }
    fn checkpoint_for(h: &Head) -> Result<Checkpoint> {
        Ok(Checkpoint {
            installation: h.installation,
            sequence: h.sequence,
            digest: digest(&encode(&(
                h.version,
                h.installation,
                h.sequence,
                h.transaction,
                &h.root,
                &h.current,
            ))?),
        })
    }
    fn read_locked(&self, expected: Option<&Checkpoint>) -> Result<Snapshot> {
        let h = self.head()?;
        if h.pending.is_some() || h.sequence == 0 {
            return Err(invalid());
        }
        if let Some(expected) = expected {
            if Self::checkpoint_for(&h)? != *expected {
                return Err(err("external recovery checkpoint mismatch"));
            }
        }
        let i = self.current(&h)?.ok_or_else(invalid)?;
        let allowed = h.current.as_ref().ok_or_else(invalid)?;
        for name in self.store.names("transactions")? {
            if name != allowed.name && i.base.current.as_ref().is_none_or(|r| r.name != name) {
                return Err(invalid());
            }
        }
        let mut refs = Vec::new();
        index::visit(
            &self.store,
            h.root.as_deref(),
            &mut String::new(),
            &mut refs,
        )?;
        let mut records = Vec::new();
        for r in refs {
            let bytes = self.store.read(r.sub(), &r.name())?;
            if digest(&bytes) != r.digest {
                return Err(invalid());
            }
            let record: Record = decode(&bytes)?;
            if record.key != r.key
                || record.version != 2
                || record.installation != self.installation
            {
                return Err(invalid());
            }
            records.push(record);
        }
        state::assemble(self.installation, h.transaction, h.sequence, records)
    }
    fn prepare_inner(&self, c: &Connection) -> Result<String> {
        if c.is_autocommit() {
            return Err(err("prepare requires a write transaction"));
        }
        let _guard = self.gate.lock().map_err(err)?;
        let path = c.path().filter(|s| !s.is_empty()).ok_or_else(invalid)?;
        let committed =
            Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(err)?;
        // Do not ever use a transaction's own uncommitted witness for replay.
        if Self::witness(c)? != Self::witness(&committed)? {
            return Err(err("transaction already prepared"));
        }
        let base = self.replay_locked(&committed)?;
        let keys = state::dirty(c)?;
        let mut records = BTreeMap::new();
        let mut updates = Vec::new();
        for key in keys {
            let record = state::capture_record(c, self.installation, &key)?;
            let next = record
                .as_ref()
                .map(|r| {
                    encode(r).map(|bytes| RecordRef {
                        key: key.clone(),
                        digest: digest(&bytes),
                    })
                })
                .transpose()?;
            if index::get(&self.store, base.root.as_deref(), &key)? == next {
                continue;
            }
            if let Some(record) = record {
                records.insert(key.clone(), record);
            }
            updates.push((key, next));
        }
        c.execute("DELETE FROM security_recovery_dirty", [])
            .map_err(err)?;
        if updates.is_empty() {
            return Ok(String::new());
        }
        let mut changes = Changes::default();
        let root = index::update(&self.store, base.root.clone(), &updates, 0, &mut changes)?;
        let transaction = Uuid::now_v7();
        let sequence = base.sequence.checked_add(1).ok_or_else(invalid)?;
        let intent = Intent {
            version: 2,
            installation: self.installation,
            transaction,
            sequence,
            base: base.clone(),
            root,
            records,
            index: changes,
        };
        let bytes = encode(&intent)?;
        let r = IntentRef {
            name: format!("{sequence:020}-{transaction}.intent"),
            digest: digest(&bytes),
        };
        self.store.tick(Step::BeforeIntent)?;
        self.store.write("transactions", &r.name, &bytes)?;
        self.store.tick(Step::AfterIntent)?;
        self.store.tick(Step::BeforePendingHead)?;
        let mut pending = base;
        pending.pending = Some(r.clone());
        self.write_head(&pending)?;
        self.store.tick(Step::AfterPendingHead)?;
        c.execute("DELETE FROM security_recovery_commits", [])
            .map_err(err)?;
        c.execute(
            "INSERT INTO security_recovery_commits(id,digest) VALUES(?1,?2)",
            rusqlite::params![transaction.to_string(), r.digest],
        )
        .map_err(err)?;
        self.store.tick(Step::BeforeCommit)?;
        Ok(transaction.to_string())
    }
    #[cfg(test)]
    fn intents(&self) -> Result<Vec<String>> {
        self.store.names("transactions")
    }
}
impl RecoverySink for Coordinator {
    fn reconcile(&self, c: &Connection) -> calternal_auth::Result<()> {
        let run = || -> Result<()> {
            if !c.is_autocommit() {
                return Err(err("reconcile requires committed database visibility"));
            }
            let _guard = self.gate.lock().map_err(err)?;
            let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='security_recovery_commits')",[],|r|r.get(0)).map_err(err)?;
            let h = self.head()?;
            if !exists
                && (h.sequence > 0
                    || h.pending.is_some()
                    || !self.store.names("transactions")?.is_empty())
            {
                return Err(invalid());
            }
            state::initialize(c)?;
            self.replay_locked(c)?;
            Ok(())
        };
        run().map_err(auth_error)?;
        // Admission checks all current records once, rather than on each write.
        if self.head().map_err(auth_error)?.sequence > 0 {
            self.validate_and_read().map_err(auth_error)?;
        }
        Ok(())
    }
    fn prepare(&self, c: &Connection) -> calternal_auth::Result<String> {
        self.prepare_inner(c).map_err(auth_error)
    }
    fn complete(&self, c: &Connection, id: &str) -> calternal_auth::Result<()> {
        let run = || -> Result<()> {
            if !c.is_autocommit() {
                return Err(err("completion requires committed database visibility"));
            }
            if id.is_empty() {
                return Ok(());
            }
            let id = Uuid::parse_str(id).map_err(err)?;
            let _guard = self.gate.lock().map_err(err)?;
            self.store.tick(Step::AfterCommit)?;
            let h = self.replay_locked(c)?;
            if h.transaction != id {
                return Err(err("requested transaction did not commit"));
            }
            Ok(())
        };
        run().map_err(auth_error)
    }
}
fn auth_error(e: impl std::fmt::Display) -> calternal_auth::Error {
    calternal_auth::Error::Internal(e.to_string())
}

#[cfg(test)]
use calternal_fsio as fsio;
#[cfg(test)]
use model::Envelope;
#[cfg(test)]
use state::{capture, ids, validate};
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
mod tests;
