//! Independent review expectation (docs/reviews/provider-independent-review-2026-11-17.md):
//! an exec's environment values, which carry the per-thread credential, must not
//! appear in the host process table. Any local user can read another process's
//! argv through /proc/<pid>/cmdline, and `podman exec` stays alive for the whole
//! turn. A recording stand-in replaces `podman`; nothing is pulled or started.

use calternal_machine::protocol::{Limits, SandboxSpec};
use calternal_machine::sandbox::podman::PodmanBackend;
use calternal_machine::sandbox::{Backend, ExecRequest, OutputEvent};
use std::os::unix::fs::PermissionsExt;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread")]
async fn podman_exec_keeps_environment_values_out_of_host_argv() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("argv.log");
    let fake = dir.path().join("podman");
    // Records every argument vector; reports an already running container so
    // start() goes straight to exec.
    std::fs::write(
        &fake,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\ncase \"$1 $2\" in\n  'container inspect') echo running ;;\nesac\nexit 0\n",
            log.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    let backend = PodmanBackend::new(&fake, dir.path().join("root"), "unused:image");
    let sandbox = backend
        .start(&SandboxSpec {
            space_id: Some(calternal_types::SpaceId::new()),
            provider_login: false,
            shared_provider_sponsor: None,
            shared_tools: None,
            code_directory: None,
            worker: None,
            account_id: Uuid::new_v4(),
            thread_id: Uuid::new_v4(),
            image: None,
            limits: Limits::default(),
        })
        .await
        .unwrap();
    let secret = "ct1_00000000-0000-0000-0000-000000000000_reviewsentinel";
    let exec = sandbox
        .exec(ExecRequest {
            interactive: false,
            stdin: String::new(),
            argv: vec!["calternal-provider-claude-code".into()],
            env: vec![("CALTERNAL_AGENT_TOKEN".into(), secret.into())],
        })
        .await
        .unwrap();
    let mut events = exec.events;
    while let Some(event) = events.recv().await {
        if matches!(event, OutputEvent::Exit(_)) {
            break;
        }
    }
    let recorded = std::fs::read_to_string(&log).unwrap();
    assert!(
        recorded.lines().any(|l| l.starts_with("exec ")),
        "precondition: exec reached podman: {recorded}"
    );
    assert!(
        !recorded.contains(secret),
        "the thread credential was passed on the podman command line, visible in the \
         host process table for the whole turn:\n{recorded}"
    );
}
