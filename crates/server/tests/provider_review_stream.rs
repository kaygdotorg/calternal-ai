//! Independent review expectation (docs/reviews/provider-independent-review-2026-11-17.md):
//! the thread credential is in the adapter's environment, so the model or one of
//! its tools can print it. Output fans out to every member who can read the space
//! (`agent.events` needs only SpaceRead) and its text tail is checkpointed for
//! seven days, so the server must not publish or persist the credential value it
//! issued. Real server, machine role (direct backend) and Claude Code adapter; the
//! official CLI is replaced by a fixture that echoes its environment.
mod machines_common;
use machines_common::*;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
use uuid::Uuid;

/// Documented stream-json output whose text repeats the inherited credential.
const ECHO_CLI: &str = r#"#!/usr/bin/env node
const args=process.argv.slice(2);
if(args.join(' ')==='auth status'){console.log(JSON.stringify({authMethod:'claude.ai'}));process.exit(0);}
for await(const _ of process.stdin){}
const emit=m=>process.stdout.write(JSON.stringify(m)+'\n');
emit({type:'system',subtype:'init',session_id:'22222222-2222-4222-8222-222222222222'});
const text='env: CALTERNAL_AGENT_TOKEN='+(process.env.CALTERNAL_AGENT_TOKEN||'absent')+'\n';
emit({type:'stream_event',event:{type:'content_block_delta',delta:{type:'text_delta',text}}});
emit({type:'result',subtype:'success',is_error:false,usage:{input_tokens:1,output_tokens:1}});
"#;

fn machine_with_adapter(dir: &Path, bin: &Path) -> Proc {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = format!(
        "{}:{}:{}",
        bin.display(),
        root.join("plugins/calternal-claude-code/bin").display(),
        std::env::var("PATH").unwrap()
    );
    Proc(
        Command::new(BIN)
            .args(["machine", "run", "--backend", "direct", "--dir"])
            .arg(dir)
            .env("PATH", path)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}

async fn op(server: &Server, operation: &str, input: Value) -> Value {
    let (status, v) = server
        .call(
            &server.owner,
            "/api/v1/operations",
            Some(json!({"request_id":Uuid::new_v4(),"operation":operation,"input":input})),
        )
        .await;
    assert_eq!(status, 200, "{operation}: {v}");
    v["data"].clone()
}

/// Every file under `root` whose bytes contain `needle`.
fn files_containing(root: &Path, needle: &[u8]) -> Vec<PathBuf> {
    let mut found = vec![];
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(path);
            } else if std::fs::read(&path)
                .is_ok_and(|b| b.windows(needle.len()).any(|w| w == needle))
            {
                found.push(path);
            }
        }
    }
    found
}

#[tokio::test(flavor = "multi_thread")]
async fn issued_thread_credential_is_not_published_or_checkpointed() {
    let data = TempDir::new("review-stream-data");
    let server = Server::start_with_plugins(
        &data.0,
        free_port(),
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins"),
    );
    let bin = TempDir::new("review-stream-bin");
    std::fs::write(bin.join("claude"), ECHO_CLI).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(bin.join("claude"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mdir = TempDir::new("review-stream-machine");
    let code = server.code(&server.owner).await;
    assert_eq!(enrol(&server.url(), &code, &mdir.0), 0);
    let _machine = machine_with_adapter(&mdir.0, &bin.0);
    wait_online(&server, &server.owner, 1).await;
    let space = server.owned_space(&server.owner).await;
    let start = op(
        &server,
        "agent.run",
        json!({"space_id":space,"thread_id":Uuid::new_v4(),"prompt":"print env"}),
    )
    .await;
    let id = start["job"]["id"].as_str().unwrap().to_owned();
    let end = std::time::Instant::now() + Duration::from_secs(30);
    let (mut next, mut epoch, mut events) = (0u64, String::new(), Vec::<Value>::new());
    let job = loop {
        let v = op(
            &server,
            "agent.events",
            json!({"job_id":id,"after":next,"epoch":epoch}),
        )
        .await;
        next = v["next"].as_u64().unwrap();
        epoch = v["epoch"].as_str().unwrap().into();
        events.extend(v["events"].as_array().unwrap().iter().cloned());
        if v["job"]["state"]
            .as_str()
            .is_some_and(|s| s != "running" && s != "admitted")
        {
            break v["job"].clone();
        }
        assert!(std::time::Instant::now() < end, "{v}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    let text = serde_json::to_string(&events).unwrap();
    assert!(
        text.contains("CALTERNAL_AGENT_TOKEN="),
        "precondition: the fixture's text reached the stream: {job}; {text}"
    );
    assert!(
        !text.contains("ct1_"),
        "the issued thread credential was published to space readers: {text}"
    );
    assert!(
        !job.to_string().contains("ct1_"),
        "the issued thread credential is in the job checkpoint: {job}"
    );
    let stored = files_containing(&data.0, b"ct1_");
    assert!(
        stored.is_empty(),
        "the issued thread credential was persisted on the server: {stored:?}"
    );
}
