//! Podman-backed run harness acceptance with a fake echo provider.

#[path = "../../../tests/support/podman_guard.rs"]
mod podman_guard;

use calternal_machine::{
    protocol::{HarnessSandbox, Limits, ProviderLoginScope, SandboxSpec, Stream},
    sandbox::{Backend, ExecRequest, OutputEvent, podman::PodmanBackend},
};
use calternal_types::{AccountId, SpaceId};
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use tokio::{sync::mpsc, time::timeout};
use uuid::Uuid;

use podman_guard::PodmanGuard;

const PROGRAM: &str = "/opt/calternal-chat-claude-code/programs/calternal-provider-claude-code";

fn image() -> String {
    std::env::var("CALTERNAL_TEST_PROVIDER_IMAGE")
        .unwrap_or_else(|_| "localhost/calternal-chat-claude-code-fake:dev".into())
}

async fn backend(root: &Path) -> Option<PodmanBackend> {
    let podman = Path::new("podman");
    if !PodmanBackend::available(podman).await {
        eprintln!("SKIPPED: podman is not available");
        return None;
    }
    let image = image();
    let present = tokio::process::Command::new(podman)
        .args(["image", "exists", &image])
        .status()
        .await
        .is_ok_and(|status| status.success());
    if !present {
        eprintln!("SKIPPED: image {image} is not present locally");
        return None;
    }
    Some(PodmanBackend::new(podman, root, image))
}

fn spec() -> (SandboxSpec, AccountId) {
    let account = AccountId::new();
    (
        SandboxSpec {
            space_id: Some(SpaceId::new()),
            provider_login: false,
            shared_provider_sponsor: None,
            shared_tools: None,
            code_directory: None,
            worker: None,
            account_id: account.as_uuid(),
            thread_id: Uuid::now_v7(),
            image: None,
            limits: Limits::default(),
        },
        account,
    )
}

fn harness(account: AccountId) -> HarnessSandbox {
    HarnessSandbox {
        provider_id: "calternal-chat-claude-code".into(),
        login_scope: ProviderLoginScope::Account(account),
        workspace_read_only: true,
        disposable_profile: true,
        legacy_profile: false,
        guest_image_digest: None,
    }
}

async fn send_command(input: &mpsc::Sender<Vec<u8>>, seq: u64, command: Value) {
    input
        .send(
            format!(
                "{}\n",
                json!({
                    "version": 1, "seq": seq, "command": command
                })
            )
            .into_bytes(),
        )
        .await
        .unwrap();
}

async fn next_frame(events: &mut mpsc::Receiver<OutputEvent>, pending: &mut Vec<u8>) -> Value {
    loop {
        if let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = pending.drain(..=end).collect();
            return serde_json::from_slice(&line[..line.len() - 1]).unwrap();
        }
        match events.recv().await.expect("adapter ended before its frame") {
            OutputEvent::Data(Stream::Stdout, bytes) => pending.extend(bytes),
            OutputEvent::Data(Stream::Stderr, _) => {}
            OutputEvent::Exit(exit) => panic!("adapter exited before its frame: {exit:?}"),
        }
    }
}

fn start_command() -> Value {
    json!({"type":"start","model":"claude-haiku-5-5","options":{},
        "cwd":"/work","instructions":"Echo the supplied message.","resume":null})
}

fn send_prompt(prompt: &str) -> Value {
    json!({"type":"send","prompt":prompt,"context":[]})
}

#[derive(Debug, PartialEq, Eq)]
struct ReplyRecord {
    item: Uuid,
    text: String,
}

#[derive(Default)]
struct ReplySinkStub {
    received: Vec<(Uuid, ReplyRecord)>,
}

impl ReplySinkStub {
    fn append(&mut self, run: Uuid, item: Uuid, text: String) {
        self.received.push((run, ReplyRecord { item, text }));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn fake_echo_stream_becomes_one_reply_and_stop_kills_the_family() {
    let root = tempfile::tempdir().unwrap();
    let Some(backend) = backend(root.path()).await else {
        return;
    };
    let _containers = PodmanGuard::new(root.path());
    let (spec, account) = spec();
    let target = Uuid::now_v7();
    let sandbox = backend
        .start_harness(&spec, &harness(account))
        .await
        .unwrap();

    let container = PodmanBackend::container_name(spec.account_id, spec.thread_id);
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

    let run_id = Uuid::now_v7();
    let mut sink = ReplySinkStub::default();
    let mut exec = sandbox
        .exec(ExecRequest {
            argv: vec![PROGRAM.into()],
            interactive: true,
            stdin: String::new(),
            env: Vec::new(),
        })
        .await
        .unwrap();
    let input = exec.input.take().expect("interactive adapter input");
    send_command(&input, 1, start_command()).await;
    send_command(&input, 2, send_prompt("hello from slice 6")).await;
    drop(input);

    let mut pending = Vec::new();
    let mut partial = String::new();
    loop {
        let event = next_frame(&mut exec.events, &mut pending).await["event"].clone();
        match event["type"].as_str() {
            Some("text_delta") => partial.push_str(event["text"].as_str().unwrap()),
            Some("done") => {
                assert_eq!(event["outcome"], "done");
                break;
            }
            _ => {}
        }
    }
    sink.append(run_id, target, partial.clone());
    assert_eq!(partial, "Echo: hello from slice 6");
    assert_eq!(
        sink.received,
        vec![(
            run_id,
            ReplyRecord {
                item: target,
                text: "Echo: hello from slice 6".into(),
            }
        )]
    );

    let mut active = sandbox
        .exec(ExecRequest {
            argv: vec![PROGRAM.into()],
            interactive: true,
            stdin: String::new(),
            env: Vec::new(),
        })
        .await
        .unwrap();
    let input = active.input.take().expect("interactive adapter input");
    send_command(&input, 1, start_command()).await;
    send_command(&input, 2, send_prompt("hold: stop family")).await;
    drop(input);

    let mut pending = Vec::new();
    loop {
        let frame = next_frame(&mut active.events, &mut pending).await;
        if frame["event"]["type"] == "text_delta" {
            break;
        }
    }
    let started = Instant::now();
    sandbox.release().await.unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "Podman run stop took {:?}",
        started.elapsed()
    );

    let exit = timeout(Duration::from_secs(2), async {
        loop {
            match active.events.recv().await {
                Some(OutputEvent::Exit(exit)) => break Some(exit),
                Some(OutputEvent::Data(..)) => {}
                None => break None,
            }
        }
    })
    .await
    .expect("the run process family remained after Stop");
    assert!(exit.is_some(), "the adapter exec did not end after Stop");
}
