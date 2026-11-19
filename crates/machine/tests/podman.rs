//! Rootless Podman backend isolation basics.
//!
//! SKIPPED (each test prints why and passes) when `podman` is missing or the image
//! `CALTERNAL_TEST_IMAGE` (default `docker.io/library/alpine:3`) is not present
//! locally; the tests never pull.

#[path = "../../../tests/support/podman_guard.rs"]
mod podman_guard;
use podman_guard::PodmanGuard;

use std::sync::Arc;

use calternal_machine::protocol::{Limits, SandboxSpec};
use calternal_machine::sandbox::podman::PodmanBackend;
use calternal_machine::sandbox::{Backend, ExecRequest, OutputEvent, Sandbox};
use uuid::Uuid;

fn image() -> String {
    std::env::var("CALTERNAL_TEST_IMAGE").unwrap_or_else(|_| "docker.io/library/alpine:3".into())
}

async fn backend(root: &std::path::Path) -> Option<PodmanBackend> {
    let podman = std::path::Path::new("podman");
    if !PodmanBackend::available(podman).await {
        eprintln!("SKIPPED: podman is not available");
        return None;
    }
    let present = tokio::process::Command::new("podman")
        .args(["image", "exists", &image()])
        .status()
        .await
        .is_ok_and(|s| s.success());
    if !present {
        eprintln!("SKIPPED: image {} is not present locally", image());
        return None;
    }
    Some(PodmanBackend::new(podman, root, image()))
}

fn spec(limits: Limits) -> SandboxSpec {
    SandboxSpec {
        space_id: Some(calternal_types::SpaceId::new()),
        provider_login: false,
        shared_provider_sponsor: None,
        shared_tools: None,
        code_directory: None,
        worker: None,
        account_id: Uuid::new_v4(),
        thread_id: Uuid::new_v4(),
        image: None,
        limits,
    }
}

async fn sh(sb: &Arc<dyn Sandbox>, script: &str) -> (String, i32) {
    command(sb, vec!["sh".into(), "-c".into(), script.into()], vec![]).await
}

async fn command(
    sb: &Arc<dyn Sandbox>,
    argv: Vec<String>,
    env: Vec<(String, String)>,
) -> (String, i32) {
    let exec = sb
        .exec(ExecRequest {
            interactive: false,
            stdin: String::new(),
            argv,
            env,
        })
        .await
        .unwrap();
    let (mut text, mut events) = (Vec::new(), exec.events);
    while let Some(ev) = events.recv().await {
        match ev {
            OutputEvent::Data(_, b) => text.extend(b),
            OutputEvent::Exit(e) => {
                return (
                    String::from_utf8_lossy(&text).into_owned(),
                    e.code.unwrap_or(-1),
                );
            }
        }
    }
    panic!("exec ended without exit")
}

#[tokio::test(flavor = "multi_thread")]
async fn threads_are_isolated_and_limited() {
    let root = tempfile::tempdir().unwrap();
    let Some(b) = backend(root.path()).await else {
        return;
    };
    let _containers = PodmanGuard::new(root.path());
    let limits = Limits {
        memory_bytes: Some(256 << 20),
        cpu_millis: Some(500),
        pids: Some(64),
    };
    let (sa, sb) = (spec(limits), spec(Limits::default()));
    let a = b.start(&sa).await.unwrap();
    let c = b.start(&sb).await.unwrap();

    // Work directory is the thread's own and is visible on the host.
    assert_eq!(
        sh(&a, "pwd; echo mine > note; ls").await,
        ("/work\nnote\n".into(), 0)
    );
    assert!(
        root.path()
            .join("accounts")
            .join(sa.account_id.to_string())
            .join("threads")
            .join(sa.thread_id.to_string())
            .join("work/note")
            .exists()
    );
    // The other thread sees neither the file nor the host's files.
    assert_eq!(sh(&c, "ls /work; cat /work/note").await.1, 1);
    let host = root.path().display().to_string();
    assert_ne!(
        sh(&c, &format!("ls {host}")).await.1,
        0,
        "host path visible in sandbox"
    );
    // Writes outside /work go to the container's private layer only.
    sh(&a, "echo private > /etc/calternal-test").await;
    assert_ne!(sh(&c, "cat /etc/calternal-test").await.1, 0);
    // Limits reach the cgroup.
    assert_eq!(
        sh(&a, "cat /sys/fs/cgroup/memory.max").await.0.trim(),
        (256u64 << 20).to_string()
    );
    assert_eq!(sh(&a, "cat /sys/fs/cgroup/pids.max").await.0.trim(), "64");
    assert_eq!(
        sh(&a, "cat /sys/fs/cgroup/cpu.max").await.0.trim(),
        "50000 100000"
    );
    assert_eq!(
        sh(&c, "cat /sys/fs/cgroup/memory.max").await.0.trim(),
        (1u64 << 30).to_string()
    );
    // Name-only Podman flags still deliver the requested value inside the container.
    let received = command(
        &a,
        vec!["printenv".into(), "CALTERNAL_AGENT_TOKEN".into()],
        vec![(
            "CALTERNAL_AGENT_TOKEN".into(),
            "synthetic-private-token".into(),
        )],
    )
    .await;
    assert_eq!(received, ("synthetic-private-token\n".into(), 0));
    // The machine's environment does not leak in.
    assert!(!sh(&a, "env").await.0.contains("CARGO"));

    a.stop().await.unwrap();
    c.stop().await.unwrap();
    assert!(!root.path().join(sa.thread_id.to_string()).exists());
    let gone = tokio::process::Command::new("podman")
        .args([
            "container",
            "exists",
            &PodmanBackend::container_name(sa.account_id, sa.thread_id),
        ])
        .status()
        .await
        .unwrap();
    assert!(!gone.success(), "container left behind");
}

#[tokio::test(flavor = "multi_thread")]
async fn release_and_restart_remove_containers_but_keep_work() {
    let root = tempfile::tempdir().unwrap();
    let Some(b) = backend(root.path()).await else {
        return;
    };
    let _containers = PodmanGuard::new(root.path());
    let s = spec(Limits::default());
    let sb = b.start(&s).await.unwrap();
    sh(&sb, "echo before > state").await;
    sb.freeze().await.unwrap();
    sb.resume().await.unwrap();
    let other_root = tempfile::tempdir().unwrap();
    let other = backend(other_root.path()).await.unwrap();
    let _foreign = PodmanGuard::new(other_root.path());
    let foreign = other.start(&spec(Limits::default())).await.unwrap();
    b.initialize().await.unwrap();
    let name = PodmanBackend::container_name(s.account_id, s.thread_id);
    let exists = || {
        std::process::Command::new("podman")
            .args(["container", "exists", &name])
            .status()
            .unwrap()
            .success()
    };
    let orphan_was_removed = !exists();
    let again = b.start(&s).await.unwrap();
    assert_eq!(sh(&again, "cat state").await, ("before\n".into(), 0));
    assert_eq!(sh(&foreign, "echo ok").await.1, 0);
    again.release().await.unwrap();
    let released_was_removed = !exists();
    again.stop().await.unwrap();
    foreign.stop().await.unwrap();
    assert!(
        orphan_was_removed,
        "startup left its orphan container behind"
    );
    assert!(released_was_removed, "release retained its container");
}

#[tokio::test]
async fn persistent_homes_are_shared_only_with_the_same_account() {
    let root = tempfile::tempdir().unwrap();
    let Some(backend) = backend(root.path()).await else {
        return;
    };
    let _containers = PodmanGuard::new(root.path());
    let first = spec(Limits::default());
    let mut same = spec(Limits::default());
    same.account_id = first.account_id;
    same.space_id = first.space_id;
    let mut other = spec(Limits::default());
    other.thread_id = first.thread_id;
    let a = backend.start(&first).await.unwrap();
    assert_eq!(sh(&a, "echo account-a > \"$HOME/login-marker\"").await.1, 0);
    let b = backend.start(&same).await.unwrap();
    assert_eq!(sh(&b, "cat \"$HOME/login-marker\"").await.0, "account-a\n");
    let c = backend.start(&other).await.unwrap();
    assert_eq!(sh(&c, "test ! -e \"$HOME/login-marker\"").await.1, 0);
    assert_eq!(sh(&c, "echo account-b > \"$HOME/login-marker\"").await.1, 0);
    assert_eq!(sh(&b, "cat \"$HOME/login-marker\"").await.0, "account-a\n");
    a.release().await.unwrap();
    backend
        .discard(first.account_id, first.thread_id)
        .await
        .unwrap();
    assert_eq!(sh(&b, "cat \"$HOME/login-marker\"").await.0, "account-a\n");
    b.stop().await.unwrap();
    c.stop().await.unwrap();
}

#[tokio::test]
async fn container_guard_cleans_up_during_unwind() {
    let root = tempfile::tempdir().unwrap();
    let Some(backend) = backend(root.path()).await else {
        return;
    };
    let guard = PodmanGuard::new(root.path());
    let sandbox = spec(Limits::default());
    let _active = backend.start(&sandbox).await.unwrap();
    let panic = std::panic::catch_unwind(move || {
        let _guard = guard;
        panic!("synthetic test failure");
    });
    assert!(panic.is_err());
    let name = PodmanBackend::container_name(sandbox.account_id, sandbox.thread_id);
    let status = tokio::process::Command::new("podman")
        .args(["container", "exists", &name])
        .status()
        .await
        .unwrap();
    assert_eq!(status.code(), Some(1));
}

#[tokio::test]
async fn parking_a_lease_removes_its_container_and_restart_reuses_its_mount() {
    use calternal_machine::{
        link::Link,
        protocol::{Down, SandboxState},
        runner::Runner,
    };
    use calternal_types::{AccountId, SpaceId};
    use std::time::Duration;

    let root = tempfile::tempdir().unwrap();
    let Some(backend) = backend(root.path()).await else {
        return;
    };
    let _guard = PodmanGuard::new(root.path());
    let runner = Arc::new(Runner::new(
        Arc::new(backend),
        Arc::new(Link::new("http://127.0.0.1:1").unwrap()),
        1,
    ));
    let account_id = AccountId::new();
    let space_id = SpaceId::new();
    let lease_id = Uuid::new_v4();
    let mut sandbox = spec(Limits::default());
    sandbox.account_id = account_id.as_uuid();
    let assign = || Down::Assign {
        lease_id,
        fence: 1,
        space_id,
        account_id,
        spec: sandbox.clone(),
    };
    let wait_running = || async {
        tokio::time::timeout(Duration::from_secs(35), async {
            loop {
                match &runner.reports()[0].sandbox {
                    SandboxState::Running => return,
                    SandboxState::Failed { reason } => panic!("sandbox start failed: {reason}"),
                    _ => tokio::time::sleep(Duration::from_millis(25)).await,
                }
            }
        })
        .await
        .expect("sandbox did not start");
    };
    runner.handle(assign()).await;
    wait_running().await;
    let work = root.path().join(format!(
        "accounts/{account_id}/threads/{}/work",
        sandbox.thread_id,
    ));
    tokio::fs::write(work.join("marker"), "retained")
        .await
        .unwrap();

    // Test cleanup without coupling the assertion to either side of a long poll.
    runner.park_idle(Duration::ZERO).await;
    let name = PodmanBackend::container_name(sandbox.account_id, sandbox.thread_id);
    let status = tokio::process::Command::new("podman")
        .args(["container", "exists", &name])
        .status()
        .await
        .unwrap();
    assert_eq!(status.code(), Some(1));
    assert_eq!(runner.capacity().running_sandboxes, 1);

    runner.handle(assign()).await;
    wait_running().await;
    let exec = tokio::process::Command::new("podman")
        .args(["exec", &name, "cat", "/work/marker"])
        .output()
        .await
        .unwrap();
    assert!(exec.status.success());
    assert_eq!(exec.stdout, b"retained");
    assert!(runner.release_all().await);
}

#[tokio::test(flavor = "multi_thread")]
async fn one_account_shares_claude_login_but_not_space_transcripts() {
    let root = tempfile::tempdir().unwrap();
    let Some(backend) = backend(root.path()).await else {
        return;
    };
    let _containers = PodmanGuard::new(root.path());
    let a_spec = spec(Limits::default());
    let mut b_spec = spec(Limits::default());
    b_spec.account_id = a_spec.account_id;
    let credentials = root
        .path()
        .join(format!("accounts/{}/home/.claude", a_spec.account_id));
    std::fs::create_dir_all(&credentials).unwrap();
    std::fs::write(
        credentials.join(".credentials.json"),
        r#"{"claudeAiOauth":{"accessToken":"fixture-login"},"mcpOAuth":"private-mcp"}"#,
    )
    .unwrap();
    let a = backend.start(&a_spec).await.unwrap();
    let b = backend.start(&b_spec).await.unwrap();
    assert_eq!(
        sh(
            &a,
            "mkdir -p ~/.claude/projects; echo private-space-a > ~/.claude/projects/transcript"
        )
        .await
        .1,
        0
    );
    for sandbox in [&a, &b] {
        assert_eq!(
            sh(
                sandbox,
                "cat \"$CALTERNAL_CLAUDE_LOGIN_DIR/.credentials.json\""
            )
            .await,
            (
                r#"{"claudeAiOauth":{"accessToken":"fixture-login"}}"#.into(),
                0
            )
        );
    }
    assert_eq!(sh(&b, "test ! -e ~/.claude/projects/transcript").await.1, 0);
    assert_ne!(
        sh(
            &b,
            "echo replaced > \"$CALTERNAL_CLAUDE_LOGIN_DIR/.credentials.json\""
        )
        .await
        .1,
        0
    );
    a.stop().await.unwrap();
    b.stop().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn preview_reads_only_its_podman_loopback_and_limits_bytes() {
    use calternal_machine::protocol::{PREVIEW_BODY_BYTES, PreviewMetadata, PreviewRequest};
    let root = tempfile::tempdir().unwrap();
    let Some(backend) = backend(root.path()).await else {
        return;
    };
    let _containers = PodmanGuard::new(root.path());
    let sb = backend.start(&spec(Limits::default())).await.unwrap();
    let mut service = sb.exec(ExecRequest {
        argv: vec!["node".into(), "-e".into(), format!(r#"
const http = require('node:http');
http.createServer((req, res) => {{
  if (req.url === '/redirect') {{ res.writeHead(302, {{location: 'http://127.0.0.1:1'}}); res.end(); return; }}
  if (req.url === '/oversize') {{ res.end(Buffer.alloc({PREVIEW_BODY_BYTES} + 1)); return; }}
  res.setHeader('Content-Type', 'text/plain');
  res.end(req.url);
}}).listen(35111, '127.0.0.1', () => console.log('ready'));
"#)],
        ..ExecRequest::default()
    }).await.unwrap();
    let startup = tokio::time::timeout(std::time::Duration::from_secs(8), service.events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(startup, OutputEvent::Data(_, bytes) if bytes.starts_with(b"ready")));

    for (path, expected) in [("/hello?probe=1", 0), ("/redirect", 1), ("/oversize", 1)] {
        let mut relay = sb
            .preview(PreviewRequest {
                port: 35111,
                path: path.into(),
            })
            .await
            .unwrap();
        let (bytes, code) = tokio::time::timeout(std::time::Duration::from_secs(12), async {
            let mut bytes = Vec::new();
            loop {
                match relay.events.recv().await.expect("preview exit") {
                    OutputEvent::Data(calternal_machine::protocol::Stream::Stdout, chunk) => {
                        bytes.extend_from_slice(&chunk)
                    }
                    OutputEvent::Data(..) => (),
                    OutputEvent::Exit(exit) => break (bytes, exit.code),
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(code, Some(expected), "unexpected result for {path}");
        assert!(bytes.len() <= PREVIEW_BODY_BYTES + 1024);
        if expected == 0 {
            let newline = bytes.iter().position(|b| *b == b'\n').unwrap();
            let meta: PreviewMetadata = serde_json::from_slice(&bytes[..newline]).unwrap();
            assert_eq!(meta.status, 200);
            assert_eq!(&bytes[newline + 1..], path.as_bytes());
        }
    }
    sb.release().await.unwrap();
}
