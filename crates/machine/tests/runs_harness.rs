//! Podman-backed run harness acceptance with a fake echo provider.

#[path = "../../../tests/support/podman_guard.rs"]
mod podman_guard;

use calternal_machine::{
    protocol::Stream,
    run_service::{ProgramBinding, RunAuthorizer, RunEventKind, RunScope, RunService, StartRun},
    sandbox::{BoxFuture, podman::PodmanBackend},
};
use calternal_types::{AccountId, Actor, SpaceId};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc, time::Duration};
use tokio::time::timeout;
use uuid::Uuid;

use podman_guard::PodmanGuard;

async fn backend(root: &Path) -> Option<(Arc<PodmanBackend>, String)> {
    let explicit = std::env::var("CALTERNAL_TEST_PROVIDER_IMAGE").ok();
    let image = explicit
        .clone()
        .unwrap_or_else(|| "localhost/calternal-chat-claude-code-fake:dev".into());
    let available = PodmanBackend::available(Path::new("podman")).await
        && tokio::process::Command::new("podman")
            .args(["image", "exists", &image])
            .status()
            .await
            .is_ok_and(|status| status.success());
    if !available {
        assert!(
            explicit.is_none(),
            "Podman or requested fake image {image} is unavailable"
        );
        eprintln!("SKIPPED: build the local fake provider image first");
        return None;
    }
    Some((
        Arc::new(PodmanBackend::new("podman", root, image.clone())),
        image,
    ))
}

struct Owner;
impl RunAuthorizer for Owner {
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

#[tokio::test]
async fn installed_wrapper_echoes_with_machine_managed_egress_and_control() {
    let root = tempfile::tempdir().unwrap();
    let Some((backend, image)) = backend(root.path()).await else {
        return;
    };
    let _containers = PodmanGuard::new(root.path());
    let actor = Actor::account(AccountId::new());
    let service = RunService::new(backend, image, Arc::new(Owner));
    let mut run = service
        .start(StartRun {
            program: ProgramBinding::new(
                "calternal-chat-claude-code",
                "calternal-provider-claude-code",
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../plugins-v2/calternal-chat-claude-code"),
                "programs/calternal-provider-claude-code",
                false,
                ["api.anthropic.com"],
            )
            .unwrap(),
            space: SpaceId::new(),
            scope: RunScope::Session,
            targets: vec![Uuid::now_v7()],
            actor: actor.clone(),
        })
        .await
        .unwrap();
    for (seq, command) in [
        (
            1,
            json!({"type":"start","model":"claude-haiku-5-5","options":{},
                   "cwd":"/work","instructions":"Echo the supplied message.","resume":null}),
        ),
        (
            2,
            json!({"type":"send","prompt":"hello from slice 6","context":[]}),
        ),
    ] {
        let frame = format!("{}\n", json!({"version":1,"seq":seq,"command":command}));
        service
            .send_input(run.id, &actor, frame.into_bytes())
            .await
            .unwrap();
    }
    let reply = timeout(Duration::from_secs(15), async {
        let mut pending = Vec::new();
        let mut reply = String::new();
        loop {
            match run
                .events
                .recv()
                .await
                .expect("run closed before its reply")
                .event
            {
                RunEventKind::Data {
                    stream: Stream::Stdout,
                    bytes,
                } => pending.extend(bytes),
                RunEventKind::Data { .. } => continue,
                RunEventKind::Exit(exit) => panic!("run exited before its reply: {exit:?}"),
            }
            while let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
                let line: Vec<_> = pending.drain(..=end).collect();
                let frame: Value = serde_json::from_slice(&line).unwrap();
                let event = &frame["event"];
                match event["type"].as_str() {
                    Some("text_delta") => reply.push_str(event["text"].as_str().unwrap()),
                    Some("done") => {
                        assert_eq!(event["outcome"], "done");
                        return reply;
                    }
                    _ => {}
                }
            }
        }
    })
    .await
    .expect("fake provider did not reply");
    assert_eq!(reply, "Echo: hello from slice 6");
    service.stop(run.id, &actor).await.unwrap();
}
