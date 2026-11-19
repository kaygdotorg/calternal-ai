//! Disposable space carrier over real TCP with the existing machine enrollment.
#[path = "v2_step0_carrier/effects.rs"]
mod effects;
#[path = "v2_step0_carrier/fixture.rs"]
mod fixture;
#[path = "v2_step0_carrier/socket.rs"]
mod socket;
use effects::Effects;
use fixture::{Enrollment, Hello, Packages, Welcome, enroll, source};
#[path = "v2_step0_carrier/wire.rs"]
mod wire;
use socket::{Socket, pair};

use calternal_plugin_host::{Authority, Frame, Message as ChildMessage, decode_frame};
use serde_json::json;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{io::AsyncWriteExt, time::timeout};
use uuid::Uuid;
use wire::{Envelope, Message};

const RUNS: usize = 100;
const SLOW: Duration = Duration::from_secs(2);
const BUDGET: Duration = Duration::from_secs(2);
const PACKAGES: [&str; 2] = ["slow", "fast"];
async fn remote(
    mut socket: Socket,
    packages: Arc<Packages>,
    stall: Arc<AtomicBool>,
    renewed: tokio::sync::oneshot::Sender<Instant>,
) -> Instant {
    let mut renewed = Some(renewed);
    let mut deadline = Instant::now() + wire::LEASE;
    let mut pending = tokio::task::JoinSet::new();
    loop {
        let event = tokio::select! {
            biased;
            () = tokio::time::sleep_until(deadline.into()) => break,
            result = pending.join_next(), if !pending.is_empty() => {
                let (package, response, bytes): (usize, Frame, usize) = result.unwrap().unwrap();
                if socket.write(package, "", Message::Credit { bytes }).await.is_err() {
                    break;
                }
                if socket.write_child(package, response).await.is_err() {
                    break;
                }
                continue;
            }
            result = socket.read() => result,
        };
        let Ok((package, envelope, size)) = event else {
            break;
        };
        match envelope.message {
            Message::Renew => {
                let now = Instant::now();
                deadline = now + wire::LEASE;
                socket.write(package, "", Message::Ready).await.unwrap();
                if let Some(ready) = renewed.take() {
                    ready.send(now).unwrap();
                }
                if stall.load(Ordering::Acquire) {
                    // Leave the socket open and unread through lease expiry,
                    // including any cancel or network EOF after the stall.
                    tokio::time::sleep_until(deadline.into()).await;
                    break;
                }
            }
            Message::Cancel => socket
                .write(package, &envelope.ticket, Message::Ready)
                .await
                .unwrap(),
            Message::Child { frame } => {
                let process = packages.clone();
                pending.spawn_blocking(move || {
                    let response = process.request(package, frame);
                    assert_eq!(response.invocation_id, envelope.ticket);
                    (package, response, size)
                });
            }
            Message::Credit { .. } => {}
            _ => panic!("unexpected remote direction"),
        }
    }
    assert_eq!(
        tokio::task::spawn_blocking(move || packages.stop())
            .await
            .unwrap(),
        2
    );
    Instant::now()
}

fn invoke(ticket: &str, authority: Authority) -> Frame {
    Frame {
        protocol_version: 2,
        invocation_id: ticket.into(),
        message: ChildMessage::Invoke {
            operation: "fixture.get".into(),
            input: json!({}),
            authority,
        },
    }
}

async fn ready(socket: &mut Socket, ticket: &str) {
    loop {
        let (_, response, _) = socket.read().await.unwrap();
        match response.message {
            Message::Ready => {
                assert_eq!(response.ticket, ticket);
                return;
            }
            Message::Credit { .. } => {}
            _ => panic!("unexpected control response"),
        }
    }
}

async fn child(socket: &mut Socket) -> (usize, Frame) {
    loop {
        let (package, response, bytes) = socket.read().await.unwrap();
        match response.message {
            Message::Child { frame } => {
                let frame = decode_frame(&wire::encode(&frame)).unwrap();
                assert_eq!(frame.invocation_id, response.ticket);
                socket
                    .write(package, "", Message::Credit { bytes })
                    .await
                    .unwrap();
                return (package, frame);
            }
            Message::Credit { .. } => {}
            _ => panic!("unexpected child response"),
        }
    }
}

async fn finish(socket: &mut Socket, package: usize, requested: Frame) {
    let ChildMessage::HostCall {
        call_id,
        method: calternal_core::HostCallKind::ReadItem,
        ..
    } = requested.message
    else {
        panic!("expected a callback")
    };
    let id = requested.invocation_id;
    socket
        .child(
            package,
            Frame {
                protocol_version: 2,
                invocation_id: id.clone(),
                message: ChildMessage::HostReply {
                    call_id,
                    ok: true,
                    data: json!({}),
                },
            },
        )
        .await;
    let (finished_package, finished) = child(socket).await;
    assert_eq!((finished_package, finished.invocation_id), (package, id));
    assert!(matches!(finished.message, ChildMessage::Complete { .. }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "disposable perf fixture; see step 0 measurements"]
async fn enrolled_space_channels() {
    let fixture = tempfile::tempdir().unwrap();
    let enrollment = tokio::task::block_in_place(|| enroll(fixture.path(), 0));
    let digest: Arc<str> =
        calternal_server::store::sha256_hex(&std::fs::read(source()).unwrap()).into();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let runs = std::env::var("STEP0_RUNS").map_or(RUNS, |n| n.parse().unwrap());
    assert!(runs > 0);
    let mut measurements = [const { Vec::new() }; 10];
    for run in 0..runs {
        let (mut host, machine) = pair(
            &listener,
            &enrollment,
            enrollment.allowed[0],
            digest.clone(),
        )
        .await;
        let (mut b, b_machine) = pair(
            &listener,
            &enrollment,
            enrollment.allowed[1],
            digest.clone(),
        )
        .await;
        let (pa, pb) = tokio::task::spawn_blocking(|| {
            (Arc::new(Packages::start()), Arc::new(Packages::start()))
        })
        .await
        .unwrap();
        let stall = Arc::new(AtomicBool::new(false));
        let b_task = tokio::spawn(async move {
            let mut b_machine = b_machine;
            loop {
                let Ok((package, frame, size)) = b_machine.read().await else {
                    return;
                };
                match frame.message {
                    Message::Child { frame } => {
                        let p = pb.clone();
                        let reply = tokio::task::spawn_blocking(move || p.request(package, frame))
                            .await
                            .unwrap();
                        b_machine
                            .write(package, "", Message::Credit { bytes: size })
                            .await
                            .unwrap();
                        b_machine.child(package, reply).await;
                    }
                    Message::Credit { .. } => {}
                    _ => panic!("unexpected space B frame"),
                }
            }
        });
        let (notify, renewed) = tokio::sync::oneshot::channel();
        let remote_task = tokio::spawn(remote(machine, pa, stall.clone(), notify));
        let (held, slow) = timeout(BUDGET, async {
            host.child(0, invoke("held", enrollment.authority.clone()))
                .await;
            let (package, slow) = child(&mut host).await;
            assert_eq!(package, 0);
            let held = Instant::now();
            let fast = serde_json::to_value(invoke("fast", enrollment.authority.clone())).unwrap();
            host.enqueue(1, "fast", Message::Child { frame: fast });
            host.enqueue(0, "other", Message::Cancel);
            host.flush_one().await.unwrap();
            ready(&mut host, "other").await;
            measurements[0].push(held.elapsed().as_micros());
            host.flush_one().await.unwrap();
            let (package, fast) = child(&mut host).await;
            assert_eq!(package, 1);
            finish(&mut host, 1, fast).await;
            measurements[1].push(held.elapsed().as_micros());
            let mut b_authority = enrollment.authority.clone();
            b_authority.space_id = enrollment.allowed[1];
            let b_start = Instant::now();
            b.child(1, invoke("b_warm", b_authority.clone())).await;
            let (_, callback) = child(&mut b).await;
            finish(&mut b, 1, callback).await;
            assert!(b_start.elapsed() < BUDGET);
            (held, slow)
        })
        .await
        .unwrap();
        // Leave the slow callback pending for the full two seconds, wholly
        // independent of the carrier's reads and of the other package.
        tokio::time::sleep_until((held + SLOW).into()).await;
        let stale_body = serde_json::to_value(&slow).unwrap();
        finish(&mut host, 0, slow).await;

        let old = host.welcome.epoch;
        let effect_root = fixture.path().join(old.to_string());
        let mut gate = tokio::task::block_in_place(|| Effects {
            epoch: old,
            ticket: "held".into(),
            fenced: false,
            content: effects::effect_content(&effect_root),
            authority: enrollment.authority.clone(),
        });
        assert_eq!(
            tokio::task::block_in_place(|| gate.apply(old, "held", "request")),
            Some(true)
        );
        // Pause immediately after the final authenticated renewal. The receiver
        // defers every further read, including socket EOF and any cancel.
        stall.store(true, Ordering::Release);
        host.write(0, "", Message::Renew).await.unwrap();
        ready(&mut host, "").await;
        let remote_renewal = renewed.await.unwrap();
        let started = Instant::now();
        host.enqueue(
            0,
            "held",
            Message::Fill {
                bytes: "x".repeat(wire::CAP - 1024),
            },
        );
        let bytes = host.dequeue().unwrap();
        let first = bytes.len() / 64;
        if measurements[3].is_empty() {
            println!(
                "stalled_frame_bytes={} prefix_written_bytes={first}",
                bytes.len()
            );
        }
        host.output.write_all(&bytes[..first]).await.unwrap();
        let a = async {
            let mut sent = first;
            let outcome = timeout(wire::WRITE_LIMIT, async {
                while sent < bytes.len() {
                    let n = host.output.write(&bytes[sent..]).await.unwrap();
                    assert!(n > 0);
                    sent += n;
                }
            })
            .await;
            assert!(outcome.is_err(), "socket never stalled");
            assert!(sent >= first && sent < bytes.len());
            drop(host);
            (started.elapsed().as_micros(), sent as u128)
        };
        let revoke = async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let injected = Instant::now();
            gate.fenced = true;
            assert_eq!(gate.apply(old, "held", "late"), None);
            (
                started.elapsed().as_micros(),
                injected.elapsed().as_micros(),
            )
        };
        let b_during_stall = async {
            let start = Instant::now();
            let mut auth = enrollment.authority.clone();
            auth.space_id = enrollment.allowed[1];
            b.child(1, invoke("b_stall", auth)).await;
            let (_, call) = child(&mut b).await;
            finish(&mut b, 1, call).await;
            (start.elapsed().as_micros(), started.elapsed().as_micros())
        };
        let ((closed, sent), (fenced, invalidation), (b_elapsed, b_finished)) =
            timeout(BUDGET, async { tokio::join!(a, revoke, b_during_stall) })
                .await
                .unwrap();
        assert!(fenced < closed);
        assert!(
            b_finished < closed,
            "Space B must finish while A is still stalled"
        );
        measurements[7].push(invalidation);
        measurements[8].push(sent);
        measurements[2].push(b_elapsed);
        measurements[3].push(fenced);
        measurements[6].push(closed);
        assert!(closed < 2_000_000);
        let reconnect = Instant::now();
        let (mut fresh, mut fresh_machine) = pair(
            &listener,
            &enrollment,
            enrollment.allowed[0],
            digest.clone(),
        )
        .await;
        measurements[9].push(reconnect.elapsed().as_micros());
        let fresh_epoch = fresh.welcome.epoch;
        assert_ne!(fresh_epoch, old);
        assert_eq!(fresh.welcome.space, enrollment.allowed[0]);
        if run == 0 {
            println!("reconnect minted new epoch with fresh data and control credits");
        }
        gate.fenced = false;
        gate.epoch = fresh_epoch;
        gate.ticket = "fresh".into();
        assert_eq!(gate.apply(old, "held", "old"), None);
        assert_eq!(gate.apply(fresh_epoch, "held", "old"), None);
        let replacement = tokio::task::spawn_blocking(Packages::start).await.unwrap();
        let live = async {
            fresh
                .child(0, invoke("fresh", enrollment.authority.clone()))
                .await;
            let (package, callback) = child(&mut fresh).await;
            assert_eq!(package, 0);
            let ticket = callback.invocation_id.clone();
            tokio::task::block_in_place(|| {
                assert!(
                    enrollment
                        .auth
                        .capabilities(gate.authority.account_id, gate.authority.space_id)
                        .unwrap()
                        .is_some()
                );
                assert_eq!(gate.apply(fresh_epoch, &ticket, "request"), Some(false));
                assert_eq!(
                    gate.content.journal(gate.authority.space_id).unwrap().len(),
                    1
                );
            });
            finish(&mut fresh, 0, callback).await;
        };
        let peer = tokio::spawn(async move {
            let replacement = Arc::new(replacement);
            for _ in 0..2 {
                loop {
                    let (package, message, bytes) = fresh_machine.read().await.unwrap();
                    if let Message::Child { frame } = message.message {
                        let worker = replacement.clone();
                        let result =
                            tokio::task::spawn_blocking(move || worker.request(package, frame))
                                .await
                                .unwrap();
                        fresh_machine
                            .write(package, "", Message::Credit { bytes })
                            .await
                            .unwrap();
                        fresh_machine.child(package, result).await;
                        break;
                    }
                }
            }
            tokio::task::spawn_blocking(move || replacement.stop())
                .await
                .unwrap();
            fresh_machine
        });
        timeout(BUDGET, live).await.unwrap();
        let mut fresh_machine = timeout(BUDGET, peer).await.unwrap().unwrap();
        measurements[4].push(reconnect.elapsed().as_micros());
        let stale = Envelope {
            v: 1,
            epoch: old,
            seq: fresh_machine.next_sequence(),
            package: "slow".into(),
            digest: digest.to_string(),
            ticket: "held".into(),
            message: Message::Child { frame: stale_body },
        };
        wire::send(&mut fresh_machine.output, &wire::encode(&stale))
            .await
            .unwrap();
        assert!(fresh.read().await.is_err());
        drop(b);
        b_task.await.unwrap();
        let expired = timeout(Duration::from_secs(5), remote_task)
            .await
            .unwrap()
            .unwrap();
        measurements[5].push(expired.duration_since(remote_renewal).as_micros());
    }
    for (name, samples) in [
        "control",
        "other_callback",
        "space_b",
        "local_fence",
        "reconnect_and_replay",
        "remote_expiry",
        "local_close",
        "fence_apply",
        "stalled_byte_count",
        "reconnect_handshake",
    ]
    .into_iter()
    .zip(&mut measurements)
    {
        samples.sort_unstable();
        let max = samples[samples.len() - 1];
        let percentile = |p: usize| samples[(samples.len() * p).div_ceil(100) - 1];
        let unit = if name == "stalled_byte_count" {
            "bytes"
        } else {
            "us"
        };
        println!(
            "{name} n={} p50_{unit}={} p95_{unit}={} max_{unit}={max}",
            samples.len(),
            percentile(50),
            percentile(95)
        );
        let gate = match name {
            "control" | "other_callback" | "local_fence" | "fence_apply" => 500_000,
            "space_b" | "local_close" => 2_000_000,
            "remote_expiry" => 5_000_000,
            _ => u128::MAX,
        };
        let observed = if name == "remote_expiry" {
            max
        } else {
            percentile(95)
        };
        assert!(observed <= gate, "{name} missed its gate");
    }
    println!(
        "prior_epoch_effects=0 prior_ticket_effects=0 extra_commits=0; sockets_per_awake_space=1"
    );
}

fn process_rss_kib() -> usize {
    std::fs::read_to_string("/proc/self/smaps_rollup")
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("Rss:"))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "disposable perf fixture; see step 0 measurements"]
async fn idle_channel_cost() {
    const IDLE: usize = 128;
    let fixture = tempfile::tempdir().unwrap();
    let enrollment = tokio::task::block_in_place(|| enroll(fixture.path(), IDLE));
    let digest: Arc<str> = "idle".into();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    // Warm the handshake and runtime before taking the RSS baseline.
    drop(
        pair(
            &listener,
            &enrollment,
            enrollment.allowed[0],
            digest.clone(),
        )
        .await,
    );
    let mut idle = Vec::with_capacity(IDLE);
    let before = process_rss_kib();
    for &space in &enrollment.allowed[2..] {
        idle.push(pair(&listener, &enrollment, space, digest.clone()).await);
    }
    let delta = process_rss_kib().saturating_sub(before);
    let (send, receive) = idle[0].0.kernel_buffers();
    assert_eq!(idle.len(), IDLE);
    println!(
        "idle_spaces={IDLE} endpoint_objects_bytes_per_space={} read_capacity_bytes_per_space=8192 process_rss_delta_kib={delta} socket_buffer_request_bytes_per_endpoint_direction=4096 measured_kernel_send_bytes={send} measured_kernel_receive_bytes={receive}",
        std::mem::size_of::<(Socket, Socket)>()
    );
}
