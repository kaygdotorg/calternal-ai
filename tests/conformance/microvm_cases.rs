//! Real pool machine routing and account boundary. Requires the pinned KVM guest.

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
    let opened = f
        .req(
            "POST",
            "/api/v1/machines/runs",
            Some(&f.cookies[who]),
            json!({"space_id": f.spaces[space], "thread_id": uuid::Uuid::new_v4()}),
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
    if let Err(error) = calternal_machine::sandbox::microvm::probe_kvm().await
    {
        eprintln!("SKIP Firecracker conformance: {error}");
        return;
    }
    let Some(config) = std::env::var_os("CALTERNAL_MICROVM_TEST_CONFIG") else {
        eprintln!("SKIP Firecracker conformance: CALTERNAL_MICROVM_TEST_CONFIG is unset");
        return;
    };

    let f = Fixture::new().await;
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
    let a = run(&f, 1, 0).await;
    let b = run(&f, 2, 3).await;
    let a_exec = exec(
        &f,
        1,
        &a,
        "echo private > /work/file; cat /proc/sys/kernel/random/boot_id",
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

#[tokio::test]
async fn container_fallback_is_visible_only_to_entitled_accounts() {
    let f = Fixture::new().await;
    machine_admin(&f, &["pool-grant", &f.ids[1]]);
    let code = machine_admin(&f, &["code", "--pool"]);
    let mut pool = requests_cases::Machine::enroll_with_code(&f, code["code"].as_str().unwrap()).await;
    assert_eq!(
        pool.connect_as(&f, calternal_machine::protocol::PODMAN_KVM_FALLBACK).await,
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
