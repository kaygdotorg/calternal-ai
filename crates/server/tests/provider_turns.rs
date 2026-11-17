//! Official CLI fixture through the real server, machine role and provider adapter.
mod machines_common;
#[path = "provider_turns/readiness.rs"]
mod readiness;
#[path = "provider_turns/saved_threads.rs"]
mod saved_threads;
use machines_common::*;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
use uuid::Uuid;

fn require_adapter_build() {
    let built = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/calternal-claude-code/dist/main.js");
    assert!(
        built.is_file() && built.with_file_name("adapter.js").is_file(),
        "Claude Code adapter build is missing; run npm run build:plugins."
    );
}

async fn op(server: &Server, operation: &str, input: Value) -> Value {
    op_as(server, &server.owner, operation, input).await
}
async fn op_as(server: &Server, token: &str, operation: &str, input: Value) -> Value {
    let (status, v) = server
        .call(
            token,
            "/api/v1/operations",
            Some(json!({"request_id":Uuid::new_v4(),"operation":operation,"input":input})),
        )
        .await;
    assert_eq!(status, 200, "{operation}: {v}");
    v["data"].clone()
}
fn machine_with_adapter(dir: &Path, bin: &Path) -> Proc {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = format!(
        "{}:{}:{}",
        bin.display(),
        root.join("plugins/calternal-claude-code/bin").display(),
        std::env::var("PATH").unwrap()
    );
    Proc(
        Command::new(BIN)
            .args(["machine", "run", "--backend", "direct", "--dir"])
            .arg(dir)
            .env("PATH", path)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}
async fn until(
    server: &Server,
    id: &str,
    predicate: impl Fn(&Value) -> bool,
) -> (Value, Vec<Value>) {
    let end = std::time::Instant::now() + Duration::from_secs(20);
    let (mut next, mut epoch, mut events) = (0u64, String::new(), vec![]);
    loop {
        let value = op(
            server,
            "agent.events",
            json!({"job_id":id,"after":next,"epoch":epoch}),
        )
        .await;
        next = value["next"].as_u64().unwrap();
        epoch = value["epoch"].as_str().unwrap().into();
        events.extend(
            value["events"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["data"].clone()),
        );
        if predicate(&value["job"]) {
            return (value["job"].clone(), events);
        }
        assert!(std::time::Instant::now() < end, "{value}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn fake_official_cli_streams_checkpoints_crashes_stops_and_requires_explicit_resume() {
    require_adapter_build();
    let data = TempDir::new("provider-data");
    let server = Server::start_with_plugins(
        &data.0,
        free_port(),
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins"),
    );
    let bin = TempDir::new("provider-bin");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/calternal-claude-code/test/fake-claude.mjs");
    let fixture = std::fs::read_to_string(fixture).unwrap();
    let fixture=fixture.replace("const scenarioFile=", r#"if(prompt.includes('checkpoint-story')) {for(let i=0;i<6;i++)emit({type:'stream_event',event:{type:'content_block_delta',delta:{type:'text_delta',text:'segment-'+i+':'+String(i).repeat(8192)+'\n'}}});}
const scenarioFile="#);
    std::fs::write(bin.join("claude"), fixture).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(bin.join("claude"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mdir = TempDir::new("provider-machine");
    let code = server.code(&server.owner).await;
    assert_eq!(enrol(&server.url(), &code, &mdir.0), 0);
    let machine = machine_with_adapter(&mdir.0, &bin.0);
    wait_online(&server, &server.owner, 1).await;
    let space = server.owned_space(&server.owner).await;
    let initial_thread = Uuid::new_v4();
    let start = op(
        &server,
        "agent.run",
        json!({"space_id":space,"thread_id":initial_thread,"prompt":"hello"}),
    )
    .await;
    let id = start["job"]["id"].as_str().unwrap();
    let (job, events) = until(&server, id, |j| {
        j["state"] == "done" || j["state"] == "outcome_unknown"
    })
    .await;
    assert_eq!(job["state"], "done", "{job}; {events:?}");
    assert!(
        events.iter().any(|e| e["event"]["type"] == "text_delta"),
        "{events:?}"
    );
    assert!(events.iter().any(|e| e["event"]["type"] == "tool_finished"));
    assert!(job["checkpoint"]["session_id"].is_string());
    assert!(job["checkpoint_seq"].as_u64().unwrap() >= 4);
    assert!(
        !serde_json::to_string(&events)
            .unwrap()
            .contains("PRIVATE_LOGIN_SENTINEL")
    );
    saved_threads::exercise(&server, &data, &space).await;
    // Exercise the public CLI through the full transport in both output modes.
    for json_output in [false, true] {
        let mut command = tokio::process::Command::new(BIN);
        command.kill_on_drop(true);
        if json_output {
            command.arg("--json");
        }
        command
            .args(["agent", "--base-url", &server.url(), "--token-file"])
            .arg(data.join("appdata/dev-tokens/owner.token"))
            .args([
                "run",
                "--space-id",
                &space,
                "--thread-id",
                &Uuid::new_v4().to_string(),
                "--prompt",
                "hello",
            ]);
        if json_output {
            command.args(["--provider", "calternal-claude-code"]);
        }
        let output = tokio::time::timeout(Duration::from_secs(20), command.output())
            .await
            .unwrap()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        if json_output {
            let frames: Vec<Value> = stdout
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            assert!(frames.iter().any(|e| e["event"]["type"] == "text_delta"));
            assert!(frames.iter().any(|e| e["event"]["type"] == "tool_finished"));
            assert_eq!(frames.last().unwrap()["job"]["state"], "done");
        } else {
            assert!(stdout.contains("Hello 🌏"));
            assert!(stdout.contains("Step:"));
        }
    }
    let calternal_machine::protocol::Owner::Account { id: account } =
        calternal_machine::identity::Credentials::load(&mdir.0)
            .unwrap()
            .owner
    else {
        panic!("account machine")
    };
    let workspace = mdir.join(&format!(
        "sandboxes/accounts/{account}/threads/{initial_thread}"
    ));
    let home = mdir.join(&format!("sandboxes/accounts/{account}/home"));
    assert!(workspace.exists());
    // The machine owner's local command refuses deletion while its role is running.
    let local = tokio::process::Command::new(BIN)
        .kill_on_drop(true)
        .args([
            "machine",
            "discard",
            "--backend",
            "direct",
            "--account-id",
            &account.to_string(),
            "--thread-id",
            &initial_thread.to_string(),
            "--dir",
        ])
        .arg(&mdir.0)
        .output()
        .await
        .unwrap();
    assert!(!local.status.success());
    assert!(workspace.exists());
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        let discarded = op(&server, "agent.discard", json!({"job_id":id})).await;
        if discarded["discard"] == "done" {
            break;
        }
        assert_eq!(discarded["discard"], "pending", "{discarded}");
        assert!(std::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(!workspace.exists());
    assert!(home.exists(), "discard removed account login home");
    // Check runs in the same persistent account home and reveals only status.
    let check = op(
        &server,
        "agent.check",
        json!({"space_id":space,"thread_id":Uuid::new_v4()}),
    )
    .await;
    let (_, events) = until(&server, check["job"]["id"].as_str().unwrap(), |j| {
        j["state"] == "done"
    })
    .await;
    assert!(events.iter().any(|e| e["event"]["type"] == "check"));
    let crash = op(
        &server,
        "agent.run",
        json!({"space_id":space,"thread_id":Uuid::new_v4(),"prompt":"crash"}),
    )
    .await;
    let (job, _) = until(&server, crash["job"]["id"].as_str().unwrap(), |j| {
        j["state"] == "outcome_unknown"
    })
    .await;
    assert_eq!(job["state"], "outcome_unknown");
    let hang = op(
        &server,
        "agent.run",
        json!({"space_id":space,"thread_id":Uuid::new_v4(),"prompt":"hang"}),
    )
    .await;
    let hid = hang["job"]["id"].as_str().unwrap();
    until(&server, hid, |j| j["checkpoint"]["session_id"].is_string()).await;
    op(&server, "agent.stop", json!({"job_id":hid})).await;
    assert_eq!(
        op(&server, "agent.events", json!({"job_id":hid})).await["job"]["state"],
        "cancelled"
    );
    let silent = op(
        &server,
        "agent.run",
        json!({"space_id":space,"thread_id":Uuid::new_v4(),"prompt":"hang"}),
    )
    .await;
    let sid = silent["job"]["id"].as_str().unwrap();
    until(&server, sid, |j| j["checkpoint"]["session_id"].is_string()).await;
    machine.signal("-STOP");
    until(&server, sid, |j| j["state"] == "interrupted").await;
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_eq!(
        op(&server, "agent.events", json!({"job_id":sid})).await["job"]["state"],
        "interrupted"
    );
    let bdir = TempDir::new("provider-resume-machine");
    let code = server.code(&server.owner).await;
    assert_eq!(enrol(&server.url(), &code, &bdir.0), 0);
    let _other = machine_with_adapter(&bdir.0, &bin.0);
    wait_online(&server, &server.owner, 1).await;
    let previous =
        op(&server, "agent.events", json!({"job_id":sid})).await["job"]["checkpoint"]["machine_id"]
            .clone();
    op(&server, "agent.resume", json!({"job_id":sid})).await;
    // Claiming a run marks it running before it saves the new lease checkpoint.
    until(&server, sid, |j| {
        let machine_id = &j["checkpoint"]["machine_id"];
        (j["state"] == "running" || j["state"] == "done")
            && machine_id.is_string()
            && *machine_id != previous
    })
    .await;
    machine.signal("-CONT");
    op(&server, "agent.stop", json!({"job_id":sid})).await;
    drop(machine);
    let scratch = Uuid::new_v4();
    let scratch_dir = mdir.join(&format!(
        "sandboxes/accounts/{account}/threads/{scratch}/work"
    ));
    std::fs::create_dir_all(&scratch_dir).unwrap();
    let local = tokio::process::Command::new(BIN)
        .kill_on_drop(true)
        .args([
            "--json",
            "machine",
            "discard",
            "--backend",
            "direct",
            "--account-id",
            &account.to_string(),
            "--thread-id",
            &scratch.to_string(),
            "--dir",
        ])
        .arg(&mdir.0)
        .output()
        .await
        .unwrap();
    assert!(
        local.status.success(),
        "{}",
        String::from_utf8_lossy(&local.stderr)
    );
    assert!(!scratch_dir.exists());
    assert!(home.exists());
}

fn expedite(data: &Path, space: &str, id: &str) {
    let c = rusqlite::Connection::open(data.join("appdata/spaces").join(space).join("jobs.db"))
        .unwrap();
    c.execute(
        "UPDATE jobs SET checkpoint=json_set(checkpoint,'$.retry.next_attempt_ms',0) WHERE id=?1",
        [id],
    )
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn classified_provider_retries_are_durable_and_do_not_repeat_unknown_effects() {
    require_adapter_build();
    let data = TempDir::new("provider-retry-data");
    let port = free_port();
    let plugins = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let mut server = Server::start_with_plugins(&data.0, port, &plugins);
    let bin = TempDir::new("provider-retry-bin");
    std::fs::copy(
        plugins.join("calternal-claude-code/test/fake-claude.mjs"),
        bin.join("claude"),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(bin.join("claude"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mdir = TempDir::new("provider-retry-machine");
    let code = server.code(&server.owner).await;
    assert_eq!(enrol(&server.url(), &code, &mdir.0), 0);
    let _machine = machine_with_adapter(&mdir.0, &bin.0);
    wait_online(&server, &server.owner, 1).await;
    let space = server.owned_space(&server.owner).await;
    let once = op(
        &server,
        "agent.run",
        json!({"space_id":space,"thread_id":Uuid::new_v4(),"prompt":"transient-once"}),
    )
    .await;
    let id = once["job"]["id"].as_str().unwrap();
    let (job, _) = until(&server, id, |j| j["state"] == "retry_waiting").await;
    assert_eq!(job["checkpoint"]["retry"]["retry_count"], 1);
    expedite(&data.0, &space, id);
    until(&server, id, |j| j["state"] == "done").await;
    let always = op(
        &server,
        "agent.run",
        json!({"space_id":space,"thread_id":Uuid::new_v4(),"prompt":"transient-always"}),
    )
    .await;
    let id = always["job"]["id"].as_str().unwrap();
    for count in 1..=6 {
        let (job, _) = until(&server, id, |j| {
            j["state"] == "retry_waiting" && j["checkpoint"]["retry"]["retry_count"] == count
        })
        .await;
        if count == 6 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            let delay = job["checkpoint"]["retry"]["next_attempt_ms"]
                .as_i64()
                .unwrap()
                - now;
            assert!((10_790_000..=10_800_000).contains(&delay), "{job}");
        } else {
            expedite(&data.0, &space, id);
        }
    }
    op(
        &server,
        "agent.settings",
        json!({"job_id":id,"retry_settings":{"provider_errors":false,"subscription_limits":true}}),
    )
    .await;
    until(&server, id, |j| j["state"] == "interrupted").await;
    op(&server, "agent.stop", json!({"job_id":id})).await;
    let (account, _) = server.principal(&server.owner).await;
    let home = mdir.join(&format!("sandboxes/accounts/{account}/home"));
    std::fs::write(home.join("subscription"), "disposable billing fixture").unwrap();
    let limit = op(
        &server,
        "agent.run",
        json!({"space_id":space,"thread_id":Uuid::new_v4(),"prompt":"usage-limit"}),
    )
    .await;
    let id = limit["job"]["id"].as_str().unwrap().to_owned();
    for count in 1..=5 {
        until(&server, &id, |j| {
            j["state"] == "retry_waiting" && j["checkpoint"]["retry"]["retry_count"] == count
        })
        .await;
        expedite(&data.0, &space, &id);
    }
    let (job, _) = until(&server, &id, |j| {
        j["state"] == "retry_waiting" && j["checkpoint"]["retry"]["reason"] == "subscription_limit"
    })
    .await;
    let wake = job["checkpoint"]["retry"]["next_attempt_ms"].clone();
    server.proc.kill();
    server = Server::start_with_plugins(&data.0, port, &plugins);
    let resumed = op(&server, "agent.events", json!({"job_id":id})).await;
    assert_eq!(resumed["job"]["state"], "retry_waiting");
    assert_eq!(
        resumed["job"]["checkpoint"]["retry"]["next_attempt_ms"],
        wake
    );
    op(&server, "agent.stop", json!({"job_id":id})).await;
    let auth = op(
        &server,
        "agent.run",
        json!({"space_id":space,"thread_id":Uuid::new_v4(),"prompt":"auth-failure"}),
    )
    .await;
    let (job, _) = until(&server, auth["job"]["id"].as_str().unwrap(), |j| {
        j["state"] == "failed"
    })
    .await;
    assert!(job["checkpoint"]["retry"].is_null());
}

#[tokio::test(flavor = "multi_thread")]
async fn offline_account_placement_does_not_hold_another_accounts_execution_slot() {
    require_adapter_build();
    let data = TempDir::new("provider-placement-data");
    let plugins = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let server = Server::start_with_plugins(&data.0, free_port(), &plugins);
    let space = server.owned_space(&server.owner).await;
    let mut waiting = Vec::new();
    for _ in 0..calternal_server::plugins::runner::MAX_RUNNING {
        let start=op(&server,"agent.run",json!({"space_id":space,"thread_id":Uuid::new_v4(),"prompt":"waiting for my offline machine"})).await;
        let id = start["job"]["id"].as_str().unwrap().to_owned();
        until(&server, &id, |j| j["checkpoint"]["lease_id"].is_string()).await;
        waiting.push(id);
    }
    let ready = op_as(
        &server,
        &server.guest,
        "spaces.create",
        json!({"name":"Ready account"}),
    )
    .await;
    let ready_space = ready["space"]["id"].as_str().unwrap();
    let bin = TempDir::new("provider-placement-bin");
    std::fs::copy(
        plugins.join("calternal-claude-code/test/fake-claude.mjs"),
        bin.join("claude"),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(bin.join("claude"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mdir = TempDir::new("provider-placement-machine");
    let code = server.code(&server.guest).await;
    assert_eq!(enrol(&server.url(), &code, &mdir.0), 0);
    let _machine = machine_with_adapter(&mdir.0, &bin.0);
    wait_online(&server, &server.guest, 1).await;
    let start = op_as(
        &server,
        &server.guest,
        "agent.run",
        json!({"space_id":ready_space,"thread_id":Uuid::new_v4(),"prompt":"hello"}),
    )
    .await;
    let id = start["job"]["id"].as_str().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        let view = op_as(&server, &server.guest, "agent.events", json!({"job_id":id})).await;
        if view["job"]["state"] == "done" {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "offline account blocked ready work: {view}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    for id in waiting {
        op(&server, "agent.stop", json!({"job_id":id})).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_adapter_and_missing_build_are_clear_failed_turns() {
    // This regression intentionally needs neither a built adapter nor an account.
    // It runs the launcher from a fixture with no dist directory.
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for missing_executable in [true, false] {
        let data = TempDir::new("provider-missing-data");
        let plugins = TempDir::new("provider-missing-plugins");
        for name in ["calternal-orchestrator", "calternal-claude-code"] {
            std::fs::create_dir_all(plugins.join(name)).unwrap();
            let mut manifest: Value = serde_json::from_slice(
                &std::fs::read(repo.join("plugins").join(name).join("manifest.json")).unwrap(),
            )
            .unwrap();
            if missing_executable && name == "calternal-claude-code" {
                manifest["provider"]["executable"] = json!("calternal-definitely-missing-provider");
            }
            std::fs::write(
                plugins.join(name).join("manifest.json"),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
        }
        let server = Server::start_with_plugins(&data.0, free_port(), &plugins.0);
        let bin = TempDir::new("provider-missing-bin");
        std::fs::create_dir_all(bin.join("bin")).unwrap();
        std::fs::copy(
            repo.join("plugins/calternal-claude-code/bin/calternal-provider-claude-code"),
            bin.join("bin/calternal-provider-claude-code"),
        )
        .unwrap();
        let mdir = TempDir::new("provider-missing-machine");
        let code = server.code(&server.owner).await;
        assert_eq!(enrol(&server.url(), &code, &mdir.0), 0);
        let _machine = machine_with_adapter(&mdir.0, &bin.join("bin"));
        wait_online(&server, &server.owner, 1).await;
        let space = server.owned_space(&server.owner).await;
        let start = op(
            &server,
            "agent.run",
            json!({"space_id":space,"thread_id":Uuid::now_v7(),"prompt":"setup probe"}),
        )
        .await;
        let id = start["job"]["id"].as_str().unwrap();
        let (job, events) = until(&server, id, |j| {
            j["state"] == "failed" || j["state"] == "outcome_unknown"
        })
        .await;
        assert_eq!(job["state"], "failed", "{job}; {events:?}");
        let expected = if missing_executable {
            "adapter_missing"
        } else {
            "adapter_not_built"
        };
        assert_eq!(job["checkpoint"]["error"]["code"], expected);
        assert!(
            job["checkpoint"]["error"]["message"]
                .as_str()
                .unwrap()
                .contains("npm run build:plugins")
        );
        assert!(
            events
                .iter()
                .any(|e| e["event"]["type"] == "error" && e["event"]["code"] == expected)
        );
        assert!(
            job["checkpoint"]["retry"].is_null(),
            "setup failures must not retry"
        );
    }
}
