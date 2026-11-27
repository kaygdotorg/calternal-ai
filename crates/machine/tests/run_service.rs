//! Integration tests for the bounded Podman run service.
//!
//! These tests use only the local fake Claude executable. Set
//! CALTERNAL_TEST_PROVIDER_IMAGE after building its image on the dev VM.
#[path = "../../../tests/support/podman_guard.rs"]
mod podman_guard;

use calternal_machine::{
    egress::{EgressDialer, EgressResolver},
    protocol::Stream,
    run_service::{
        ProgramBinding, RunAuthorizer, RunError, RunEventKind, RunScope, RunService, StartRun,
    },
    sandbox::{BoxFuture, podman::PodmanBackend},
};
use calternal_types::{AccountId, Actor, SpaceId};
use serde_json::{Value, json};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time::timeout,
};
use uuid::Uuid;

use podman_guard::PodmanGuard;

// Rootless Podman startup contention can exhaust the wrapper readiness deadline.
static CONTAINER_TEST: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct AllowAll;
impl RunAuthorizer for AllowAll {
    fn may_read<'a>(
        &'a self,
        _: &'a Actor,
        _: SpaceId,
        _: RunScope,
        _: &'a [Uuid],
    ) -> BoxFuture<'a, bool> {
        Box::pin(async { true })
    }
}

struct FixedResolver;
impl EgressResolver for FixedResolver {
    fn resolve<'a>(&'a self, _: &'a str) -> BoxFuture<'a, std::io::Result<Vec<IpAddr>>> {
        Box::pin(async { Ok(vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))]) })
    }
}

struct RedirectDialer(SocketAddr);
impl EgressDialer for RedirectDialer {
    fn connect<'a>(&'a self, _: SocketAddr) -> BoxFuture<'a, std::io::Result<TcpStream>> {
        Box::pin(async move { TcpStream::connect(self.0).await })
    }
}

fn image() -> String {
    std::env::var("CALTERNAL_TEST_PROVIDER_IMAGE")
        .unwrap_or_else(|_| "localhost/calternal-chat-claude-code-fake:dev".into())
}

async fn backend(root: &Path) -> Option<Arc<PodmanBackend>> {
    if !PodmanBackend::available(Path::new("podman")).await {
        eprintln!("SKIPPED: podman is not available");
        return None;
    }
    let image = image();
    let present = tokio::process::Command::new("podman")
        .args(["image", "exists", &image])
        .status()
        .await
        .is_ok_and(|status| status.success());
    if !present {
        eprintln!("SKIPPED: image {image} is not present locally");
        return None;
    }
    Some(Arc::new(PodmanBackend::new("podman", root, image)))
}

fn make_service(backend: Arc<PodmanBackend>, dialer: SocketAddr) -> RunService {
    RunService::with_egress(
        backend,
        image(),
        Arc::new(AllowAll),
        Arc::new(FixedResolver),
        Arc::new(RedirectDialer(dialer)),
    )
}

fn request() -> (StartRun, Actor) {
    let actor = Actor::account(AccountId::new());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("plugins-v2/calternal-chat-claude-code");
    let program = ProgramBinding::new(
        "calternal-chat-claude-code",
        "calternal-provider-claude-code",
        root,
        "programs/calternal-provider-claude-code",
        false,
        ["api.anthropic.com"],
    )
    .unwrap();
    (
        StartRun {
            program,
            space: SpaceId::new(),
            scope: RunScope::Session,
            targets: vec![Uuid::now_v7()],
            actor: actor.clone(),
        },
        actor,
    )
}

fn command(seq: u64, command: Value) -> Vec<u8> {
    format!(
        "{}\n",
        json!({"version": 1, "seq": seq, "command": command})
    )
    .into_bytes()
}

fn start_command() -> Value {
    json!({"type":"start","model":"claude-haiku-5-5","options":{},
        "cwd":"/work","instructions":"Echo supplied text.","resume":null})
}

fn send_prompt(prompt: &str) -> Value {
    json!({"type":"send","prompt":prompt,"context":[]})
}

async fn send(service: &RunService, run: Uuid, actor: &Actor, seq: u64, value: Value) {
    service
        .send_input(run, actor, command(seq, value))
        .await
        .unwrap();
}

async fn send_start(service: &RunService, run: Uuid, actor: &Actor) {
    send(service, run, actor, 1, start_command()).await;
}

async fn collect_reply(
    events: &mut mpsc::Receiver<calternal_machine::run_service::RunEvent>,
    pending: &mut Vec<u8>,
    last_sequence: &mut u64,
) -> String {
    let mut text = String::new();
    loop {
        if let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = pending.drain(..=end).collect();
            let value: Value = serde_json::from_slice(&line[..line.len() - 1]).unwrap();
            let event = &value["event"];
            match event["type"].as_str() {
                Some("text_delta") => text.push_str(event["text"].as_str().unwrap()),
                Some("done") => return text,
                _ => {}
            }
            continue;
        }
        let event = timeout(Duration::from_secs(12), events.recv())
            .await
            .unwrap()
            .expect("run closed before done");
        assert_eq!(event.sequence, *last_sequence + 1);
        *last_sequence = event.sequence;
        match event.event {
            RunEventKind::Data {
                stream: Stream::Stdout,
                bytes,
            } => pending.extend(bytes),
            RunEventKind::Data { .. } => {}
            RunEventKind::Exit(exit) => panic!("run exited before done: {exit:?}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn input_is_actor_bound_logs_are_ordered_and_direct_network_is_unavailable() {
    let _serial = CONTAINER_TEST.lock().await;
    let root = tempfile::tempdir().unwrap();
    let Some(backend) = backend(root.path()).await else {
        return;
    };
    let _containers = PodmanGuard::new(root.path());
    let service = make_service(backend, "127.0.0.1:9".parse().unwrap());
    let (request, actor) = request();
    let account = actor.account.as_uuid();
    let mut run = service.start(request).await.unwrap();
    let other = Actor::account(AccountId::new());
    assert!(matches!(
        service
            .send_input(run.id, &other, command(1, start_command()))
            .await,
        Err(RunError::InputActor)
    ));
    let container = PodmanBackend::container_name(account, run.id);
    let network = tokio::process::Command::new("podman")
        .args([
            "inspect",
            "--format",
            "{{.HostConfig.NetworkMode}}",
            &container,
        ])
        .output()
        .await
        .unwrap();
    assert!(network.status.success());
    assert_eq!(String::from_utf8_lossy(&network.stdout).trim(), "none");

    send_start(&service, run.id, &actor).await;
    let mut pending = Vec::new();
    let mut last_sequence = 0;
    send(&service, run.id, &actor, 2, send_prompt("direct-check")).await;
    assert_eq!(
        collect_reply(&mut run.events, &mut pending, &mut last_sequence).await,
        "Direct: blocked"
    );
    send(&service, run.id, &actor, 3, send_prompt("second")).await;
    assert_eq!(
        collect_reply(&mut run.events, &mut pending, &mut last_sequence).await,
        "Echo: second"
    );
    service.stop(run.id, &other).await.unwrap();
    let exit = run.wait_exit().await;
    assert!(exit.code.is_some() || exit.signal.is_some() || exit.error.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn declared_host_tunnels_to_the_local_test_server_from_the_fake_provider() {
    let _serial = CONTAINER_TEST.lock().await;
    let root = tempfile::tempdir().unwrap();
    let Some(backend) = backend(root.path()).await else {
        return;
    };
    let _containers = PodmanGuard::new(root.path());
    let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_address = server.local_addr().unwrap();
    let server_task = tokio::spawn(async move {
        let (mut socket, _) = server.accept().await.unwrap();
        let mut input = [0; 5];
        socket.read_exact(&mut input).await.unwrap();
        assert_eq!(&input, b"hello");
        socket.write_all(b"world").await.unwrap();
    });
    let service = make_service(backend, server_address);
    let (request, actor) = request();
    let mut run = service.start(request).await.unwrap();
    send_start(&service, run.id, &actor).await;
    send(&service, run.id, &actor, 2, send_prompt("egress-check")).await;
    let mut pending = Vec::new();
    let mut last_sequence = 0;
    assert_eq!(
        collect_reply(&mut run.events, &mut pending, &mut last_sequence).await,
        "Egress: world"
    );
    server_task.await.unwrap();
    service.stop(run.id, &actor).await.unwrap();
    let _ = run.wait_exit().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_kills_the_ignoring_fake_process_family_within_two_seconds() {
    let _serial = CONTAINER_TEST.lock().await;
    let root = tempfile::tempdir().unwrap();
    let Some(backend) = backend(root.path()).await else {
        return;
    };
    let _containers = PodmanGuard::new(root.path());
    let service = make_service(backend, "127.0.0.1:9".parse().unwrap());
    let (request, actor) = request();
    let mut run = service.start(request).await.unwrap();
    send_start(&service, run.id, &actor).await;
    send(
        &service,
        run.id,
        &actor,
        2,
        send_prompt("hold: ignore signals"),
    )
    .await;

    let mut pending = Vec::new();
    let mut last_sequence = 0;
    loop {
        if let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = pending.drain(..=end).collect();
            let value: Value = serde_json::from_slice(&line[..line.len() - 1]).unwrap();
            if value["event"]["type"] == "text_delta" {
                break;
            }
            continue;
        }
        let event = timeout(Duration::from_secs(12), run.events.recv())
            .await
            .unwrap()
            .expect("fake provider exited before it started");
        assert_eq!(event.sequence, last_sequence + 1);
        last_sequence = event.sequence;
        if let RunEventKind::Data {
            stream: Stream::Stdout,
            bytes,
        } = event.event
        {
            pending.extend(bytes);
        }
    }

    let authorized_reader = Actor::account(AccountId::new());
    let started = Instant::now();
    service.stop(run.id, &authorized_reader).await.unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "stop took {:?}",
        started.elapsed()
    );
    timeout(Duration::from_secs(2), run.wait_exit())
        .await
        .expect("run did not report exit after Stop");
}

#[tokio::test(flavor = "multi_thread")]
async fn operator_sign_in_mounts_one_file_for_one_account_space_and_program() {
    let _serial = CONTAINER_TEST.lock().await;
    let root = tempfile::tempdir().unwrap();
    if backend(root.path()).await.is_none() {
        return;
    }
    let _containers = PodmanGuard::new(root.path());
    let profile = tempfile::tempdir().unwrap();
    use std::os::unix::fs::PermissionsExt;
    let credential = profile.path().join(".credentials.json");
    std::fs::write(&credential, "test fixture").unwrap();
    std::fs::set_permissions(&credential, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(profile.path().join("unrelated"), "unrelated file").unwrap();
    let (request, actor) = request();
    let qualified = "calternal-chat-claude-code.calternal-provider-claude-code";
    let backend = PodmanBackend::new("podman", root.path(), image())
        .with_run_sign_in(actor.account, request.space, qualified.into(), credential)
        .unwrap();
    let service = make_service(Arc::new(backend), "127.0.0.1:9".parse().unwrap());
    for (account, space, other_program) in [
        (AccountId::new(), request.space, false),
        (actor.account, SpaceId::new(), false),
        (actor.account, request.space, true),
    ] {
        let program = if other_program {
            ProgramBinding::new(
                "calternal-chat-claude-code",
                "untrusted",
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../plugins-v2/calternal-chat-claude-code"),
                "programs/calternal-provider-claude-code",
                false,
                ["api.anthropic.com"],
            )
            .unwrap()
        } else {
            request.program.clone()
        };
        let denied = service
            .start(StartRun {
                program,
                space,
                scope: request.scope,
                targets: request.targets.clone(),
                actor: Actor::account(account),
            })
            .await;
        assert!(matches!(denied, Err(RunError::Sandbox(error)) if
            error.to_string().contains("cannot use operator sign-in")));
    }
    let mut run = service.start(request).await.unwrap();
    let name = PodmanBackend::container_name(actor.account.as_uuid(), run.id);
    let result = tokio::process::Command::new("podman")
        .args([
            "exec",
            &name,
            "/bin/sh",
            "-ec",
            r#"test "$(cat /run/calternal-provider-login/.credentials.json)" = 'test fixture'
               test ! -e /run/calternal-provider-login/unrelated
               ! printf changed >> /run/calternal-provider-login/.credentials.json
               test "$(stat -f -c %T /home/agent)" = tmpfs
               grep -Eq ' /home/agent tmpfs [^ ]*size=65536k' /proc/mounts
               ! touch /opt/outside-tmpfs"#,
        ])
        .output()
        .await
        .unwrap();
    assert!(
        result.status.success(),
        "credential mount boundary check failed"
    );
    assert_eq!(
        std::fs::read_to_string(profile.path().join(".credentials.json")).unwrap(),
        "test fixture"
    );
    send_start(&service, run.id, &actor).await;
    send(&service, run.id, &actor, 2, send_prompt("egress-denied")).await;
    assert_eq!(
        collect_reply(&mut run.events, &mut Vec::new(), &mut 0).await,
        "Egress: refused"
    );
    service.stop(run.id, &actor).await.unwrap();
    let _ = run.wait_exit().await;
}
