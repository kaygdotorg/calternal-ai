//! Fault injections on the product carrier and its local authority fence.
use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::{
    io::{AsyncReadExt, duplex},
    net::{TcpListener, TcpSocket, TcpStream},
};

fn pin(name: &str) -> PackagePin {
    PackagePin {
        package: name.into(),
        digest: "b".repeat(64),
    }
}

fn invoke(input: serde_json::Value) -> SpaceMessage {
    SpaceMessage::Invoke {
        operation: "fixture.head".into(),
        input,
        deadline_ms: 60_000,
    }
}

fn callback() -> SpaceMessage {
    SpaceMessage::Call {
        callback_id: Uuid::new_v4(),
        method: SpaceMethod::ItemHead,
        input: json!({}),
    }
}

async fn connect(listener: &TcpListener, socket: TcpSocket) -> (TcpStream, TcpStream) {
    let (outbound, accepted) = tokio::join!(
        socket.connect(listener.local_addr().unwrap()),
        listener.accept(),
    );
    (outbound.unwrap(), accepted.unwrap().0)
}

async fn raw_peer() -> (SpaceChannel, BufReader<TcpStream>, PackagePin) {
    let accept = TcpSocket::new_v4().unwrap();
    accept.set_recv_buffer_size(4096).unwrap();
    accept.bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let listener = accept.listen(4).unwrap();
    let socket = TcpSocket::new_v4().unwrap();
    socket.set_send_buffer_size(4096).unwrap();
    let (host, peer) = connect(&listener, socket).await;
    let space = SpaceId::new();
    let assignment = Uuid::new_v4();
    let pin = pin("fixture-a");
    let raw = async {
        let mut peer = BufReader::new(peer);
        write_handshake(
            peer.get_mut(),
            &Offer {
                v: V,
                space,
                assignment,
            },
        )
        .await
        .unwrap();
        let mut line = Vec::new();
        read_line(&mut peer, &mut line, CONTROL).await.unwrap();
        let welcome: Welcome = parse(&line).unwrap();
        assert_eq!(welcome.assignment, assignment);
        peer
    };
    let (host, peer) = tokio::join!(
        SpaceChannel::host(host, space, assignment, vec![pin.clone()]),
        raw
    );
    (host.unwrap(), peer, pin)
}

async fn independent_space() -> (SpaceChannel, SpaceChannel, PackagePin) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (host, runtime) = connect(&listener, TcpSocket::new_v4().unwrap()).await;
    let pin = pin("fixture-b");
    let space = SpaceId::new();
    let assignment = Uuid::new_v4();
    let (host, runtime) = tokio::join!(
        SpaceChannel::host(host, space, assignment, vec![pin.clone()]),
        SpaceChannel::runtime(runtime, space, assignment),
    );
    (host.unwrap(), runtime.unwrap(), pin)
}

#[tokio::test]
async fn physical_midframe_stall_fences_locally_and_preserves_space_b() {
    for explicit_fence in [false, true] {
        let (host, mut peer, package) = raw_peer().await;
        let (mut second_host, second_runtime, second_pin) = independent_space().await;
        let length = 245_000;
        let started = Instant::now();
        host.send(&package, Uuid::new_v4(), invoke(json!("w".repeat(length))))
            .unwrap();
        let mut prefix = [0_u8; 512];
        timeout(Duration::from_millis(200), peer.read_exact(&mut prefix))
            .await
            .unwrap()
            .unwrap();
        assert!(!prefix.contains(&b'\n'));
        assert!(host.is_live());
        let now = Instant::now();
        second_runtime
            .send(&second_pin, Uuid::new_v4(), callback())
            .unwrap();
        let delivered = timeout(Duration::from_secs(2), second_host.receive())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(delivered.frame().package.as_deref(), Some("fixture-b"));
        assert!(now.elapsed() < Duration::from_secs(2));
        assert!(
            host.is_live(),
            "B must finish while A's physical write is stalled"
        );
        if explicit_fence {
            let local = Instant::now();
            host.fence_handle().close();
            assert!(!host.is_live());
            assert!(
                host.send(&package, Uuid::new_v4(), invoke(json!({})))
                    .is_err()
            );
            assert!(local.elapsed() < Duration::from_millis(500));
        }
        timeout(
            Duration::from_secs(2),
            host.lifetime().wait_for(|live| !*live),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(started.elapsed() < Duration::from_secs(2));
        let mut remainder = Vec::new();
        timeout(Duration::from_secs(2), peer.read_to_end(&mut remainder))
            .await
            .unwrap()
            .unwrap();
        assert!(
            remainder.len() + prefix.len() < length,
            "the TCP writer must stall mid-frame"
        );
        assert!(!remainder.contains(&b'\n'));
        assert!(second_host.is_live() && second_runtime.is_live());
    }
}

#[tokio::test]
async fn withholding_one_packages_delivery_does_not_delay_control_or_another_package() {
    let (host_io, runtime_io) = duplex(4096);
    let a = pin("fixture-a");
    let b = pin("fixture-b");
    let space = SpaceId::new();
    let assignment = Uuid::new_v4();
    let (host, runtime) = tokio::join!(
        SpaceChannel::host(host_io, space, assignment, vec![a.clone(), b.clone()]),
        SpaceChannel::runtime(runtime_io, space, assignment),
    );
    let mut host = host.unwrap();
    let mut runtime = runtime.unwrap();
    let a_ticket = Uuid::new_v4();
    runtime.send(&a, a_ticket, callback()).unwrap();
    let held = host.receive().await.unwrap();
    let slow = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        drop(held);
    });
    // Enqueue both without yielding. Only the control may become the next frame.
    host.send(&a, a_ticket, invoke(json!({}))).unwrap();
    host.send(&a, a_ticket, SpaceMessage::Cancel {}).unwrap();
    let cancel = timeout(Duration::from_millis(500), runtime.receive())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(cancel.frame().message, SpaceMessage::Cancel {}));
    assert_eq!(cancel.frame().seq, 1);
    let b_ticket = Uuid::new_v4();
    runtime.send(&b, b_ticket, callback()).unwrap();
    let other = timeout(Duration::from_millis(500), host.receive())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(other.frame().ticket, Some(b_ticket));
    assert!(!slow.is_finished());
    slow.await.unwrap();
}

#[tokio::test]
async fn prior_epoch_frame_with_next_sequence_closes_a_fresh_carrier() {
    let (previous, _, _) = raw_peer().await;
    let previous_epoch = previous.epoch();
    drop(previous);
    let (mut host, mut peer, pin) = raw_peer().await;
    assert_ne!(host.epoch(), previous_epoch);
    let stale = SpaceFrame {
        v: V,
        epoch: previous_epoch,
        seq: 1,
        package: Some(pin.package),
        digest: Some(pin.digest),
        ticket: Some(Uuid::new_v4()),
        message: callback(),
    };
    write_bytes(peer.get_mut(), &stale.encode(MAX_LINE).unwrap())
        .await
        .unwrap();
    assert!(
        timeout(Duration::from_millis(500), host.receive())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn stale_queued_reply_is_discarded_without_consuming_an_ordinal() {
    let (host_io, runtime_io) = duplex(4096);
    let pin = pin("fixture-a");
    let space = SpaceId::new();
    let id = Uuid::new_v4();
    let (host, runtime) = tokio::join!(
        SpaceChannel::host(host_io, space, id, vec![pin.clone()]),
        SpaceChannel::runtime(runtime_io, space, id),
    );
    let host = host.unwrap();
    let mut runtime = runtime.unwrap();
    let live = Arc::new(AtomicBool::new(true));
    let guard = live.clone();
    host.send_checked(
        &pin,
        Uuid::new_v4(),
        SpaceMessage::Reply {
            callback_id: Uuid::new_v4(),
            result: Ok(json!({"secret":"refused"})),
        },
        move || guard.load(Ordering::Acquire),
    )
    .unwrap();
    live.store(false, Ordering::Release);
    let next = Uuid::new_v4();
    host.send(&pin, next, invoke(json!({}))).unwrap();
    let received = timeout(Duration::from_millis(500), runtime.receive())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(received.frame().seq, 1);
    assert_eq!(received.frame().ticket, Some(next));
}
