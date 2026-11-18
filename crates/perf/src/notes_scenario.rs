//! HTTP Notes, live, draft and durable-file measurements for one arm.
use crate::{Args, fixtures, http::Result, live, materialization, search_stage, server, timings};
use serde_json::json;
use std::{collections::BTreeMap, path::Path, time::Duration};

/// Completed samples and an independent file placement verdict.
pub(super) struct ScenarioObservations {
    pub(super) samples: BTreeMap<String, Vec<f64>>,
    pub(super) placement_breach: bool,
}

/// Measure one disposable Notes arm on the real server and its two SSE feeds.
pub(super) fn run(
    a: &Args,
    bin: &Path,
    plugins: &Path,
    root: &Path,
    fixture: &fixtures::Fixture,
    require_commit_timings: bool,
    placement_budget: Duration,
) -> Result<ScenarioObservations> {
    let (server, mut h, start) = server::Server::start(
        bin,
        root,
        plugins,
        a.test_plugin_root.as_deref(),
        a.startup_timeout_seconds,
    )?;
    h.cookie = fixture.cookie.clone();
    let space = fixture.spaces[0].clone();
    let mut first = fixture.first.clone();
    let mut samples: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut deliveries = BTreeMap::new();
    samples.insert(
        format!("startup.{}_spaces", fixture.spaces.len()),
        vec![start],
    );
    // External read-only evidence: startup must leave all space writers unopened
    // before the first list can qualify as a genuinely cold-space activation.
    std::fs::write(
        root.join("startup-state.json"),
        serde_json::to_vec_pretty(
            &json!({"space_writer_threads_before_first_use":server.writers()?}),
        )?,
    )?;
    let (_, ms) = h.op("notes.list", json!({"space_id":space}))?;
    samples.insert("space.process_cold_open".into(), vec![ms]);
    if a.startup_only {
        return Ok(ScenarioObservations {
            samples,
            placement_breach: false,
        });
    }
    if a.paired_search {
        search_stage::first_use(&server, &mut h, root, &space, &mut first, &mut samples)?;
        let enabled = samples["search.rebuild.index_on"][0] == 1.0;
        if enabled != (plugins == a.plugins) {
            return Err("search opt-in did not match the paired arm".into());
        }
    }
    let mut live = live::Live::open(
        &h,
        "/api/v1/live",
        &[(
            "subscriptions".into(),
            json!([{"space_id":space,"bookmark":null}]).to_string(),
        )],
    )?;
    live.wait_type("ready")?;
    let mut placement =
        materialization::Probe::open(root, space.as_str().ok_or("invalid fixture space")?)?;
    let mut account = live::Live::open(&h, "/api/v1/account/live", &[])?;
    account.wait_type("ready")?;
    let mut placement_breach = false;
    for i in 0..a.samples {
        let (_, _, ms) = h.request("/api/v1/auth/me", None)?;
        samples.entry("auth.session".into()).or_default().push(ms);
        let (_, ms) = h.op("notes.get", json!({"id":first["id"]}))?;
        samples.entry("notes.get".into()).or_default().push(ms);
        let (_, ms) = h.op("notes.list", json!({"space_id":space}))?;
        samples
            .entry("notes.list.first100".into())
            .or_default()
            .push(ms);
        let (_, ms) = h.op("resolve", json!({"kind":"notes","id":first["id"]}))?;
        samples.entry("resolve".into()).or_default().push(ms);
        let input = json!({"id":first["id"],"base_revision":first["revision"],"markdown":format!("seed {} save {i}",a.seed)});
        let rid = format!("timed-save-{i}");
        let delivery = std::time::Instant::now();
        // Read the subscription concurrently: HTTP acknowledgement may follow delivery.
        let placed_by = &mut placement;
        let (updated, ms, placed, event, delivery_ms, received_at_ns) =
            std::thread::scope(|scope| -> Result<_> {
                let write = scope.spawn(|| -> std::result::Result<_, String> {
                    let placement = placed_by;
                    let (updated, ms) = h
                        .op_id("notes.update", input.clone(), &rid)
                        .map_err(|e| format!("update failed: {e}"))?;
                    let acknowledged = std::time::Instant::now();
                    let placed = placement
                        .observe(&updated["note"], acknowledged, placement_budget)
                        .map_err(|e| format!("file placement probe failed: {e}"))?;
                    Ok((updated, ms, placed))
                });
                let event = live.wait_type("entry")?;
                let received_at_ns = calternal_types::perf_monotonic_ns();
                let elapsed = delivery.elapsed().as_secs_f64() * 1000.;
                let (updated, ms, placed) = write.join().map_err(|_| "write thread panicked")??;
                Ok((updated, ms, placed, event, elapsed, received_at_ns))
            })?;
        deliveries.insert(rid.clone(), received_at_ns);
        samples.entry("notes.update".into()).or_default().push(ms);
        if event["entry"]["item"] != first["id"] || event["entry"]["request_id"] != rid {
            return Err("live event did not correlate with request and note".into());
        }
        samples
            .entry("live.update_to_client".into())
            .or_default()
            .push(delivery_ms);
        samples
            .entry(materialization::METRIC.into())
            .or_default()
            .push(placed.elapsed.as_secs_f64() * 1000.);
        if placed.breached {
            placement_breach = true;
            break;
        }
        first = updated["note"].clone();
        let (_, ms) = h.op_id("notes.update", input, &rid)?;
        samples
            .entry("receipt.round_trip".into())
            .or_default()
            .push(ms);
    }
    timings::collect(
        &root.join("server.log"),
        &deliveries,
        &mut samples,
        require_commit_timings,
    )?;
    if a.commit_timing_only {
        samples.retain(|metric, _| {
            metric == timings::COMMIT_TO_CLIENT || metric == materialization::METRIC
        });
    }
    if a.commit_timing_only || placement_breach {
        return Ok(ScenarioObservations {
            samples,
            placement_breach,
        });
    }
    for i in 0..a.samples {
        let t = std::time::Instant::now();
        let (_, save_ack_ms) = h.op(
            "drafts.save",
            json!({"space_id":space,"item_id":first["id"],"text":format!("draft {i}")}),
        )?;
        samples
            .entry("account.draft_save_ack".into())
            .or_default()
            .push(save_ack_ms);
        account.wait_type("changed")?;
        samples
            .entry("account.draft_write_to_feed".into())
            .or_default()
            .push(t.elapsed().as_secs_f64() * 1000.);
    }
    if a.paired_search {
        search_stage::lag(&mut h, &placement, &space, &mut first, &mut samples)?;
    }
    drop(live);
    drop(account);
    for space in &fixture.spaces {
        h.op("notes.list", json!({"space_id":space}))?;
    }
    std::thread::sleep(Duration::from_secs(2));
    samples.insert(
        format!("memory.{}_spaces.after_2s", fixture.spaces.len()),
        vec![server.rss()?],
    );
    Ok(ScenarioObservations {
        samples,
        placement_breach,
    })
}
