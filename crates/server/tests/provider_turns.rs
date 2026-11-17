//! Official CLI fixture through the real server, machine role and provider adapter.
mod machines_common;
use machines_common::*;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
use uuid::Uuid;

async fn op(server: &Server, operation: &str, input: Value) -> Value {
    let (status, v) = server
        .call(
            &server.owner,
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
    let data = TempDir::new("provider-data");
    let server = Server::start_with_plugins(
        &data.0,
        free_port(),
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins"),
    );
    let bin = TempDir::new("provider-bin");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/calternal-claude-code/test/fake-claude.mjs");
    std::fs::copy(fixture, bin.join("claude")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(bin.join("claude"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mdir = TempDir::new("provider-machine");
    let code = server.code(&server.owner).await;
    assert_eq!(enrol(&server.url(), &code, &mdir.0), 0);
    let machine = machine_with_adapter(&mdir.0, &bin.0);
    wait_online(&server, &server.owner, 1).await;
    let space = server.owned_space(&server.owner).await;
    let start = op(
        &server,
        "agent.run",
        json!({"space_id":space,"thread_id":Uuid::new_v4(),"prompt":"hello"}),
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
    until(&server, sid, |j| {
        j["state"] == "running" || j["state"] == "done"
    })
    .await;
    let current = op(&server, "agent.events", json!({"job_id":sid})).await;
    assert_ne!(current["job"]["checkpoint"]["machine_id"], previous);
    machine.signal("-CONT");
    op(&server, "agent.stop", json!({"job_id":sid})).await;
}

fn expedite(data: &Path, space: &str, id: &str) {
    let c = rusqlite::Connection::open(data.join("spaces").join(space).join("jobs.db")).unwrap();
    c.execute(
        "UPDATE jobs SET checkpoint=json_set(checkpoint,'$.retry.next_attempt_ms',0) WHERE id=?1",
        [id],
    )
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn classified_provider_retries_are_durable_and_do_not_repeat_unknown_effects() {
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
    let (job, _) = until(&server, &id, |j| j["state"] == "retry_waiting").await;
    assert_eq!(job["checkpoint"]["retry"]["reason"], "subscription_limit");
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
