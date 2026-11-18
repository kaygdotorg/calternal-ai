//! Pool admission and live routing, account isolation and lifecycle with the pinned KVM guest.

use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use erasure_cases::machine_admin;

struct Role(Child);

impl Drop for Role {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn run(f: &Fixture, who: usize, space: usize) -> String {
    run_thread(f, who, space, uuid::Uuid::new_v4()).await
}

async fn run_thread(f: &Fixture, who: usize, space: usize, thread: uuid::Uuid) -> String {
    let opened = f
        .req(
            "POST",
            "/api/v1/machines/runs",
            Some(&f.cookies[who]),
            json!({"space_id": f.spaces[space], "thread_id": thread}),
            Some(ORIGIN),
        )
        .await;
    opened.ok();
    opened.body["lease_id"].as_str().unwrap().to_owned()
}

async fn exec(f: &Fixture, who: usize, lease: &str, script: &str) -> String {
    let started = f
        .req(
            "POST",
            &format!("/api/v1/machines/runs/{lease}/execs"),
            Some(&f.cookies[who]),
            json!({"argv": ["sh", "-ec", script]}),
            Some(ORIGIN),
        )
        .await;
    started.ok();
    started.body["exec_id"].as_str().unwrap().to_owned()
}

async fn output(f: &Fixture, who: usize, exec: &str) -> String {
    let path = format!("/api/v1/machines/execs/{exec}/output");
    let mut next = 0;
    let mut stdout = Vec::new();
    tokio::time::timeout(Duration::from_secs(90), async {
        loop {
            let answer = f
                .req(
                    "GET",
                    &format!("{path}?after={next}&wait_ms=1000"),
                    Some(&f.cookies[who]),
                    json!({}),
                    Some(ORIGIN),
                )
                .await;
            answer.ok();
            for chunk in answer.body["chunks"].as_array().unwrap() {
                if chunk["kind"] == "data" && chunk["stream"] == "stdout" {
                    stdout.extend(STANDARD.decode(chunk["data"].as_str().unwrap()).unwrap());
                }
            }
            next = answer.body["next"].as_u64().unwrap();
            if !answer.body["exit"].is_null() {
                assert_eq!(answer.body["exit"]["code"], 0, "{}", answer.body);
                return String::from_utf8(stdout).unwrap();
            }
        }
    })
    .await
    .expect("microVM exec did not end")
}

#[tokio::test]
async fn microvm_pool_keeps_account_execs_and_boot_instances_separate() {
    if let Err(error) = calternal_machine::sandbox::microvm::probe_kvm().await {
        eprintln!("SKIP Firecracker conformance: {error}");
        return;
    }
    let Some(config) = std::env::var_os("CALTERNAL_MICROVM_TEST_CONFIG") else {
        eprintln!("SKIP Firecracker conformance: CALTERNAL_MICROVM_TEST_CONFIG is unset");
        return;
    };

    let mut f = Fixture::new().await;
    for who in [1, 2] {
        machine_admin(&f, &["pool-grant", &f.ids[who]]);
    }
    let code = machine_admin(&f, &["code", "--pool"]);
    let dir = f.dir.path().join("vm");
    let binary = std::env::var_os("CALTERNAL_SERVER_BIN").unwrap();
    let enrolled = Command::new(&binary)
        .args(["machine", "enroll", "--server", &f.base])
        .arg("--dir")
        .arg(&dir)
        .env("CALTERNAL_ENROLL_CODE", code["code"].as_str().unwrap())
        .output()
        .unwrap();
    assert!(
        enrolled.status.success(),
        "{}",
        String::from_utf8_lossy(&enrolled.stderr)
    );
    let mut machine = Role(
        Command::new(&binary)
            .args(["machine", "run", "--backend", "microvm", "--dir"])
            .arg(&dir)
            .arg("--microvm-config")
            .arg(config)
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let thread = uuid::Uuid::new_v4();
    let a = run_thread(&f, 1, 0, thread).await;
    let b = run(&f, 2, 3).await;
    let a_exec = exec(
        &f,
        1,
        &a,
        "echo private > /work/file; sync; cat /proc/sys/kernel/random/boot_id",
    )
    .await;
    let a_boot = output(&f, 1, &a_exec).await;
    let b_exec = exec(
        &f,
        2,
        &b,
        "test ! -e /work/file; cat /proc/sys/kernel/random/boot_id",
    )
    .await;
    let b_boot = output(&f, 2, &b_exec).await;
    assert_ne!(a_boot, b_boot);
    let sibling = run(&f, 1, 3).await;
    let sibling_exec = exec(
        &f,
        1,
        &sibling,
        "test ! -e /work/file; cat /proc/sys/kernel/random/boot_id",
    )
    .await;
    assert_eq!(output(&f, 1, &sibling_exec).await, a_boot);
    f.req(
        "POST",
        &format!("/api/v1/machines/runs/{sibling}/stop"),
        Some(&f.cookies[1]),
        json!({}),
        Some(ORIGIN),
    )
    .await
    .ok();
    let foreign = f
        .req(
            "GET",
            &format!("/api/v1/machines/execs/{a_exec}/output"),
            Some(&f.cookies[2]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(foreign.status, 404);

    let disk = dir
        .join("sandboxes/microvm/accounts")
        .join(format!("{}.ext4", f.ids[1]));
    assert!(disk.exists());
    let active = exec(&f, 1, &a, "echo active; exec sleep 120").await;
    let path = format!("/api/v1/machines/execs/{active}/output");
    let cursor = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let answer = f
                .req(
                    "GET",
                    &format!("{path}?after=0&wait_ms=1000"),
                    Some(&f.cookies[1]),
                    json!({}),
                    Some(ORIGIN),
                )
                .await;
            answer.ok();
            assert!(answer.body["exit"].is_null(), "{}", answer.body);
            if let Some(next) = answer.body["next"].as_u64().filter(|&next| next > 0) {
                break next;
            }
        }
    })
    .await
    .expect("guest command did not start");
    kill_guest(&machine, &disk);
    let exit = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let answer = f
                .req(
                    "GET",
                    &format!("{path}?after={cursor}&wait_ms=1000"),
                    Some(&f.cookies[1]),
                    json!({}),
                    Some(ORIGIN),
                )
                .await;
            answer.ok();
            if !answer.body["exit"].is_null() {
                break answer.body["exit"].clone();
            }
        }
    })
    .await
    .expect("guest loss did not end the command");
    assert_eq!(
        exit["error"],
        calternal_machine::protocol::GUEST_LOST,
        "{exit}"
    );
    let interrupted = f
        .req(
            "GET",
            &format!("/api/v1/machines/runs/{a}"),
            Some(&f.cookies[1]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    interrupted.ok();
    assert_eq!(interrupted.body["state"], "interrupted");

    // Close the interrupted direct machine run before opening the same thread.
    // Provider jobs use their separate explicit checkpoint resume operation.
    f.req(
        "POST",
        &format!("/api/v1/machines/runs/{a}/stop"),
        Some(&f.cookies[1]),
        json!({}),
        Some(ORIGIN),
    )
    .await
    .ok();
    let recovered = run_thread(&f, 1, 0, thread).await;
    let read_back = exec(
        &f,
        1,
        &recovered,
        "cat /work/file; cat /proc/sys/kernel/random/boot_id",
    )
    .await;
    let recovered_text = output(&f, 1, &read_back).await;
    let (file, boot_id) = recovered_text.split_once('\n').unwrap();
    assert_eq!(file, "private");
    assert_ne!(boot_id, a_boot);
    assert_ne!(boot_id, b_boot);

    for space in &f.spaces[2..] {
        f.op(
            1,
            "spaces.transfer",
            json!({"space_id":space,"account_id":f.ids[2]}),
        )
        .await
        .ok();
    }
    erasure_cases::reverify(&mut f, 0).await;
    f.auth(
        "account/delete",
        Some(&f.cookies[0]),
        json!({"account_id":f.ids[1]}),
    )
    .await
    .ok();
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let pending: i64 = Connection::open(f.appdata().join("system/machines.db"))
                .unwrap()
                .query_row(
                    "SELECT COUNT(*) FROM machine_erasures WHERE thread=char()",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            if pending == 0 && !disk.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("durable erasure did not remove the account disk");
    let still_live = exec(&f, 2, &b, "cat /proc/sys/kernel/random/boot_id").await;
    assert_eq!(output(&f, 2, &still_live).await, b_boot);

    let status = Command::new("kill")
        .args(["-TERM", &machine.0.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let ended = tokio::time::timeout(Duration::from_secs(80), async {
        loop {
            if let Some(status) = machine.0.try_wait().unwrap() {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(ended.success());
    assert!(
        !dir.join("sandboxes/microvm/run").exists(),
        "guest runtime leaked"
    );
}

fn kill_guest(machine: &Role, disk: &Path) {
    let parent = machine.0.id().to_string();
    let guest = std::fs::read_dir("/proc")
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            std::fs::read_link(path.join("exe")).is_ok_and(|bin| bin.ends_with("firecracker"))
                && std::fs::read_to_string(path.join("status")).is_ok_and(|status| {
                    status.lines().any(|line| {
                        line.strip_prefix("PPid:")
                            .is_some_and(|pid| pid.trim() == parent)
                    })
                })
                && std::fs::read_dir(path.join("fd")).is_ok_and(|fds| {
                    fds.filter_map(Result::ok)
                        .any(|fd| std::fs::read_link(fd.path()).is_ok_and(|file| file == disk))
                })
        })
        .expect("fixture account's Firecracker process");
    assert!(
        Command::new("kill")
            .arg("-KILL")
            .arg(guest.file_name().unwrap())
            .status()
            .unwrap()
            .success()
    );
}

#[tokio::test]
async fn container_fallback_is_visible_only_to_entitled_accounts() {
    let f = Fixture::new().await;
    machine_admin(&f, &["pool-grant", &f.ids[1]]);
    let code = machine_admin(&f, &["code", "--pool"]);
    let mut pool =
        requests_cases::Machine::enroll_with_code(&f, code["code"].as_str().unwrap()).await;
    assert_eq!(
        pool.connect_as(&f, calternal_machine::protocol::PODMAN_KVM_FALLBACK)
            .await,
        "podman",
    );
    let input = json!({"space_id":f.spaces[3]});
    let own = f.op(1, "agent.readiness", input.clone()).await;
    own.ok();
    let row = &own.body["data"]["pool"][0];
    assert_eq!(row["execution"], "sandboxed");
    assert_eq!(row["sandbox_fallback"], "kvm_unavailable");
    let foreign = f.op(2, "agent.readiness", input).await;
    foreign.ok();
    assert_eq!(foreign.body["data"]["pool"], json!([]));
}

#[tokio::test]
async fn small_pool_guests_queue_a_third_thread_across_spaces() {
    let f = Fixture::new().await;
    for who in [1, 2] {
        machine_admin(&f, &["pool-grant", &f.ids[who]]);
    }
    let code = machine_admin(&f, &["code", "--pool"]);
    let mut pool =
        requests_cases::Machine::enroll_with_code(&f, code["code"].as_str().unwrap()).await;
    assert_eq!(
        pool.connect_sized(&f, "microvm", Some(512)).await,
        "microvm"
    );
    let (_, first) = tokio::join!(pool.sync_with_slots(&f, json!([]), 10), run(&f, 1, 0));
    let (second, third) = tokio::join!(run(&f, 1, 3), run(&f, 1, 0));
    let foreign = run(&f, 2, 3).await;
    let mut waiting = None;
    let mut assigned = Vec::new();
    for lease in [first, second, third] {
        let view = f
            .req(
                "GET",
                &format!("/api/v1/machines/runs/{lease}"),
                Some(&f.cookies[1]),
                json!({}),
                Some(ORIGIN),
            )
            .await;
        view.ok();
        if view.body["state"] == "waiting" {
            assert_eq!(view.body["queue_position"], 1);
            assert!(waiting.replace(lease).is_none());
        } else {
            assert_eq!(view.body["state"], "assigned");
            assigned.push(lease);
        }
    }
    assert_eq!(assigned.len(), 2);
    let queued = waiting.expect("the third thread must be queued");
    let unrelated = f
        .req(
            "GET",
            &format!("/api/v1/machines/runs/{foreign}"),
            Some(&f.cookies[2]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    unrelated.ok();
    assert_eq!(unrelated.body["state"], "assigned");
    let later = run(&f, 1, 3).await;
    let cancel = f
        .req(
            "POST",
            &format!("/api/v1/machines/runs/{later}/stop"),
            Some(&f.cookies[1]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    cancel.ok();
    assert_eq!(cancel.body["state"], "ended");
    let stop = f
        .req(
            "POST",
            &format!("/api/v1/machines/runs/{}/stop", assigned[0]),
            Some(&f.cookies[1]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    stop.ok();
    let result = pool.sync_with_slots(&f, json!([]), 10).await;
    assert!(
        !result
            .iter()
            .any(|r| r["type"] == "assign" && r["lease_id"] == later)
    );
    let stop_index = result
        .iter()
        .position(|r| r["type"] == "stop" && r["lease_id"] == assigned[0])
        .unwrap();
    let assign_index = result
        .iter()
        .position(|r| r["type"] == "assign" && r["lease_id"] == queued)
        .unwrap();
    assert!(stop_index < assign_index);
}
