//! Record-file measurement spike (#107). See
//! `docs/measurements/record-files-2026-11-21.md` for what it measures.
//!
//! bench      --dir D [--sizes 1,16,64,256] [--layouts A,A2,B,C] [--samples 200]
//!            [--reanchor-samples 20] [--raw raw.tsv] [--markers]
//! crashtest  --dir D
//! scrubtest  --dir D

mod chat;
mod synth;

use chat::{Chat, Cost, Crash, Layout, R};
use synth::Rng;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn args() -> (String, HashMap<String, String>) {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_default();
    let mut m = HashMap::new();
    let rest: Vec<String> = it.collect();
    let mut i = 0;
    while i < rest.len() {
        let k = rest[i].trim_start_matches("--").to_string();
        if i + 1 < rest.len() && !rest[i + 1].starts_with("--") {
            m.insert(k, rest[i + 1].clone());
            i += 2;
        } else {
            m.insert(k, "1".into());
            i += 1;
        }
    }
    (cmd, m)
}

/// `/proc/self/io`: rchar, wchar, syscr, syscw, read_bytes, write_bytes.
fn proc_io() -> [u64; 6] {
    let s = std::fs::read_to_string("/proc/self/io").unwrap_or_default();
    let mut v = [0u64; 6];
    for line in s.lines() {
        let (k, n) = line.split_once(": ").unwrap_or(("", "0"));
        let n = n.trim().parse().unwrap_or(0);
        let i = match k {
            "rchar" => 0,
            "wchar" => 1,
            "syscr" => 2,
            "syscw" => 3,
            "read_bytes" => 4,
            "write_bytes" => 5,
            _ => continue,
        };
        v[i] = n;
    }
    v
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Op {
    Append,
    ReadLast50,
    UnreadAfterP,
    EditRecent,
    EditRandom,
    RedactRecent,
    RedactRandom,
    ReanchorTypo,
    ReanchorInsert,
    ReanchorTrim,
}

impl Op {
    fn name(self) -> &'static str {
        match self {
            Op::Append => "append",
            Op::ReadLast50 => "read_last_50",
            Op::UnreadAfterP => "unread_after_p",
            Op::EditRecent => "edit_recent",
            Op::EditRandom => "edit_random",
            Op::RedactRecent => "redact_recent",
            Op::RedactRandom => "redact_random",
            Op::ReanchorTypo => "reanchor_typo",
            Op::ReanchorInsert => "reanchor_insert",
            Op::ReanchorTrim => "reanchor_trim",
        }
    }
}

struct Driver {
    rng: Rng,
    clock: i64,
    marker: Option<std::fs::File>,
}

enum Input {
    Append(String, String),
    ReadLast,
    Unread(i64),
    Edit(String, String, bool),
    Check(i64),
}

fn last_seq(c: &Chat) -> R<i64> {
    Ok(c.ledger.query_row("SELECT max(seq) FROM ledger", [], |r| r.get(0))?)
}

impl Driver {
    fn pick(&mut self, c: &Chat, recent: bool) -> R<(String, i64, usize)> {
        let last = last_seq(c)?;
        loop {
            let seq = if recent { last - self.rng.below(500.min(last as u64)) as i64 } else { self.rng.range(1, last as u64) as i64 };
            let row: Option<(String, i64)> = c
                .ledger
                .query_row("SELECT anchor, file FROM ledger WHERE seq = ?1 AND state < 2", [seq], |r| Ok((r.get(0)?, r.get(1)?)))
                .ok();
            if let Some((anchor, file)) = row {
                let blen: i64 = c.ledger.query_row(
                    "SELECT length(body) FROM versions WHERE seq = ?1 ORDER BY rev DESC LIMIT 1",
                    [seq],
                    |r| r.get(0),
                )?;
                return Ok((anchor, file, blen as usize));
            }
        }
    }

    /// Edit size mix: 70% a small fix (±1-20 bytes), 20% a sentence added
    /// (+30-120), 10% rewritten as a fresh message.
    fn edit_body(&mut self, old: usize) -> String {
        let r = self.rng.below(100);
        let n = if r < 70 {
            let d = self.rng.range(1, 20) as i64;
            (old as i64 + if self.rng.below(2) == 0 { d } else { -d }).max(4) as usize
        } else if r < 90 {
            old + self.rng.range(30, 120) as usize
        } else {
            return synth::body(&mut self.rng);
        };
        synth::body_of_len(&mut self.rng, n)
    }

    /// An outside editor's save between two host writes (write a temporary
    /// file, rename over: what vim and most GUI editors do).
    fn external_edit(&mut self, c: &Chat, op: Op) -> R<i64> {
        let (anchor, file, _) = self.pick(c, false)?;
        let path = c.path_of(file);
        let mut data = std::fs::read(&path)?;
        let (off, len): (i64, i64) =
            c.ledger.query_row("SELECT off, len FROM ledger WHERE anchor = ?1", [&anchor], |r| Ok((r.get(0)?, r.get(1)?)))?;
        // First byte of the body: past the header line and the blank line.
        let body_at = (off as usize..(off + len) as usize).find(|&i| data[i] == b'\n').unwrap_or(off as usize) + 2;
        match op {
            Op::ReanchorTypo => {
                let end = (off + len) as usize;
                if let Some(i) = (body_at..end).find(|&i| data[i].is_ascii_lowercase()) {
                    data[i] = if data[i] == b'e' { b'a' } else { b'e' };
                }
            }
            Op::ReanchorInsert => {
                data.splice(body_at..body_at, b"(Edited by hand.) ".iter().copied());
            }
            _ => {
                let mut out = Vec::with_capacity(data.len());
                for line in data.split(|&b| b == b'\n') {
                    let t = line.trim_ascii_end();
                    out.extend_from_slice(t);
                    out.push(b'\n');
                }
                out.pop();
                data = out;
            }
        }
        let tmp = path.with_extension("md~");
        std::fs::write(&tmp, &data)?;
        std::fs::rename(&tmp, &path)?;
        Ok(file)
    }

    fn prepare(&mut self, c: &Chat, op: Op) -> R<Input> {
        Ok(match op {
            Op::Append => {
                self.clock += self.rng.range(30, 258) as i64;
                Input::Append(synth::NAMES[self.rng.below(synth::NAMES.len() as u64) as usize].into(), synth::body(&mut self.rng))
            }
            Op::ReadLast50 => Input::ReadLast,
            Op::UnreadAfterP => Input::Unread(last_seq(c)? - self.rng.below(2000) as i64),
            Op::EditRecent | Op::EditRandom | Op::RedactRecent | Op::RedactRandom => {
                let recent = matches!(op, Op::EditRecent | Op::RedactRecent);
                let (anchor, _, blen) = self.pick(c, recent)?;
                let redact = matches!(op, Op::RedactRecent | Op::RedactRandom);
                Input::Edit(anchor, if redact { String::new() } else { self.edit_body(blen) }, redact)
            }
            Op::ReanchorTypo | Op::ReanchorInsert | Op::ReanchorTrim => Input::Check(self.external_edit(c, op)?),
        })
    }

    fn exec(&mut self, c: &mut Chat, input: Input) -> R<Cost> {
        Ok(match input {
            Input::Append(author, body) => c.append(&author, "calternal-chat", &body, self.clock)?.1,
            Input::ReadLast => {
                c.read_last(50)?;
                Cost::default()
            }
            Input::Unread(p) => {
                c.unread_after(p, 50)?;
                Cost::default()
            }
            Input::Edit(anchor, body, redact) => c.edit(&anchor, &body, redact, self.clock)?,
            Input::Check(file) => {
                let t = Instant::now();
                let n = c.check(file)?;
                Cost { reanchored: n.is_some(), changed_rows: n.unwrap_or(0), db_ns: t.elapsed().as_nanos() as u64, ..Cost::default() }
            }
        })
    }

    fn mark(&mut self, s: &str) {
        if let Some(m) = self.marker.as_mut() {
            let _ = m.write_all(s.as_bytes());
        }
    }
}

struct Sample {
    ns: u64,
    cost: Cost,
    io: [u64; 6],
    sql: u64,
}

fn pct(v: &mut [u64], p: f64) -> f64 {
    v.sort_unstable();
    let i = ((v.len() as f64 - 1.0) * p).round() as usize;
    v[i] as f64 / 1e6
}

fn mean(v: impl Iterator<Item = u64>) -> f64 {
    let (s, n) = v.fold((0u128, 0u64), |(s, n), x| (s + u128::from(x), n + 1));
    if n == 0 { 0.0 } else { s as f64 / n as f64 }
}

fn sync_all() {
    let _ = std::process::Command::new("sync").status();
}

fn run_case(dir: &Path, layout: Layout, mib: u64, a: &HashMap<String, String>, raw: &mut impl Write) -> R<()> {
    let samples: usize = a.get("samples").map_or(200, |s| s.parse().unwrap());
    let re_samples: usize = a.get("reanchor-samples").map_or(20, |s| s.parse().unwrap());
    let history: usize = a.get("history").map_or(1000, |s| s.parse().unwrap());
    let _ = std::fs::remove_dir_all(dir);
    let mut c = Chat::open(dir, layout)?;
    let t = Instant::now();
    let (records, last_at) = c.build(mib << 20, 42 + mib)?;
    let build_s = t.elapsed().as_secs_f64();
    sync_all();
    std::thread::sleep(std::time::Duration::from_secs(1));
    let segs: i64 = c.ledger.query_row("SELECT count(*) FROM heads", [], |r| r.get(0))?;
    let idx_bytes = std::fs::metadata(dir.join("index.db")).map(|m| m.len()).unwrap_or(0);
    println!(
        "case\t{mib}MiB\t{layout:?}\trecords={records}\tfiles={segs}\tfile_bytes={}\tledger_bytes={}\tindex_bytes={idx_bytes}\tbuild_s={build_s:.1}",
        c.files_bytes()?,
        c.ledger_bytes()
    );
    let mut d = Driver {
        rng: Rng::new(7 + mib),
        clock: last_at + 60,
        marker: a.contains_key("markers").then(|| std::fs::OpenOptions::new().write(true).open("/dev/null").unwrap()),
    };
    let io_overhead = {
        let a0 = proc_io();
        let a1 = proc_io();
        [0, 1, 2, 3, 4, 5].map(|i| a1[i] - a0[i])
    };
    let ops: Vec<Op> = if layout == Layout::A2 {
        vec![Op::EditRecent, Op::EditRandom]
    } else {
        vec![Op::Append, Op::ReadLast50, Op::UnreadAfterP, Op::EditRecent, Op::EditRandom, Op::RedactRecent, Op::RedactRandom]
    };
    let run = |d: &mut Driver, c: &mut Chat, op: Op, n: usize, raw: &mut dyn Write| -> R<()> {
        let mut out: Vec<Sample> = Vec::with_capacity(n);
        for i in 0..n {
            let input = d.prepare(c, op)?;
            let sql0 = c.sql_reads;
            let io0 = proc_io();
            d.mark(&format!("@@B {mib}-{layout:?} {} {i}", op.name()));
            let t = Instant::now();
            let cost = d.exec(c, input)?;
            let ns = t.elapsed().as_nanos() as u64;
            d.mark(&format!("@@E {mib}-{layout:?} {} {i}", op.name()));
            let io1 = proc_io();
            let io = [0, 1, 2, 3, 4, 5].map(|k| (io1[k] - io0[k]).saturating_sub(io_overhead[k]));
            writeln!(
                raw,
                "{mib}\t{layout:?}\t{}\t{i}\t{ns}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                op.name(),
                cost.db_ns,
                cost.file_ns,
                cost.scrub_ns,
                io[0],
                io[1],
                io[2],
                io[3],
                io[4],
                io[5],
                c.sql_reads - sql0,
                u8::from(cost.fallback),
                u8::from(cost.reanchored),
                cost.changed_rows
            )?;
            out.push(Sample { ns, cost, io, sql: c.sql_reads - sql0 });
        }
        let mut ns: Vec<u64> = out.iter().map(|s| s.ns).collect();
        let (p50, p95, p99) = (pct(&mut ns, 0.5), pct(&mut ns, 0.95), pct(&mut ns, 0.99));
        let max = *ns.last().unwrap() as f64 / 1e6;
        println!(
            "op\t{mib}MiB\t{layout:?}\t{}\tn={n}\tp50={p50:.3}\tp95={p95:.3}\tp99={p99:.3}\tmax={max:.3}\tdb_ms={:.3}\tfile_ms={:.3}\tscrub_ms={:.3}\twchar={:.0}\twrite_bytes={:.0}\trchar={:.0}\tsyscr={:.1}\tsyscw={:.1}\tsql_reads={:.1}\tfallback={:.1}%\tchanged_rows={:.0}",
            op.name(),
            mean(out.iter().map(|s| s.cost.db_ns)) / 1e6,
            mean(out.iter().map(|s| s.cost.file_ns)) / 1e6,
            mean(out.iter().map(|s| s.cost.scrub_ns)) / 1e6,
            mean(out.iter().map(|s| s.io[1])),
            mean(out.iter().map(|s| s.io[5])),
            mean(out.iter().map(|s| s.io[0])),
            mean(out.iter().map(|s| s.io[2])),
            mean(out.iter().map(|s| s.io[3])),
            mean(out.iter().map(|s| s.sql)),
            100.0 * mean(out.iter().map(|s| u64::from(s.cost.fallback))),
            mean(out.iter().map(|s| s.cost.changed_rows)),
        );
        std::io::stdout().flush()?;
        Ok(())
    };
    for op in ops {
        run(&mut d, &mut c, op, samples, raw)?;
    }
    if layout != Layout::A2 && history > 0 {
        let size = |c: &Chat| -> R<(u64, i64, i64, u64)> {
            c.ledger.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
            let (n, b): (i64, i64) = c.ledger.query_row("SELECT count(*), sum(length(body)) FROM versions", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
            Ok((c.ledger_bytes(), n, b, c.files_bytes()?))
        };
        let before = size(&c)?;
        let mut q = Driver { rng: Rng::new(99), clock: d.clock, marker: None };
        let t = Instant::now();
        for _ in 0..history {
            let input = q.prepare(&c, Op::EditRecent)?;
            q.exec(&mut c, input)?;
        }
        let after = size(&c)?;
        println!(
            "history\t{mib}MiB\t{layout:?}\tedits={history}\tsecs={:.1}\tledger_bytes {} -> {} (+{})\tversions {} -> {} (+{})\tversion_body_bytes +{}\tfile_bytes {} -> {} (+{})",
            t.elapsed().as_secs_f64(),
            before.0, after.0, after.0 as i64 - before.0 as i64,
            before.1, after.1, after.1 - before.1,
            after.2 - before.2,
            before.3, after.3, after.3 as i64 - before.3 as i64
        );
    }
    if layout != Layout::A2 {
        run(&mut d, &mut c, Op::ReanchorTypo, re_samples, raw)?;
        run(&mut d, &mut c, Op::ReanchorInsert, re_samples, raw)?;
        run(&mut d, &mut c, Op::ReanchorTrim, 1, raw)?;
        // The host's next append after the foreign edits must still land.
        run(&mut d, &mut c, Op::Append, 5, raw)?;
    }
    let t = Instant::now();
    match c.verify() {
        Ok(()) => println!("verify\t{mib}MiB\t{layout:?}\tok\t{:.1}s", t.elapsed().as_secs_f64()),
        Err(e) => println!("verify\t{mib}MiB\t{layout:?}\tFAILED: {e}"),
    }
    drop(c);
    std::fs::remove_dir_all(dir)?;
    sync_all();
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> R<()> {
    let _ = std::fs::remove_dir_all(to);
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let p = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &p)?;
        } else {
            std::fs::copy(e.path(), p)?;
        }
    }
    Ok(())
}

const CRASH_OPS: &[&str] = &["append", "append_new_segment", "edit_grow", "edit_shrink", "edit_old_grow", "redact_old"];

/// Child of `crashtest`: performs one operation and aborts at `crash`.
fn crash_child(a: &HashMap<String, String>) -> R<()> {
    let dir = PathBuf::from(&a["dir"]);
    let layout = Layout::parse(&a["layout"])?;
    let mut c = Chat::open(&dir, layout)?;
    c.adopt()?;
    c.crash = Some(match a["crash"].as_str() {
        "after_commit" => Crash::AfterCommit,
        "mid_write" => Crash::MidWrite,
        _ => Crash::BeforeSync,
    });
    let last = last_seq(&c)?;
    let at: i64 = c.ledger.query_row("SELECT max(at) FROM ledger", [], |r| r.get(0))?;
    let anchor_of = |seq: i64| -> R<String> { Ok(c.ledger.query_row("SELECT anchor FROM ledger WHERE seq = ?1", [seq], |r| r.get(0))?) };
    let mut rng = Rng::new(5);
    match a["op"].as_str() {
        "append" => drop(c.append("Lee", "calternal-chat", "A message that may be lost.", at + 60)?),
        "append_new_segment" => drop(c.append("Lee", "calternal-chat", "First of a new month.", at + 40 * 86_400)?),
        "edit_grow" => {
            let a = anchor_of(last - 3)?;
            drop(c.edit(&a, &synth::body_of_len(&mut rng, 900), false, at)?)
        }
        "edit_shrink" => {
            let a = anchor_of(last - 3)?;
            drop(c.edit(&a, "ok", false, at)?)
        }
        "edit_old_grow" => {
            let a = anchor_of(10)?;
            drop(c.edit(&a, &synth::body_of_len(&mut rng, 900), false, at)?)
        }
        _ => {
            let a = anchor_of(10)?;
            drop(c.edit(&a, "", true, at)?)
        }
    }
    Err("the operation finished without crashing".into())
}

fn crashtest(a: &HashMap<String, String>) -> R<()> {
    let dir = PathBuf::from(&a["dir"]);
    let exe = std::env::current_exe()?;
    let mut failures = 0;
    for layout in [Layout::A, Layout::A2, Layout::B, Layout::C] {
        let pristine = dir.join(format!("pristine-{layout:?}"));
        let _ = std::fs::remove_dir_all(&pristine);
        let mut c = Chat::open(&pristine, layout)?;
        c.build(if layout == Layout::C { 6 << 20 } else { 256 << 10 }, 1)?;
        drop(c);
        for op in CRASH_OPS {
            for crash in ["after_commit", "mid_write", "before_sync"] {
                let case = dir.join("case");
                copy_dir(&pristine, &case)?;
                let status = std::process::Command::new(&exe)
                    .args(["crash-child", "--dir", case.to_str().unwrap(), "--layout", &format!("{layout:?}"), "--op", op, "--crash", crash])
                    .stderr(std::process::Stdio::null())
                    .status()?;
                let mut c = Chat::open(&case, layout)?;
                let rewritten = c.recover()?;
                let verdict = match c.verify() {
                    Ok(()) => "ok".to_string(),
                    Err(e) => {
                        failures += 1;
                        format!("FAILED: {e}")
                    }
                };
                println!("crash\t{layout:?}\t{op}\t{crash}\tchild={}\trecovered_files={rewritten}\t{verdict}", status.code().map_or("killed".into(), |c| c.to_string()));
            }
        }
    }
    println!("crashtest failures: {failures}");
    Ok(())
}

/// Edits a record twice with marker words, redacts it, then looks for the
/// words in every file under the chat's folder.
fn scrubtest(a: &HashMap<String, String>) -> R<()> {
    let dir = PathBuf::from(&a["dir"]);
    for layout in [Layout::A, Layout::B, Layout::C] {
        let d = dir.join(format!("scrub-{layout:?}"));
        let _ = std::fs::remove_dir_all(&d);
        let mut c = Chat::open(&d, layout)?;
        let (_, at) = c.build(if layout == Layout::C { 6 << 20 } else { 512 << 10 }, 3)?;
        let anchor: String = c.ledger.query_row("SELECT anchor FROM ledger WHERE seq = 20", [], |r| r.get(0))?;
        c.edit(&anchor, "first draft with zebraword one", false, at)?;
        for i in 0..30 {
            c.append("Sam", "calternal-chat", &format!("filler {i}"), at + i)?;
        }
        c.edit(&anchor, "second draft with quokkaword two", false, at)?;
        let scan = |d: &Path| -> R<Vec<String>> {
            let mut hits = Vec::new();
            let mut stack = vec![d.to_path_buf()];
            while let Some(p) = stack.pop() {
                for e in std::fs::read_dir(&p)? {
                    let e = e?;
                    if e.file_type()?.is_dir() {
                        stack.push(e.path());
                        continue;
                    }
                    let data = std::fs::read(e.path())?;
                    for w in ["zebraword", "quokkaword"] {
                        if data.windows(w.len()).any(|x| x == w.as_bytes()) {
                            hits.push(format!("{w} in {}", e.file_name().to_string_lossy()));
                        }
                    }
                }
            }
            hits.sort();
            Ok(hits)
        };
        println!("scrub\t{layout:?}\tbefore redact: {:?}", scan(&d)?);
        c.edit(&anchor, "", true, at)?;
        println!("scrub\t{layout:?}\tafter redact: {:?}", scan(&d)?);
        drop(c);
        std::fs::remove_dir_all(&d)?;
    }
    Ok(())
}

fn main() -> R<()> {
    let (cmd, a) = args();
    match cmd.as_str() {
        "bench" => {
            let dir = PathBuf::from(&a["dir"]);
            let sizes: Vec<u64> = a.get("sizes").map_or("1,16,64,256", |s| s).split(',').map(|s| s.parse().unwrap()).collect();
            let layouts: Vec<Layout> =
                a.get("layouts").map_or("A,A2,B,C", |s| s).split(',').map(Layout::parse).collect::<R<_>>()?;
            let raw_path = a.get("raw").cloned().unwrap_or_else(|| "raw.tsv".into());
            let mut raw = std::io::BufWriter::new(std::fs::File::create(raw_path)?);
            writeln!(raw, "mib\tlayout\top\ti\tns\tdb_ns\tfile_ns\tscrub_ns\trchar\twchar\tsyscr\tsyscw\tread_bytes\twrite_bytes\tsql_reads\tfallback\treanchored\tchanged_rows")?;
            for &mib in &sizes {
                for &layout in &layouts {
                    run_case(&dir.join(format!("{mib}-{layout:?}")), layout, mib, &a, &mut raw)?;
                    raw.flush()?;
                }
            }
            Ok(())
        }
        "crashtest" => crashtest(&a),
        "crash-child" => crash_child(&a),
        "scrubtest" => scrubtest(&a),
        _ => Err("usage: bench | crashtest | scrubtest (see src/main.rs)".into()),
    }
}
