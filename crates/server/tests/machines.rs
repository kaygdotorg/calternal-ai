//! Machine role end to end: the real binary as server and as machine (process
//! backend), on loopback with a 3 s lease TTL and 1 s poll hold.

mod machines_common;

use std::time::Duration;

use machines_common::*;
use serde_json::{Value, json};
use uuid::Uuid;

const T: Duration = Duration::from_secs(15);

#[tokio::test(flavor = "multi_thread")]
async fn enrolment_codes_are_single_use_and_expire() {
    let data = TempDir::new("enrol-data");
    let server = Server::start(&data.0, free_port());
    let code = server.code(&server.owner).await;
    let dir = TempDir::new("enrol-m1");
    assert_eq!(enrol(&server.url(), &code, &dir.0), 0);
    // Replay of the same code from another computer fails.
    let other = TempDir::new("enrol-m2");
    assert_ne!(enrol(&server.url(), &code, &other.0), 0);
    assert!(!other.join("credentials").exists());
    // A code that expired (issued by the operator CLI with zero minutes).
    let (owner_id, _) = server.principal(&server.owner).await;
    let out = std::process::Command::new(BIN)
        .args([
            "machines",
            "code",
            "--minutes",
            "0",
            "--account",
            &owner_id,
            "--json",
            "--data-dir",
        ])
        .arg(&data.0)
        .output()
        .unwrap();
    assert!(out.status.success());
    let issued: Value = serde_json::from_slice(&out.stdout).unwrap();
    let expired = issued["data"]["code"].as_str().unwrap();
    assert_ne!(enrol(&server.url(), expired, &other.0), 0);
    // Garbage and a re-enrol over existing credentials are refused.
    assert_ne!(enrol(&server.url(), "not-a-code", &other.0), 0);
    let fresh = server.code(&server.owner).await;
    assert_ne!(
        enrol(&server.url(), &fresh, &dir.0),
        0,
        "must not overwrite credentials"
    );
    // The machine belongs to the account that issued the code.
    let (_, v) = server.call(&server.owner, "/api/v1/machines", None).await;
    assert_eq!(v["machines"].as_array().unwrap().len(), 1);
    assert_eq!(
        v["machines"][0]["owner"],
        json!({"kind": "account", "id": owner_id})
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn placement_by_capacity_and_output_streaming() {
    let data = TempDir::new("place-data");
    let server = Server::start(&data.0, free_port());
    let space = server.owned_space(&server.owner).await;
    let mdir = TempDir::new("place-m");
    let _m = connected_machine(&server, &server.owner, &mdir.0, 1).await;

    let first = server
        .request_run(&server.owner, &space, Uuid::new_v4())
        .await;
    let lease = first["lease_id"].as_str().unwrap().to_owned();
    server
        .wait_run(&server.owner, &lease, T, |v| v["state"] == "running")
        .await;
    let exec = server
        .exec(
            &server.owner,
            &lease,
            &["sh", "-c", "echo hi; echo err >&2; exit 3"],
        )
        .await;
    let (out, err, exit) = server.output(&server.owner, &exec, T).await;
    assert_eq!((out.as_str(), err.as_str()), ("hi\n", "err\n"));
    assert_eq!(exit["code"], 3);

    // Capacity 1: a second thread waits visibly, then runs when the first ends.
    let second = server
        .request_run(&server.owner, &space, Uuid::new_v4())
        .await;
    assert_eq!(second["state"], "waiting");
    assert_eq!(second["queue_position"], 1);
    let lease2 = second["lease_id"].as_str().unwrap().to_owned();
    // An exec queued while waiting runs once the run is placed.
    let queued = server
        .exec(&server.owner, &lease2, &["echo", "queued"])
        .await;
    let (s, stopped) = server
        .call(
            &server.owner,
            &format!("/api/v1/machines/runs/{lease}/stop"),
            Some(json!({})),
        )
        .await;
    assert_eq!((s, stopped["state"].as_str()), (200, Some("ended")));
    server
        .wait_run(&server.owner, &lease2, T, |v| v["state"] == "running")
        .await;
    assert_eq!(server.output(&server.owner, &queued, T).await.0, "queued\n");

    // Large output passes through the bounded buffers intact.
    let big = server
        .exec(
            &server.owner,
            &lease2,
            &["sh", "-c", "head -c 3000000 /dev/zero | tr '\\0' a"],
        )
        .await;
    let (out, _, exit) = server
        .output(&server.owner, &big, Duration::from_secs(60))
        .await;
    assert_eq!((out.len(), exit["code"].as_i64()), (3_000_000, Some(0)));
    // The same thread asked again returns the same run.
    let again = server
        .request_run(
            &server.owner,
            &space,
            Uuid::parse_str(second["thread_id"].as_str().unwrap()).unwrap(),
        )
        .await;
    assert_eq!(again["lease_id"], second["lease_id"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn other_accounts_cannot_use_see_or_touch_a_machine() {
    let data = TempDir::new("owner-data");
    let server = Server::start(&data.0, free_port());
    let space = server.owned_space(&server.owner).await;
    let mdir = TempDir::new("owner-m");
    let _m = connected_machine(&server, &server.owner, &mdir.0, 4).await;
    let (_, mine) = server.call(&server.owner, "/api/v1/machines", None).await;
    let machine = mine["machines"][0]["id"].as_str().unwrap().to_owned();
    let lease = server
        .request_run(&server.owner, &space, Uuid::new_v4())
        .await["lease_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let exec = server
        .exec(&server.owner, &lease, &["echo", "secret"])
        .await;

    let g = &server.guest;
    let (_, theirs) = server.call(g, "/api/v1/machines", None).await;
    assert_eq!(theirs["machines"], json!([]));
    assert_eq!(
        server
            .call(
                g,
                &format!("/api/v1/machines/{machine}/revoke"),
                Some(json!({}))
            )
            .await
            .0,
        404
    );
    assert_eq!(
        server
            .call(g, &format!("/api/v1/machines/runs/{lease}"), None)
            .await
            .0,
        404
    );
    assert_eq!(
        server
            .call(
                g,
                &format!("/api/v1/machines/runs/{lease}/stop"),
                Some(json!({}))
            )
            .await
            .0,
        404
    );
    assert_eq!(
        server
            .call(g, &format!("/api/v1/machines/execs/{exec}/output"), None)
            .await
            .0,
        404
    );
    // Running in a space the guest may not use plugins in (or not at all) is not found.
    let body = json!({"space_id": space, "thread_id": Uuid::new_v4()});
    assert_eq!(
        server.call(g, "/api/v1/machines/runs", Some(body)).await.0,
        404
    );
    // In the guest's own space, the owner's free machine is not used: the run waits.
    let guest_space = server.create_space(g).await;
    let waiting = server.request_run(g, &guest_space, Uuid::new_v4()).await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let waiting = server.run(g, waiting["lease_id"].as_str().unwrap()).await;
    assert_eq!(waiting["state"], "waiting");
    // A pool grant alone does not help: there is no pool machine.
    let (guest_id, _) = server.principal(g).await;
    let st = std::process::Command::new(BIN)
        .args(["machines", "pool-grant", &guest_id, "--data-dir"])
        .arg(&data.0)
        .status()
        .unwrap();
    assert!(st.success());
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(
        server.run(g, waiting["lease_id"].as_str().unwrap()).await["state"],
        "waiting"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn pool_machines_serve_only_granted_accounts() {
    let data = TempDir::new("pool-data");
    let server = Server::start(&data.0, free_port());
    let out = std::process::Command::new(BIN)
        .args(["machines", "code", "--pool", "--json", "--data-dir"])
        .arg(&data.0)
        .output()
        .unwrap();
    let issued: Value = serde_json::from_slice(&out.stdout).unwrap();
    let code = issued["data"]["code"].as_str().unwrap();
    let mdir = TempDir::new("pool-m");
    assert_eq!(enrol(&server.url(), code, &mdir.0), 0);
    let _m = machine(&mdir.0, 4);
    let space = server.owned_space(&server.owner).await;
    let run = server
        .request_run(&server.owner, &space, Uuid::new_v4())
        .await;
    let lease = run["lease_id"].as_str().unwrap().to_owned();
    tokio::time::sleep(Duration::from_millis(2000)).await;
    assert_eq!(
        server.run(&server.owner, &lease).await["state"],
        "waiting",
        "pool used without a grant"
    );
    let (owner_id, _) = server.principal(&server.owner).await;
    let st = std::process::Command::new(BIN)
        .args(["machines", "pool-grant", &owner_id, "--data-dir"])
        .arg(&data.0)
        .status()
        .unwrap();
    assert!(st.success());
    server
        .wait_run(&server.owner, &lease, T, |v| v["state"] == "running")
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn revocation_mid_run_stops_work_and_the_machine() {
    let data = TempDir::new("revoke-data");
    let server = Server::start(&data.0, free_port());
    let space = server.owned_space(&server.owner).await;
    let mdir = TempDir::new("revoke-m");
    let mut m = connected_machine(&server, &server.owner, &mdir.0, 2).await;
    let thread = Uuid::new_v4();
    let lease = server.request_run(&server.owner, &space, thread).await["lease_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let exec = server
        .exec(
            &server.owner,
            &lease,
            &["sh", "-c", "echo started; sleep 30; echo late"],
        )
        .await;
    server
        .wait_run(&server.owner, &lease, T, |v| v["state"] == "running")
        .await;
    let (_, mine) = server.call(&server.owner, "/api/v1/machines", None).await;
    let machine_id = mine["machines"][0]["id"].as_str().unwrap().to_owned();

    let (s, _) = server
        .call(
            &server.owner,
            &format!("/api/v1/machines/{machine_id}/revoke"),
            Some(json!({})),
        )
        .await;
    assert_eq!(s, 200);
    let run = server.run(&server.owner, &lease).await;
    assert_eq!(
        (run["state"].as_str(), run["reason"].as_str()),
        (Some("ended"), Some("the machine was revoked"))
    );
    let (out, _, exit) = server.output(&server.owner, &exec, T).await;
    assert!(!out.contains("late"));
    assert_eq!(exit["error"], "the machine was revoked");
    // The machine role stops its sandboxes and exits with the revoked code.
    assert_eq!(m.wait_exit(T), Some(3));
    assert!(
        !mdir.join(&format!("sandboxes/{thread}")).exists(),
        "sandbox kept after revocation"
    );
    let (_, mine) = server.call(&server.owner, "/api/v1/machines", None).await;
    assert_eq!(mine["machines"][0]["state"], "revoked");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_machine_that_lost_its_lease_cannot_deliver() {
    let data = TempDir::new("fence-data");
    let server = Server::start(&data.0, free_port());
    let space = server.owned_space(&server.owner).await;
    let a_dir = TempDir::new("fence-a");
    let a = connected_machine(&server, &server.owner, &a_dir.0, 2).await;
    let thread = Uuid::new_v4();
    let lease = server.request_run(&server.owner, &space, thread).await["lease_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let before = server
        .wait_run(&server.owner, &lease, T, |v| v["state"] == "running")
        .await;
    let ticking = server
        .exec(
            &server.owner,
            &lease,
            &["sh", "-c", "while true; do echo tick; sleep 0.2; done"],
        )
        .await;

    // Partition A (stop the process), and bring up B.
    a.signal("-STOP");
    let b_dir = TempDir::new("fence-b");
    let code = server.code(&server.owner).await;
    assert_eq!(enrol(&server.url(), &code, &b_dir.0), 0);
    let _b = machine(&b_dir.0, 2);
    let after = server
        .wait_run(&server.owner, &lease, Duration::from_secs(20), |v| {
            v["state"] == "running" && v["machine_id"] != before["machine_id"]
        })
        .await;
    assert_ne!(after["machine_id"], before["machine_id"]);

    // The exec on A is reported lost; whatever A still sends is refused.
    let (_, _, exit) = server.output(&server.owner, &ticking, T).await;
    assert_eq!(exit["error"], "the machine stopped responding");
    a.signal("-CONT");
    // A reconnects, learns it lost the lease, and removes its sandbox.
    let t0 = std::time::Instant::now();
    while a_dir.join(&format!("sandboxes/{thread}")).exists() {
        assert!(t0.elapsed() < T, "A kept the fenced sandbox");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let (s, v) = server
        .call(
            &server.owner,
            &format!("/api/v1/machines/execs/{ticking}/output?after=0"),
            None,
        )
        .await;
    assert_eq!(s, 200);
    let data_after_loss = v["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .skip_while(|c| c["kind"] == "data")
        .filter(|c| c["kind"] == "data")
        .count();
    assert_eq!(
        data_after_loss, 0,
        "output accepted after the lease was fenced: {v}"
    );
    // The thread keeps working on B.
    let exec = server.exec(&server.owner, &lease, &["echo", "on-b"]).await;
    assert_eq!(server.output(&server.owner, &exec, T).await.0, "on-b\n");
}

#[tokio::test(flavor = "multi_thread")]
async fn both_roles_survive_kill_and_restart() {
    let data = TempDir::new("restart-data");
    let port = free_port();
    let mut server = Server::start(&data.0, port);
    let space = server.owned_space(&server.owner).await;
    let mdir = TempDir::new("restart-m");
    let mut m = connected_machine(&server, &server.owner, &mdir.0, 2).await;
    let lease = server
        .request_run(&server.owner, &space, Uuid::new_v4())
        .await["lease_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let write = server
        .exec(&server.owner, &lease, &["sh", "-c", "echo kept > state"])
        .await;
    assert_eq!(server.output(&server.owner, &write, T).await.2["code"], 0);

    // Kill the machine role mid-exec; the restarted one adopts the sandbox.
    let long = server
        .exec(&server.owner, &lease, &["sh", "-c", "echo go; sleep 5"])
        .await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    m.kill();
    let mut m = machine(&mdir.0, 2);
    let (_, _, exit) = server.output(&server.owner, &long, T).await;
    assert_eq!(exit["error"], "the machine restarted");
    let read = server.exec(&server.owner, &lease, &["cat", "state"]).await;
    assert_eq!(server.output(&server.owner, &read, T).await.0, "kept\n");

    // Kill the server; restart it on the same port and data. The run persists.
    server.proc.kill();
    server = Server::start(&data.0, port);
    wait_online(&server, &server.owner, 1).await;
    let run = server.run(&server.owner, &lease).await;
    assert!(
        matches!(run["state"].as_str(), Some("assigned" | "running")),
        "{run}"
    );
    let read = server.exec(&server.owner, &lease, &["cat", "state"]).await;
    assert_eq!(server.output(&server.owner, &read, T).await.0, "kept\n");
    m.kill();
}
