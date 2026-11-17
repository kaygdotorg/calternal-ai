//! Rootless Podman backend isolation basics.
//!
//! SKIPPED (each test prints why and passes) when `podman` is missing or the image
//! `CALTERNAL_TEST_IMAGE` (default `docker.io/library/alpine:3`) is not present
//! locally; the tests never pull.

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
async fn freeze_resume_and_adoption_after_restart() {
    let root = tempfile::tempdir().unwrap();
    let Some(b) = backend(root.path()).await else {
        return;
    };
    let s = spec(Limits::default());
    let sb = b.start(&s).await.unwrap();
    sh(&sb, "echo before > state").await;
    sb.freeze().await.unwrap();
    // A new backend instance (a restarted machine role) adopts the paused container.
    let again = PodmanBackend::new("podman", root.path(), image())
        .start(&s)
        .await
        .unwrap();
    assert_eq!(sh(&again, "cat state").await, ("before\n".into(), 0));
    again.freeze().await.unwrap();
    again.resume().await.unwrap();
    assert_eq!(sh(&again, "echo ok").await.1, 0);
    again.stop().await.unwrap();
}

#[tokio::test]
async fn persistent_homes_are_shared_only_with_the_same_account() {
    let root = tempfile::tempdir().unwrap();
    let Some(backend) = backend(root.path()).await else {
        return;
    };
    let first = spec(Limits::default());
    let mut same = spec(Limits::default());
    same.account_id = first.account_id;
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
