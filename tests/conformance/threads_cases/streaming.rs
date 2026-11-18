//! Real passkey sessions, a fixture provider and the ordinary space and job feeds.
use super::*;
use crate::{jobs_cases::Sse, live_cases::Live};

struct DirectMachine(Child);
impl Drop for DirectMachine {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .args(["-TERM", &self.0.id().to_string()])
            .status();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.0.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn direct_fixture(f: &Fixture) -> DirectMachine {
    let code = f
        .req(
            "POST",
            "/api/v1/machines/codes",
            Some(&f.cookies[1]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    code.ok();
    let binary = PathBuf::from(std::env::var_os("CALTERNAL_SERVER_BIN").unwrap());
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let adapter = repo.join("plugins/calternal-claude-code");
    let dir = f.dir.path().join("streaming-machine");
    let bin = f.dir.path().join("fixture-bin");
    std::fs::create_dir(&bin).unwrap();
    std::os::unix::fs::symlink(adapter.join("test/fake-claude.mjs"), bin.join("claude")).unwrap();
    let search_path = std::env::join_paths(
        [
            bin,
            adapter.join("bin"),
            binary.parent().unwrap().to_owned(),
        ]
        .into_iter()
        .chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )),
    )
    .unwrap();
    let enroll = Command::new(&binary)
        .args(["machine", "enroll", "--server", &f.base, "--direct"])
        .arg("--dir")
        .arg(&dir)
        .env("CALTERNAL_ENROLL_CODE", code.body["code"].as_str().unwrap())
        .output()
        .unwrap();
    assert!(
        enroll.status.success(),
        "{}",
        String::from_utf8_lossy(&enroll.stderr)
    );
    DirectMachine(
        Command::new(&binary)
            .args(["machine", "run", "--backend", "direct", "--dir"])
            .arg(dir)
            .env("PATH", search_path)
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

fn body(thread: &Value) -> &str {
    thread["markdown"]
        .as_str()
        .unwrap()
        .split_once("\n---\n")
        .unwrap()
        .1
}

#[tokio::test]
async fn checkpoint_replay_reads_only_its_own_append_and_stop_keeps_the_step() {
    let f = Fixture::new().await;
    let _machine = direct_fixture(&f).await;
    let space = &f.spaces[2];
    let mut journal = Live::open(&f, 2, json!([{"space_id":space}]))
        .await
        .unwrap();
    let bookmark = journal.next().await.unwrap()["bookmark"].clone();
    let created = f
        .op(
            1,
            "threads.create",
            json!({"space_id":space,"title":"Checkpoint range"}),
        )
        .await;
    created.ok();
    let thread = &created.body["data"]["thread"]["id"];
    let sent = f
        .op(
            1,
            "threads.send",
            json!({"id":thread,"message":"scripted-tool-steps hang"}),
        )
        .await;
    sent.ok();
    let data = &sent.body["data"];
    let job = data["job"]["id"].as_str().unwrap();
    let mut revision = data["thread"]["revision"].clone();
    let mut cached_body = body(&data["thread"]).to_owned();

    let mut replay = Live::open(&f, 2, json!([{"space_id":space,"bookmark":bookmark}]))
        .await
        .unwrap();
    let mut saved_step_input = Value::Null;
    let mut first = None;
    for _ in 0..16 {
        let frame = replay.next().await.expect("checkpoint journal entry");
        if frame["entry"]["kind"] != "threads.checkpoint" || frame["entry"]["item"] != *thread {
            continue;
        }
        let since = &frame["entry"]["data"];
        assert_eq!(since["append"]["base_revision"], revision);
        let input = json!({"id":thread,"since":since});
        let chunk = f.op(2, "threads.get", input.clone()).await;
        chunk.ok();
        let delta = &chunk.body["data"]["delta"];
        let markdown = delta["markdown"].as_str().unwrap();
        assert_eq!(
            markdown.len(),
            since["append"]["length"].as_u64().unwrap() as usize
        );
        assert_eq!(delta["base_revision"], revision);
        revision = delta["revision"].clone();
        cached_body.insert_str(delta["offset"].as_u64().unwrap() as usize, markdown);
        if first.is_none() {
            first = Some((input.clone(), delta.clone()));
        }
        if cached_body.contains("(started)") && cached_body.contains("Tool Read") {
            saved_step_input = input;
            break;
        }
    }
    assert!(
        !saved_step_input.is_null(),
        "fixture start checkpoint was not replayed"
    );
    assert!(cached_body.contains("Hello 🌏"));
    assert!(cached_body.contains("Run ls (finished)"));
    let (old_input, old_delta) = first.unwrap();
    let historical = f.op(2, "threads.get", old_input).await;
    historical.ok();
    assert_eq!(historical.body["data"]["delta"], old_delta);

    let mut live_job = Sse::open(&f, &f.cookies[2], job, None).await.unwrap();
    let mut saw_step = false;
    for _ in 0..8 {
        let (kind, _, chunk) = live_job.next().await.expect("job chunk");
        if kind == "chunk" && chunk["data"]["event"]["type"] == "tool_started" {
            assert_eq!(chunk["data"]["event"]["name"], "Bash");
            assert_eq!(chunk["data"]["event"]["summary"], "Run ls");
            saw_step = true;
            break;
        }
    }
    assert!(saw_step, "a reload did not replay the running step");

    let missing = f
        .op(
            3,
            "threads.get",
            json!({
                "id":uuid::Uuid::now_v7(),"since":saved_step_input["since"]
            }),
        )
        .await;
    let foreign = f.op(3, "threads.get", saved_step_input.clone()).await;
    assert_eq!(foreign.status, 404);
    assert_eq!(foreign.signature(), missing.signature());

    let other = f
        .op(
            1,
            "threads.create",
            json!({"space_id":space,"title":"Other thread"}),
        )
        .await;
    other.ok();
    let wrong_thread = f
        .op(
            2,
            "threads.get",
            json!({
                "id":other.body["data"]["thread"]["id"], "since":saved_step_input["since"]
            }),
        )
        .await;
    assert_eq!(wrong_thread.status, 409);
    let mut wrong_offset = saved_step_input.clone();
    wrong_offset["since"]["append"]["offset"] = json!(0);
    assert_eq!(f.op(2, "threads.get", wrong_offset).await.status, 409);

    f.op(1, "threads.stop", json!({"id":thread})).await.ok();
    let saved = f.op(2, "threads.get", json!({"id":thread})).await;
    saved.ok();
    assert_eq!(body(&saved.body["data"]["thread"]), cached_body);
    assert!(!cached_body.contains("(failed)"));
    f.op(1, "threads.delete", json!({"id":thread})).await.ok();
    assert_eq!(f.op(2, "threads.get", saved_step_input).await.status, 409);
}
