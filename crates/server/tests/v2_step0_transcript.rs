//! A real durable journal, process restart and blocked TCP transcript fixture.
#[path = "../../../spikes/plugin-api-v2/fixture.rs"]
mod fixture;
#[path = "v2_step0_transcript/wire.rs"]
mod wire;

use ring::rand::{SecureRandom, SystemRandom};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use uuid::Uuid;
use wire::{Config, Request};

struct Process {
    child: Child,
    address: SocketAddr,
}

impl Process {
    fn boot(config: &Config) -> Self {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "transcript_process", "--nocapture"])
            .env(
                "STEP0_TRANSCRIPT_CONFIG",
                serde_json::to_string(config).unwrap(),
            )
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        for line in (&mut output).lines() {
            let line = line.unwrap();
            if let Some((_, address)) = line.split_once("STEP0_LISTEN ") {
                return Self {
                    child,
                    address: address.parse().unwrap(),
                };
            }
        }
        let status = child.wait().unwrap();
        panic!("transcript process exited before readiness: {status}");
    }

    fn send(&self, token: &str, route: &str, body: Value, stall: bool) -> TcpStream {
        let mut stream = TcpStream::connect_timeout(&self.address, Duration::from_secs(2)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream.set_nodelay(true).unwrap();
        if stall {
            wire::set_buffer(&stream, libc::SO_RCVBUF);
        }
        let request = Request {
            token: token.into(),
            route: route.into(),
            body,
            stall,
        };
        let mut line = serde_json::to_vec(&request).unwrap();
        line.push(b'\n');
        stream.write_all(&line).unwrap();
        stream
    }

    fn call(&self, token: &str, route: &str, body: Value) -> (Value, usize) {
        let mut response = Vec::new();
        BufReader::new(self.send(token, route, body, false))
            .read_until(b'\n', &mut response)
            .unwrap();
        assert!(!response.is_empty());
        (serde_json::from_slice(&response).unwrap(), response.len())
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "child process for the disposable transcript"]
async fn transcript_process() {
    let Ok(raw) = std::env::var("STEP0_TRANSCRIPT_CONFIG") else {
        return;
    };
    wire::serve(serde_json::from_str(&raw).unwrap()).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "disposable step 0 transcript perf fixture"]
async fn transcript_recovery_and_control() {
    let roots_dir = tempfile::tempdir().unwrap();
    let roots = fixture::roots(roots_dir.path());
    let (state, ids) = tokio::task::block_in_place(|| fixture::open(&roots, &fixture::packages()));
    let thread = fixture::run(
        &state,
        fixture::identity(ids.owner),
        "threads.create",
        json!({"space_id":ids.shared,"title":"Transcript"}),
    )
    .await
    .unwrap();
    let thread = Uuid::parse_str(thread["thread"]["id"].as_str().unwrap()).unwrap();
    drop(state);
    let token = |account: &str| {
        std::fs::read_to_string(roots.appdata.join(format!("dev-tokens/{account}.token"))).unwrap()
    };
    let (owner, guest) = (token("owner"), token("guest"));
    let mut key = [0u8; 32];
    SystemRandom::new().fill(&mut key).unwrap();
    let config = Config {
        root: roots_dir.path().into(),
        space: ids.shared,
        thread,
        thread_ref: Uuid::new_v4(),
        key,
    };
    let runs: usize = std::env::var("STEP0_RUNS").map_or(100, |n| n.parse().unwrap());
    let mut process = Process::boot(&config);
    let base = process
        .call(
            &guest,
            "transcript.page",
            json!({"thread":config.thread_ref}),
        )
        .0;
    let mut stall_from = base["committed_cursor"].as_str().unwrap().to_owned();
    let seed = process
        .call(&owner, "fixture.append", json!({"batch":runs,"cut":false}))
        .0;
    let mut prior_seq = seed["seq"].as_i64().unwrap();
    let seed_page = process
        .call(
            &guest,
            "transcript.page",
            json!({"thread":config.thread_ref,"after":stall_from}),
        )
        .0;
    assert_ids(&seed_page, runs, wire::EVENTS);
    let mut cursor = seed_page["committed_cursor"].as_str().unwrap().to_owned();
    let mut first = Vec::new();
    let mut control = Vec::new();
    let mut first_bytes = Vec::new();
    let mut full_bytes = Vec::new();
    let mut denied_bytes = Vec::new();

    for batch in 0..runs {
        let (baseline, _) = process.call(&owner, "fixture.probe", json!({}));
        let stalled = process.send(
            &guest,
            "transcript.page",
            json!({"thread":config.thread_ref,"after":stall_from,"limit":128}),
            true,
        );
        let timeout = Instant::now() + Duration::from_secs(2);
        loop {
            let (status, _) = process.call(&owner, "fixture.probe", json!({}));
            if status["blocked"].as_u64() > baseline["blocked"].as_u64() && status["pending"] == 1 {
                break;
            }
            assert!(
                Instant::now() < timeout,
                "socket did not stall after a partial write"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let start = Instant::now();
        let (result, _) = process.call(&owner, "threads.get", json!({}));
        control.push(start.elapsed());
        assert!(result["revision"].is_string());
        let (status, _) = process.call(&owner, "fixture.probe", json!({}));
        assert_eq!(
            status["pending"], 1,
            "transcript write completed before the control reply"
        );
        let mut lost = process.send(
            &owner,
            "fixture.append",
            json!({"batch":batch,"cut":true}),
            false,
        );
        let mut ack = Vec::new();
        lost.read_to_end(&mut ack).unwrap();
        assert!(ack.is_empty());
        assert_eq!(process.child.wait().unwrap().code(), Some(wire::CUT));
        drop(stalled);
        process = Process::boot(&config);

        let start = Instant::now();
        let (screen, bytes) = process.call(
            &guest,
            "transcript.page",
            json!({"thread":config.thread_ref,"after":cursor,"limit":32}),
        );
        first.push(start.elapsed());
        first_bytes.push(bytes);
        assert_ids(&screen, batch, 32);

        let (full, bytes) = process.call(
            &guest,
            "transcript.page",
            json!({"thread":config.thread_ref,"after":cursor,"limit":128}),
        );
        full_bytes.push(bytes);
        assert!(bytes <= wire::MAX_PAGE);
        assert_ids(&full, batch, wire::EVENTS);
        let end = full["committed_cursor"].as_str().unwrap().to_owned();
        assert_ne!(end, cursor);
        let (replay, _) =
            process.call(&owner, "fixture.append", json!({"batch":batch,"cut":false}));
        assert_eq!(replay["replay"], true);
        assert_eq!(replay["count"], wire::EVENTS);
        let seq = replay["seq"].as_i64().unwrap();
        assert_eq!(seq - prior_seq, wire::EVENTS as i64);
        prior_seq = seq;
        let (same, _) = process.call(
            &guest,
            "transcript.page",
            json!({"thread":config.thread_ref,"after":cursor}),
        );
        assert_eq!(same, full);
        let (none, _) = process.call(
            &guest,
            "transcript.page",
            json!({"thread":config.thread_ref,"after":end}),
        );
        assert_eq!(none, json!({"events":[],"committed_cursor":end}));

        assert_eq!(
            process.call(&owner, "fixture.revoke", json!({})).0,
            json!({"ok":true})
        );
        let (denied, bytes) = process.call(
            &guest,
            "transcript.page",
            json!({"thread":config.thread_ref,"after":end}),
        );
        assert_eq!(denied, json!({"error":"not_found"}));
        denied_bytes.push(bytes);
        assert_eq!(
            process.call(&owner, "fixture.restore", json!({})).0,
            json!({"ok":true})
        );
        stall_from = cursor;
        cursor = end;
    }

    fixture::summary("screenful ready for scripted client", &mut first);
    fixture::summary("independent Turns control request", &mut control);
    eprintln!(
        "first_screen_bytes={} page_bytes={} denied_error_bytes={} recovered_events={} max_pos={prior_seq}",
        first_bytes.into_iter().max().unwrap(),
        full_bytes.into_iter().max().unwrap(),
        denied_bytes.into_iter().max().unwrap(),
        wire::EVENTS * runs
    );
    let p95 = (runs * 95).div_ceil(100) - 1;
    if first[p95] > Duration::from_millis(150) || control[p95] > Duration::from_millis(500) {
        eprintln!("gate_miss transcript");
    }
}

fn assert_ids(page: &Value, batch: usize, count: usize) {
    let events = page["events"].as_array().unwrap();
    assert_eq!(events.len(), count);
    for (i, event) in events.iter().enumerate() {
        assert_eq!(event["event_id"], format!("{batch}-{i}"));
    }
}
