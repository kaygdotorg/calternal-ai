//! Compare staged Markdown-origin projection with current synchronous first access.
mod admission;
mod files;

use crate::{
    Args, fixtures,
    http::{Http, Result},
    server::Server,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Barrier,
    time::{Duration, Instant},
};

const THREADS: usize = 32;

pub(crate) fn run(a: &Args) -> Result<i32> {
    let source = a
        .rebuild_source
        .as_deref()
        .ok_or("--rebuild-source needs a stopped, single-space --search-only fixture")?;
    if !source.join("search-summary.json").is_file() || a.work.exists() {
        return Err("use a stopped, generated search fixture and a fresh --work directory".into());
    }
    fixtures::private_work(&a.work)?;
    let template = a.work.join("template");
    fixtures::copy(source, &template)?;
    let (space, first) = prepare(a, &template)?;
    let mut observations = Vec::new();
    for size in a.sizes.split(',').map(str::parse::<usize>) {
        let size = size?;
        if !(1..=100_000).contains(&size) {
            return Err("rebuild sizes must be 1..100000".into());
        }
        for cold in [true, false] {
            let name = if cold { "cold" } else { "warm" };
            let root = a.work.join(format!("{size}-{name}"));
            fixtures::copy(&template, &root)?;
            let db = content_path(&root, &space);
            reset(&db)?;
            let userdata = root.join("userdata-ssd/spaces").join(&space);
            let routes = root.join("appdata/system/content-index.db");
            let docs = files::select(&db, &userdata, &routes, &space, size)?;
            if docs.iter().filter(|doc| doc.kind == "notes").count() != size
                || docs.iter().filter(|doc| doc.kind == "threads").count() != THREADS
            {
                return Err("unexpected note or thread count in rebuild fixture".into());
            }
            let jobs = db.with_file_name("jobs.db");
            if jobs.is_file() {
                Connection::open(&jobs)?.execute_batch("DROP INDEX IF EXISTS jobs_thread;")?;
            }
            before_trial(cold)?;
            if !cold {
                files::warm(&userdata, &db, &docs)?;
            }
            let (file_measure, ()) =
                footprint(std::process::id(), || files::stage(&userdata, &db, &docs))?;
            before_trial(cold)?;
            let (server, mut http, startup_ms) = dev_server(a, &root)?;
            let baseline = server.rss()?;
            let (runtime, results) =
                footprint(server.pid(), || measure_open(&mut http, &space, &first))?;
            observations.push(json!({
                "notes":size,"threads":THREADS,"cache":name,"file_stage":file_measure,
                "server_startup_ms":startup_ms,"server_baseline_rss_mib":baseline,"server_peak_rss_mib":runtime["peak_rss_mib"],
                "server_first_requests":results,
            }));
            drop(server);
            fixtures::remove_roots(&root)?;
            eprintln!("rebuild fixture {size} {name}: complete");
        }
    }
    fixtures::remove_roots(&template)?;
    let concurrency = if a.rebuild_no_admission {
        Vec::new()
    } else {
        admission::run(a)?
    };
    let report = json!({
        "commit":a.commit,"case":"file-origin staging and actual first-use reconstruction",
        "observations":observations,"admission":concurrency,
        "limitations":"The file stage rewrites retained fixture heads and current revision bytes offline. The server then rebuilds the current derived projections under the normal read-slot policy. This measures both stages, without providing an upgrade workflow or file-only install recovery."
    });
    let output = a.work.join("rebuild-summary.json");
    std::fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    crate::report::print(a, &report, &output);
    Ok(0)
}

fn prepare(a: &Args, root: &Path) -> Result<(String, String)> {
    let authority = root.join("appdata/system/auth.db");
    let space: String = Connection::open(&authority)?.query_row(
        "SELECT id FROM spaces ORDER BY created,id LIMIT 1",
        [],
        |r| r.get(0),
    )?;
    // Provision the normal development identity first. Grant it access only in
    // this stopped disposable copy. Never use the originating fixture's secret.
    let (server, _, _) = dev_server(a, root)?;
    drop(server);
    Connection::open(&authority)?.execute(
        "INSERT INTO space_memberships(space_id,account_id,role)
         SELECT ?1,id,'manager' FROM development_directory WHERE name='owner'",
        [&space],
    )?;
    let (server, mut http, _) = dev_server(a, root)?;
    let (first, _) = http.op("notes.list", json!({"space_id":space}))?;
    if first["notes"].as_array().is_none_or(Vec::is_empty) {
        return Err("the source space contains no notes".into());
    }
    let db = content_path(root, &space);
    let first_id: String = Connection::open(&db)?.query_row(
        "SELECT id FROM items WHERE kind='notes' ORDER BY rowid LIMIT 1",
        [],
        |r| r.get(0),
    )?;
    for i in 0..THREADS {
        http.op(
            "threads.create",
            json!({"space_id":space,"title":format!("Rebuild thread {i}")}),
        )?;
    }
    http.op("threads.list", json!({"space_id":space}))?;
    // Wait for the deferred placement of the last acknowledged thread.
    wait_files(&db)?;
    drop(server);
    Ok((space, first_id))
}

fn wait_files(db: &Path) -> Result<()> {
    let db = Connection::open(db)?;
    let until = Instant::now() + Duration::from_secs(4);
    loop {
        let pending: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM items WHERE mat_state='pending')",
            [],
            |r| r.get(0),
        )?;
        if !pending {
            return Ok(());
        }
        if Instant::now() >= until {
            return Err("benchmark fixture still has pending userdata after four seconds".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn dev_server(a: &Args, root: &Path) -> Result<(Server, Http, f64)> {
    let (server, mut http, started) =
        Server::start_development(&a.bin, root, &a.plugins, a.startup_timeout_seconds)?;
    http.bearer = Some(
        std::fs::read_to_string(root.join("appdata/dev-tokens/owner.token"))?
            .trim()
            .to_owned(),
    );
    Ok((server, http, started))
}

fn content_path(root: &Path, space: &str) -> PathBuf {
    root.join("appdata/spaces").join(space).join("content.db")
}

fn reset(path: &Path) -> Result<()> {
    for file in ["search-v2.db", "search-v2.db-wal", "search-v2.db-shm"] {
        match std::fs::remove_file(path.with_file_name(file)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Connection::open(path)?.execute_batch(
        "DROP TRIGGER IF EXISTS search_version;
         DROP TRIGGER IF EXISTS item_terms_insert;
         DROP TRIGGER IF EXISTS item_terms_update;
         DROP VIEW IF EXISTS search_texts;
         DROP VIEW IF EXISTS search_heads;
         DROP TABLE IF EXISTS search_docs;
         DROP TABLE IF EXISTS search_text;
         DROP TABLE IF EXISTS search_title_grams;
         DROP TABLE IF EXISTS search_configuration;
         DROP TABLE IF EXISTS item_terms;
         DROP INDEX IF EXISTS item_list;
         DROP INDEX IF EXISTS item_live_list;
         DROP INDEX IF EXISTS item_title_key;
         DROP INDEX IF EXISTS item_trash;
         DROP INDEX IF EXISTS item_trash_page;",
    )?;
    Ok(())
}

fn before_trial(cold: bool) -> Result<()> {
    let ok = if cold {
        Command::new("sudo")
            .args(["-n", "/usr/local/sbin/perf-drop-caches"])
            .status()?
            .success()
    } else {
        Command::new("sync").status()?.success()
    };
    if !ok {
        return Err("could not prepare cache state under the perf lock".into());
    }
    Ok(())
}

fn measure_open(http: &mut Http, space: &str, first: &str) -> Result<Value> {
    let mut list = http.peer();
    let mut open = http.peer();
    let start = Barrier::new(2);
    let (list_ms, get_ms) = std::thread::scope(|scope| -> Result<(f64, f64)> {
        let pending_list = scope.spawn(|| {
            start.wait();
            list.op("notes.list", json!({"space_id":space}))
                .and_then(|(v, ms)| {
                    if v["notes"].as_array().is_none_or(Vec::is_empty) {
                        return Err("missing rebuilt note list".into());
                    }
                    Ok(ms)
                })
                .map_err(|e| e.to_string())
        });
        start.wait();
        std::thread::sleep(Duration::from_millis(5));
        let (v, get_ms) = open.op("notes.get", json!({"id":first}))?;
        if v["note"]["id"] != first {
            return Err("item opening changed identity".into());
        }
        Ok((
            pending_list.join().map_err(|_| "first list panicked")??,
            get_ms,
        ))
    })?;
    let (tags, tags_ms) = http.op("tags.list", json!({"space_id":space}))?;
    if !tags["tags"]
        .as_array()
        .is_some_and(|rows| rows.iter().any(|tag| tag["name"] == "perf"))
    {
        return Err("rebuilt tag index did not serve fixture tags".into());
    }
    let (threads, threads_ms) = http.op("threads.list", json!({"space_id":space}))?;
    if threads["threads"].as_array().map(Vec::len) != Some(THREADS) {
        return Err("rebuilt thread listing missed fixture threads".into());
    }
    let search_started = Instant::now();
    crate::search_stage::wait_ready(http, Some(&json!(space)))?;
    let search_ready_ms = search_started.elapsed().as_secs_f64() * 1000.;
    let (search, search_ms) = http.op(
        "search.query",
        json!({"space_id":space,"query":"deterministic"}),
    )?;
    if search["hits"].as_array().is_none_or(Vec::is_empty) {
        return Err("rebuilt search index did not serve fixture text".into());
    }
    Ok(json!({
        "notes_list_ms":list_ms,"same_space_open_by_id_ms_started_5ms_later":get_ms,
        "initialized_tags_list_ms":tags_ms,"first_threads_list_ms":threads_ms,
        "initialized_search_ms":search_ms,"search_ready_ms":search_ready_ms,
    }))
}

fn rss(pid: u32) -> f64 {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                line.strip_prefix("VmRSS:")
                    .and_then(|v| v.split_whitespace().next())
                    .and_then(|kb| kb.parse::<f64>().ok())
            })
        })
        .map_or(0., |kb| kb / 1024.)
}

fn footprint<T>(pid: u32, work: impl FnOnce() -> Result<T>) -> Result<(Value, T)> {
    let stop = std::sync::atomic::AtomicBool::new(false);
    let before = rss(pid);
    let start = Instant::now();
    let (peak, elapsed, result) = std::thread::scope(|scope| {
        let sampler = scope.spawn(|| {
            let mut peak = before;
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                peak = peak.max(rss(pid));
                std::thread::sleep(Duration::from_millis(2));
            }
            peak.max(rss(pid))
        });
        let result = work();
        let elapsed = start.elapsed().as_secs_f64() * 1000.;
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        (sampler.join().expect("RSS sampler"), elapsed, result)
    });
    Ok((
        json!({"elapsed_ms":elapsed,"baseline_rss_mib":before,"peak_rss_mib":peak}),
        result?,
    ))
}
