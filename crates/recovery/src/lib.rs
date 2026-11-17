#![doc = include_str!("../PROTOCOL.md")]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

mod model;
mod state;
mod storage;
use calternal_auth::spaces::RecoverySink;
pub use model::{Cell, Error, Result, Snapshot, Step, Table};
use model::{decode, encode, err, invalid};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use state::Record;
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, Mutex},
};
use storage::Storage;
use uuid::Uuid;

const DEFAULT_THRESHOLD: u64 = 64 * 1024 * 1024;
const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";
fn digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
fn key_ok(key: &str) -> bool {
    key.len() == 33
        && matches!(key.as_bytes()[0], b'0'..=b'4')
        && key
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Independently retained freshness pin. The host must persist it in a rollback-
/// resistant store before acknowledgement when both local stores may be lost.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Checkpoint {
    /// Externally provisioned installation UUID.
    pub installation: Uuid,
    /// Physical chain sequence, including proposals and resolution markers.
    pub sequence: u64,
    /// BLAKE3 hash of the exact final committed or aborted marker frame.
    pub digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Entry {
    installation: Uuid,
    sequence: u64,
    previous: String,
    body: Body,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
enum Body {
    Proposed {
        transaction: Uuid,
        delta: BTreeMap<String, Option<Record>>,
    },
    Resolved {
        committed: bool,
    },
}
fn frame(entry: &Entry) -> Result<Vec<u8>> {
    let bytes = encode(entry)?;
    let mut out = (bytes.len() as u64).to_le_bytes().to_vec();
    out.extend(bytes);
    Ok(out)
}
#[derive(Clone, Serialize, Deserialize)]
struct Image {
    version: u32,
    head: Checkpoint,
    transaction: Uuid,
    proposal_digest: String,
    committed_sequence: u64,
    segment: String,
    records: BTreeMap<String, Record>,
}
struct Loaded {
    image: Image,
    pending: Option<Entry>,
    tail: Vec<u8>,
    length: u64,
    stamps: (Vec<u64>, Vec<u64>),
    needs_sync: bool,
}
impl Loaded {
    fn resolution(&self, committed: bool) -> Result<(Vec<u8>, Checkpoint)> {
        let pending = self.pending.as_ref().ok_or_else(invalid)?;
        let proposed = frame(pending)?;
        let marker = Entry {
            installation: pending.installation,
            sequence: pending.sequence.checked_add(1).ok_or_else(invalid)?,
            previous: digest(&proposed),
            body: Body::Resolved { committed },
        };
        let bytes = frame(&marker)?;
        let head = Checkpoint {
            installation: marker.installation,
            sequence: marker.sequence,
            digest: digest(&bytes),
        };
        Ok((bytes, head))
    }
    fn apply(&mut self, committed: bool, head: Checkpoint) -> Result<()> {
        let entry = self.pending.take().ok_or_else(invalid)?;
        self.image.proposal_digest = digest(&frame(&entry)?);
        let Body::Proposed { transaction, delta } = entry.body else {
            return Err(invalid());
        };
        if committed {
            for (key, record) in delta {
                if let Some(record) = record {
                    self.image.records.insert(key, record);
                } else {
                    self.image.records.remove(&key);
                }
            }
            self.image.transaction = transaction;
            self.image.committed_sequence = entry.sequence;
        }
        self.image.head = head;
        Ok(())
    }
}

/// Installation-private append-only security delta log. The host holds root locks,
/// protects trusted ancestry and configures every auth writer before admission.
/// Two file fsyncs publish any nonempty delta; compaction is threshold-triggered.
pub struct Coordinator {
    installation: Uuid,
    store: Storage,
    gate: Mutex<Option<Loaded>>,
    threshold: u64,
}
impl Coordinator {
    /// Opens a mode-0700 Linux root; provisions only an entirely empty directory.
    /// Unknown formats and partial provisioning refuse rather than guessing state.
    pub fn open(root: &Path, installation: Uuid) -> Result<Self> {
        Self::with_observer(root, installation, Arc::new(|_| Ok(())))
    }
    /// Opens with fault observation and the default 64 MiB segment threshold.
    /// FileSynced and DirectorySynced report completed durability system calls.
    pub fn with_observer(
        root: &Path,
        installation: Uuid,
        observe: Arc<dyn Fn(Step) -> Result<()> + Send + Sync>,
    ) -> Result<Self> {
        Self::with_compaction_threshold(root, installation, DEFAULT_THRESHOLD, observe)
    }
    /// Opens with an explicit nonzero byte threshold for snapshot compaction.
    /// A change crossing this threshold also pays the documented compaction cost.
    pub fn with_compaction_threshold(
        root: &Path,
        installation: Uuid,
        threshold: u64,
        observe: Arc<dyn Fn(Step) -> Result<()> + Send + Sync>,
    ) -> Result<Self> {
        if threshold == 0 {
            return Err(invalid());
        }
        let (store, empty) = Storage::open(root, observe)?;
        let this = Self {
            installation,
            store,
            gate: Mutex::new(None),
            threshold,
        };
        if empty {
            let segment = format!("log-{}.bin", Uuid::now_v7());
            this.store.create(&segment)?;
            this.store.snapshot(&encode(&Image {
                version: 3,
                head: Checkpoint {
                    installation,
                    sequence: 0,
                    digest: ZERO.into(),
                },
                transaction: Uuid::nil(),
                proposal_digest: ZERO.into(),
                committed_sequence: 0,
                segment,
                records: BTreeMap::new(),
            })?)?;
        }
        this.load()?;
        Ok(this)
    }
    fn stamps(&self, segment: &str) -> Result<(Vec<u64>, Vec<u64>)> {
        Ok((
            self.store.stamp("snapshot.json")?,
            self.store.stamp(segment)?,
        ))
    }
    fn load_cached(&self, cache: &mut Option<Loaded>) -> Result<Loaded> {
        if let Some(s) = cache.take() {
            if self.stamps(&s.image.segment)? == s.stamps {
                return Ok(s);
            }
        }
        self.load()
    }
    fn load(&self) -> Result<Loaded> {
        let image: Image = decode(&self.store.read("snapshot.json")?)?;
        if image.version != 3
            || image.head.installation != self.installation
            || !Self::segment_name(&image.segment)
            || image.committed_sequence > image.head.sequence
            || (image.head.sequence == 0
                && (image.head.digest != ZERO || !image.records.is_empty()))
        {
            return Err(invalid());
        }
        for (key, record) in &image.records {
            self.check_record(key, record)?;
        }
        let bytes = self.store.read(&image.segment)?;
        let mut s = Loaded {
            image,
            pending: None,
            tail: Vec::new(),
            length: 0,
            stamps: (vec![], vec![]),
            needs_sync: true,
        };
        let mut pos = 0;
        while pos < bytes.len() {
            if bytes.len() - pos < 8 {
                break;
            }
            let length = u64::from_le_bytes(bytes[pos..pos + 8].try_into().map_err(err)?);
            let length = usize::try_from(length).map_err(err)?;
            let end = pos
                .checked_add(8)
                .and_then(|p| p.checked_add(length))
                .ok_or_else(invalid)?;
            if end > bytes.len() {
                break;
            }
            let entry: Entry = decode(&bytes[pos + 8..end])?;
            let (sequence, previous) = match &s.pending {
                Some(p) => (p.sequence, digest(&frame(p)?)),
                None => (s.image.head.sequence, s.image.head.digest.clone()),
            };
            if entry.installation != self.installation
                || entry.sequence != sequence.checked_add(1).ok_or_else(invalid)?
                || entry.previous != previous
            {
                return Err(invalid());
            }
            match &entry.body {
                Body::Proposed { transaction, delta }
                    if s.pending.is_none() && !transaction.is_nil() && !delta.is_empty() =>
                {
                    for (key, value) in delta {
                        if !key_ok(key) {
                            return Err(invalid());
                        }
                        if let Some(r) = value {
                            self.check_record(key, r)?;
                        }
                    }
                    s.pending = Some(entry);
                }
                Body::Resolved { committed } if s.pending.is_some() => {
                    let head = Checkpoint {
                        installation: self.installation,
                        sequence: entry.sequence,
                        digest: digest(&bytes[pos..end]),
                    };
                    s.apply(*committed, head)?;
                }
                _ => return Err(invalid()),
            }
            pos = end;
        }
        s.length = pos as u64;
        s.tail = bytes[pos..].to_vec();
        s.stamps = self.stamps(&s.image.segment)?;
        Ok(s)
    }
    fn check_record(&self, key: &str, record: &Record) -> Result<()> {
        if !key_ok(key)
            || record.key != key
            || record.installation != self.installation
            || record.version != 3
        {
            return Err(invalid());
        }
        Ok(())
    }
    fn segment_name(name: &str) -> bool {
        name.strip_prefix("log-")
            .and_then(|s| s.strip_suffix(".bin"))
            .is_some_and(|s| Uuid::parse_str(s).is_ok())
    }
    fn witness(c: &Connection) -> Result<Option<(u64, String, String)>> {
        let mut stmt = c.prepare("SELECT sequence,digest,proposal_sequence,proposal_digest FROM security_recovery_commits WHERE singleton=1").map_err(err)?;
        let mut rows = stmt.query([]).map_err(err)?;
        rows.next()
            .map_err(err)?
            .map(|r| {
                let sequence = u64::try_from(r.get::<_, i64>(0).map_err(err)?).map_err(err)?;
                let proposed = u64::try_from(r.get::<_, i64>(2).map_err(err)?).map_err(err)?;
                if sequence.checked_sub(1) != Some(proposed) {
                    return Err(invalid());
                }
                Ok((sequence, r.get(1).map_err(err)?, r.get(3).map_err(err)?))
            })
            .transpose()
    }
    fn matches(w: &Option<(u64, String, String)>, h: &Checkpoint, proposal: &str) -> bool {
        match w {
            Some((seq, hash, p)) => *seq == h.sequence && *hash == h.digest && p == proposal,
            None => h.sequence == 0 && h.digest == ZERO,
        }
    }
    fn set_witness(c: &Connection, head: &Checkpoint, proposal: &str) -> Result<()> {
        c.execute("INSERT INTO security_recovery_commits(singleton,sequence,digest,proposal_sequence,proposal_digest) VALUES(1,?1,?2,?3,?4) ON CONFLICT(singleton) DO UPDATE SET sequence=excluded.sequence,digest=excluded.digest,proposal_sequence=excluded.proposal_sequence,proposal_digest=excluded.proposal_digest",rusqlite::params![i64::try_from(head.sequence).map_err(err)?,head.digest,i64::try_from(head.sequence.checked_sub(1).ok_or_else(invalid)?).map_err(err)?,proposal]).map_err(err)?;
        Ok(())
    }
    fn replay_loaded(&self, c: &Connection, s: &mut Loaded) -> Result<()> {
        if !c.is_autocommit() {
            return Err(err("replay requires committed database visibility"));
        }
        let w = Self::witness(c)?;
        if s.pending.is_some() {
            self.store.tick(Step::Replay)?;
            let proposal = digest(&frame(s.pending.as_ref().ok_or_else(invalid)?)?);
            let (yes, committed_head) = s.resolution(true)?;
            let (no, aborted_head) = s.resolution(false)?;
            let committed = if Self::matches(&w, &committed_head, &proposal) {
                true
            } else if Self::matches(&w, &aborted_head, &proposal) {
                false
            } else if Self::matches(&w, &s.image.head, &s.image.proposal_digest) {
                // Persist the abort decision before its marker. A reader-only
                // prepare cannot advance this witness: caller must roll back
                // and reconcile using its original writable connection.
                Self::set_witness(c, &aborted_head, &proposal)?;
                false
            } else {
                return Err(invalid());
            };
            let bytes = if committed { yes } else { no };
            if !bytes.starts_with(&s.tail) {
                return Err(invalid());
            }
            self.store.tick(Step::BeforeCompletion)?;
            // Even a complete marker needs fsync again after an uncertain write.
            self.store
                .append(&s.image.segment, &bytes[s.tail.len()..])?;
            s.length += bytes.len() as u64;
            s.tail.clear();
            s.apply(
                committed,
                if committed {
                    committed_head
                } else {
                    aborted_head
                },
            )?;
            self.store.tick(Step::AfterCompletion)?;
        } else {
            if !Self::matches(&w, &s.image.head, &s.image.proposal_digest) {
                return Err(invalid());
            }
            if !s.tail.is_empty() {
                // No committed witness can name this incomplete proposal. This
                // is the only permitted truncation, never a valid log entry.
                self.store.trim(&s.image.segment, s.length)?;
                s.tail.clear();
            }
        }
        if s.needs_sync {
            // A prior write may have reached the page cache without reaching its
            // fsync. Cold replay establishes both file and snapshot durability.
            self.store.flush(&s.image.segment)?;
            self.store.sync()?;
            s.needs_sync = false;
        }
        s.stamps = self.stamps(&s.image.segment)?;
        Ok(())
    }
    fn compact(&self, s: &mut Loaded) -> Result<()> {
        if s.length >= self.threshold {
            let old = s.image.segment.clone();
            self.store.tick(Step::BeforeSegment)?;
            let next = format!("log-{}.bin", Uuid::now_v7());
            self.store.create(&next)?;
            self.store.tick(Step::AfterSegment)?;
            s.image.segment = next;
            self.store.snapshot(&encode(&s.image)?)?;
            s.length = 0;
            self.store.remove(&old)?;
            self.store.sync()?;
        }
        s.stamps = self.stamps(&s.image.segment)?;
        Ok(())
    }
    /// Resolves pending proposals only from an original surviving autocommit DB.
    /// Also repairs witnessed torn marker tails and uncommitted torn proposals.
    /// A recreated database or a chain/witness mismatch refuses recovery.
    pub fn replay(&self, c: &Connection) -> Result<()> {
        let mut cache = self.gate.lock().map_err(err)?;
        let mut s = self.load_cached(&mut cache)?;
        self.replay_loaded(c, &mut s)?;
        self.compact(&mut s)?;
        *cache = Some(s);
        Ok(())
    }
    fn read_locked(&self, expected: Option<&Checkpoint>) -> Result<Snapshot> {
        let s = self.load()?;
        if s.pending.is_some()
            || !s.tail.is_empty()
            || s.image.committed_sequence == 0
            || expected.is_some_and(|e| e != &s.image.head)
        {
            return Err(invalid());
        }
        state::assemble(
            self.installation,
            s.image.transaction,
            s.image.committed_sequence,
            s.image.records.into_values().collect(),
        )
    }
    /// Replays the full snapshot and log, refusing gaps, corruption and unresolved
    /// proposals. A valid old prefix needs a database witness or external pin to
    /// detect suffix deletion; local hash chains alone cannot prove freshness.
    pub fn validate_and_read(&self) -> Result<Snapshot> {
        let _guard = self.gate.lock().map_err(err)?;
        self.read_locked(None)
    }
    /// Validates all recovery bytes and requires the exact independent chain pin
    /// under the same gate; a valid older prefix cannot satisfy a newer pin.
    pub fn validate_and_read_at(&self, expected: &Checkpoint) -> Result<Snapshot> {
        let _guard = self.gate.lock().map_err(err)?;
        self.read_locked(Some(expected))
    }
    /// Returns the resolved chain head for external durable retention. This crate
    /// supplies no external checkpoint store or acknowledgement integration.
    pub fn checkpoint(&self) -> Result<Checkpoint> {
        let _guard = self.gate.lock().map_err(err)?;
        let s = self.load()?;
        if s.pending.is_some() || !s.tail.is_empty() || s.image.committed_sequence == 0 {
            return Err(invalid());
        }
        Ok(s.image.head)
    }
    fn prepare_inner(&self, c: &Connection) -> Result<String> {
        if c.is_autocommit() {
            return Err(err("prepare requires a write transaction"));
        }
        let mut cache = self.gate.lock().map_err(err)?;
        let committed = Connection::open_with_flags(
            c.path().filter(|s| !s.is_empty()).ok_or_else(invalid)?,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(err)?;
        if Self::witness(c)? != Self::witness(&committed)? {
            return Err(err("transaction already prepared"));
        }
        let mut s = self.load_cached(&mut cache)?;
        self.replay_loaded(&committed, &mut s)?;
        let mut delta = BTreeMap::new();
        for key in state::dirty(c)? {
            let value = state::capture_record(c, self.installation, &key)?;
            if s.image.records.get(&key) != value.as_ref() {
                delta.insert(key, value);
            }
        }
        c.execute("DELETE FROM security_recovery_dirty", [])
            .map_err(err)?;
        if delta.is_empty() {
            *cache = Some(s);
            return Ok(String::new());
        }
        let transaction = Uuid::now_v7();
        let entry = Entry {
            installation: self.installation,
            sequence: s.image.head.sequence.checked_add(1).ok_or_else(invalid)?,
            previous: s.image.head.digest.clone(),
            body: Body::Proposed { transaction, delta },
        };
        let bytes = frame(&entry)?;
        self.store.tick(Step::BeforeIntent)?;
        self.store.append(&s.image.segment, &bytes)?;
        self.store.tick(Step::AfterIntent)?;
        s.length += bytes.len() as u64;
        s.pending = Some(entry);
        Self::set_witness(c, &s.resolution(true)?.1, &digest(&bytes))?;
        self.store.tick(Step::BeforeCommit)?;
        s.stamps = self.stamps(&s.image.segment)?;
        *cache = Some(s);
        Ok(transaction.to_string())
    }
}
impl RecoverySink for Coordinator {
    fn reconcile(&self, c: &Connection) -> calternal_auth::Result<()> {
        let run = || -> Result<()> {
            if !c.is_autocommit() {
                return Err(invalid());
            }
            let mut cache = self.gate.lock().map_err(err)?;
            let mut s = self.load()?;
            let exists: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='security_recovery_commits')",[],|r|r.get(0)).map_err(err)?;
            if !exists && (s.image.head.sequence > 0 || s.pending.is_some() || !s.tail.is_empty()) {
                return Err(invalid());
            }
            state::initialize(c)?;
            self.replay_loaded(c, &mut s)?;
            self.compact(&mut s)?;
            // Obsolete segments and fsio temporary files are harmless; only the
            // exact snapshot-selected segment participates in validation.
            let mut removed = false;
            for name in self.store.names()? {
                if Self::segment_name(&name) && name != s.image.segment {
                    self.store.remove(&name)?;
                    removed = true;
                }
            }
            if removed {
                self.store.sync()?;
            }
            if s.image.committed_sequence > 0 {
                self.read_locked(None)?;
            }
            *cache = Some(s);
            Ok(())
        };
        run().map_err(auth_error)
    }
    fn prepare(&self, c: &Connection) -> calternal_auth::Result<String> {
        self.prepare_inner(c).map_err(auth_error)
    }
    fn complete(&self, c: &Connection, id: &str) -> calternal_auth::Result<()> {
        let run = || -> Result<()> {
            if !c.is_autocommit() {
                return Err(invalid());
            }
            if id.is_empty() {
                return Ok(());
            }
            let id = Uuid::parse_str(id).map_err(err)?;
            let mut cache = self.gate.lock().map_err(err)?;
            let mut s = self.load_cached(&mut cache)?;
            self.store.tick(Step::AfterCommit)?;
            self.replay_loaded(c, &mut s)?;
            if s.image.transaction != id {
                return Err(err("requested transaction did not commit"));
            }
            self.compact(&mut s)?;
            *cache = Some(s);
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
use state::{capture, ids, validate};
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
mod tests;
