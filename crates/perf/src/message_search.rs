//! Gate the wired search request over the same framed corpus as the layout probe.
use crate::{
    Args, fixtures,
    http::{Http, Result},
    rebuild::before_trial,
    server::Server,
    stats,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::Path,
    time::{Duration, Instant},
};
use uuid::Uuid;

const COMMON: &str = "search.messages.common.warm";
const RARE: &str = "search.messages.rare.warm";
const PHRASE: &str = "search.messages.phrase.warm";
const COLD: &str = "search.messages.phrase.cold";
const CAPACITY: usize = 1000;

pub(crate) fn run(a: &Args) -> Result<i32> {
    let count: usize = a.sizes.parse()?;
    if !(500_000..=10_000_000).contains(&count) || a.samples < 20 || a.cold_pairs == 0 {
        return Err(
            "message search needs 500k–10M records, at least 20 warm probes and a cold sample"
                .into(),
        );
    }
    let budgets: stats::Budgets = serde_json::from_slice(&std::fs::read(&a.budgets)?)?;
    for name in [COMMON, RARE, PHRASE, COLD] {
        if !budgets.entries.iter().any(|e| {
            e.metric == name && e.budget.is_some() && matches!(e.percentile, stats::Percentile::P95)
        }) {
            return Err(format!("missing accepted p95 budget for {name}").into());
        }
    }
    fixtures::private_work(&a.work)?;
    let fixture = fixtures::build(a, &a.work, 0, 1)?;
    let space = fixture.spaces[0].as_str().ok_or("missing fixture space")?;
    {
        let (_server, mut http, _) = Server::start(
            &a.bin,
            &a.work,
            &a.plugins,
            a.test_plugin_root.as_deref(),
            a.startup_timeout_seconds,
        )?;
        http.cookie = fixture.cookie.clone();
        http.op(
            "threads.create",
            json!({"space_id":space,"title":"Seed journal"}),
        )?;
    }
    let source_bytes = seed(&a.work, space, count)?;
    before_trial(true)?;
    let (_server, mut http, _) = Server::start(
        &a.bin,
        &a.work,
        &a.plugins,
        a.test_plugin_root.as_deref(),
        a.startup_timeout_seconds,
    )?;
    http.cookie = fixture.cookie;
    let needle = json!({"space_id":space,"kinds":["threads"],"query":"rarest","limit":50});
    let started = Instant::now();
    let initial_partial = http.op("search.query", needle.clone())?.0["partial"] == true;
    while http.op("search.query", needle.clone())?.0["partial"] == true {
        if started.elapsed() > Duration::from_secs(900) {
            return Err("message generation did not catch up in 15 minutes".into());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let rebuild_s = started.elapsed().as_secs_f64();
    let mut metrics = Vec::new();
    let mut open_ms = Vec::new();
    let mut continue_ms = None;
    for (name, query) in [
        (COMMON, "canvas"),
        (RARE, "rarest"),
        (PHRASE, "\"canvas readyx\""),
    ] {
        let input = json!({"space_id":space,"kinds":["threads"],"query":query,"limit":50});
        let (first, _) = measured(&mut http, input.clone())?;
        let mut expected: usize = count;
        let stride = match name {
            COMMON => 2,
            RARE => 8191,
            _ => 64,
        };
        for hit in first["hits"].as_array().ok_or("missing hit list")? {
            expected -= 1;
            expected -= expected % stride;
            let got =
                Uuid::parse_str(hit["message_id"].as_str().ok_or("missing hit id")?)?.as_u128();
            if (got & 0xffff_ffff) != (expected as u128 + 1)
                || hit["message_at"] != "2026-11-18T12:00:00Z"
            {
                return Err(format!("wrong newest-first result for {name}: {hit}").into());
            }
        }
        if name == COMMON {
            let mut next = input.clone();
            next["cursor"] = first["next_cursor"].clone();
            let (second, ms) = measured(&mut http, next)?;
            if second["hits"][0]["message_id"].as_str() >= first["hits"][49]["message_id"].as_str()
            {
                return Err("cursor repeated a message".into());
            }
            continue_ms = Some(ms);
        }
        let mut times = Vec::with_capacity(a.samples);
        for _ in 0..a.samples {
            times.push(measured(&mut http, input.clone())?.1);
        }
        metrics.push(stats::summarize(name, times, &budgets));
        if name == COMMON {
            for hit in first["hits"].as_array().unwrap() {
                let id = hit["address"]
                    .as_str()
                    .and_then(|s| s.strip_prefix("/threads/"))
                    .ok_or("incorrect thread address")?;
                let (opened, ms) = http.op(
                    "threads.get",
                    json!({"id":id,"message_id":hit["message_id"]}),
                )?;
                if opened["thread"]["messages"][0]["id"] != hit["message_id"] {
                    return Err("message locator opened the wrong body".into());
                }
                open_ms.push(ms);
            }
        }
    }
    let mut cold = Vec::new();
    let phrase =
        json!({"space_id":space,"kinds":["threads"],"query":"\"canvas readyx\"","limit":50});
    for _ in 0..a.cold_pairs {
        before_trial(true)?;
        cold.push(measured(&mut http, phrase.clone())?.1);
    }
    metrics.push(stats::summarize(COLD, cold, &budgets));
    let failed = metrics.iter().any(|m| m.breached);
    let path = a
        .work
        .join("appdata/spaces")
        .join(space)
        .join("search-messages-v1.all.db");
    let index_bytes = path.metadata()?.len();
    let report = json!({
        "messages":count,"source_bytes":source_bytes,"index_bytes":index_bytes,
        "index_bytes_per_million":index_bytes as f64 * 1e6 / count as f64,
        "rebuild_seconds":rebuild_s,"first_query_partial":initial_partial,
        "cursor_continuation_ms":continue_ms,
        "open_by_id":stats::summarize("search.messages.open",open_ms,&budgets),
        "measurements":metrics,"breached":failed
    });
    std::fs::write(
        a.work.join("message-search.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    if a.json {
        a.output.success(&calternal_cli::output::Reply::new(report));
    } else {
        for m in &metrics {
            println!(
                "{:<36} {:>8.3} ms  {}",
                m.metric,
                m.p95,
                if m.breached { "over budget" } else { "passed" }
            );
        }
    }
    Ok(i32::from(failed))
}

fn measured(http: &mut Http, input: Value) -> Result<(Value, f64)> {
    let (data, ms) = http.op("search.query", input)?;
    if data["partial"] != false || data["hits"].as_array().map(Vec::len) != Some(50) {
        return Err(format!("first page should return exactly 50 complete hits: {data}").into());
    }
    Ok((data, ms))
}

fn seed(root: &Path, space: &str, count: usize) -> Result<u64> {
    let base = root.join("appdata");
    let mut db = Connection::open(base.join("spaces").join(space).join("content.db"))?;
    db.execute(
        "ATTACH DATABASE ?1 AS routes",
        [base.join("system/content-index.db").to_string_lossy()],
    )?;
    let files = root.join("userdata-ssd/spaces").join(space);
    calternal_fsio::private::directory(&files.join("AI/Chats"))?;
    let mut source_bytes = 0;
    for chunk in (0..count).step_by(CAPACITY * 64) {
        let tx = db.transaction()?;
        let mut head = tx.prepare_cached(
            "INSERT INTO items VALUES(?1,'threads',?2,?3,1,?4,?5,?6,NULL,'materialized',?7)",
        )?;
        let mut version =
            tx.prepare_cached("INSERT INTO versions VALUES(?1,1,?2,?3,?4,?5,'perf')")?;
        let mut route = tx.prepare_cached("INSERT INTO routes.items VALUES(?1,?2,'threads')")?;
        for first in (chunk..(chunk + CAPACITY * 64).min(count)).step_by(CAPACITY) {
            let id = Uuid::from_u128(0x01990000_0000_7000_8000_000000000000 | (first as u128 + 1));
            let (metadata, stored) = calternal_server::rebuild_probe::message_fixture(
                first,
                CAPACITY.min(count - first),
            );
            let placed = format!(
                "---\ncalternal-id: {id}\n{}",
                stored
                    .strip_prefix("---\n")
                    .ok_or("invalid fixture frame")?
            );
            source_bytes += placed.len() as u64;
            let hash = blake3::hash(placed.as_bytes()).to_hex().to_string();
            let relative = format!("AI/Chats/Corpus-{first:09}.md");
            let mut file = calternal_fsio::private::file(&files.join(&relative))?;
            file.write_all(placed.as_bytes())?;
            let rowid = id.to_string();
            let rev = format!("perf-{id}");
            let stamp = metadata["updated_at"]
                .as_str()
                .ok_or("missing fixture timestamp")?;
            head.execute(params![
                rowid,
                metadata.to_string(),
                rev,
                stamp,
                relative,
                relative.to_lowercase(),
                hash
            ])?;
            version.execute(params![
                rowid,
                rev,
                metadata.to_string(),
                stored.as_bytes(),
                stamp
            ])?;
            route.execute(params![rowid, space])?;
        }
        drop((head, version, route));
        tx.commit()?;
    }
    Ok(source_bytes)
}
