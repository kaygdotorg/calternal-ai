//! Exercise zero, one and two simultaneous first opens on the real read slots.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) fn run(a: &Args) -> Result<Vec<Value>> {
    const NOTES: usize = 10_000;
    let staging = tempfile::Builder::new()
        .prefix("rebuild-admission-")
        .tempdir_in("/dev/shm")?;
    let fixture = fixtures::build(a, staging.path(), NOTES, 3)?;
    let spaces = fixture
        .spaces
        .iter()
        .map(|s| s.as_str().ok_or("fixture has a non-UUID space"))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut rows = Vec::new();
    for count in 0..=2 {
        let root = a.work.join(format!("admission-{count}"));
        fixtures::copy(staging.path(), &root)?;
        for space in &spaces[..count] {
            reset(&content_path(&root, space))?;
        }
        before_trial(false)?;
        let (server, mut http, _) = Server::start(
            &a.bin,
            &root,
            &a.plugins,
            a.test_plugin_root.as_deref(),
            a.startup_timeout_seconds,
        )?;
        http.cookie = fixture.cookie.clone();
        let (healthy, _) = http.op("notes.list", json!({"space_id":spaces[2]}))?;
        let id = &healthy["notes"][0]["id"];
        let revision = &healthy["notes"][0]["revision"];
        let barrier = Barrier::new(count + 1);
        let remaining = AtomicUsize::new(count);
        let (memory, observation) = footprint(server.pid(), || {
            std::thread::scope(|scope| -> Result<Value> {
                let mut rebuilding = Vec::new();
                for space in &spaces[..count] {
                    let mut cold = http.peer();
                    let barrier = &barrier;
                    let remaining = &remaining;
                    rebuilding.push(scope.spawn(move || {
                        barrier.wait();
                        let res = cold
                            .op("notes.list", json!({"space_id":space}))
                            .map(|(_, ms)| ms)
                            .map_err(|e| e.to_string());
                        remaining.fetch_sub(1, Ordering::Release);
                        res
                    }));
                }
                barrier.wait();
                std::thread::sleep(Duration::from_millis(25));
                let overlapping = remaining.load(Ordering::Acquire);
                let mut writer = http.peer();
                let write = scope.spawn(move || {
                    writer
                        .op(
                            "notes.update",
                            json!({
                                "id":id,"base_revision":revision,"markdown":"# Healthy write\n"
                            }),
                        )
                        .map(|(_, ms)| ms)
                        .map_err(|e| e.to_string())
                });
                let (_, get_ms) = http.op("notes.get", json!({"id":id}))?;
                let write_ms = write.join().map_err(|_| "healthy write panicked")??;
                let mut rebuild_times = Vec::new();
                for handle in rebuilding {
                    rebuild_times.push(handle.join().map_err(|_| "rebuild request panicked")??);
                }
                Ok(
                    json!({"cold_spaces":count,"notes_per_rebuild":NOTES,"rebuilds_live_at_probe":overlapping,
                "healthy_get_ms":get_ms,"healthy_save_ms":write_ms,"first_open_ms":rebuild_times}),
                )
            })
        })?;
        rows.push(json!({"requests":observation,"process":memory}));
        drop(server);
        fixtures::remove_roots(&root)?;
        eprintln!("admission fixture {count}: complete");
    }
    Ok(rows)
}
