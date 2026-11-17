//! Disposable real-server benchmark runner. Never links the server or changes product code.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
mod fixtures;
mod http;
mod idle;
mod live;
mod server;
mod stats;
use clap::Parser;
use http::Result;
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf, time::Duration};
#[derive(Parser)]
#[command(about = "Measure a disposable Calternal server under the perf VM lock")]
struct Args {
    #[arg(long)]
    bin: PathBuf,
    #[arg(long)]
    baseline: PathBuf,
    #[arg(long)]
    work: PathBuf,
    #[arg(long, default_value = "plugins")]
    plugins: PathBuf,
    #[arg(long, default_value = "perf/budgets.json")]
    budgets: PathBuf,
    #[arg(long, default_value = "1000,10000,100000")]
    sizes: String,
    #[arg(long, default_value_t = 5)]
    pairs: usize,
    #[arg(long, default_value_t = 3)]
    cold_pairs: usize,
    #[arg(long, default_value_t = 100)]
    samples: usize,
    #[arg(long, default_value_t = 20261117)]
    seed: u64,
    #[arg(long)]
    json: bool,
    #[arg(long, default_value = "1,100,1000")]
    spaces: String,
    #[arg(long, default_value = "unknown")]
    commit: String,
    #[arg(long)]
    baseline_commit: String,
    #[arg(long)]
    idle_only: bool,
}
fn main() {
    let args = Args::parse();
    let code = match run(&args) {
        Ok(code) => code,
        Err(e) => {
            if args.json {
                println!("{}", json!({"ok":false,"error":e.to_string()}));
            } else {
                eprintln!(
                    "Performance run failed: {e}\nNext: inspect the run directory or use --help."
                );
            }
            2
        }
    };
    std::process::exit(code);
}
fn run(a: &Args) -> Result<i32> {
    if std::env::var("CALTERNAL_PERF_HELD").as_deref() != Ok("1") {
        return Err("run under /usr/local/bin/perf-lock.sh on the perf VM".into());
    }
    if a.idle_only {
        return idle::run(a);
    }
    if a.samples < 20 || a.pairs < 3 {
        return Err("at least 20 samples and three interleaved pairs are required".into());
    }
    std::fs::create_dir_all(&a.work)?;
    let budgets: stats::Budgets = serde_json::from_slice(&std::fs::read(&a.budgets)?)?;
    let sizes: Vec<usize> = a
        .sizes
        .split(',')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()?;
    if sizes.iter().any(|n| *n == 0 || *n > 100_000) {
        return Err("fixture sizes must be 1..100000; 1M needs a capacity decision".into());
    }
    let mut all = Vec::new();
    let mut reproduced = false;
    let mut pooled: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut cases: Vec<(usize, usize)> = sizes.into_iter().map(|n| (n, 1)).collect();
    for n in a.spaces.split(',') {
        let n: usize = n.parse()?;
        if n == 0 || n > 1000 {
            return Err("spaces must be 1..1000".into());
        }
        if n > 1 {
            cases.push((100, n));
        }
    }
    for (size, spaces) in cases {
        let seed_root = a.work.join(format!("seed-{spaces}x{size}"));
        if seed_root.exists() {
            return Err("seed roots already exist; choose fresh --work".into());
        }
        // Untimed provisioning on tmpfs avoids spending hours on fixture-only fsyncs.
        // Timed arms always use disk copies, followed by sync/cache drop for cold runs.
        let staging = tempfile::Builder::new()
            .prefix("calternal-perf-")
            .tempdir_in("/dev/shm")?;
        let fixture = fixtures::build(a, staging.path(), size, spaces)?;
        fixtures::copy(staging.path(), &seed_root)?;
        std::fs::copy(
            staging.path().join("server.log"),
            seed_root.join("server.log"),
        )?;
        drop(staging);
        let mut breaches: BTreeMap<String, usize> = BTreeMap::new();
        for pair in 0..(a.pairs + a.cold_pairs) {
            // Alternate AB/BA to avoid always giving one binary the warmer host.
            let arms = if pair % 2 == 0 {
                ["baseline", "head"]
            } else {
                ["head", "baseline"]
            };
            for arm in arms {
                let bin = if arm == "head" { &a.bin } else { &a.baseline };
                let root = a.work.join(format!("{spaces}x{size}-{pair}-{arm}"));
                if root.exists() {
                    return Err(format!("refusing reused roots: {}", root.display()).into());
                }
                fixtures::copy(&seed_root, &root)?;
                if pair < a.cold_pairs
                    && !std::process::Command::new("sudo")
                        .args(["-n", "/usr/local/sbin/perf-drop-caches"])
                        .status()?
                        .success()
                {
                    return Err("cold run cache drop failed".into());
                }
                let samples = scenario(a, bin, &root, &fixture)?;
                std::fs::write(root.join("raw.json"), serde_json::to_vec(&samples)?)?;
                let summary: Vec<_> = samples
                    .into_iter()
                    .map(|(name, v)| stats::summarize(&name, v, &budgets))
                    .collect();
                for s in &summary {
                    pooled
                        .entry(format!("{spaces}x{size}/{arm}/{}", s.metric))
                        .or_default()
                        .push(s.p50);
                    if arm == "head" && s.breached {
                        *breaches.entry(s.metric.clone()).or_default() += 1;
                    }
                }
                all.push(
                    json!({"size":size,"spaces":spaces,"pair":pair,"cache":if pair<a.cold_pairs {"disk-cold"}else{"warm"},"arm":arm,"summary":summary}),
                );
                // Keep log/raw; roots contain private fixture credentials and are disposable.
                fixtures::remove_roots(&root)?;
            }
        }
        fixtures::remove_roots(&seed_root)?;
        reproduced |= breaches.values().any(|n| *n >= 2);
    }
    let noise: BTreeMap<String, f64> = pooled
        .into_iter()
        .map(|(k, mut v)| {
            v.sort_by(f64::total_cmp);
            let mid = stats::quantile(&v, 0.5);
            let pct = if mid == 0. {
                0.
            } else {
                100. * (v[v.len() - 1] - v[0]) / mid
            };
            (k, pct)
        })
        .collect();
    let machine = std::process::Command::new("sh")
        .args([
            "-c",
            "uname -a; lscpu; free -b; findmnt -T .; lsblk -o NAME,SIZE,ROTA,FSTYPE,MOUNTPOINTS",
        ])
        .output()?;
    let hashes = std::process::Command::new("sha256sum")
        .arg(&a.bin)
        .arg(&a.baseline)
        .output()?;
    let report = json!({"commit":a.commit,"baseline_commit":a.baseline_commit,"binary_sha256":String::from_utf8_lossy(&hashes.stdout),"machine":String::from_utf8_lossy(&machine.stdout),"noise_pct":noise,"qualification":"incomplete: admission, commit timestamp, three-host-call worker, job stream and idle-release proofs remain","seed":a.seed,"operation_concurrency":1,"subscriptions":2,"cache":"three disk-cold pairs then warm pairs","warm_pairs":a.pairs,"cold_pairs":a.cold_pairs,"reproduced_breach":reproduced,"runs":all});
    std::fs::write(
        a.work.join("summary.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    if a.json {
        println!("{report}");
    } else {
        println!(
            "Performance report: {}\nReproduced breach: {reproduced}\nNext: inspect summary.json and raw.json in the run directory.",
            a.work.display()
        );
    }
    // Missing mandatory scenarios cannot produce a green coordinator gate.
    Ok(if reproduced { 1 } else { 2 })
}
fn scenario(
    a: &Args,
    bin: &std::path::Path,
    root: &std::path::Path,
    fixture: &fixtures::Fixture,
) -> Result<BTreeMap<String, Vec<f64>>> {
    let (server, mut h, start) = server::Server::start(bin, root, &a.plugins)?;
    h.cookie = fixture.cookie.clone();
    let space = fixture.spaces[0].clone();
    let mut first = fixture.first.clone();
    let mut samples: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    samples.insert(
        format!("startup.{}_spaces", fixture.spaces.len()),
        vec![start],
    );
    let (_, ms) = h.op("notes.list", json!({"space_id":space}))?;
    samples.insert("space.process_cold_open".into(), vec![ms]);
    let mut live = live::Live::open(
        &h,
        "/api/v1/live",
        &[(
            "subscriptions".into(),
            json!([{"space_id":space,"bookmark":null}]).to_string(),
        )],
    )?;
    live.wait_type("ready")?;
    let mut account = live::Live::open(&h, "/api/v1/account/live", &[])?;
    account.wait_type("ready")?;
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
        let (updated, ms, event, delivery_ms) = std::thread::scope(|scope| -> Result<_> {
            let write = scope.spawn(|| {
                h.op_id("notes.update", input.clone(), &rid)
                    .map_err(|e| e.to_string())
            });
            let event = live.wait_type("entry")?;
            let elapsed = delivery.elapsed().as_secs_f64() * 1000.;
            let (updated, ms) = write
                .join()
                .map_err(|_| "write thread panicked")?
                .map_err(|e| format!("update failed: {e}"))?;
            Ok((updated, ms, event, elapsed))
        })?;
        samples.entry("notes.update".into()).or_default().push(ms);
        if event["entry"]["item"] != first["id"] || event["entry"]["request_id"] != rid {
            return Err("live event did not correlate with request and note".into());
        }
        samples
            .entry("live.update_to_client".into())
            .or_default()
            .push(delivery_ms);
        first = updated["note"].clone();
        let (_, ms) = h.op_id("notes.update", input, &rid)?;
        samples
            .entry("receipt.round_trip".into())
            .or_default()
            .push(ms);
    }
    for i in 0..a.samples {
        let t = std::time::Instant::now();
        h.op(
            "drafts.save",
            json!({"space_id":space,"item_id":first["id"],"text":format!("draft {i}")}),
        )?;
        account.wait_type("changed")?;
        samples
            .entry("account.draft_write_to_feed".into())
            .or_default()
            .push(t.elapsed().as_secs_f64() * 1000.);
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
    Ok(samples)
}
