//! Account-free provider proof in the shipped container layout, when its fixture image exists.
mod machines_common;
#[path = "../../../tests/support/podman_guard.rs"]
mod podman_guard;
use machines_common::*;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread")]
async fn fake_provider_streams_through_podman_and_the_mounted_operation_cli() {
    let Ok(image) = std::env::var("CALTERNAL_TEST_PROVIDER_IMAGE") else {
        eprintln!("SKIPPED: set CALTERNAL_TEST_PROVIDER_IMAGE to the account-free fixture image");
        return;
    };
    let data = TempDir::new("provider-container-data");
    let mut server = Server::start_with_standard_timing(
        &data.0,
        free_port(),
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins"),
    );
    let mdir = TempDir::new("provider-container-machine");
    let code = server.code(&server.owner).await;
    assert_eq!(enrol(&server.url(), &code, &mdir.0), 0);
    let _containers = podman_guard::PodmanGuard::new(mdir.0.join("sandboxes"));
    let _machine = Proc(
        Command::new(BIN)
            .args([
                "machine",
                "run",
                "--backend",
                "podman",
                "--image",
                &image,
                "--dir",
            ])
            .arg(&mdir.0)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    wait_online(&server, &server.owner, 1).await;
    let space = server.owned_space(&server.owner).await;
    let account = Uuid::parse_str(&server.principal(&server.owner).await.0).unwrap();
    let thread = Uuid::new_v4();
    let mut command = tokio::process::Command::new(BIN);
    command.kill_on_drop(true);
    command
        .args([
            "--json",
            "agent",
            "--base-url",
            &server.url(),
            "--token-file",
        ])
        .arg(data.join("appdata/dev-tokens/owner.token"))
        .args([
            "run",
            "--space-id",
            &space,
            "--thread-id",
            &thread.to_string(),
            "--prompt",
            "hello",
        ]);
    let out = tokio::time::timeout(Duration::from_secs(30), command.output())
        .await
        .unwrap()
        .unwrap();
    assert!(
        out.status.success(),
        "{} / {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(!stdout.contains("PRIVATE_LOGIN_SENTINEL"));
    let frames: Vec<Value> = stdout
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert!(frames.iter().any(|v| v["event"]["type"] == "text_delta"));
    assert!(frames.iter().any(|v| v["event"]["type"] == "tool_finished"));
    let job = &frames.last().unwrap()["job"];
    assert_eq!(job["state"], "done");
    wait_container_removed(account, thread, Duration::from_secs(20)).await;
    let root = mdir.join(&format!("sandboxes/accounts/{account}"));
    assert!(root.join("threads").join(thread.to_string()).exists());
    let id = job["id"].as_str().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        let (status,v)=server.call(&server.owner,"/api/v1/operations",Some(json!({"request_id":Uuid::new_v4(),"operation":"agent.discard","input":{"job_id":id}}))).await;
        assert_eq!(status, 200, "{v}");
        if v["data"]["discard"] == "done" {
            break;
        }
        assert_eq!(v["data"]["discard"], "pending", "{v}");
        assert!(std::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(!root.join("threads").join(thread.to_string()).exists());
    assert!(root.join("home").exists());

    let live_thread = Uuid::new_v4();
    let run = server.request_run(&server.owner, &space, live_thread).await;
    let lease = run["lease_id"].as_str().unwrap();
    server
        .wait_run(&server.owner, lease, Duration::from_secs(20), |v| {
            v["state"] == "running"
        })
        .await;
    let exec = server
        .exec(
            &server.owner,
            lease,
            &["sh", "-c", "echo retained > marker"],
        )
        .await;
    server
        .output(&server.owner, &exec, Duration::from_secs(20))
        .await;
    server.proc.signal("-TERM");
    wait_container_removed(account, live_thread, Duration::from_secs(20)).await;
    assert_eq!(
        std::fs::read_to_string(
            root.join("threads")
                .join(live_thread.to_string())
                .join("work/marker")
        )
        .unwrap(),
        "retained\n"
    );
    assert_eq!(server.proc.wait_exit(Duration::from_secs(25)), Some(0));
}
