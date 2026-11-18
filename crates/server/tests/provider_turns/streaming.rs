//! Scripted tool steps and the saved-thread size ceiling across the real provider path.
use super::*;

pub(super) async fn exercise(server: &Server, data: &TempDir, space: &str) {
    let thread = op(
        server,
        "threads.create",
        json!({"space_id":space,"title":"Scripted tool output"}),
    )
    .await;
    let id = &thread["thread"]["id"];
    let turn = op(
        server,
        "threads.send",
        json!({"id":id,"message":"scripted-tool-steps long-output"}),
    )
    .await;
    let (job, live) = until(server, turn["job"]["id"].as_str().unwrap(), |j| {
        matches!(
            j["state"].as_str(),
            Some("done" | "failed" | "outcome_unknown")
        )
    })
    .await;
    assert_eq!(job["state"], "done", "{job}");
    let steps: Vec<_> = live
        .iter()
        .map(|frame| &frame["event"])
        .filter(|event| {
            event["type"]
                .as_str()
                .is_some_and(|t| t.starts_with("tool_"))
        })
        .collect();
    assert_eq!(steps.len(), 6, "{steps:?}");
    assert_eq!(steps[0]["name"], "Bash");
    assert_eq!(steps[0]["summary"], "Run ls");
    assert_eq!(steps[1]["type"], "tool_output");
    assert!(steps[1]["bytes"].as_u64().unwrap() >= 96 * 1024);
    assert_eq!(steps[2]["detail"], "Run ls completed");
    assert_eq!(steps[5]["failed"], true);
    assert_eq!(steps[5]["summary"], "Read ./missing/[redacted]");
    let serialized = serde_json::to_string(&steps).unwrap();
    assert!(!serialized.contains("ct1_"));
    assert!(!serialized.contains("PRIVATE_LOGIN_SENTINEL"));

    let saved = op(server, "threads.get", json!({"id":id})).await;
    let markdown = saved["thread"]["markdown"].as_str().unwrap();
    for status in ["started", "finished"] {
        assert!(markdown.contains(&format!("Tool Bash · Run ls ({status})")));
    }
    assert!(markdown.contains("bytes of tool output"));
    assert!(markdown.contains("Read ./missing/\\[redacted\\] (failed)"));
    assert!(!markdown.contains("ct1_"));
    assert!(!markdown.contains("PRIVATE_LOGIN_SENTINEL"));

    let path = data.join(&format!(
        "userdata-ssd/spaces/{space}/AI/Chats/Scripted tool output.md"
    ));
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        if text.contains("Read ./missing/\\[redacted\\] (failed)") {
            assert!(!text.contains("ct1_"));
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "saved tool step was not placed"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let full = op(
        server,
        "threads.create",
        json!({"space_id":space,"title":"Checkpoint ceiling"}),
    )
    .await;
    let full_id = &full["thread"]["id"];
    let turn = op(
        server,
        "threads.send",
        json!({"id":full_id,"message":"thread-size-limit"}),
    )
    .await;
    let (failed, live) = until_for(
        server,
        turn["job"]["id"].as_str().unwrap(),
        Duration::from_secs(60),
        |j| {
            matches!(
                j["state"].as_str(),
                Some("done" | "failed" | "outcome_unknown")
            )
        },
    )
    .await;
    assert_eq!(failed["state"], "failed");
    assert_eq!(failed["checkpoint"]["error"]["code"], "thread_full");
    assert!(
        live.iter()
            .any(|frame| frame["event"]["code"] == "thread_full")
    );
    assert!(!live.iter().any(|frame| frame["event"]["type"] == "done"));
    let saved = op(server, "threads.get", json!({"id":full_id})).await;
    let reason = &saved["thread"]["turn_reason"];
    assert_eq!(reason["code"], "thread_full");
    assert!(reason["message"].as_str().unwrap().contains("not saved"));
    let markdown = saved["thread"]["markdown"].as_str().unwrap();
    assert!(markdown.len() <= calternal_content::MAX_THREAD_BYTES);
    assert!(markdown.len() > calternal_content::MAX_THREAD_BYTES - 32 * 1024);
    assert!(markdown.contains("overflow-segment-0:"));
    assert!(!markdown.contains("overflow-segment-256:"));
    assert!(markdown.ends_with('\n'));
    assert_eq!(saved["thread"]["messages"].as_array().unwrap().len(), 2);
}
