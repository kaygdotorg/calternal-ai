//! Real HTTP and one disposable near-ceiling transcript on the locked perf VM.
use crate::{
    Args, fixtures,
    http::{Http, Result},
    server::Server,
    stats,
};
use rusqlite::{Connection, params};
use serde_json::json;
use std::{
    path::Path,
    time::{Duration, Instant},
};

const SIZE: usize = 16 * 1024 * 1024;
const SMALL_APPEND: &str = "threads.small.append";
const APPEND: &str = "threads.16mib.append";
const TAIL: &str = "threads.16mib.tail";
const EXACT: &str = "threads.16mib.exact";
const REBUILD: &str = "threads.16mib.message_index_rebuild";

pub(crate) fn run(a: &Args) -> Result<i32> {
    if !(20..=100).contains(&a.samples) {
        return Err("long-thread proof needs 20–100 samples".into());
    }
    let budgets: stats::Budgets = serde_json::from_slice(&std::fs::read(&a.budgets)?)?;
    fixtures::private_work(&a.work)?;
    let fixture = fixtures::build(a, &a.work, 0, 1)?;
    let space = fixture.spaces[0].as_str().ok_or("fixture space missing")?;
    let id = {
        let (_server, mut http, _) = start(a)?;
        http.cookie = fixture.cookie.clone();
        http.op(
            "threads.create",
            json!({"space_id":space,"title":"Long thread"}),
        )?
        .0["thread"]["id"]
            .as_str()
            .ok_or("fixture thread missing")?
            .to_owned()
    };
    let ceiling = if a.small_thread {
        64 * 1024
    } else if a.legacy_thread_limit {
        2 * 1024 * 1024 - 48 * 1024
    } else {
        SIZE - 48 * 1024
    };
    let size = seed(&a.work, space, &id, ceiling)?;
    let (_server, mut http, _) = start(a)?;
    http.cookie = fixture.cookie;
    let started = Instant::now();
    let _ = indexed(&mut http, space)?;
    let rebuild = started.elapsed().as_secs_f64() * 1000.0;
    let mut appends = Vec::new();
    for sample in 0..=a.samples {
        let request = format!("perf-thread-append-{sample}");
        let (result, ms) = http.op_id(
            "threads.send",
            json!({"id":id,"message":"Append probe to a near-ceiling thread."}),
            &request,
        )?;
        let job = result["job"]["id"].as_str().ok_or("fixture turn missing")?;
        // Job cancellation is outside the timed request. There is no provider client.
        http.op("agent.stop", json!({"job_id":job}))?;
        if sample != 0 {
            appends.push(ms);
        }
    }
    if a.small_thread || a.legacy_thread_limit {
        let metric = if a.small_thread { SMALL_APPEND } else { "threads.2mib.append" };
        return report(a, size, &[stats::summarize(metric, appends.clone(), &budgets)], &appends);
    }
    let message = indexed(&mut http, space)?;
    let mut tails = Vec::new();
    let mut exact = Vec::new();
    for _ in 0..a.samples {
        let (tail, ms) = http.op("threads.get", json!({"id":id,"tail":true}))?;
        if tail["thread"]["markdown"]
            .as_str()
            .is_none_or(|s| s.len() > 128 * 1024 || !s.contains("Append probe"))
        {
            return Err("tail did not include the latest append within 128 KiB".into());
        }
        tails.push(ms);
        let (opened, ms) = http.op("threads.get", json!({"id":id,"message_id":message}))?;
        if opened["thread"]["messages"][0]["id"] != message {
            return Err("exact read returned a different message".into());
        }
        exact.push(ms);
    }
    let measurements = [
        stats::summarize(APPEND, appends.clone(), &budgets),
        stats::summarize(TAIL, tails, &budgets),
        stats::summarize(EXACT, exact, &budgets),
        stats::summarize(REBUILD, vec![rebuild], &budgets),
    ];
    report(a, size, &measurements, &appends)
}

fn report(a: &Args, size: usize, measurements: &[stats::Summary], append_ms: &[f64]) -> Result<i32> {
    let breached = measurements.iter().any(|m| m.breached);
    let report = json!({"commit":a.commit,"physical_bytes_before_append":size,"measurements":measurements,"append_ms":append_ms,"breached":breached});
    let out = a.work.join("long-thread.json");
    std::fs::write(&out, serde_json::to_vec_pretty(&report)?)?;
    crate::report::print(a, &report, &out);
    Ok(i32::from(breached))
}

fn start(a: &Args) -> Result<(Server, Http, f64)> {
    Server::start(
        &a.bin,
        &a.work,
        &a.plugins,
        a.test_plugin_root.as_deref(),
        a.startup_timeout_seconds,
    )
}

fn indexed(http: &mut Http, space: &str) -> Result<String> {
    let start = Instant::now();
    loop {
        let (result, _) = http.op(
            "search.query",
            json!({"space_id":space,"kinds":["threads"],
            "query":"rarest","limit":1}),
        )?;
        if result["partial"] == false {
            return Ok(result["hits"][0]["message_id"]
                .as_str()
                .ok_or("message index lacks fixture hit")?
                .to_owned());
        }
        if start.elapsed() > Duration::from_secs(90) {
            return Err("single-thread projection did not catch up".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn seed(root: &Path, space: &str, id: &str, threshold: usize) -> Result<usize> {
    let base = root.join("appdata/spaces").join(space);
    // Select an old-format snapshot within a small reserve for repeated writes.
    // Fixture creation is untimed and runs with the disposable server stopped.
    let identity = format!("---\ncalternal-id: {id}\n");
    let mut low = 1;
    let mut high = threshold / 128;
    while low < high {
        let mid = (low + high).div_ceil(2);
        let (_, markdown) = calternal_server::rebuild_probe::message_fixture(0, mid);
        if markdown.len() + identity.len() - 4 > threshold {
            high = mid - 1;
        } else {
            low = mid;
        }
    }
    let (mut metadata, stored) = calternal_server::rebuild_probe::message_fixture(0, low);
    metadata["title"] = json!("Corpus 000000000");
    let placed = format!(
        "{identity}{}",
        stored
            .strip_prefix("---\n")
            .ok_or("invalid fixture envelope")?
    );
    if placed.len() < threshold.saturating_sub(1024) {
        return Err("fixture did not approach the requested size".into());
    }
    let mut db = Connection::open(base.join("content.db"))?;
    let path: String =
        db.query_row("SELECT path FROM items WHERE id=?1", [id], |row| row.get(0))?;
    let full = root.join("userdata-ssd/spaces").join(space).join(path);
    std::fs::write(full, placed.as_bytes())?;
    let tx = db.transaction()?;
    let encoded = metadata.to_string();
    let digest = blake3::hash(placed.as_bytes()).to_hex().to_string();
    tx.execute(
        "UPDATE items SET metadata=?2,mat_state='materialized',file_hash=?3 WHERE id=?1",
        params![id, encoded, digest],
    )?;
    tx.execute(
        "UPDATE versions SET metadata=?2,bytes=?3 WHERE item=?1 AND seq=1",
        params![id, encoded, stored.as_bytes()],
    )?;
    tx.commit()?;
    // Rebuild the message projection for the actual seeded snapshot.
    let index = base.join("search-messages-v1.all.db");
    for path in [
        index.clone(),
        index.with_extension("db-wal"),
        index.with_extension("db-shm"),
    ] {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(placed.len())
}
