//! Revoke activation at the real worker's completion boundary.
use super::*;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn disabling_after_worker_completion_discards_the_http_result() {
    let mut fixture = Fixture::new().await;
    let space = fixture.spaces[0].clone();
    let note = fixture.note(1, &space).await;
    let target = json!({"space_id":space,"plugin_id":"calternal-notes-insights"});
    fixture.op(1, "plugins.enable", target.clone()).await.ok();
    fixture.stop();
    gate_completion(&fixture);
    fixture.start().await;

    let call = fixture.op(1, "notes-insights.summarize", json!({"id":note["id"]}));
    tokio::pin!(call);
    let marker = fixture.dir.path().join("completion-held");
    tokio::select! {
        response = &mut call => panic!("completed before gate: {}", response.body),
        () = async {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !marker.exists() {
                assert!(Instant::now() < deadline, "worker did not complete");
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        } => {}
    }
    fixture.op(1, "plugins.disable", target).await.ok();
    std::fs::write(fixture.dir.path().join("completion-release"), "").unwrap();
    let response = call.await;
    assert_eq!(response.status, 409, "{}", response.body);
    assert_eq!(
        response.body["error"]["details"]["reason"],
        "plugin_generation_changed"
    );
    assert!(response.body.get("data").is_none());
}

fn gate_completion(fixture: &Fixture) {
    let tool = |name: &str| {
        std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|p| p.join(name))
            .find(|p| p.is_file())
            .unwrap()
    };
    let wrapper = fixture.dir.path().join("node-gate");
    let script = format!(
        r#"#!{}
import json, os, pathlib, subprocess, sys, time
node = {:?}
if sys.argv[1:] == ["--version"]:
    os.execv(node, [node, "--version"])
child = subprocess.Popen([node, *sys.argv[1:]], stdout=subprocess.PIPE)
for line in child.stdout:
    if json.loads(line)["message"]["type"] == "complete":
        pathlib.Path({:?}).touch()
        until = time.monotonic() + 2
        while time.monotonic() < until and not pathlib.Path({:?}).exists():
            time.sleep(0.005)
    sys.stdout.buffer.write(line)
    sys.stdout.buffer.flush()
sys.exit(child.wait())
"#,
        tool("python3").display(),
        tool("node").to_str().unwrap(),
        fixture.dir.path().join("completion-held").to_str().unwrap(),
        fixture
            .dir
            .path()
            .join("completion-release")
            .to_str()
            .unwrap()
    );
    std::fs::write(&wrapper, script).unwrap();
    std::fs::set_permissions(wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
}
