//! Measure first-page search over seeded, authorized space content.
use crate::{Args, fixtures, http::Result, server::Server, stats};
use serde_json::json;

pub(crate) fn run(a: &Args) -> Result<i32> {
    let sizes: usize = a.sizes.parse()?;
    let spaces: usize = a.spaces.parse()?;
    if a.samples < 20
        || spaces == 0
        || sizes == 0
        || spaces.checked_mul(sizes).is_none_or(|n| n > 100_000)
    {
        return Err(
            "search measurement needs at least 20 samples and 1..100000 total notes".into(),
        );
    }
    fixtures::private_work(&a.work)?;
    let staging = tempfile::Builder::new()
        .prefix("calternal-search-")
        .tempdir_in("/dev/shm")?;
    let fixture = fixtures::build(a, staging.path(), sizes, spaces)?;
    fixtures::copy(staging.path(), &a.work)?;
    drop(staging);
    let (_server, mut http, _) = Server::start(
        &a.bin,
        &a.work,
        &a.plugins,
        a.test_plugin_root.as_deref(),
        a.startup_timeout_seconds,
    )?;
    http.cookie = fixture.cookie;
    crate::search_stage::wait_ready(&mut http, None)?;
    let budgets = stats::Budgets {
        entries: Vec::new(),
    };
    let mut summaries = Vec::new();
    for (name, query, expect) in [
        ("title_selective", format!("Note {}", sizes - 1), true),
        ("title_common", "Note".into(), true),
        ("body_common", "deterministic".into(), true),
        ("no_match", "zzzzzzzzzzzzzzzzzzzz".into(), false),
    ] {
        let input = json!({"query":query,"limit":20});
        let (result, first) = http.op("search.query", input.clone())?;
        let hits = result["hits"].as_array().ok_or("search omitted hits")?;
        if result["partial"] != false || (!hits.is_empty()) != expect {
            return Err(format!("fixture search class {name} returned unexpected results").into());
        }
        let mut samples = Vec::with_capacity(a.samples);
        for _ in 0..a.samples {
            samples.push(http.op("search.query", input.clone())?.1);
        }
        summaries.push(json!({
            "query":query,"first_ms":first,
            "observations_ms":stats::summarize(name,samples,&budgets),
        }));
    }
    let space = fixture.spaces[0].as_str().ok_or("invalid fixture space")?;
    let placement = crate::materialization::Probe::open(&a.work, space)?;
    let mut revision = fixture.first["revision"].clone();
    let mut samples = Vec::new();
    for i in 0..a.samples {
        let (result, elapsed) = http.op(
            "notes.update",
            json!({
                "id":fixture.first["id"], "base_revision":revision,
                "markdown":format!("# Note 0\n\nSave fixture {i} for the search index.\n"),
            }),
        )?;
        revision = result["note"]["revision"].clone();
        samples.push(elapsed);
        placement.wait(&result["note"])?;
    }
    let report = json!({
        "spaces":spaces,"notes_per_space":sizes,
        "search":summaries,"save_ms":stats::summarize("notes.update",samples,&budgets),
        "commit":a.commit,
    });
    std::fs::write(
        a.work.join("search-summary.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    crate::report::print(a, &report, &a.work);
    Ok(0)
}
