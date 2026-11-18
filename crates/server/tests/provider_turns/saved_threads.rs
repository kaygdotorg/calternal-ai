//! Saved-thread assertions through the real machine/provider fixture.
use super::*;

/// Verify output placement, identity and deletion of retained permission state.
pub(super) async fn exercise(server: &Server, data: &TempDir, space: &str) {
    // Saved threads use this same provider/session path and checkpoint to Markdown.
    let thread = op(
        server,
        "threads.create",
        json!({"space_id":space,"title":"Saved provider output"}),
    )
    .await;
    let thread_id = thread["thread"]["id"].as_str().unwrap();
    let turn = op(
        server,
        "threads.send",
        json!({"id":thread_id,"message":"hello saved thread checkpoint-story"}),
    )
    .await;
    let (finished, _) = until(server, turn["job"]["id"].as_str().unwrap(), |j| {
        j["state"] == "done"
    })
    .await;
    assert_eq!(finished["state"], "done");
    let saved = op(server, "threads.get", json!({"id":thread_id})).await;
    let markdown = saved["thread"]["markdown"].as_str().unwrap();
    assert!(markdown.contains("hello saved thread"));
    assert_eq!(markdown.matches("Hello 🌏").count(), 1, "{markdown}");
    assert!(!markdown.contains("PRIVATE_LOGIN_SENTINEL"));
    let markers: Vec<_> = markdown
        .lines()
        .filter(|line| line.starts_with("<!-- id:"))
        .collect();
    assert_eq!(markers.len(), 2);
    for message in saved["thread"]["messages"].as_array().unwrap() {
        time::OffsetDateTime::parse(
            message["at"].as_str().unwrap(),
            &time::format_description::well_known::Rfc3339,
        )
        .expect("message timestamp must be RFC 3339");
    }
    assert_eq!(saved["thread"]["metadata"]["message_count"], 2);
    assert_eq!(saved["thread"]["turn_state"], "idle");
    assert!(saved.get("job").is_none());
    assert_eq!(
        markdown.matches("role:agent").count(),
        1,
        "checkpoints split one response into multiple messages"
    );
    for n in 0..6 {
        assert_eq!(
            markdown
                .matches(&format!("segment-{n}:{}\n", n.to_string().repeat(8192)))
                .count(),
            1
        );
    }

    let text = std::fs::read_to_string(data.join(&format!(
        "userdata-ssd/spaces/{space}/AI/Chats/Saved provider output.md"
    )))
    .unwrap();
    assert!(text.contains(&format!("calternal-id: {thread_id}")));
    assert!(text.contains("Hello 🌏"));
    let jobs =
        rusqlite::Connection::open(data.join(&format!("appdata/spaces/{space}/jobs.db"))).unwrap();
    assert_eq!(
        jobs.execute(
            "DELETE FROM jobs WHERE id=?1",
            [finished["id"].as_str().unwrap()]
        )
        .unwrap(),
        1
    );
    drop(jobs);
    // A send applies wider and narrower modes without a fresh passkey ceremony.
    let auth = rusqlite::Connection::open(data.join("appdata/system/auth.db")).unwrap();
    for mode in ["full_access", "least_access", "approve_for_me"] {
        let turn = op(
            server,
            "threads.send",
            json!({"id":thread_id,"message":"require-resume and apply the chosen mode","mode":mode}),
        )
        .await;
        let (job, _) = until(server, turn["job"]["id"].as_str().unwrap(), terminal).await;
        assert_eq!(job["state"], "done", "follow-up must resume: {job}");
        let scope: String = auth
            .query_row(
                "SELECT scope FROM agent_access WHERE space=?1 AND thread=?2",
                rusqlite::params![space, thread_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(serde_json::from_str::<Value>(&scope).unwrap()["mode"], mode);
        let saved = op(server, "threads.get", json!({"id":thread_id})).await;
        assert_eq!(saved["thread"]["metadata"]["mode"], mode);
    }
    // A content commit failure must not remove the saved permission mode.
    let count = || {
        auth.query_row(
            "SELECT count(*) FROM agent_access WHERE space=?1 AND thread=?2",
            rusqlite::params![space, thread_id],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
    };
    assert_eq!(count(), 1);
    let saved_scope: String = auth
        .query_row(
            "SELECT scope FROM agent_access WHERE space=?1 AND thread=?2",
            rusqlite::params![space, thread_id],
            |r| r.get(0),
        )
        .unwrap();
    let content =
        rusqlite::Connection::open(data.join(&format!("appdata/spaces/{space}/content.db")))
            .unwrap();
    content.execute_batch("CREATE TRIGGER refuse_thread_delete BEFORE UPDATE ON items WHEN json_extract(NEW.metadata,'$.deleted')=1 BEGIN SELECT RAISE(ABORT,'injected delete commit failure'); END;").unwrap();
    let (status,_)=server.call(&server.owner,"/api/v1/operations",Some(json!({"request_id":Uuid::new_v4(),"operation":"threads.delete","input":{"id":thread_id}}))).await;
    assert_eq!(status, 500);
    assert_eq!(
        count(),
        1,
        "failed deletion forgot the retained permission mode"
    );
    let unchanged: String = auth
        .query_row(
            "SELECT scope FROM agent_access WHERE space=?1 AND thread=?2",
            rusqlite::params![space, thread_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(unchanged, saved_scope);
    let preserved = op(server, "threads.get", json!({"id":thread_id})).await;
    assert_eq!(preserved["thread"]["metadata"]["deleted"], false);
    content
        .execute_batch("DROP TRIGGER refuse_thread_delete")
        .unwrap();
    op(server, "threads.delete", json!({"id":thread_id})).await;
    let retained: i64 = auth
        .query_row(
            "SELECT count(*) FROM agent_access WHERE space=?1 AND thread=?2",
            rusqlite::params![space, thread_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        retained, 0,
        "thread deletion retained its saved permission mode"
    );
}

fn terminal(job: &Value) -> bool {
    matches!(
        job["state"].as_str(),
        Some("done" | "failed" | "outcome_unknown")
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn saved_sessions_do_not_follow_a_thread_to_another_machine_or_account() {
    require_adapter_build();
    let data = TempDir::new("thread-session-data");
    let plugins = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let server = Server::start_with_plugins(&data.0, free_port(), &plugins);
    let bin = plugins.join("calternal-claude-code/test");
    let fixture = TempDir::new("thread-session-fixture");
    std::os::unix::fs::symlink(bin.join("fake-claude.mjs"), fixture.join("claude")).unwrap();
    let first = TempDir::new("thread-session-first");
    let code = server.code(&server.owner).await;
    assert_eq!(enrol(&server.url(), &code, &first.0), 0);
    let _first = machine_with_adapter(&first.0, &fixture.0);
    wait_online(&server, &server.owner, 1).await;

    let (guest, spaces) = server.principal(&server.guest).await;
    let (space, _) = spaces
        .into_iter()
        .find(|(_, role)| role == "reader")
        .unwrap();
    op(
        &server,
        "spaces.set_role",
        json!({"space_id":space,"account_id":guest,"role":"editor"}),
    )
    .await;
    let thread = op(
        &server,
        "threads.create",
        json!({"space_id":space,"title":"Session boundaries"}),
    )
    .await;
    let id = thread["thread"]["id"].as_str().unwrap();
    let turn = op(
        &server,
        "threads.send",
        json!({"id":id,"message":"require-fresh first turn"}),
    )
    .await;
    let (job, _) = until(&server, turn["job"]["id"].as_str().unwrap(), terminal).await;
    assert_eq!(job["state"], "done");
    let first_machine = job["checkpoint"]["machine_id"].as_str().unwrap();
    let (status, _) = server
        .call(
            &server.owner,
            &format!("/api/v1/machines/{first_machine}/drain"),
            Some(json!({"draining":true})),
        )
        .await;
    assert_eq!(status, 200);

    let next = TempDir::new("thread-session-next");
    let code = server.code(&server.owner).await;
    assert_eq!(enrol(&server.url(), &code, &next.0), 0);
    let _next = machine_with_adapter(&next.0, &fixture.0);
    wait_online(&server, &server.owner, 1).await;
    let turn = op(
        &server,
        "threads.send",
        json!({"id":id,"message":"require-fresh on the second machine"}),
    )
    .await;
    let (job, events) = until(&server, turn["job"]["id"].as_str().unwrap(), terminal).await;
    assert_eq!(job["state"], "done");
    assert_ne!(job["checkpoint"]["machine_id"], first_machine);
    assert!(events.iter().any(|e| {
        e["type"] == "notice"
            && e["message"]
                .as_str()
                .is_some_and(|message| message.contains("machine changed"))
    }));

    let guest_machine = TempDir::new("thread-session-guest");
    let code = server.code(&server.guest).await;
    assert_eq!(enrol(&server.url(), &code, &guest_machine.0), 0);
    let _guest_machine = machine_with_adapter(&guest_machine.0, &fixture.0);
    wait_online(&server, &server.guest, 1).await;
    let turn = op_as(
        &server,
        &server.guest,
        "threads.send",
        json!({"id":id,"message":"require-fresh different account"}),
    )
    .await;
    let (guest_job, events) = until(&server, turn["job"]["id"].as_str().unwrap(), terminal).await;
    assert_eq!(guest_job["state"], "done");
    assert_ne!(
        guest_job["checkpoint"]["machine_id"],
        job["checkpoint"]["machine_id"]
    );
    assert!(!events.iter().any(|e| e["type"] == "notice"));
}
