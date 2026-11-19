use super::*;
use calternal_plugin_host::MAX_FRAME_BYTES;
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_lease_reaps_a_real_package_with_no_cancel_or_eof() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (runtime, accepted) = tokio::join!(
        TcpStream::connect(listener.local_addr().unwrap()),
        listener.accept(),
    );
    let mut raw_host = BufReader::new(accepted.unwrap().0);
    let id = Uuid::new_v4();
    let epoch = Uuid::new_v4();
    let space = SpaceId::new();
    let pin = PackagePin {
        package: "fixture-lease".into(),
        digest: sha256_hex(CHILD.as_bytes()),
    };
    let client = tokio::spawn(async move {
        let channel = SpaceChannel::runtime(runtime.unwrap(), space, id)
            .await
            .unwrap();
        real_package(channel, pin).await.unwrap();
    });
    let mut line = String::new();
    raw_host.read_line(&mut line).await.unwrap();
    let offer: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(offer["assignment"], json!(id));
    let welcome = json!({
        "v":1, "space":space, "assignment":id, "epoch":epoch,
        "lease_ms":4000, "data_credit":2*(MAX_FRAME_BYTES+1), "control_credit":4096,
        "packages":[{"package":"fixture-lease","digest":sha256_hex(CHILD.as_bytes())}],
    });
    raw_host
        .get_mut()
        .write_all(format!("{welcome}\n").as_bytes())
        .await
        .unwrap();
    line.clear();
    raw_host.read_line(&mut line).await.unwrap();
    let ready: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(ready["message"]["type"], "ready");
    let renewal = json!({
        "v":1, "epoch":epoch, "seq":1, "package":null, "digest":null, "ticket":null,
        "message":{"type":"renew"}
    });
    raw_host
        .get_mut()
        .write_all(format!("{renewal}\n").as_bytes())
        .await
        .unwrap();
    let last_renewal = Instant::now();
    tokio::time::timeout(Duration::from_secs(5), client)
        .await
        .unwrap()
        .unwrap();
    assert!(
        (Duration::from_millis(3500)..Duration::from_secs(5)).contains(&last_renewal.elapsed())
    );
    // Keep the physical carrier open until the package's stop/reap already returned.
    drop(raw_host);
}
