//! One chat as a record file (or monthly segment files) plus its host state:
//! the ledger (`ledger.db`, host state, synchronous=FULL, secure_delete) and
//! the index (`index.db`, appdata, rebuildable, synchronous=NORMAL).
//!
//! Write order is ledger before file. Every host write commits, in the same
//! transaction as its ledger rows, `heads.pending_from`: the first file byte
//! the write may touch. Recovery regenerates the file from that byte onward
//! out of the ledger (latest version of each record) and truncates to
//! `heads.len`, so a crash at any point leaves file and ledger agreeing.

use crate::synth::{self, Rng};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom};
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub type R<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Layout {
    /// One file; an edit rewrites the file from the edited record to the end.
    A,
    /// One file; an edit replaces the whole file through `fsio::write_atomic`.
    A2,
    /// One file; each record carries a line of trailing spaces (slack) so an
    /// edit that fits rewrites only that record.
    B,
    /// Monthly segment files; an edit rewrites its segment's tail (as A).
    C,
}

impl Layout {
    pub fn parse(s: &str) -> R<Self> {
        Ok(match s {
            "A" => Layout::A,
            "A2" => Layout::A2,
            "B" => Layout::B,
            "C" => Layout::C,
            _ => return Err(format!("unknown layout {s}").into()),
        })
    }
}

/// Where a crash test aborts the process.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Crash {
    AfterCommit,
    MidWrite,
    BeforeSync,
}

#[derive(Default, Clone, Debug)]
pub struct Cost {
    pub db_ns: u64,
    pub file_ns: u64,
    pub scrub_ns: u64,
    pub fallback: bool,
    pub reanchored: bool,
    pub changed_rows: u64,
}

#[derive(Clone, Copy, PartialEq)]
struct Stamp {
    len: u64,
    mtime: i128,
    ino: u64,
}

fn stamp(m: &std::fs::Metadata) -> Stamp {
    Stamp { len: m.size(), mtime: i128::from(m.mtime()) * 1_000_000_000 + i128::from(m.mtime_nsec()), ino: m.ino() }
}

struct Seg {
    f: File,
    seen: Option<Stamp>,
}

pub struct Chat {
    pub dir: PathBuf,
    pub layout: Layout,
    pub ledger: Connection,
    pub index: Connection,
    segs: HashMap<i64, Seg>,
    pub slack_pct: usize,
    pub rng: Rng,
    /// Indexed SQL reads issued by operations (the design's "indexed reads").
    pub sql_reads: u64,
    pub crash: Option<Crash>,
}

const LEDGER_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS heads(file INTEGER PRIMARY KEY, len INTEGER NOT NULL, pending_from INTEGER, last_seq INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS receipts(id TEXT PRIMARY KEY, seq INTEGER NOT NULL, at INTEGER NOT NULL) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS ledger(seq INTEGER PRIMARY KEY, anchor TEXT NOT NULL, file INTEGER NOT NULL,
  author TEXT NOT NULL, plugin TEXT NOT NULL, at INTEGER NOT NULL, hash BLOB NOT NULL,
  off INTEGER NOT NULL, len INTEGER NOT NULL, rev INTEGER NOT NULL, state INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS versions(id INTEGER PRIMARY KEY, seq INTEGER NOT NULL, rev INTEGER NOT NULL,
  at INTEGER NOT NULL, author TEXT NOT NULL, body BLOB NOT NULL);
";
const LEDGER_INDEXES: &str = "
CREATE UNIQUE INDEX IF NOT EXISTS ledger_anchor ON ledger(anchor);
CREATE INDEX IF NOT EXISTS ledger_file_off ON ledger(file, off);
CREATE UNIQUE INDEX IF NOT EXISTS versions_seq_rev ON versions(seq, rev);
";
const INDEX_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS records(seq INTEGER PRIMARY KEY, file INTEGER NOT NULL, anchor TEXT NOT NULL,
  author TEXT NOT NULL, at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0);
";

pub fn frontmatter(layout: Layout, file: i64) -> String {
    let seg = if layout == Layout::C { format!("segment: {}\n", synth::month_name(file)) } else { String::new() };
    format!("---\nid: 019a7c3e-55d1-7c2a-9f3e-0c4b2a1d9e77\nkind: chats\nformat: calternal-chat/v2\n{seg}---\n\n")
}

pub fn base_len(name: &str, at: i64, anchor: &str, body: &str) -> usize {
    synth::header(name, at, anchor).len() + 2 + body.len() + 2
}

/// Record bytes: header, blank line, body, then (layout B) a line of `len -
/// base` spaces, then the blank line that ends the record.
pub fn render(name: &str, at: i64, anchor: &str, body: &str, len: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(len);
    v.extend_from_slice(synth::header(name, at, anchor).as_bytes());
    v.extend_from_slice(b"\n\n");
    v.extend_from_slice(body.as_bytes());
    v.push(b'\n');
    assert!(len >= v.len() + 1, "record longer than its slot");
    v.resize(len - 1, b' ');
    v.push(b'\n');
    v
}

pub fn path_for(dir: &Path, layout: Layout, file: i64) -> PathBuf {
    match layout {
        Layout::C => dir.join("segments").join(format!("{}.md", synth::month_name(file))),
        _ => dir.join("chat.md"),
    }
}

fn slot_for(layout: Layout, slack_pct: usize, base: usize) -> usize {
    if layout == Layout::B { base + (base * slack_pct / 100).max(16) } else { base }
}

impl Chat {
    pub fn path_of(&self, file: i64) -> PathBuf {
        path_for(&self.dir, self.layout, file)
    }

    pub fn open(dir: &Path, layout: Layout) -> R<Self> {
        std::fs::create_dir_all(dir.join("segments"))?;
        let ledger = Connection::open(dir.join("ledger.db"))?;
        ledger.pragma_update(None, "journal_mode", "WAL")?;
        ledger.pragma_update(None, "synchronous", "FULL")?;
        ledger.pragma_update(None, "secure_delete", "ON")?;
        ledger.execute_batch(LEDGER_SCHEMA)?;
        let index = Connection::open(dir.join("index.db"))?;
        index.pragma_update(None, "journal_mode", "WAL")?;
        index.pragma_update(None, "synchronous", "NORMAL")?;
        index.execute_batch(INDEX_SCHEMA)?;
        Ok(Chat {
            dir: dir.to_path_buf(),
            layout,
            ledger,
            index,
            segs: HashMap::new(),
            slack_pct: 25,
            rng: Rng::new(0x5eed),
            sql_reads: 0,
            crash: None,
        })
    }

    fn slot(&self, base: usize) -> usize {
        slot_for(self.layout, self.slack_pct, base)
    }

    fn seg(&mut self, file: i64) -> R<&mut Seg> {
        if !self.segs.contains_key(&file) {
            let f = OpenOptions::new().read(true).write(true).open(self.path_of(file))?;
            self.segs.insert(file, Seg { f, seen: None });
        }
        Ok(self.segs.get_mut(&file).expect("inserted"))
    }

    /// Records what the host itself just wrote, so the next operation can
    /// tell a foreign edit from its own (1 fstat).
    fn saw(&mut self, file: i64) -> R<()> {
        let s = self.seg(file)?;
        s.seen = Some(stamp(&s.f.metadata()?));
        Ok(())
    }

    /// One stat of the path. A changed size, mtime or inode (an editor's
    /// save-by-rename) means someone else wrote the file: re-anchor it.
    pub fn check(&mut self, file: i64) -> R<Option<u64>> {
        let now = stamp(&std::fs::metadata(self.path_of(file))?);
        let seen = self.seg(file)?.seen;
        if seen == Some(now) {
            return Ok(None);
        }
        Ok(Some(self.reanchor(file)?))
    }

    fn crash_if(&self, at: Crash) {
        if self.crash == Some(at) {
            std::process::abort();
        }
    }

    fn write_record(&mut self, file: i64, bytes: &[u8], off: u64) -> R<()> {
        let crash = self.crash;
        let seg = self.seg(file)?;
        if crash == Some(Crash::MidWrite) {
            seg.f.write_all_at(&bytes[..bytes.len() / 2], off)?;
            std::process::abort();
        }
        seg.f.write_all_at(bytes, off)?;
        if crash == Some(Crash::BeforeSync) {
            std::process::abort();
        }
        seg.f.sync_data()?;
        Ok(())
    }

    /// `record.append`: 2 indexed reads (receipt, head), 1 ledger commit,
    /// 1 positioned write + fdatasync, 1 index insert (not synced).
    pub fn append(&mut self, author: &str, plugin: &str, body: &str, at: i64) -> R<(i64, Cost)> {
        let mut cost = Cost::default();
        let t0 = Instant::now();
        let rid = synth::anchor(&mut self.rng);
        self.sql_reads += 1;
        let dup: Option<i64> = self
            .ledger
            .prepare_cached("SELECT seq FROM receipts WHERE id = ?1")?
            .query_row([&rid], |r| r.get(0))
            .optional()?;
        if let Some(seq) = dup {
            return Ok((seq, cost));
        }
        let head = |c: &Connection| -> rusqlite::Result<(i64, i64, i64)> {
            c.prepare_cached("SELECT file, len, last_seq FROM heads ORDER BY file DESC LIMIT 1")?
                .query_row([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        };
        self.sql_reads += 1;
        let (mut hfile, mut hlen, mut last) = head(&self.ledger)?;
        let file = if self.layout == Layout::C { synth::month_of(at) } else { 0 };
        if file == hfile {
            if let Some(n) = self.check(file)? {
                cost.reanchored = true;
                cost.changed_rows = n;
                (hfile, hlen, last) = head(&self.ledger)?;
            }
        }
        let new_seg = (file != hfile).then(|| frontmatter(self.layout, file));
        let off = match &new_seg {
            Some(fm) => fm.len() as i64,
            None => hlen,
        };
        let seq = last + 1;
        let anchor = synth::anchor(&mut self.rng);
        let body = synth::escape_body(body);
        let len = self.slot(base_len(author, at, &anchor, &body));
        let bytes = render(author, at, &anchor, &body, len);
        let hash = blake3::hash(body.as_bytes());
        let tx = self.ledger.transaction()?;
        if new_seg.is_some() {
            tx.prepare_cached("INSERT INTO heads(file, len, pending_from, last_seq) VALUES (?1, ?2, NULL, ?3)")?
                .execute(params![file, off, last])?;
        }
        tx.prepare_cached(
            "INSERT INTO ledger(seq, anchor, file, author, plugin, at, hash, off, len, rev) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,1)",
        )?
        .execute(params![seq, anchor, file, author, plugin, at, hash.as_bytes().as_slice(), off, len as i64])?;
        tx.prepare_cached("INSERT INTO versions(seq, rev, at, author, body) VALUES (?1, 1, ?2, ?3, ?4)")?
            .execute(params![seq, at, author, body.as_bytes()])?;
        tx.prepare_cached("INSERT INTO receipts(id, seq, at) VALUES (?1, ?2, ?3)")?.execute(params![rid, seq, at])?;
        tx.prepare_cached("UPDATE heads SET len = ?1, last_seq = ?2, pending_from = ?3 WHERE file = ?4")?
            .execute(params![off + len as i64, seq, off, file])?;
        tx.commit()?;
        let t1 = Instant::now();
        cost.db_ns = (t1 - t0).as_nanos() as u64;
        self.crash_if(Crash::AfterCommit);
        if let Some(fm) = new_seg {
            calternal_fsio::write_atomic(&self.path_of(file), fm.as_bytes())?;
        }
        self.write_record(file, &bytes, off as u64)?;
        self.saw(file)?;
        cost.file_ns = t1.elapsed().as_nanos() as u64;
        self.index
            .prepare_cached("INSERT INTO records(seq, file, anchor, author, at) VALUES (?1,?2,?3,?4,?5)")?
            .execute(params![seq, file, anchor, author, at])?;
        Ok((seq, cost))
    }

    /// `record.edit` (or `record.redact` when `redact`): 3 indexed reads
    /// (receipt, record by anchor, head), 1 ledger commit that also shifts
    /// the offsets of later records when the length changes, then the file
    /// write the layout needs. A redaction also deletes the record's earlier
    /// versions (secure_delete zeroes the freed cells) and truncates the WAL
    /// so the old words leave the host state.
    pub fn edit(&mut self, anchor: &str, new_body: &str, redact: bool, now: i64) -> R<Cost> {
        let mut cost = Cost::default();
        let t0 = Instant::now();
        let rid = synth::anchor(&mut self.rng);
        self.sql_reads += 1;
        let dup: Option<i64> = self
            .ledger
            .prepare_cached("SELECT seq FROM receipts WHERE id = ?1")?
            .query_row([&rid], |r| r.get(0))
            .optional()?;
        if dup.is_some() {
            return Ok(cost);
        }
        type Row = (i64, i64, String, i64, i64, i64, i64);
        let row = |c: &Connection| -> rusqlite::Result<Row> {
            c.prepare_cached("SELECT seq, file, author, at, off, len, rev FROM ledger WHERE anchor = ?1")?
                .query_row([anchor], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))
        };
        let hlen = |c: &Connection, file: i64| -> rusqlite::Result<i64> {
            c.prepare_cached("SELECT len FROM heads WHERE file = ?1")?.query_row([file], |r| r.get(0))
        };
        self.sql_reads += 2;
        let (mut seq, mut file, mut author, mut at, mut off, mut old_len, mut rev) = row(&self.ledger)?;
        let mut head_len = hlen(&self.ledger, file)?;
        if let Some(n) = self.check(file)? {
            cost.reanchored = true;
            cost.changed_rows = n;
            (seq, file, author, at, off, old_len, rev) = row(&self.ledger)?;
            head_len = hlen(&self.ledger, file)?;
        }
        let body = if redact { "*deleted*".to_string() } else { synth::escape_body(new_body) };
        let base = base_len(&author, at, anchor, &body);
        let in_place = self.layout == Layout::B && base <= old_len as usize;
        let new_len = if in_place { old_len as usize } else { self.slot(base) };
        cost.fallback = self.layout == Layout::B && !in_place;
        let bytes = render(&author, at, anchor, &body, new_len);
        let d = new_len as i64 - old_len;
        let hash = blake3::hash(body.as_bytes());
        let tx = self.ledger.transaction()?;
        tx.prepare_cached("INSERT INTO versions(seq, rev, at, author, body) VALUES (?1, ?2, ?3, ?4, ?5)")?
            .execute(params![seq, rev + 1, now, author, body.as_bytes()])?;
        tx.prepare_cached("UPDATE ledger SET hash = ?1, len = ?2, rev = ?3, state = ?4 WHERE seq = ?5")?.execute(
            params![hash.as_bytes().as_slice(), new_len as i64, rev + 1, i64::from(redact), seq],
        )?;
        if d != 0 {
            tx.prepare_cached("UPDATE ledger SET off = off + ?1 WHERE file = ?2 AND off > ?3")?
                .execute(params![d, file, off])?;
        }
        tx.prepare_cached("UPDATE heads SET len = len + ?1, pending_from = ?2 WHERE file = ?3")?
            .execute(params![d, off, file])?;
        tx.prepare_cached("INSERT INTO receipts(id, seq, at) VALUES (?1, ?2, ?3)")?.execute(params![rid, seq, now])?;
        if redact {
            tx.prepare_cached("DELETE FROM versions WHERE seq = ?1 AND rev <= ?2")?.execute(params![seq, rev])?;
        }
        tx.commit()?;
        let t1 = Instant::now();
        cost.db_ns = (t1 - t0).as_nanos() as u64;
        self.crash_if(Crash::AfterCommit);
        if in_place {
            self.write_record(file, &bytes, off as u64)?;
        } else if self.layout == Layout::A2 {
            let path = self.path_of(file);
            let mut whole = std::fs::read(&path)?;
            whole.splice(off as usize..(off + old_len) as usize, bytes);
            if self.crash == Some(Crash::MidWrite) {
                std::process::abort();
            }
            calternal_fsio::write_atomic(&path, &whole)?;
            self.segs.remove(&file);
        } else {
            self.rewrite_tail(file, off as u64, old_len as u64, head_len as u64, bytes)?;
        }
        self.saw(file)?;
        let t2 = Instant::now();
        cost.file_ns = (t2 - t1).as_nanos() as u64;
        if redact {
            self.ledger.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
            self.index.prepare_cached("UPDATE records SET deleted = 1 WHERE seq = ?1")?.execute([seq])?;
            cost.scrub_ns = t2.elapsed().as_nanos() as u64;
        }
        Ok(cost)
    }

    /// Writes `record` at `off` followed by the old bytes after the old
    /// record, through `fsio::paced::write_all` (1 MiB steps, each flushed),
    /// then truncates and flushes.
    fn rewrite_tail(&mut self, file: i64, off: u64, old_len: u64, head_len: u64, record: Vec<u8>) -> R<()> {
        let crash = self.crash;
        let seg = self.seg(file)?;
        let mut buf = record;
        let tail_from = off + old_len;
        let at = buf.len();
        buf.resize(at + (head_len - tail_from) as usize, 0);
        seg.f.read_exact_at(&mut buf[at..], tail_from)?;
        let new_end = off + buf.len() as u64;
        if crash == Some(Crash::MidWrite) {
            seg.f.write_all_at(&buf[..buf.len() / 2], off)?;
            std::process::abort();
        }
        seg.f.seek(SeekFrom::Start(off))?;
        calternal_fsio::paced::write_all(&mut seg.f, &buf)?;
        if new_end < head_len {
            seg.f.set_len(new_end)?;
        }
        if crash == Some(Crash::BeforeSync) {
            std::process::abort();
        }
        seg.f.sync_data()?;
        Ok(())
    }

    fn read_rows(&mut self, rows: &[(i64, i64, i64)]) -> R<usize> {
        // Group by file; each file's rows are contiguous: one ranged read each.
        let mut ranges: Vec<(i64, i64, i64)> = Vec::new();
        for &(file, off, len) in rows {
            match ranges.iter_mut().find(|r| r.0 == file) {
                Some(r) => {
                    r.1 = r.1.min(off);
                    r.2 = r.2.max(off + len);
                }
                None => ranges.push((file, off, off + len)),
            }
        }
        let mut total = 0;
        for (file, from, to) in ranges {
            self.check(file)?;
            let mut buf = vec![0u8; (to - from) as usize];
            self.seg(file)?.f.read_exact_at(&mut buf, from as u64)?;
            total += buf.len();
        }
        Ok(total)
    }

    /// The last `n` records: 1 indexed read, 1 stat and 1 ranged read per file touched.
    pub fn read_last(&mut self, n: i64) -> R<usize> {
        self.sql_reads += 1;
        let rows: Vec<(i64, i64, i64)> = self
            .ledger
            .prepare_cached("SELECT file, off, len FROM ledger WHERE state < 2 ORDER BY seq DESC LIMIT ?1")?
            .query_map([n], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        self.read_rows(&rows)
    }

    /// Unread count after position `p` and the first `page` unread records:
    /// 2 indexed reads and one ranged read per file touched.
    pub fn unread_after(&mut self, p: i64, page: i64) -> R<(i64, usize)> {
        self.sql_reads += 2;
        let count: i64 = self
            .ledger
            .prepare_cached("SELECT count(*) FROM ledger WHERE seq > ?1 AND state < 2")?
            .query_row([p], |r| r.get(0))?;
        let rows: Vec<(i64, i64, i64)> = self
            .ledger
            .prepare_cached("SELECT file, off, len FROM ledger WHERE seq > ?1 AND state < 2 ORDER BY seq LIMIT ?2")?
            .query_map(params![p, page], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        Ok((count, self.read_rows(&rows)?))
    }

    /// Re-anchors a file someone else wrote: reads it whole, finds every
    /// header line, and moves each known anchor's offset and length to what
    /// the file now says. A record whose body changed gets a new version
    /// authored `external`; an unknown anchor becomes an `external` record;
    /// a missing anchor is marked gone (state 2). Returns rows changed.
    pub fn reanchor(&mut self, file: i64) -> R<u64> {
        let path = self.path_of(file);
        let data = std::fs::read(&path)?;
        let f = OpenOptions::new().read(true).write(true).open(&path)?;
        let mut found: Vec<(&[u8], usize, usize)> = Vec::new(); // anchor, start, header end
        let mut pos = 0;
        while pos < data.len() {
            let end = data[pos..].iter().position(|&b| b == b'\n').map_or(data.len(), |i| pos + i);
            if let Some(a) = synth::header_anchor(&data[pos..end]) {
                found.push((a, pos, end));
            }
            pos = end + 1;
        }
        let mut known: HashMap<Vec<u8>, (i64, i64, i64, Vec<u8>, i64)> = HashMap::new();
        {
            let mut st = self.ledger.prepare_cached("SELECT anchor, seq, off, len, hash, rev FROM ledger WHERE file = ?1 AND state < 2")?;
            let rows = st.query_map([file], |r| {
                Ok((r.get::<_, String>(0)?.into_bytes(), (r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
            })?;
            for row in rows {
                let (a, v) = row?;
                known.insert(a, v);
            }
        }
        let now = 1_900_000_000i64;
        let mut changed = 0u64;
        let mut max_seq: i64 = self.ledger.query_row("SELECT coalesce(max(seq), 0) FROM ledger", [], |r| r.get(0))?;
        let tx = self.ledger.transaction()?;
        for (i, &(a, start, hdr_end)) in found.iter().enumerate() {
            let end = found.get(i + 1).map_or(data.len(), |n| n.1);
            let body = data[(hdr_end + 1).min(end)..end].trim_ascii();
            let len = (end - start) as i64;
            let h = blake3::hash(body);
            match known.remove(a) {
                Some((seq, off, old_len, hash, rev)) => {
                    let new_body = h.as_bytes().as_slice() != hash.as_slice();
                    if off != start as i64 || old_len != len || new_body {
                        changed += 1;
                        tx.prepare_cached("UPDATE ledger SET off = ?1, len = ?2, hash = ?3, rev = ?4 WHERE seq = ?5")?.execute(
                            params![start as i64, len, h.as_bytes().as_slice(), rev + i64::from(new_body), seq],
                        )?;
                    }
                    if new_body {
                        tx.prepare_cached("INSERT INTO versions(seq, rev, at, author, body) VALUES (?1, ?2, ?3, 'external', ?4)")?
                            .execute(params![seq, rev + 1, now, body])?;
                    }
                }
                None => {
                    // The name in the header is only a readable copy; the
                    // stamp (plugin `external`) records who really wrote it.
                    changed += 1;
                    max_seq += 1;
                    let (name, at) = synth::parse_header(&data[start..hdr_end]).unwrap_or(("unknown".into(), now));
                    tx.prepare_cached(
                        "INSERT INTO ledger(seq, anchor, file, author, plugin, at, hash, off, len, rev) VALUES (?1,?2,?3,?4,'external',?5,?6,?7,?8,1)",
                    )?
                    .execute(params![max_seq, String::from_utf8_lossy(a), file, name, at, h.as_bytes().as_slice(), start as i64, len])?;
                    tx.prepare_cached("INSERT INTO versions(seq, rev, at, author, body) VALUES (?1, 1, ?2, 'external', ?3)")?
                        .execute(params![max_seq, now, body])?;
                }
            }
        }
        for (_, (seq, ..)) in known {
            changed += 1;
            tx.prepare_cached("UPDATE ledger SET state = 2 WHERE seq = ?1")?.execute([seq])?;
        }
        tx.prepare_cached("UPDATE heads SET len = ?1, pending_from = NULL, last_seq = max(last_seq, ?2) WHERE file = ?3")?
            .execute(params![data.len() as i64, max_seq, file])?;
        tx.commit()?;
        let seen = Some(stamp(&f.metadata()?));
        self.segs.insert(file, Seg { f, seen });
        Ok(changed)
    }

    fn latest_body(&self, seq: i64) -> R<Vec<u8>> {
        Ok(self
            .ledger
            .prepare_cached("SELECT body FROM versions WHERE seq = ?1 ORDER BY rev DESC LIMIT 1")?
            .query_row([seq], |r| r.get(0))?)
    }

    /// The bytes the ledger says file `file` holds from `from` on, plus the
    /// file length it expects.
    fn expected_from(&self, file: i64, from: i64) -> R<(Vec<u8>, i64)> {
        let len: i64 = self.ledger.query_row("SELECT len FROM heads WHERE file = ?1", [file], |r| r.get(0))?;
        let rows: Vec<(i64, String, i64, String, i64, i64)> = self
            .ledger
            .prepare("SELECT seq, author, at, anchor, off, len FROM ledger WHERE file = ?1 AND off >= ?2 AND state < 2 ORDER BY off")?
            .query_map(params![file, from], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
            .collect::<Result<_, _>>()?;
        let mut out = Vec::new();
        let mut at_off = from;
        for (seq, author, at, anchor, off, rlen) in rows {
            if off != at_off {
                return Err(format!("ledger gap in file {file} at {at_off} (next record at {off})").into());
            }
            let body = String::from_utf8(self.latest_body(seq)?)?;
            if base_len(&author, at, &anchor, &body) > rlen as usize {
                return Err(format!("record {anchor} is not in host form (hand-edited header or spacing)").into());
            }
            out.extend(render(&author, at, &anchor, &body, rlen as usize));
            at_off += rlen;
        }
        if at_off != len {
            return Err(format!("ledger rows end at {at_off}, head says {len}").into());
        }
        Ok((out, len))
    }

    /// Startup recovery: for each file with a pending write, regenerate the
    /// file from `pending_from` out of the ledger and truncate to its length.
    /// Returns how many files it rewrote.
    pub fn recover(&mut self) -> R<usize> {
        let pending: Vec<(i64, i64)> = self
            .ledger
            .prepare("SELECT file, pending_from FROM heads WHERE pending_from IS NOT NULL")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for &(file, from) in &pending {
            let path = self.path_of(file);
            if !path.exists() {
                calternal_fsio::write_atomic(&path, frontmatter(self.layout, file).as_bytes())?;
            }
            let (bytes, len) = self.expected_from(file, from)?;
            let f = OpenOptions::new().read(true).write(true).open(&path)?;
            f.write_all_at(&bytes, from as u64)?;
            f.set_len(len as u64)?;
            f.sync_data()?;
            self.ledger.execute("UPDATE heads SET pending_from = NULL WHERE file = ?1", [file])?;
            self.segs.remove(&file);
        }
        Ok(pending.len())
    }

    /// Checks every file byte for byte against what the ledger renders.
    pub fn verify(&self) -> R<()> {
        let files: Vec<i64> =
            self.ledger.prepare("SELECT file FROM heads")?.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        for file in files {
            let data = std::fs::read(self.path_of(file))?;
            let fm = frontmatter(self.layout, file);
            if !data.starts_with(fm.as_bytes()) {
                return Err(format!("file {file}: frontmatter differs").into());
            }
            let (bytes, len) = self.expected_from(file, fm.len() as i64)?;
            if data.len() as i64 != len || data[fm.len()..] != bytes[..] {
                return Err(format!("file {file}: {} bytes on disk, ledger expects {len}; contents differ", data.len()).into());
            }
        }
        Ok(())
    }

    /// Opens every file and records its stamp (after setup or recovery).
    pub fn adopt(&mut self) -> R<()> {
        let files: Vec<i64> =
            self.ledger.prepare("SELECT file FROM heads")?.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        for file in files {
            self.segs.remove(&file);
            self.saw(file)?;
        }
        Ok(())
    }

    /// Bulk-builds a chat of about `target` bytes of record files. Returns
    /// (records, last time).
    pub fn build(&mut self, target: u64, seed: u64) -> R<(i64, i64)> {
        use std::io::Write;
        self.ledger.pragma_update(None, "synchronous", "OFF")?;
        self.index.pragma_update(None, "synchronous", "OFF")?;
        let mut rng = Rng::new(seed);
        let (layout, slack_pct, dir) = (self.layout, self.slack_pct, self.dir.clone());
        let mut at: i64 = 1_577_836_800; // 2020-01-01
        let mut total: u64 = 0;
        let mut seq: i64 = 0;
        let mut cur: Option<(i64, std::io::BufWriter<File>, i64)> = None; // file, writer, off
        let tx = self.ledger.transaction()?;
        let itx = self.index.unchecked_transaction()?;
        {
            let mut ins = tx.prepare(
                "INSERT INTO ledger(seq, anchor, file, author, plugin, at, hash, off, len, rev) VALUES (?1,?2,?3,?4,'calternal-chat',?5,?6,?7,?8,1)",
            )?;
            let mut ver = tx.prepare("INSERT INTO versions(seq, rev, at, author, body) VALUES (?1, 1, ?2, ?3, ?4)")?;
            let mut head = tx.prepare("INSERT INTO heads(file, len, pending_from, last_seq) VALUES (?1, ?2, NULL, ?3)")?;
            let mut idx = itx.prepare("INSERT INTO records(seq, file, anchor, author, at) VALUES (?1,?2,?3,?4,?5)")?;
            while total < target {
                seq += 1;
                at += rng.range(30, 258) as i64;
                let file = if layout == Layout::C { synth::month_of(at) } else { 0 };
                if cur.as_ref().map(|c| c.0) != Some(file) {
                    if let Some((f, mut w, off)) = cur.take() {
                        w.flush()?;
                        w.get_ref().sync_all()?;
                        head.execute(params![f, off, seq - 1])?;
                    }
                    let fm = frontmatter(layout, file);
                    let mut w = std::io::BufWriter::with_capacity(1 << 20, File::create(path_for(&dir, layout, file))?);
                    w.write_all(fm.as_bytes())?;
                    total += fm.len() as u64;
                    cur = Some((file, w, fm.len() as i64));
                }
                let author = synth::NAMES[rng.below(synth::NAMES.len() as u64) as usize];
                let anchor = synth::anchor(&mut rng);
                let body = synth::escape_body(&synth::body(&mut rng));
                let len = slot_for(layout, slack_pct, base_len(author, at, &anchor, &body));
                let bytes = render(author, at, &anchor, &body, len);
                let (_, w, off) = cur.as_mut().expect("open");
                w.write_all(&bytes)?;
                ins.execute(params![seq, anchor, file, author, at, blake3::hash(body.as_bytes()).as_bytes().as_slice(), *off, len as i64])?;
                ver.execute(params![seq, at, author, body.as_bytes()])?;
                idx.execute(params![seq, file, anchor, author, at])?;
                *off += len as i64;
                total += len as u64;
            }
            if let Some((f, mut w, off)) = cur.take() {
                w.flush()?;
                w.get_ref().sync_all()?;
                head.execute(params![f, off, seq])?;
            }
        }
        tx.execute_batch(LEDGER_INDEXES)?;
        tx.commit()?;
        itx.commit()?;
        calternal_fsio::sync_dir(&dir.join("segments"))?;
        calternal_fsio::sync_dir(&self.dir)?;
        self.ledger.pragma_update(None, "synchronous", "FULL")?;
        self.index.pragma_update(None, "synchronous", "NORMAL")?;
        self.ledger.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
        self.index.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
        self.adopt()?;
        Ok((seq, at))
    }

    /// Bytes of the ledger database plus its WAL.
    pub fn ledger_bytes(&self) -> u64 {
        ["ledger.db", "ledger.db-wal"].iter().filter_map(|n| std::fs::metadata(self.dir.join(n)).ok()).map(|m| m.len()).sum()
    }

    pub fn files_bytes(&self) -> R<u64> {
        let files: Vec<i64> =
            self.ledger.prepare("SELECT file FROM heads")?.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        Ok(files.iter().filter_map(|&f| std::fs::metadata(self.path_of(f)).ok()).map(|m| m.len()).sum())
    }
}
