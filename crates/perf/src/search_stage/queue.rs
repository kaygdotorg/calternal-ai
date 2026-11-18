//! Foreground admission while one or two background rebuilds are pending.
use super::{elapsed, query};
use crate::{Args, fixtures, http::Result, materialization::Probe, server::Server};
use serde_json::{Value, json};
use std::{
    fs,
    time::{Duration, Instant},
};

pub(crate) fn run(args: &Args) -> Result<i32> {
    fs::create_dir(&args.work)?;
    let fixture_root = tempfile::Builder::new()
        .prefix("notes-search-queue-")
        .tempdir_in("/dev/shm")?;
    let fixture = fixtures::build(args, fixture_root.path(), 10_000, 3)?;
    let mut observations = Vec::new();
    for count in 0..=2 {
        let root = args.work.join(format!("pending-{count}"));
        fixtures::copy(fixture_root.path(), &root)?;
        fixtures::reset_search_catalog(&root)?;
        if !std::process::Command::new("sync").status()?.success() {
            return Err("could not settle the copied fixture".into());
        }
        let (server, mut http, _) = Server::start(
            &args.bin,
            &root,
            &args.plugins,
            args.test_plugin_root.as_deref(),
            args.startup_timeout_seconds,
        )?;
        http.cookie = fixture.cookie.clone();
        let healthy = &fixture.spaces[2];
        let (listed, _) = http.op("notes.list", json!({"space_id":healthy}))?;
        let mut first = listed["notes"][0].clone();
        let placement = Probe::open(&root, healthy.as_str().ok_or("invalid fixture space")?)?;
        wait_ready(&mut http, healthy, Instant::now())?;

        let selected = &fixture.spaces[..count];
        let started = Instant::now();
        let mut ready_at = vec![None; count];
        for space in selected {
            if query(&mut http, space, "searchqueuesentinel")?["partial"] != true {
                return Err("a cold fixture did not schedule its background rebuild".into());
            }
        }
        let before_rss = server.rss()?;
        let mut peak_rss = before_rss;
        let mut requests = Vec::new();
        for i in 0..12 {
            update_readiness(&mut http, selected, started, &mut ready_at)?;
            let pending = ready_at.iter().filter(|mark| mark.is_none()).count();
            let (_, list_ms) = http.op("notes.list", json!({"space_id":healthy}))?;
            let (_, id_ms) = http.op("notes.get", json!({"id":first["id"]}))?;
            let (updated, save_ms) = http.op(
                "notes.update",
                json!({"id":first["id"],"base_revision":first["revision"],"markdown":format!("search queue validation {i}")}),
            )?;
            first = updated["note"].clone();
            placement.wait(&first)?;
            requests.push(json!({
                "pending_at_start":pending,"list_ms":list_ms,"id_ms":id_ms,"save_ms":save_ms
            }));
            peak_rss = peak_rss.max(server.rss()?);
        }
        while ready_at.iter().any(Option::is_none) {
            if started.elapsed() >= Duration::from_secs(120) {
                return Err("queued projections did not catch up".into());
            }
            update_readiness(&mut http, selected, started, &mut ready_at)?;
            peak_rss = peak_rss.max(server.rss()?);
            std::thread::sleep(Duration::from_millis(20));
        }
        observations.push(json!({
            "queued":count,"notes_per_space":10_000,"healthy_requests":requests,
            "ready_ms":ready_at,"baseline_rss_mib":before_rss,"peak_rss_mib":peak_rss
        }));
        drop(server);
        fixtures::remove_roots(&root)?;
        eprintln!("online search: {count} queued rebuilds finished");
    }
    let report = json!({"commit":args.commit,"observations":observations});
    let output = args.work.join("summary.json");
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    crate::report::print(args, &report, &output);
    Ok(0)
}

fn wait_ready(http: &mut crate::http::Http, space: &Value, start: Instant) -> Result<()> {
    loop {
        if query(http, space, "searchqueuesentinel")?["partial"] == false {
            return Ok(());
        }
        if start.elapsed() >= Duration::from_secs(120) {
            return Err("healthy space did not become searchable".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn update_readiness(
    http: &mut crate::http::Http,
    selected: &[Value],
    started: Instant,
    ready: &mut [Option<f64>],
) -> Result<()> {
    for (space, mark) in selected.iter().zip(ready.iter_mut()) {
        if mark.is_none() && query(http, space, "searchqueuesentinel")?["partial"] == false {
            *mark = Some(elapsed(started));
        }
    }
    Ok(())
}
