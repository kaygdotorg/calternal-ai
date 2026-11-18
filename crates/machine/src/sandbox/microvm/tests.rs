//! Host-only framing regression and optional KVM substrate coverage.

use std::path::Path;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

use super::{Backend, Config, MicrovmBackend, Request, vmm, wire};
use crate::protocol::{Limits, SandboxSpec};
use crate::sandbox::{ExecRequest, Sandbox, collect};
use calternal_types::SpaceId;
use uuid::Uuid;

#[tokio::test]
async fn handshake_preserves_first_frame_and_rejects_oversized_frames() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("vsock");
    let listener = UnixListener::bind(&socket).unwrap();
    let peer = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut command = String::new();
        BufReader::new(&mut stream)
            .read_line(&mut command)
            .await
            .unwrap();
        assert_eq!(command, "CONNECT 5000\n");
        let mut reply = b"OK 1073741824\n".to_vec();
        let bytes = serde_json::to_vec(&Request::Ready).unwrap();
        reply.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        reply.extend_from_slice(&bytes);
        reply.extend_from_slice(&u32::MAX.to_be_bytes());
        stream.write_all(&reply).await.unwrap();
    });
    let mut channel = vmm::connect(&socket).await.unwrap();
    assert!(matches!(
        wire::receive::<Request>(&mut channel).await.unwrap(),
        Request::Ready
    ));
    assert!(wire::receive::<Request>(&mut channel).await.is_err());
    assert_eq!(channel.read(&mut [0]).await.unwrap(), 0);
    peer.await.unwrap();
}

#[tokio::test]
async fn guest_control_proof_is_bound_to_one_boot() {
    use ring::hmac;
    let key = hmac::Key::new(hmac::HMAC_SHA256, &[1; 32]);
    let other_key = hmac::Key::new(hmac::HMAC_SHA256, &[2; 32]);
    let (mut writer, mut reader) = tokio::io::duplex(64);
    wire::prove(&mut writer, &key).await.unwrap();
    wire::verify(&mut reader, &key).await.unwrap();
    wire::prove(&mut writer, &key).await.unwrap();
    assert_eq!(
        wire::verify(&mut reader, &other_key)
            .await
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
}

async fn shell(sandbox: &dyn Sandbox, script: &str) -> String {
    let test = sandbox
        .exec(ExecRequest {
            argv: vec!["sh".into(), "-ec".into(), script.into()],
            ..ExecRequest::default()
        })
        .await
        .unwrap();
    let (out, err, exit) = tokio::time::timeout(Duration::from_secs(30), collect(test))
        .await
        .unwrap();
    assert_eq!(
        exit.code,
        Some(0),
        "{exit:?}, {}",
        String::from_utf8_lossy(&err)
    );
    String::from_utf8(out).unwrap()
}

#[tokio::test]
async fn kvm_separates_accounts_threads_and_spaces_and_persists_across_crash() {
    if let Err(error) = super::probe_kvm().await {
        eprintln!("SKIP Firecracker isolation: {error}");
        return;
    }
    let Some(config) = std::env::var_os("CALTERNAL_MICROVM_TEST_CONFIG") else {
        eprintln!("SKIP Firecracker isolation: CALTERNAL_MICROVM_TEST_CONFIG is unset");
        return;
    };
    let mut config = Config::load(Path::new(&config)).await.unwrap();
    config.warm_guests = 0;
    config.max_guests = 2;
    let dir = tempfile::tempdir().unwrap();
    let backend = MicrovmBackend::new(dir.path().to_owned(), config.clone());
    backend.initialize().await.unwrap();
    let first = SandboxSpec {
        resource_owner: None,
        guest_idle_seconds: None,
        resource_assignment: None,
        space_id: Some(SpaceId::new()),
        provider_login: false,
        account_id: Uuid::new_v4(),
        thread_id: Uuid::new_v4(),
        code_directory: None,
        image: None,
        worker: None,
        limits: Limits {
            memory_bytes: Some(256 << 20),
            cpu_millis: Some(500),
            pids: Some(64),
        },
    };
    let a = backend.start(&first).await.unwrap();
    assert_eq!(
        std::fs::metadata(backend.0.disk(first.account_id))
            .unwrap()
            .len(),
        u64::from(config.disk_mib) << 20,
    );
    let ram_kib: u64 = shell(&*a, "awk '/^MemTotal:/ {print $2}' /proc/meminfo")
        .await
        .trim()
        .parse()
        .unwrap();
    let configured_kib = u64::from(config.memory_mib) * 1024;
    assert!((configured_kib / 2..=configured_kib).contains(&ram_kib));
    let kernel = shell(&*a, "cat /proc/sys/kernel/random/boot_id").await;
    let host = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap();
    assert_ne!(kernel, host);
    shell(
        &*a,
        "echo saved > /work/private; echo home > $HOME/private; touch /root/calternal-overlay-test",
    )
    .await;
    assert_eq!(
        shell(
            &*a,
            "cat /sys/fs/cgroup/memory.max /sys/fs/cgroup/cpu.max /sys/fs/cgroup/pids.max"
        )
        .await,
        "268435456\n50000 100000\n64\n"
    );
    assert_eq!(
        shell(&*a, "grep -c '^processor' /proc/cpuinfo")
            .await
            .trim(),
        config.vcpus.to_string(),
    );
    let sibling = backend
        .start(&SandboxSpec {
            thread_id: Uuid::new_v4(),
            space_id: Some(SpaceId::new()),
            ..first.clone()
        })
        .await
        .unwrap();
    assert_eq!(
        shell(
            &*sibling,
            "test ! -e /work/private; test ! -e $HOME/private; test ! -e /root/calternal-overlay-test; cat /proc/sys/kernel/random/boot_id"
        )
        .await,
        kernel
    );
    let other = SandboxSpec {
        account_id: Uuid::new_v4(),
        ..first.clone()
    };
    let other_thread = backend.start(&other).await.unwrap();
    assert_ne!(
        shell(
            &*other_thread,
            "test ! -e /work/private; test ! -e $HOME/private; test ! -e /root/calternal-overlay-test; cat /proc/sys/kernel/random/boot_id"
        )
        .await,
        kernel
    );
    let exec = a
        .exec(ExecRequest {
            argv: vec!["sleep".into(), "120".into()],
            ..ExecRequest::default()
        })
        .await
        .unwrap();
    // Sync the fixture's files before modeling a crash; discard is exercised
    // through the same machine backend primitive used by durable erasure.
    shell(&*a, "sync").await;
    let slot = backend.0.account(first.account_id);
    slot.lock().await.vm.take().unwrap().stop().await.unwrap();
    let (_, _, status) = tokio::time::timeout(Duration::from_secs(30), collect(exec))
        .await
        .unwrap();
    assert_eq!(status.error.as_deref(), Some(crate::protocol::GUEST_LOST));
    assert!(!a.available());
    let resumed = backend.start(&first).await.unwrap();
    assert_eq!(
        shell(&*resumed, "cat /work/private $HOME/private").await,
        "saved\nhome\n"
    );
    resumed.release().await.unwrap();
    backend
        .erase(first.account_id, Some(first.thread_id))
        .await
        .unwrap();
    let clean = backend.start(&first).await.unwrap();
    shell(&*clean, "test ! -e /work/private; test -e $HOME/private").await;
    clean.release().await.unwrap();
    backend.erase(first.account_id, None).await.unwrap();
    assert!(!backend.0.disk(first.account_id).exists());
    shell(&*other_thread, "true").await;
    other_thread.release().await.unwrap();
    backend.shutdown().await.unwrap();
    assert!(!backend.0.root.join("run").exists());
}

#[tokio::test]
async fn offline_fallback_removes_only_the_erased_accounts_disk() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("microvm");
    std::fs::create_dir_all(root.join("accounts")).unwrap();
    std::fs::write(root.join("vmm.lock"), []).unwrap();
    let account = Uuid::new_v4();
    let other = Uuid::new_v4();
    let disk = super::account_disk(&root, account);
    let foreign = super::account_disk(&root, other);
    std::fs::write(&disk, b"one").unwrap();
    std::fs::write(&foreign, b"two").unwrap();
    assert!(
        super::erase_without_kvm(dir.path(), account, Some(Uuid::new_v4()))
            .await
            .is_err()
    );
    assert!(disk.exists());
    super::erase_without_kvm(dir.path(), account, None)
        .await
        .unwrap();
    assert!(!disk.exists());
    assert!(foreign.exists());
}

#[tokio::test]
async fn networked_guest_can_use_only_the_declared_egress() {
    if let Err(error) = super::probe_kvm().await {
        eprintln!("SKIP Firecracker TAP conformance: {error}");
        return;
    }
    let Some(path) = std::env::var_os("CALTERNAL_MICROVM_EGRESS_TEST_CONFIG") else {
        eprintln!("SKIP Firecracker TAP conformance: no disposable firewall namespace configured");
        return;
    };
    let config = Config::load(Path::new(&path)).await.unwrap();
    assert!(config.network_policy.is_some());
    let dir = tempfile::tempdir().unwrap();
    let backend = MicrovmBackend::new(dir.path().to_owned(), config);
    backend.initialize().await.unwrap();
    let sandbox = backend
        .start(&SandboxSpec {
            resource_owner: None,
            guest_idle_seconds: None,
            resource_assignment: None,
            space_id: Some(SpaceId::new()),
            provider_login: false,
            account_id: Uuid::new_v4(),
            thread_id: Uuid::new_v4(),
            code_directory: None,
            image: None,
            worker: None,
            limits: Limits::default(),
        })
        .await
        .unwrap();
    shell(
        &*sandbox,
        r#"node --input-type=module -e '
          import {connect} from "node:net";
          import {resolve4} from "node:dns/promises";
          const canConnect = (host, port) => new Promise(resolve => {
            const conn = connect({host, port});
            conn.on("connect", () => { conn.destroy(); resolve(true); });
            conn.on("error", () => resolve(false));
            conn.setTimeout(600, () => { conn.destroy(); resolve(false); });
          });
          const answer = await resolve4("egress.test");
          if (answer[0] !== "192.0.2.2") process.exit(1);
          if (!await canConnect(answer[0], 18080)) process.exit(2);
          if (await canConnect(answer[0], 18081)) process.exit(3);
          if (await canConnect("10.213.0.1", 18080)) process.exit(4);
          if (await canConnect("169.254.169.254", 18080)) process.exit(5);
          if (await canConnect("203.0.113.3", 18080)) process.exit(6);
          if (await canConnect("10.213.0.6", 18080)) process.exit(7);
        '"#,
    )
    .await;
    sandbox.release().await.unwrap();
    backend.shutdown().await.unwrap();
}
