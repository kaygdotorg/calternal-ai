//! Trace one warmed update separately; instrumented latency never enters budgets.
use crate::{Args, fixtures, http::Result, materialization::Probe, server::Server};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

pub(crate) fn run(a: &Args) -> Result<i32> {
    if a.work.exists() {
        return Err("choose a fresh fsync probe directory".into());
    }
    std::fs::create_dir_all(&a.work)?;
    let fixture = fixtures::build(a, &a.work, 1, 1)?;
    let (server, mut http, _) =
        Server::start(&a.bin, &a.work, &a.plugins, None, a.startup_timeout_seconds)?;
    http.cookie = fixture.cookie.clone();
    http.op("notes.list", json!({"space_id":fixture.spaces[0]}))?;
    let probe = Probe::open(&a.work, fixture.spaces[0].as_str().ok_or("space")?)?;
    probe.wait(&fixture.first)?;
    // A real untraced write establishes the journal WAL directory entry. Merely
    // listing would make the traced request the first journal write after open.
    let (warm, _) = http.op_id("notes.update", json!({"id":fixture.first["id"],"base_revision":fixture.first["revision"],"markdown":"untraced warmup"}), "fsync-probe-warmup")?;
    probe.wait(&warm["note"])?;
    let head = &warm["note"];
    // Let startup witness work settle; its later update is counted separately.
    std::thread::sleep(Duration::from_millis(1200));
    let tools = PathBuf::from(std::env::var("HOME")?).join("calternal-ai-perf/tools/extracted");
    let trace_path = a.work.join("fsync");
    let errors = a.work.join("strace.stderr");
    let mut trace = Command::new("sudo")
        .args(["-n", "env"])
        .arg(format!(
            "LD_LIBRARY_PATH={}",
            tools.join("usr/lib/x86_64-linux-gnu").display()
        ))
        .args(["timeout", "-s", "INT", "4"])
        .arg(tools.join("usr/bin/strace"))
        .args([
            "-ff",
            "-ttt",
            "-T",
            "-yy",
            "-e",
            "trace=fsync,fdatasync",
            "-p",
        ])
        .arg(server.pid().to_string())
        .arg("-o")
        .arg(&trace_path)
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&errors)?)
        .spawn()?;
    std::thread::sleep(Duration::from_millis(300));
    if trace.try_wait()?.is_some() || !std::fs::read_to_string(&errors)?.contains("attached") {
        return Err("strace did not attach; inspect strace.stderr".into());
    }
    let started = calternal_types::now_ms();
    let (saved, ack_ms) = http.op_id(
        "notes.update",
        json!({"id":head["id"],"base_revision":head["revision"],"markdown":"one traced update"}),
        "fsync-probe-update",
    )?;
    let acknowledged = calternal_types::now_ms();
    probe.wait(&saved["note"])?;
    let placed = calternal_types::now_ms();
    let status = trace.wait()?;
    if status.code() != Some(124) {
        return Err("strace timeout did not complete as expected".into());
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for entry in std::fs::read_dir(&a.work)? {
        let entry = entry?;
        if !entry.file_name().to_string_lossy().starts_with("fsync.") {
            continue;
        }
        for line in std::fs::read_to_string(entry.path())?.lines() {
            let call = line
                .split_once("fsync(")
                .or_else(|| line.split_once("fdatasync("));
            let Some((_, call)) = call else { continue };
            if !call.contains("= 0") {
                return Err(format!("unsuccessful sync in {}", entry.path().display()).into());
            }
            let path = call
                .split_once('<')
                .and_then(|(_, v)| v.split_once('>').map(|(p, _)| p))
                .ok_or("missing sync descriptor path")?;
            let kind = if path.ends_with("content.db-wal") {
                "content_wal"
            } else if path.ends_with("index.db-wal") {
                "journal_wal"
            } else if path.contains("userdata-ssd/") && path.ends_with("/Notes") {
                "userdata_directory"
            } else if path.contains("userdata-ssd/") {
                "userdata_file"
            } else {
                "other_including_witness"
            };
            *counts.entry(kind.into()).or_default() += 1;
        }
    }
    let report = json!({"commit":a.commit,"binary":a.bin,"instrumented":true,"updates":1,"sync_calls":counts,"request_started_ms":started,"acknowledged_ms":acknowledged,"file_observed_durable_ms":placed,"instrumented_ack_ms":ack_ms,"trace":trace_path,"qualification":"single warmed same-title update; witness/checkpoints counted separately; not latency acceptance"});
    let path = a.work.join("fsync-summary.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    drop(server);
    fixtures::remove_roots(&a.work)?;
    crate::report::print(a, &report, &path);
    Ok(0)
}
