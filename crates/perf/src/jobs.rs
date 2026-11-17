//! Measure the existing, digest-pinned conformance worker's real job stream.
use crate::{Args, fixtures, http::Result, live::Live, server::Server, stats};
use serde_json::json;
use std::time::Instant;
pub(crate) fn run(a: &Args) -> Result<i32> {
    if a.test_plugin_root.is_none() {
        return Err("jobs require --test-plugin-root and a test-plugins server build".into());
    }
    if a.samples < 20 {
        return Err("jobs require at least twenty samples".into());
    }
    let staging = tempfile::Builder::new()
        .prefix("calternal-perf-jobs-")
        .tempdir_in("/dev/shm")?;
    let fixture = fixtures::build(a, staging.path(), 100, 1)?;
    if a.work.exists() {
        return Err("job work directory already exists".into());
    }
    fixtures::copy(staging.path(), &a.work)?;
    drop(staging);
    let (server, mut h, _) = Server::start(
        &a.bin,
        &a.work,
        &a.plugins,
        a.test_plugin_root.as_deref(),
        a.startup_timeout_seconds,
    )?;
    h.cookie = fixture.cookie;
    let space = fixture.spaces[0].clone();
    h.op(
        "plugins.enable",
        json!({"space_id":space,"plugin_id":"jobs-demo"}),
    )?;
    let mut samples = Vec::new();
    for _ in 0..a.samples {
        let started = Instant::now();
        // No artificial delay or paid provider; every=1000 keeps checkpoint_seq
        // at zero so a fast completed job still exposes the buffered first chunk.
        let (job, _) = h.op(
            "jobs-demo.start",
            json!({"space_id":space,"steps":1,"delay_ms":0,"every":1000}),
        )?;
        let id = job["job_id"].as_str().ok_or("job ID missing")?;
        let mut stream = Live::open(&h, &format!("/api/v1/jobs/{id}/events"), &[])?;
        let mut seen = false;
        for _ in 0..10 {
            let value = stream.next()?;
            if value.get("seq").is_some() {
                if value["seq"] != 0 || value["data"]["step"] != 0 {
                    return Err("first chunk did not match the fixture".into());
                }
                samples.push(started.elapsed().as_secs_f64() * 1000.);
                seen = true;
                break;
            }
        }
        if !seen {
            return Err("first stream chunk was not observed".into());
        }
    }
    let budgets: stats::Budgets = serde_json::from_slice(&std::fs::read(&a.budgets)?)?;
    std::fs::write(a.work.join("raw.json"), serde_json::to_vec(&samples)?)?;
    let cold = samples[0];
    let summary = stats::summarize("job.first_chunk", samples[1..].to_vec(), &budgets);
    let report = json!({"commit":a.commit,"worker":"tests/plugins/jobs-demo","provider":"none","delay_ms":0,"summary":summary,"cold_first_ms":cold,"qualification":"fixture stream only; not provider startup and not a three-host-call operation"});
    std::fs::write(
        a.work.join("jobs-summary.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    drop(server);
    fixtures::remove_roots(&a.work)?;
    crate::report::print(a, &report, &a.work.join("jobs-summary.json"));
    Ok(2) // One run does not qualify repeated baseline/head regressions.
}
