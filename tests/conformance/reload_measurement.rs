//! Opt-in benchmark from a TypeScript edit through container readiness and a broker read.
use super::*;
use std::{io::BufRead, sync::mpsc};

struct Watch(Child);
impl Drop for Watch {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn edit_to_first_valid_plugin_result() {
    let Some(mode) = std::env::var_os("CALTERNAL_MEASURE_RELOAD") else {
        return;
    };
    let remote = mode == "remote";
    assert!(remote || mode == "direct", "use remote or direct");
    let mut f = Fixture::new().await;
    f.stop();
    f.development = true;
    f.start().await;
    let _machine = if remote {
        assert!(
            calternal_machine::sandbox::podman::PodmanBackend::available(Path::new("podman")).await
        );
        let machine = f.worker_machine(false).await;
        jobs_cases::enable(&f, 1, &f.spaces[0]).await;
        let end = Instant::now() + Duration::from_secs(15);
        loop {
            let first = f
                .op(
                    1,
                    "jobs-demo.write",
                    json!({"space_id":f.spaces[0],"text":"transport ready"}),
                )
                .await;
            if first.status == 200 {
                break;
            }
            assert!(
                Instant::now() < end,
                "worker machine unavailable: {}",
                first.body
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Some(machine)
    } else {
        None
    };

    let binary = std::env::var_os("CALTERNAL_SERVER_BIN").unwrap();
    let sdk = source_root();
    let id = "reload-measurement";
    let stage = f.dir.path();
    let scaffold = Command::new(&binary)
        .args(["plugin", "new", id, "--sdk-root"])
        .arg(sdk)
        .current_dir(stage)
        .output()
        .unwrap();
    assert!(scaffold.status.success(), "{scaffold:?}");
    let dir = stage.join(id);
    assert!(
        Command::new("npm")
            .args(["install", "--ignore-scripts", "--no-audit", "--no-fund"])
            .current_dir(&dir)
            .status()
            .unwrap()
            .success()
    );
    let worker = dir.join("worker.ts");
    let space = &f.spaces[0];
    let note = f.note(1, space).await;
    let note_id = note["id"].as_str().unwrap();
    let source = std::fs::read_to_string(&worker)
        .unwrap()
        .replace("kind: \"reload-measurement\"", "kind: \"notes\"")
        .replace("01900000-0000-7000-8000-000000000003", note_id)
        .replace("length: 3", "length: 1");
    std::fs::write(&worker, &source).unwrap();

    let server_pid = f.child.as_ref().unwrap().id();
    let mut watch = Watch(
        Command::new(&binary)
            .args(["plugin", "dev", "--json"])
            .arg(&dir)
            .arg("--appdata")
            .arg(f.appdata())
            .arg("--space")
            .arg(space)
            .arg("--account")
            .arg(&f.ids[1])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let stdout = watch.0.stdout.take().unwrap();
    let (send, events) = mpsc::sync_channel(2);
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines() {
            if send.send(line).is_err() {
                break;
            }
        }
    });
    let receive = || {
        let line = events
            .recv_timeout(Duration::from_secs(25))
            .unwrap()
            .unwrap();
        serde_json::from_str::<Value>(&line).unwrap()
    };
    let ready = || {
        let reply = receive();
        assert_eq!(reply["data"]["status"], "ready", "{reply}");
        reply
    };
    ready();
    let temp = dir.join(".next.ts");
    std::fs::write(&temp, "const = invalid;").unwrap();
    std::fs::rename(&temp, &worker).unwrap();
    let rejected = receive();
    assert_eq!(
        rejected["error"]["code"], "plugin_script_failed",
        "{rejected}"
    );
    assert!(
        rejected["error"]["message"]
            .as_str()
            .unwrap()
            .contains("worker.ts:1:")
    );
    let invoke = || {
        f.op(
            1,
            "reload-measurement.hello",
            json!({"space_id":space,"reads":true}),
        )
    };
    let previous = invoke().await;
    previous.ok();
    assert!(
        previous.body["data"]["view"]["source"]
            .as_str()
            .unwrap()
            .contains("Ready")
    );
    let mut samples = Vec::new();
    for trial in 0..if remote { 32 } else { 4 } {
        let expected = format!("Reload {trial}");
        let edit = source.replace(
            "Metric(\"Plugin\", \"Ready\")",
            &format!("Metric(\"Plugin\", \"{expected}\")"),
        );
        assert_ne!(edit, source);
        std::fs::write(&temp, edit).unwrap();
        let start = Instant::now();
        std::fs::rename(&temp, &worker).unwrap();
        let activation = ready();
        let ready_ms = start.elapsed().as_secs_f64() * 1000.0;
        let reply = invoke().await;
        let full_ms = start.elapsed().as_secs_f64() * 1000.0;
        reply.ok();
        assert!(
            reply.body["data"]["view"]["source"]
                .as_str()
                .unwrap()
                .contains(&expected)
        );
        assert_eq!(f.child.as_ref().unwrap().id(), server_pid);
        if trial >= 2 {
            samples.push(json!({
                "edit_to_result_ms":full_ms,
                "edit_to_ready_ms":ready_ms,
                "build_ms":activation["data"]["build_ms"],
                "activation_ms":activation["data"]["activation_ms"],
                "ready_to_result_ms":full_ms-ready_ms,
            }));
        }
    }
    let percentile = |field: &str, rank: usize| {
        let mut values = samples
            .iter()
            .map(|s| s[field].as_f64().unwrap())
            .collect::<Vec<_>>();
        values.sort_by(f64::total_cmp);
        values[(values.len() * rank).div_ceil(100) - 1]
    };
    let summary = json!({
        "mode":mode.to_string_lossy(),
        "p50_ms":percentile("edit_to_result_ms", 50),
        "p95_ms":percentile("edit_to_result_ms", 95),
        "p99_ms":percentile("edit_to_result_ms", 99),
        "edit_to_ready_p95_ms":percentile("edit_to_ready_ms", 95),
        "build_p95_ms":percentile("build_ms", 95),
        "activation_p95_ms":percentile("activation_ms", 95),
        "ready_to_result_p95_ms":percentile("ready_to_result_ms", 95),
        "samples":samples
    });
    eprintln!("PLUGIN_RELOAD_MEASUREMENTS {summary}");
    if remote {
        assert!(percentile("edit_to_result_ms", 95) < 1000.0, "{summary}");
    }
}
