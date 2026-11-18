//! Bounded closed-loop directory workloads matching the people-lane fixture.
//! Credentials stay in memory/disposable roots; only latency vectors leave a run.
use crate::{
    Args, environment, fixtures,
    http::{Http, Result},
    report,
    server::Server,
    stats,
};
use calternal_types::{AccountId, SpaceId};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
    time::Duration,
};

#[derive(Clone, Copy)]
enum Mode {
    Reads,
    Writes,
    Mixed,
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Reads => "reads",
            Self::Writes => "writes",
            Self::Mixed => "mixed",
        }
    }
    fn writes(self, client: usize) -> bool {
        match self {
            Self::Reads => false,
            Self::Writes => true,
            Self::Mixed => client % 2 == 1,
        }
    }
}

// No Debug/Serialize: private bearer values must never enter reports or errors.
struct Fixture {
    owner: String,
    target: AccountId,
    spaces: Vec<SpaceId>,
    seed_spaces: usize,
}

// Own only freshly created benchmark roots. Even a setup/copy error removes
// partial credentials; raw vectors and diagnostic logs remain for bounded review.
struct DiscardRoots(std::path::PathBuf);
impl Drop for DiscardRoots {
    fn drop(&mut self) {
        for name in ["appdata", "userdata-ssd", "userdata-hdd"] {
            let path = self.0.join(name);
            if path.exists()
                && let Err(error) = std::fs::remove_dir_all(&path)
            {
                eprintln!("fixture cleanup failed for {}: {error}", path.display());
            }
        }
    }
}

fn provision(a: &Args, root: &Path, clients: usize) -> Result<Fixture> {
    let (server, mut h, _) =
        Server::start_development(&a.bin, root, &a.plugins, a.startup_timeout_seconds)?;
    let startup: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("appdata/startup.json"))?)?;
    let token = |name: &str| -> Result<String> {
        let path = startup["token_files"][name]
            .as_str()
            .ok_or("development fixture token path missing")?;
        let value = std::fs::read_to_string(path)?.trim().to_owned();
        if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("invalid development fixture credential".into());
        }
        Ok(value)
    };
    h.bearer = Some(token("guest")?);
    let (describe, _, _) = h.request("/api/v1/describe", None)?;
    let target = AccountId::parse(
        describe["principal"]["id"]
            .as_str()
            .ok_or("fixture account missing")?,
    )?;
    let mut initial = BTreeSet::new();
    for row in describe["principal"]["spaces"]
        .as_array()
        .ok_or("fixture memberships missing")?
    {
        initial.insert(SpaceId::parse(
            row["id"].as_str().ok_or("fixture space missing")?,
        )?);
    }
    let owner = token("owner")?;
    h.bearer = Some(owner.clone());
    let (describe, _, _) = h.request("/api/v1/describe", None)?;
    for row in describe["principal"]["spaces"]
        .as_array()
        .ok_or("fixture memberships missing")?
    {
        initial.insert(SpaceId::parse(
            row["id"].as_str().ok_or("fixture space missing")?,
        )?);
    }
    let mut spaces = Vec::with_capacity(clients);
    for index in 0..clients {
        let (space, _) = h.op(
            "spaces.create",
            json!({"name":format!("Concurrency {index}")}),
        )?;
        let space = SpaceId::parse(
            space["space"]["id"]
                .as_str()
                .ok_or("fixture space missing")?,
        )?;
        h.op(
            "spaces.add_member",
            json!({"space_id":space,"account_id":target,"role":"reader"}),
        )?;
        spaces.push(space);
    }
    drop(server);
    Ok(Fixture {
        owner,
        target,
        spaces,
        seed_spaces: initial.len(),
    })
}

fn call(
    h: &mut Http,
    space: SpaceId,
    target: AccountId,
    write: bool,
    sequence: usize,
) -> Result<f64> {
    let (operation, input) = if write {
        (
            "spaces.set_role",
            json!({"space_id":space,"account_id":target,"role":if sequence % 2 == 0 {"reader"} else {"editor"}}),
        )
    } else {
        ("spaces.members", json!({"space_id":space}))
    };
    Ok(h.op(operation, input)?.1)
}

// Warm-up failures and thread-spawn failures release every waiter. No I/O runs
// under this fixture-only mutex; Condvar waiting releases it atomically.
#[derive(Default)]
struct Start {
    state: Mutex<(bool, bool)>,
    ready: Condvar,
}
impl Start {
    fn release(&self, cancelled: bool) {
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = (true, cancelled);
        self.ready.notify_all();
    }
    fn wait(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let state = self
            .ready
            .wait_while(state, |state| !state.0)
            .unwrap_or_else(|e| e.into_inner());
        !state.1
    }
}

// Mixed workloads synchronize each untimed round boundary so fast readers do
// not finish their sample set before writers begin. Cancellation releases peers.
struct Rounds {
    clients: usize,
    state: Mutex<(usize, usize, bool)>,
    ready: Condvar,
}
impl Rounds {
    fn new(clients: usize) -> Self {
        Self {
            clients,
            state: Mutex::new((0, 0, false)),
            ready: Condvar::new(),
        }
    }
    fn meet(&self) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.2 {
            return false;
        }
        let round = state.0;
        state.1 += 1;
        if state.1 == self.clients {
            state.0 += 1;
            state.1 = 0;
            self.ready.notify_all();
            return true;
        }
        let state = self
            .ready
            .wait_while(state, |state| state.0 == round && !state.2)
            .unwrap_or_else(|e| e.into_inner());
        state.0 != round
    }
    fn cancel(&self) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).2 = true;
        self.ready.notify_all();
    }
}
struct CancelRound(Arc<Rounds>);
impl Drop for CancelRound {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

fn measure(
    port: u16,
    fixture: &Fixture,
    mode: Mode,
    samples: usize,
) -> std::result::Result<BTreeMap<String, Vec<f64>>, String> {
    let start = Arc::new(Start::default());
    let rounds = Arc::new(Rounds::new(fixture.spaces.len()));
    let (tx, rx) = mpsc::sync_channel(fixture.spaces.len());
    let mut handles = Vec::with_capacity(fixture.spaces.len());
    let mut failure = None;
    for (client, space) in fixture.spaces.iter().copied().enumerate() {
        let start = start.clone();
        let rounds = rounds.clone();
        let tx = tx.clone();
        let token = fixture.owner.clone();
        let target = fixture.target;
        let write = mode.writes(client);
        let worker = thread::Builder::new()
            .name(format!("perf-client-{client}"))
            .spawn(move || {
                let warm = (|| -> std::result::Result<Http, String> {
                    let mut h = Http::new(port).map_err(|e| e.to_string())?;
                    h.bearer = Some(token);
                    call(&mut h, space, target, write, 0).map_err(|e| e.to_string())?;
                    Ok(h)
                })();
                let _ = tx.send(warm.as_ref().map(|_| ()).map_err(Clone::clone));
                if !start.wait() {
                    return Err("client wave cancelled".to_owned());
                }
                let _cancel = matches!(mode, Mode::Mixed).then(|| CancelRound(rounds.clone()));
                let mut h = warm?;
                let mut values = Vec::with_capacity(samples);
                for sequence in 1..=samples {
                    if matches!(mode, Mode::Mixed) && !rounds.meet() {
                        return Err("mixed wave cancelled".into());
                    }
                    values.push(
                        call(&mut h, space, target, write, sequence).map_err(|e| e.to_string())?,
                    );
                }
                Ok((write, values))
            });
        match worker {
            Ok(handle) => handles.push(handle),
            Err(error) => {
                failure = Some(format!("client thread could not start: {error}"));
                break;
            }
        }
    }
    drop(tx);
    if failure.is_none() {
        for _ in 0..handles.len() {
            match rx.recv_timeout(Duration::from_secs(150)) {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    failure = Some(format!("untimed warm-up failed: {error}"));
                    break;
                }
                Err(_) => {
                    failure = Some("untimed warm-up timed out or disconnected".into());
                    break;
                }
            }
        }
    }
    start.release(failure.is_some());
    let mut values: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for handle in handles {
        match handle.join() {
            Ok(Ok((write, samples))) => values
                .entry(
                    if write {
                        "concurrency.spaces.set_role"
                    } else {
                        "concurrency.spaces.members"
                    }
                    .into(),
                )
                .or_default()
                .extend(samples),
            Ok(Err(error)) => {
                failure.get_or_insert(error);
            }
            Err(_) => {
                failure.get_or_insert("client thread panicked".into());
            }
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(values),
    }
}

/// Measure paired warm reads, writes and mixed waves in disposable roots. Bounds
/// and setup failures reject the run; arm failures remain explicit and roots are
/// removed. At most 128 client threads/connections, 100 samples/client and the
/// caller's bounded pair count; no production latency ceiling is invented.
pub(crate) fn run(a: &Args) -> Result<i32> {
    if !(20..=100).contains(&a.samples) || a.pairs < 3 {
        return Err(
            "concurrency requires 20..100 samples per client and at least three warm pairs".into(),
        );
    }
    let clients = a
        .clients
        .split(',')
        .map(str::parse::<usize>)
        .collect::<std::result::Result<BTreeSet<_>, _>>()?;
    if clients.is_empty() || clients.len() > 4 || clients.iter().any(|&n| n == 0 || n > 128) {
        return Err("choose at most four client counts, each 1..128".into());
    }
    if a.work.exists() {
        return Err("choose a fresh concurrency work directory".into());
    }
    std::fs::create_dir_all(&a.work)?;
    let (machine, hashes) = environment(a)?;
    let budgets: stats::Budgets = serde_json::from_slice(&std::fs::read(&a.budgets)?)?;
    let mut runs = Vec::new();
    let mut failures = Vec::new();
    let mut pooled: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for clients in clients {
        eprintln!("concurrency fixture: {clients} clients/spaces");
        let seed = a.work.join(format!("seed-{clients}"));
        let _seed_cleanup = DiscardRoots(seed.clone());
        let staging = tempfile::Builder::new()
            .prefix("calternal-perf-concurrency-")
            .tempdir_in("/dev/shm")?;
        let fixture = provision(a, staging.path(), clients)?;
        fixtures::copy(staging.path(), &seed)?;
        drop(staging);
        for mode in [Mode::Reads, Mode::Writes, Mode::Mixed] {
            if clients == 1 && matches!(mode, Mode::Mixed) {
                continue;
            }
            let mut failed_arms: BTreeMap<&str, usize> = BTreeMap::new();
            for pair in 0..a.pairs {
                let order = if pair % 2 == 0 {
                    ["baseline", "head"]
                } else {
                    ["head", "baseline"]
                };
                for arm in order {
                    if failed_arms.get(arm).copied().unwrap_or(0) >= 2 {
                        continue;
                    }
                    let root = a
                        .work
                        .join(format!("{clients}-{}-{pair}-{arm}", mode.name()));
                    let _root_cleanup = DiscardRoots(root.clone());
                    fixtures::copy(&seed, &root)?;
                    let bin = if arm == "head" { &a.bin } else { &a.baseline };
                    let plugins = if arm == "head" {
                        &a.plugins
                    } else {
                        a.baseline_plugins.as_ref().unwrap_or(&a.plugins)
                    };
                    let outcome = (|| -> Result<_> {
                        if !std::process::Command::new("sync").status()?.success() {
                            return Err("fixture sync failed before warm arm".into());
                        }
                        let (server, h, _) = Server::start_development(
                            bin,
                            &root,
                            plugins,
                            a.startup_timeout_seconds,
                        )?;
                        let port = url::Url::parse(&h.base)?
                            .port()
                            .ok_or("fixture port missing")?;
                        let values = measure(port, &fixture, mode, a.samples)
                            .map_err(std::io::Error::other)?;
                        std::fs::write(root.join("raw.json"), serde_json::to_vec(&values)?)?;
                        let summary = values
                            .into_iter()
                            .map(|(name, values)| stats::summarize(&name, values, &budgets))
                            .collect::<Vec<_>>();
                        drop(server);
                        Ok(summary)
                    })();
                    match outcome {
                        Ok(summary) => {
                            for row in &summary {
                                pooled
                                    .entry(format!(
                                        "{clients}/{}/{arm}/{}",
                                        mode.name(),
                                        row.metric
                                    ))
                                    .or_default()
                                    .push(row.p50);
                            }
                            runs.push(json!({"clients":clients,"spaces":clients,"seed_spaces":fixture.seed_spaces,"mode":mode.name(),"pair":pair,"arm":arm,"summary":summary}));
                        }
                        Err(error) => {
                            *failed_arms.entry(arm).or_default() += 1;
                            failures.push(json!({"clients":clients,"spaces":clients,"mode":mode.name(),"pair":pair,"arm":arm,"error":error.to_string()}));
                        }
                    }
                    fixtures::remove_roots(&root)?;
                    std::fs::write(
                        a.work.join("summary.partial.json"),
                        serde_json::to_vec_pretty(
                            &json!({"complete":false,"runs":runs,"failures":failures}),
                        )?,
                    )?;
                    eprintln!(
                        "concurrency: {clients} {} pair {pair} {arm} complete",
                        mode.name()
                    );
                }
            }
        }
        fixtures::remove_roots(&seed)?;
    }
    let noise = pooled
        .into_iter()
        .map(|(key, mut values)| {
            values.sort_by(f64::total_cmp);
            let middle = stats::quantile(&values, 0.5);
            let noise = if middle == 0. {
                0.
            } else {
                100. * (values[values.len() - 1] - values[0]) / middle
            };
            (key, noise)
        })
        .collect::<BTreeMap<_, _>>();
    let report = json!({"commit":a.commit,"baseline_commit":a.baseline_commit,"machine":machine,"binary_sha256":hashes,
        "authentication":"explicit development bearer; no browser cookie or Origin; same workload as people-lane fixture",
        "cache":"one untimed operation per caller; fresh copied fixture per arm; 128 write spaces exceed the unchanged 64-space content cache",
        "mixed_scheduling":"synchronized rounds; boundary waits excluded from RPC samples",
        "space_count_definition":"spaces is the targeted working set; seed_spaces are additional development account/shared spaces",
        "samples_per_client":a.samples,"warm_pairs":a.pairs,"cold_pairs":0,"seed":a.seed,"noise_statistic":"range of per-arm p50s / median p50","noise_pct":noise,"runs":runs,"failures":failures,
        "qualification":"observations only: no accepted concurrency latency ceiling; not browser-session or provider-startup qualification"});
    let path = a.work.join("summary.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    report::print(a, &report, &path);
    Ok(if failures.is_empty() { 2 } else { 1 })
}
