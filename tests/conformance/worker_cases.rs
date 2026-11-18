//! Real enrolled-machine plugin workers: authority stays with the host broker.
use super::*;
#[path = "../support/podman_guard.rs"]
mod podman_guard;

struct Machine {
    process: Child,
    dir: PathBuf,
    account: String,
    _containers: podman_guard::PodmanGuard,
}
impl Drop for Machine {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

impl Fixture {
    fn worker_admin(&self, arguments: &[&str]) -> Value {
        let result = Command::new(std::env::var("CALTERNAL_SERVER_BIN").unwrap())
            .arg("machines")
            .args(arguments)
            .arg("--appdata")
            .arg(self.appdata())
            .arg("--userdata-ssd")
            .arg(self.dir.path().join("userdata-ssd"))
            .arg("--userdata-hdd")
            .arg(self.dir.path().join("userdata-hdd"))
            .args([
                "--installation-id",
                "00000000-0000-0000-0000-000000000001",
                "--json",
            ])
            .output()
            .unwrap();
        assert!(result.status.success(), "worker administration failed");
        serde_json::from_slice(&result.stdout).unwrap()
    }
    async fn worker_machine(&mut self, delay_first_launch: bool) -> Machine {
        self.stop();
        self.remote_workers = true;
        self.start().await;
        self.worker_admin(&["pool-grant", &self.ids[1]]);
        let code = self.worker_admin(&["code", "--pool"]);
        let dir = self.dir.path().join("worker-machine");
        let binary = std::env::var("CALTERNAL_SERVER_BIN").unwrap();
        let enroll = Command::new(&binary)
            .args(["machine", "enroll", "--server", &self.base])
            .arg("--dir")
            .arg(&dir)
            .env(
                "CALTERNAL_ENROLL_CODE",
                code["data"]["code"].as_str().unwrap(),
            )
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .status()
            .unwrap();
        assert!(enroll.success());
        let containers = podman_guard::PodmanGuard::new(dir.join("sandboxes"));
        let mut command = Command::new(binary);
        command.args([
            "machine",
            "run",
            "--backend",
            "podman",
            "--max-sandboxes",
            "8",
        ]);
        if delay_first_launch {
            use std::os::unix::fs::PermissionsExt;
            let podman = dir.join("podman");
            // Delay exactly one worker bootstrap past the normal warm deadline.
            std::fs::write(dir.join("first-launch"), "").unwrap();
            std::fs::write(
                &podman,
                r#"#!/bin/sh
if [ "$1" = run ] && rm "${0%/*}/first-launch" 2>/dev/null; then
    machine=$PPID
    sleep 3.2
    kill -0 "$machine" 2>/dev/null || exit 1
fi
exec podman "$@"
"#,
            )
            .unwrap();
            std::fs::set_permissions(&podman, std::fs::Permissions::from_mode(0o700)).unwrap();
            command.arg("--podman").arg(podman);
        }
        let child = command
            .arg("--dir")
            .arg(&dir)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let machine = Machine {
            process: child,
            dir,
            account: self.ids[1].clone(),
            _containers: containers,
        };
        machine
    }
}

#[tokio::test]
async fn remote_worker_keeps_broker_authority_and_restarts_after_container_death() {
    if !calternal_machine::sandbox::podman::PodmanBackend::available(Path::new("podman")).await {
        eprintln!("SKIP remote worker conformance: Podman unavailable");
        return;
    }
    let mut f = Fixture::new().await;
    // The optional fault injection proves the deadline boundary separately from
    // the standard conformance run, where it would consume the startup allowance.
    let delay_launch = std::env::var_os("CALTERNAL_TEST_DELAY_WORKER_LAUNCH").is_some();
    let machine = f.worker_machine(delay_launch).await;
    jobs_cases::enable(&f, 1, &f.spaces[0]).await;
    let write = || json!({"space_id":f.spaces[0],"text":"worker-private-space-one"});
    let end = Instant::now() + Duration::from_secs(10);
    let cold_ms;
    loop {
        let started = Instant::now();
        let first = f.op(1, "jobs-demo.write", write()).await;
        if first.status == 200 {
            cold_ms = started.elapsed().as_secs_f64() * 1000.0;
            break;
        }
        assert_ne!(
            first.body["error"]["message"], "extension timed out",
            "first invocation timed out during a cold launch: {}",
            first.body
        );
        if delay_launch {
            assert!(
                machine.dir.join("first-launch").is_file(),
                "first invocation failed after starting a cold launch: {}",
                first.body
            );
        }
        assert!(
            Instant::now() < end,
            "worker machine never admitted the call: {}",
            first.body
        );
        assert_eq!(first.body["error"]["code"], "unavailable");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    if delay_launch {
        assert!(cold_ms >= 3200.0, "first launch was not delayed");
    }
    if std::env::var_os("CALTERNAL_MEASURE_WORKERS").is_some() {
        let mut samples = Vec::new();
        for _ in 0..10 {
            let started = Instant::now();
            f.op(1, "jobs-demo.write", write()).await.ok();
            samples.push(started.elapsed().as_secs_f64() * 1000.0);
        }
        samples.sort_by(f64::total_cmp);
        eprintln!(
            "REMOTE_WORKER_MEASUREMENTS {}",
            json!({"operation":"jobs-demo.write","host_calls":1,"cold_ms":cold_ms,"warm_calls":10,"warm_ms_p50":samples[4],"warm_ms_p95":samples[9]})
        );
    }
    // Internal lease IDs can be known; they still grant no public machine access.
    let db = Connection::open(f.appdata().join("system/machines.db")).unwrap();
    let lease: String = db
        .query_row(
            "SELECT id FROM leases WHERE worker=1 AND state!='ended'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    drop(db);
    for who in [1, 2] {
        for (method, suffix, body) in [
            ("GET", "", json!({})),
            ("POST", "/stop", json!({})),
            (
                "POST",
                "/execs",
                json!({"argv":["cat","/plugin/worker.js"]}),
            ),
        ] {
            let answer = f
                .req(
                    method,
                    &format!("/api/v1/machines/runs/{lease}{suffix}"),
                    Some(&f.cookies[who]),
                    body,
                    Some(ORIGIN),
                )
                .await;
            assert_eq!(
                answer.status, 404,
                "worker lease was publicly addressable: {}",
                answer.body
            );
        }
    }
    let foreign = f.op(2, "jobs-demo.write", write()).await;
    let absent = f
        .op(
            2,
            "jobs-demo.write",
            json!({"space_id":uuid::Uuid::now_v7(),"text":"worker-private-space-one"}),
        )
        .await;
    assert_eq!(foreign.status, 404);
    assert_eq!(foreign.signature(), absent.signature());
    let dirs = std::fs::read_dir(machine.dir.join("sandboxes/workers"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(dirs.len(), 1, "one worker per plugin/space");
    let name = format!(
        "calternal-account-{}-thread-{}",
        machine.account,
        dirs[0].file_name().to_string_lossy()
    );
    assert!(
        Command::new("podman")
            .args(["kill", "--signal", "KILL", &name])
            .stdout(Stdio::null())
            .status()
            .unwrap()
            .success()
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    // A lost invocation may fail once; a later call must start a clean worker.
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let next = f.op(1, "jobs-demo.write", write()).await;
        if next.status == 200 {
            break;
        }
        assert_eq!(next.body["error"]["code"], "unavailable");
        assert!(
            Instant::now() < end,
            "worker did not restart: {}",
            next.body
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let dirs = std::fs::read_dir(machine.dir.join("sandboxes/workers"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(dirs.len(), 1);
    let name = format!(
        "calternal-account-{}-thread-{}",
        machine.account,
        dirs[0].file_name().to_string_lossy()
    );
    assert!(
        Command::new("podman")
            .args(["pause", &name])
            .stdout(Stdio::null())
            .status()
            .unwrap()
            .success()
    );
    let hung = f.op(1, "jobs-demo.write", write()).await;
    assert_eq!(hung.body["error"]["code"], "unavailable");
    assert!(
        hung.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("timed out")
    );
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        let next = f.op(1, "jobs-demo.write", write()).await;
        if next.status == 200 {
            break;
        }
        assert_eq!(next.body["error"]["code"], "unavailable");
        assert!(
            Instant::now() < end,
            "hung worker did not restart: {}",
            next.body
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    evidence(
        "machine-worker-isolation",
        json!({"foreign_missing_equal":true,"container_replaced":true,"hung_worker_replaced":true,"protocol_version":2}),
    );
}

#[path = "reload_measurement.rs"]
mod reload_measurement;
