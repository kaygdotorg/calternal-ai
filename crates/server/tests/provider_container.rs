//! Account-free provider proof in the shipped container layout, when its fixture image exists.
mod machines_common;
use machines_common::*;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};
use uuid::Uuid;

// Every fixture has a fresh account UUID. Remove only that account's test containers.
struct Containers(Uuid);
impl Drop for Containers {
    fn drop(&mut self) {
        if let Ok(out) = Command::new("podman")
            .args([
                "ps",
                "-aq",
                "--filter",
                &format!("name=calternal-account-{}-thread-", self.0),
            ])
            .output()
        {
            let ids = String::from_utf8_lossy(&out.stdout);
            if !ids.trim().is_empty() {
                let _ = Command::new("podman")
                    .args(["rm", "-f", "--ignore"])
                    .args(ids.split_whitespace())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn fake_provider_streams_through_podman_and_the_mounted_operation_cli() {
    let Ok(image) = std::env::var("CALTERNAL_TEST_PROVIDER_IMAGE") else {
        eprintln!("SKIPPED: set CALTERNAL_TEST_PROVIDER_IMAGE to the account-free fixture image");
        return;
    };
    let data = TempDir::new("provider-container-data");
    let server = Server::start_with_plugins(
        &data.0,
        free_port(),
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins"),
    );
    let mdir = TempDir::new("provider-container-machine");
    let code = server.code(&server.owner).await;
    assert_eq!(enrol(&server.url(), &code, &mdir.0), 0);
    let calternal_machine::protocol::Owner::Account { id: account } =
        calternal_machine::identity::Credentials::load(&mdir.0)
            .unwrap()
            .owner
    else {
        panic!("account machine")
    };
    let _containers = Containers(account.as_uuid());
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
    let root = mdir.join(&format!("sandboxes/accounts/{account}"));
    assert!(!root.join("threads").join(thread.to_string()).exists());
    assert!(root.join("home").exists());
}
