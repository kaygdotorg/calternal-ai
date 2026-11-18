//! Online Notes projection and paired save probes in the passkey matrix.
pub(crate) mod queue;
use crate::{
    http::{Http, Result},
    materialization::Probe,
    server::Server,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};
type Samples = BTreeMap<String, Vec<f64>>;

fn query(http: &mut Http, space: &Value, text: &str) -> Result<Value> {
    let (page, _) = http.op(
        "search.query",
        json!({"space_id":space,"query":text,"kinds":["notes"]}),
    )?;
    if page["partial"].as_bool().is_none() {
        return Err("search response lacks partial".into());
    }
    Ok(page)
}

fn elapsed(at: Instant) -> f64 {
    at.elapsed().as_secs_f64() * 1000.0
}

fn record(samples: &mut Samples, metric: &str, value: f64) {
    samples.entry(metric.into()).or_default().push(value);
}

pub(crate) fn first_use(
    server: &Server,
    http: &mut Http,
    root: &Path,
    space: &Value,
    first: &mut Value,
    samples: &mut Samples,
) -> Result<()> {
    let baseline_rss = server.rss()?;
    let started = Instant::now();
    let initial = query(http, space, "searchstageinitial")?;
    record(
        samples,
        "search.rebuild.initial_partial",
        f64::from(initial["partial"] == true),
    );

    let (listed, list_ms) = http.op("notes.list", json!({"space_id":space}))?;
    if listed["notes"].as_array().is_none_or(Vec::is_empty) {
        return Err("the active space did not list its Notes during rebuild".into());
    }
    record(samples, "search.rebuild.list_ms", list_ms);
    let (_, open_ms) = http.op("notes.get", json!({"id":first["id"]}))?;
    record(samples, "search.rebuild.id_open_ms", open_ms);
    let (updated, save_ms) = http.op(
        "notes.update",
        json!({
            "id":first["id"], "base_revision":first["revision"], "markdown":"searchstageinitial",
        }),
    )?;
    *first = updated["note"].clone();
    record(samples, "search.rebuild.save_ms", save_ms);
    record(samples, "search.rebuild.initial_rss_mib", baseline_rss);
    Probe::open(root, space.as_str().ok_or("invalid space")?)?.wait(first)?;

    let deadline = started + Duration::from_secs(240);
    let mut peak_rss = baseline_rss;
    loop {
        let page = query(http, space, "searchstageinitial")?;
        peak_rss = peak_rss.max(server.rss()?);
        if page["partial"] == false {
            record(samples, "search.rebuild.ready_ms", elapsed(started));
            record(samples, "search.rebuild.peak_rss_mib", peak_rss);
            record(
                samples,
                "search.rebuild.index_on",
                f64::from(page["hits"].as_array().is_some_and(|hits| !hits.is_empty())),
            );
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("online search rebuild exceeded the harness timeout".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(crate) fn lag(
    http: &mut Http,
    placement: &Probe,
    space: &Value,
    first: &mut Value,
    samples: &mut Samples,
) -> Result<()> {
    if samples["search.rebuild.index_on"][0] == 0.0 {
        return Ok(());
    }
    for i in 0..20 {
        let term = format!("searchstagelag{i}zz");
        let (updated, _) = http.op(
            "notes.update",
            json!({
                "id":first["id"], "base_revision":first["revision"], "markdown":term,
            }),
        )?;
        *first = updated["note"].clone();
        let ack = Instant::now();
        loop {
            let page = query(http, space, &term)?;
            if page["hits"].as_array().is_some_and(|hits| !hits.is_empty()) {
                record(samples, "search.ack_to_index_ms", elapsed(ack));
                break;
            }
            if ack.elapsed() >= Duration::from_secs(3) {
                return Err("search did not catch up after save".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        placement.wait(first)?;
    }
    Ok(())
}
