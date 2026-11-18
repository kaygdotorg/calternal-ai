//! Disposable real-server benchmark runner. HTTP scenarios use a separate server process.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
mod concurrency;
mod fixtures;
mod fsync_probe;
mod http;
mod idle;
mod jobs;
mod live;
mod materialization;
mod message_search;
mod notes_scenario;
mod rebuild;
mod report;
mod rooms;
mod search;
mod search_stage;
mod server;
mod stats;
mod timings;
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
    /// Matching historical bundle when the baseline pins an older worker digest.
    #[arg(long)]
    baseline_plugins: Option<PathBuf>,
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
    #[arg(long)]
    fsync_probe: bool,
    #[arg(long)]
    startup_only: bool,
    #[arg(long)]
    jobs_only: bool,
    /// Require release timestamps. Gate delivery and file placement independently.
    #[arg(long, conflicts_with = "paired_search")]
    commit_timing_only: bool,
    /// Measure room keystrokes and durable receipts, and correlate subscribed SSE commits.
    #[arg(long)]
    rooms_only: bool,
    /// Measure protected directory reads/writes with closed-loop client waves.
    #[arg(long)]
    concurrency_only: bool,
    /// Client wave sizes; each client owns a separate space (maximum 128).
    #[arg(long, default_value = "8,32,128")]
    clients: String,
    /// Keep the save matrix and add online search probes. Both arms use the
    /// same binary. The baseline manifest must disable the Notes index.
    #[arg(long, requires = "baseline_plugins")]
    paired_search: bool,
    /// Queue zero, one or two online rebuilds and probe healthy-space requests.
    #[arg(long, requires = "paired_search")]
    search_queue: bool,
    /// Measure authorized first-page search and same-space note saves.
    #[arg(long)]
    search_only: bool,
    /// Gate exact term, rare term and phrase search against framed thread fixtures.
    #[arg(long)]
    message_search_only: bool,
    /// Measure staged file-origin and actual first-use index rebuilding.
    #[arg(long)]
    rebuild_only: bool,
    /// A stopped, 100,000-note, one-space search benchmark fixture.
    #[arg(long)]
    rebuild_source: Option<PathBuf>,
    /// Measure the selected rebuild sizes without re-running the admission waves.
    #[arg(long, requires = "rebuild_only")]
    rebuild_no_admission: bool,
    #[arg(long)]
    prepare_only: bool,
    #[arg(long)]
    test_plugin_root: Option<PathBuf>,
    #[arg(long, default_value_t = 30)]
    startup_timeout_seconds: u64,
    #[arg(skip=calternal_cli::output::Output::detect(std::env::args_os().any(|v|v=="--json")))]
    output: calternal_cli::output::Output,
}
fn main() {
    let args = match calternal_cli::output::parse_args::<Args>() {
        Ok(args) => args,
        Err(code) => std::process::exit(code),
    };
    let code = match run(&args) {
        Ok(code) => code,
        Err(e) => args.output.failure(
            &calternal_cli::output::CliError::new("benchmark_failed", e.to_string())
                .hint("Inspect the run directory or use --help for another run.")
                .exit(2),
        ),
    };
    std::process::exit(code);
}
fn run(a: &Args) -> Result<i32> {
    if std::env::var("CALTERNAL_PERF_HELD").as_deref() != Ok("1") {
        return Err("run under /usr/local/bin/perf-lock.sh on the perf VM".into());
    }
    if a.paired_search && a.bin != a.baseline {
        return Err("paired search requires the same binary in both arms".into());
    }
    if a.samples > 10_000 || a.pairs > 20 || a.cold_pairs > 5 {
        return Err("limit samples to 10000, warm pairs to 20 and cold pairs to 5".into());
    }
    if [
        a.prepare_only,
        a.jobs_only,
        a.idle_only,
        a.fsync_probe,
        a.startup_only,
        a.commit_timing_only,
        a.rooms_only,
        a.concurrency_only,
        a.search_only,
        a.message_search_only,
        a.search_queue,
        a.rebuild_only,
    ]
    .into_iter()
    .filter(|v| *v)
    .count()
        > 1
    {
        return Err(
            "choose one measurement mode: --prepare-only, --jobs-only, --idle-only, --fsync-probe, --startup-only, --commit-timing-only, --concurrency-only, --search-only, --message-search-only or --rebuild-only".into(),
        );
    }
    if a.startup_timeout_seconds == 0 || a.startup_timeout_seconds > 300 {
        return Err("startup timeout must be 1..300 seconds".into());
    }
    if a.fsync_probe {
        return fsync_probe::run(a);
    }
    if a.prepare_only {
        return fixtures::prepare(a);
    }
    if a.jobs_only {
        return jobs::run(a);
    }
    if a.idle_only {
        return idle::run(a);
    }
    if a.concurrency_only {
        return concurrency::run(a);
    }
    if a.rebuild_only {
        return rebuild::run(a);
    }
    if a.search_queue {
        return search_stage::queue::run(a);
    }
    if a.search_only {
        return search::run(a);
    }
    if a.message_search_only {
        return message_search::run(a);
    }
    if a.samples < 20 || a.pairs < 3 {
        return Err("at least 20 samples and three interleaved pairs are required".into());
    }
    if a.work.exists() {
        return Err("work directory already exists; choose a fresh --work".into());
    }
    std::fs::create_dir_all(&a.work)?;
    let (machine, hashes) = environment(a)?;
    let run_info = json!({"commit":a.commit,"baseline_commit":a.baseline_commit,"plugins":a.plugins,"baseline_plugins":a.baseline_plugins.as_ref().unwrap_or(&a.plugins),"seed":a.seed,"sizes":a.sizes,"spaces":a.spaces,"warm_pairs":a.pairs,"cold_pairs":a.cold_pairs,"samples":a.samples,"machine":machine,"binary_sha256":hashes});
    std::fs::write(
        a.work.join("run-info.json"),
        serde_json::to_vec_pretty(&run_info)?,
    )?;
    let budgets: stats::Budgets = serde_json::from_slice(&std::fs::read(&a.budgets)?)?;
    let placement_budget = if a.startup_only {
        Duration::ZERO
    } else {
        let ms = budgets
            .entries
            .iter()
            .find(|entry| {
                entry.metric == materialization::METRIC
                    && matches!(entry.percentile, stats::Percentile::Max)
            })
            .and_then(|entry| entry.budget)
            .filter(|ms| *ms > 0. && *ms <= 1000.)
            .ok_or("file placement requires a maximum budget up to 1000 ms")?;
        Duration::try_from_secs_f64(ms / 1000.)?
    };
    if a.rooms_only && (a.samples == 0 || a.pairs + a.cold_pairs == 0) {
        return Err("room timing requires at least one keystroke per measured arm".into());
    }
    let required = if a.rooms_only {
        &[timings::COMMIT_TO_CLIENT, rooms::PEER, rooms::SAVE][..]
    } else if a.commit_timing_only {
        &[timings::COMMIT_TO_CLIENT][..]
    } else {
        &[]
    };
    for metric in required {
        if !budgets.entries.iter().any(|entry| {
            entry.metric == *metric
                && matches!(entry.percentile, stats::Percentile::P95)
                && entry.budget.is_some()
        }) {
            return Err(format!("{metric} requires a p95 budget in the catalog").into());
        }
    }
    let sizes: Vec<usize> = a
        .sizes
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()?;
    if sizes.iter().any(|n| *n == 0 || *n > 100_000) {
        return Err("fixture sizes must be 1..100000; 1M needs a capacity decision".into());
    }
    let mut all = Vec::new();
    let mut failures = Vec::new();
    let mut reproduced = false;
    let mut commit_breach = false;
    let mut commit_incomplete = false;
    let mut placement_breach = false;
    let mut stopped_for_placement = false;
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
    if cases.is_empty() {
        return Err("choose at least one fixture size or multi-space case".into());
    }
    if cases.len() > 10 {
        return Err("at most ten fixture cases per invocation".into());
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
        let mut failed_arms: BTreeMap<&str, usize> = BTreeMap::new();
        for pair in 0..(a.pairs + a.cold_pairs) {
            // Alternate AB/BA to avoid always giving one binary the warmer host.
            let arms = if pair % 2 == 0 {
                ["baseline", "head"]
            } else {
                ["head", "baseline"]
            };
            for arm in arms {
                if failed_arms.get(arm).copied().unwrap_or(0) >= 2 {
                    continue;
                }
                let bin = if arm == "head" { &a.bin } else { &a.baseline };
                let root = a.work.join(format!("{spaces}x{size}-{pair}-{arm}"));
                if root.exists() {
                    return Err(format!("refusing reused roots: {}", root.display()).into());
                }
                fixtures::copy(&seed_root, &root)?;
                if a.paired_search {
                    fixtures::reset_search_catalog(&root)?;
                }
                if pair < a.cold_pairs
                    && !std::process::Command::new("sudo")
                        .args(["-n", "/usr/local/sbin/perf-drop-caches"])
                        .status()?
                        .success()
                {
                    return Err("cold run cache drop failed".into());
                }
                // Cold helper syncs before dropping caches. Warm copies must also
                // finish writeback, while retaining the page cache, before timing.
                if pair >= a.cold_pairs && !std::process::Command::new("sync").status()?.success() {
                    return Err("warm fixture sync failed".into());
                }
                let observation = match notes_scenario::run(
                    a,
                    bin,
                    if arm == "head" {
                        &a.plugins
                    } else {
                        a.baseline_plugins.as_ref().unwrap_or(&a.plugins)
                    },
                    &root,
                    &fixture,
                    (a.commit_timing_only || a.rooms_only) && arm == "head",
                    placement_budget,
                ) {
                    Ok(observation) => observation,
                    Err(error) => {
                        commit_incomplete |=
                            (a.commit_timing_only || a.rooms_only) && arm == "head";
                        let failure = json!({"size":size,"spaces":spaces,"pair":pair,"arm":arm,"error":error.to_string(),"startup_timeout_seconds":a.startup_timeout_seconds,"log":root.join("server.log")});
                        std::fs::write(
                            root.join("failure.json"),
                            serde_json::to_vec_pretty(&failure)?,
                        )?;
                        failures.push(failure);
                        *failed_arms.entry(arm).or_default() += 1;
                        std::fs::write(
                            a.work.join("summary.partial.json"),
                            serde_json::to_vec_pretty(
                                &json!({"complete":false,"runs":all,"failures":failures}),
                            )?,
                        )?;
                        continue;
                    }
                };
                stopped_for_placement |= observation.placement_breach;
                placement_breach |= arm == "head" && observation.placement_breach;
                let placement = if a.startup_only || a.rooms_only {
                    "unmeasured"
                } else if observation.placement_breach {
                    "breach"
                } else {
                    "pass"
                };
                std::fs::write(
                    root.join("raw.json"),
                    serde_json::to_vec(&observation.samples)?,
                )?;
                let summary: Vec<_> = observation
                    .samples
                    .into_iter()
                    .map(|(name, v)| {
                        let mut summary = stats::summarize(&name, v, &budgets);
                        if name == materialization::METRIC {
                            summary.breached |= observation.placement_breach;
                        }
                        summary
                    })
                    .collect();
                if arm == "head" {
                    for metric in required {
                        commit_incomplete |= !summary
                            .iter()
                            .any(|s| s.metric == *metric && s.samples == a.samples);
                    }
                }
                for s in &summary {
                    pooled
                        .entry(format!(
                            "{spaces}x{size}/{arm}/{}/{}",
                            if pair < a.cold_pairs { "cold" } else { "warm" },
                            s.metric
                        ))
                        .or_default()
                        .push(s.p50);
                    if a.commit_timing_only
                        && arm == "head"
                        && s.metric == timings::COMMIT_TO_CLIENT
                    {
                        commit_incomplete |= s.samples < a.samples;
                    }
                    if arm == "head" && s.breached {
                        commit_breach |= (s.metric == timings::COMMIT_TO_CLIENT
                            || required.contains(&s.metric.as_str()))
                            && s.samples == a.samples;
                        *breaches.entry(s.metric.clone()).or_default() += 1;
                    }
                }
                all.push(
                    json!({"size":size,"spaces":spaces,"pair":pair,"cache":if pair<a.cold_pairs {"disk-cold"}else{"warm"},"arm":arm,"file_placement":placement,"summary":summary}),
                );
                std::fs::write(
                    a.work.join("summary.partial.json"),
                    serde_json::to_vec_pretty(
                        &json!({"complete":false,"runs":all,"failures":failures}),
                    )?,
                )?;
                // Keep log/raw; roots contain private fixture credentials and are disposable.
                fixtures::remove_roots(&root)?;
            }
        }
        fixtures::remove_roots(&seed_root)?;
        reproduced |= breaches.values().any(|n| *n >= 2)
            || failed_arms.get("head").copied().unwrap_or(0) >= 2;
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
    let qualification = if a.commit_timing_only {
        "release durable commit to client receipt and separate file placement"
    } else {
        "incomplete: see separate release commit timing, job and idle suites"
    };
    let report = json!({"commit":a.commit,"baseline_commit":a.baseline_commit,"plugins":a.plugins,"baseline_plugins":a.baseline_plugins.as_ref().unwrap_or(&a.plugins),"binary_sha256":hashes,"machine":machine,"noise_pct":noise,"qualification":qualification,"hard_commit_breach":commit_breach,"commit_to_client_incomplete":commit_incomplete,"hard_file_placement_breach":placement_breach,"seed":a.seed,"operation_concurrency":1,"subscriptions":2,"cache":"cold pairs drop OS caches; warm pairs retain OS caches","warm_pairs":a.pairs,"cold_pairs":a.cold_pairs,"reproduced_breach":reproduced,"runs":all,"failures":failures});
    std::fs::write(
        a.work.join("summary.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    report::print(a, &report, &a.work.join("summary.json"));
    Ok(
        if commit_breach || placement_breach || (!a.commit_timing_only && reproduced) {
            1
        } else if !a.commit_timing_only
            || !failures.is_empty()
            || stopped_for_placement
            || commit_incomplete
        {
            // A plain matrix is only one part of the backend performance gate.
            2
        } else {
            0
        },
    )
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn concurrency_mode_accepts_the_requested_bounded_wave_configuration() {
        let args = Args::try_parse_from([
            "calternal-perf",
            "--bin",
            "/head",
            "--baseline",
            "/before",
            "--work",
            "/disposable",
            "--baseline-commit",
            "before",
            "--concurrency-only",
            "--clients",
            "8,32,128",
            "--samples",
            "20",
            "--pairs",
            "3",
            "--cold-pairs",
            "0",
            "--json",
        ])
        .expect("the concurrency harness must accept the requested wave sizes");
        assert_eq!(args.samples, 20);
        assert_eq!(args.pairs, 3);
        assert_eq!(args.cold_pairs, 0);
    }
}

/// Capture the same machine and binary provenance for every measurement mode.
/// Two bounded child commands; missing tools/binaries fail before timed work.
fn environment(a: &Args) -> Result<(String, String)> {
    let machine = std::process::Command::new("sh")
        .args([
            "-c",
            "uname -a; lscpu; free -b; findmnt -T .; lsblk -o NAME,SIZE,ROTA,FSTYPE,MOUNTPOINTS",
        ])
        .output()?;
    let hashes = std::process::Command::new("sha256sum")
        .arg(&a.bin)
        .arg(&a.baseline)
        .arg(std::env::current_exe()?)
        .output()?;
    if !machine.status.success() || !hashes.status.success() {
        return Err("unable to capture machine/binary provenance".into());
    }
    Ok((
        String::from_utf8_lossy(&machine.stdout).into_owned(),
        String::from_utf8_lossy(&hashes.stdout).into_owned(),
    ))
}
