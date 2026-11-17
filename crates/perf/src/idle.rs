//! Separate RSS qualification after the production 600-second idle window and sweep.
use crate::{Args, fixtures, http::Result, server::Server};
use serde_json::json;
use std::time::{Duration, Instant};
pub(crate) fn run(a: &Args) -> Result<i32> {
    std::fs::create_dir_all(&a.work)?;
    let budgets: crate::stats::Budgets = serde_json::from_slice(&std::fs::read(&a.budgets)?)?;
    let mut servers = Vec::new();
    let cases: Vec<usize> = a
        .spaces
        .split(',')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()?;
    if cases.is_empty() || cases.len() > 3 || cases.iter().any(|n| ![1, 100, 1000].contains(n)) {
        return Err("idle spaces must select from 1,100,1000".into());
    }
    for spaces in cases.iter().copied() {
        let staging = tempfile::Builder::new()
            .prefix("calternal-perf-idle-")
            .tempdir_in("/dev/shm")?;
        let fixture = fixtures::build(a, staging.path(), 100, spaces)?;
        for repeat in 0..3 {
            let root = a.work.join(format!("{spaces}-spaces-{repeat}"));
            if root.exists() {
                return Err("idle roots already exist; choose fresh --work".into());
            }
            fixtures::copy(staging.path(), &root)?;
            let (server, mut h, _) = Server::start(
                &a.bin,
                &root,
                &a.plugins,
                a.test_plugin_root.as_deref(),
                a.startup_timeout_seconds,
            )?;
            h.cookie = fixture.cookie.clone();
            for space in &fixture.spaces {
                h.op("notes.list", json!({"space_id":space}))?;
            }
            let before = server.rss()?;
            let writers = server.writers()?;
            servers.push((spaces, repeat, root, server, before, writers));
        }
        drop(staging);
    }
    // All subscriptions and HTTP clients were dropped. No latency comparisons run
    // here: independent servers share one idle observation interval.
    let server_count = servers.len();
    let idle = Instant::now();
    std::thread::sleep(Duration::from_secs(632));
    let mut rows = Vec::new();
    let mut breaches = std::collections::BTreeMap::<usize, usize>::new();
    for (spaces, repeat, root, server, before, writers) in servers {
        let after = server.rss()?;
        let remaining = server.writers()?;
        let metric = format!("memory.{spaces}_spaces.idle_released");
        let limit = budgets
            .entries
            .iter()
            .find(|b| b.metric == metric)
            .and_then(|b| b.budget)
            .ok_or("missing idle memory budget")?;
        if after > limit || remaining != 0 {
            *breaches.entry(spaces).or_default() += 1;
        }
        rows.push(json!({"spaces":spaces,"repeat":repeat,"notes_per_space":100,"before_mib":before,"after_mib":after,"writers_before":writers,"writers_after":remaining,"released":remaining==0,"budget_mib":limit,"idle_seconds":idle.elapsed().as_secs_f64()}));
        drop(server);
        fixtures::remove_roots(&root)?;
    }
    let breach = breaches.values().any(|n| *n >= 2);
    let report = json!({"commit":a.commit,"servers":server_count,"space_cases":cases,"simultaneous_idle_only":true,"reproduced":breach,"observations":rows,"breach":breach,"limitation":"three independent servers per size idle concurrently; no latency samples are taken here"});
    std::fs::write(
        a.work.join("idle-summary.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    crate::report::print(a, &report, &a.work.join("idle-summary.json"));
    Ok(if breach { 1 } else { 2 })
}
