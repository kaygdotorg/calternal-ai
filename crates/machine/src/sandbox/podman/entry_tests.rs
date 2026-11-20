use super::{Backend, PodmanBackend, Sandbox, podman};
use crate::protocol::Stream;
use crate::sandbox::OutputEvent;
use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_plugin_host::entry::EntryProgram;
use serde_json::json;
use std::time::Duration;
use std::{path::Path, sync::Arc};
use uuid::Uuid;

const SCRIPT: &[u8] = br#"#!/usr/local/bin/node
const fs = require('node:fs');
const wire = fs.readFileSync(0, 'utf8');
if (!wire.endsWith('\n') || wire.split('\n').length !== 2) process.exit(1);
const {call, input} = JSON.parse(wire);
if (Object.keys(JSON.parse(wire)).length !== 2 || call !== 'one') process.exit(2);
try {
  fs.writeFileSync(__filename, 'changed');
  process.exit(3);
} catch (error) {
  if (error.code !== 'EROFS' && error.code !== 'EACCES') throw error;
}
if (fs.existsSync('/work') || fs.existsSync('/run/calternal-provider-login')) process.exit(4);
process.stderr.write('fixture diagnostics\n');
process.stdout.write('{"seq":1,"kind":"output","data":');
setTimeout(() => {
  process.stdout.write(JSON.stringify(input) + '}\n{"seq":2,"kind":"terminal","state":"ok"}\n');
}, 30);
"#;

#[tokio::test]
async fn scripted_entry_runs_from_read_only_pin_with_no_network_or_workspace() {
    let Ok(reference) = std::env::var("CALTERNAL_TEST_ENTRY_IMAGE") else {
        eprintln!("SKIP real entry sandbox: CALTERNAL_TEST_ENTRY_IMAGE is unset");
        return;
    };
    let (source, image_digest) = reference.split_once('@').unwrap();
    let root = tempfile::tempdir().unwrap();
    let backend = PodmanBackend::new("podman", root.path(), "unused");
    backend.initialize().await.unwrap();
    let entry_digest = ring::digest::digest(&ring::digest::SHA256, SCRIPT)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let attempt = Uuid::new_v4();
    let invocation = json!({"call": "one", "input": {"text": "fixture"}});
    let program = |source: &str, image_digest: &str, entry_digest: &str| EntryProgram {
        image_source: source.into(),
        image_digest: image_digest.into(),
        entry_sha256: entry_digest.into(),
        entry_base64: STANDARD.encode(SCRIPT),
        invocation: invocation.clone(),
    };
    let wrong_digest = "0".repeat(64);
    let wrong_image = format!("sha256:{wrong_digest}");
    for bad in [
        program(source, image_digest, &wrong_digest),
        program(source, &wrong_image, &entry_digest),
        program(
            "registry.example.invalid/wrong/source",
            image_digest,
            &entry_digest,
        ),
    ] {
        assert!(
            backend
                .launch_entry(attempt, &bad, Arc::new(|| true))
                .await
                .is_err()
        );
    }
    let valid = program(source, image_digest, &entry_digest);
    let (sandbox, mut exec, log) = backend
        .launch_entry(attempt, &valid, Arc::new(|| true))
        .await
        .unwrap();
    let network = podman(
        Path::new("podman"),
        &[
            "container",
            "inspect",
            "--format",
            "{{.HostConfig.NetworkMode}}",
            &sandbox.container,
        ],
    )
    .await;
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        let mut stdout = Vec::new();
        loop {
            match exec.events.recv().await.unwrap() {
                OutputEvent::Data(Stream::Stdout, bytes) => stdout.extend(bytes),
                OutputEvent::Data(Stream::Stderr, _) => {
                    panic!("entry diagnostics reached consumer")
                }
                OutputEvent::Exit(info) => return (stdout, info),
            }
        }
    })
    .await;
    sandbox.stop().await.unwrap();
    tokio::fs::remove_file(log.path).await.unwrap();
    assert_eq!(network.unwrap(), "none");
    let (stdout, exit) = result.unwrap();
    assert_eq!(exit.code, Some(0), "{exit:?}");
    assert_eq!(stdout, b"{\"seq\":1,\"kind\":\"output\",\"data\":{\"text\":\"fixture\"}}\n{\"seq\":2,\"kind\":\"terminal\",\"state\":\"ok\"}\n");
    assert!(!sandbox.dir.exists());
}
