use super::*;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn podman_exec_keeps_environment_values_out_of_host_argv() {
    let dir = tempfile::tempdir().unwrap();
    let engine = dir.path().join("podman");
    // A fake engine records argv and verifies the child environment, without
    // requiring Podman or emitting the credential in test output.
    tokio::fs::write(&engine,b"#!/bin/sh\nfor arg do printf '%s\\n' \"$arg\"; done\n[ \"$CALTERNAL_AGENT_TOKEN\" = 'synthetic-private-token' ] || exit 42\n[ \"$MULTILINE\" = 'first\nsecond=value' ] || exit 43\n").await.unwrap();
    tokio::fs::set_permissions(&engine, std::fs::Permissions::from_mode(0o700))
        .await
        .unwrap();
    let sandbox = PodmanSandbox {
        podman: engine,
        container: "fixture".into(),
        dir: dir.path().join("work"),
        clients: Mutex::new(HashSet::new()),
        worker: false,
        broker: None,
    };
    let mut exec = sandbox
        .exec(ExecRequest {
            interactive: false,
            argv: vec!["true".into()],
            stdin: String::new(),
            env: vec![
                (
                    "CALTERNAL_AGENT_TOKEN".into(),
                    "synthetic-private-token".into(),
                ),
                ("MULTILINE".into(), "first\nsecond=value".into()),
            ],
        })
        .await
        .unwrap();
    let mut output = Vec::new();
    let mut status = None;
    while let Some(event) = exec.events.recv().await {
        match event {
            super::super::OutputEvent::Data(_, bytes) => output.extend(bytes),
            super::super::OutputEvent::Exit(info) => status = info.code,
        }
    }
    assert_eq!(
        status,
        Some(0),
        "engine did not receive the environment intact"
    );
    let argv = String::from_utf8(output).unwrap();
    assert!(argv.lines().any(|s| s == "CALTERNAL_AGENT_TOKEN"));
    assert!(
        !argv.contains("synthetic-private-token"),
        "credential entered host argv"
    );
    assert!(
        !argv.contains("second=value"),
        "multiline value entered host argv"
    );
    let invalid = sandbox
        .exec(ExecRequest {
            interactive: false,
            argv: vec!["true".into()],
            env: vec![("NAME=inline-secret".into(), String::new())],
            ..ExecRequest::default()
        })
        .await;
    assert!(
        invalid.is_err(),
        "inline environment values must not become names"
    );
}
