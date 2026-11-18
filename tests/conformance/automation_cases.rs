//! Creator authority, readable automation schedules, and portable skill bundles.
use super::*;

fn data(answer: Answer) -> Value {
    answer.ok();
    answer.body["data"].clone()
}

#[tokio::test]
async fn schedule_confirmations_keep_typed_words_and_do_not_move_with_the_account() {
    let f = Fixture::new().await;
    let space = &f.spaces[3];
    let words = "  Every Monday at 9:30  ";
    let preview = data(
        f.op(
            1,
            "automations.preview_schedule",
            json!({"space_id":space,"schedule_text":words}),
        )
        .await,
    );
    assert_eq!(preview["schedule_text"], words);
    assert_eq!(preview["schedule"], "Mon *-*-* 09:30:00");
    assert_eq!(preview["timezone"], "UTC");
    let created = data(
        f.op(
            1,
            "automations.create",
            json!({
                "space_id":space,"name":"Monday review","prompt":"Summarize the review.",
                "schedule_text":words,"schedule":preview["schedule"],"timezone":preview["timezone"],
            }),
        )
        .await,
    );
    let a = &created["automation"];
    let id = &a["id"];
    assert_eq!(a["mode"], "approve_for_me");
    assert_eq!(a["enabled"], true);
    assert_eq!(a["schedule_text"], words);
    let stolen = f
        .op(
            2,
            "automations.update",
            json!({"space_id":space,"id":id,
        "base_revision":a["revision"],"prompt":"Replace the creator's prompt."}),
        )
        .await;
    assert_eq!(stolen.status, 403);
    data(
        f.op(
            1,
            "account.set_time_zone",
            json!({"time_zone":"Asia/Kolkata"}),
        )
        .await,
    );
    let status = data(
        f.op(1, "automations.read", json!({"space_id":space,"id":id}))
            .await,
    );
    assert_eq!(status["automation"]["timezone"], "UTC");
    assert_eq!(status["automation"]["account_timezone"], "Asia/Kolkata");
    let stale = f
        .op(
            1,
            "automations.update",
            json!({"space_id":space,"id":id,
        "base_revision":a["revision"],"schedule_text":words,
        "schedule":preview["schedule"],"timezone":"UTC"}),
        )
        .await;
    assert_eq!(stale.status, 409);
}

#[tokio::test]
async fn skill_update_preserves_bundle_resources_and_removal_moves_them() {
    let f = Fixture::new().await;
    let space = &f.spaces[0];
    let start = "---\nname: review\ndescription: >-\n  Review documents.\nmetadata:\n  agents: Claude, Codex\n---\n# Review";
    let added = data(
        f.op(
            1,
            "skills.create",
            json!({"space_id":space,"name":"review","markdown":start}),
        )
        .await,
    );
    let bundle = f
        .dir
        .path()
        .join("userdata-ssd/spaces")
        .join(space)
        .join("AI/Skills/review");
    assert_eq!(
        std::fs::read_to_string(bundle.join("SKILL.md")).unwrap(),
        start
    );
    std::fs::write(bundle.join("reference.md"), "Reference contents").unwrap();
    let updated = data(
        f.op(
            1,
            "skills.update",
            json!({"space_id":space,"name":"review",
        "markdown":format!("{start}\n\nUpdated."),"base_revision":added["skill"]["revision"]}),
        )
        .await,
    );
    assert_eq!(updated["skill"]["agents"], json!(["Claude", "Codex"]));
    data(
        f.op(
            1,
            "skills.remove",
            json!({"space_id":space,"name":"review",
        "base_revision":updated["skill"]["revision"]}),
        )
        .await,
    );
    assert!(!bundle.exists());
    let trash = f
        .dir
        .path()
        .join("userdata-ssd/spaces")
        .join(space)
        .join(".trash")
        .join(format!("skill-{}", added["skill"]["id"].as_str().unwrap()));
    assert_eq!(
        std::fs::read_to_string(trash.join("reference.md")).unwrap(),
        "Reference contents"
    );
}

#[tokio::test]
async fn a_replayed_manual_run_has_one_thread_and_lost_access_stops_it() {
    use crate::requests_cases::{Machine, agent};

    let f = Fixture::new().await;
    let space = &f.spaces[3]; // The creator can edit, and the owner can revoke access.
    let code = f
        .req(
            "POST",
            "/api/v1/machines/codes",
            Some(&f.cookies[2]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    code.ok();
    let mut machine = Machine::enroll_with_code(&f, code.body["code"].as_str().unwrap()).await;

    let words = "daily at 7:15";
    let p = data(
        f.op(
            2,
            "automations.preview_schedule",
            json!({"space_id":space,"schedule_text":words}),
        )
        .await,
    );
    let created = data(f.op(2,"automations.create",json!({
        "space_id":space,"name":"Check the handoff","prompt":"Read the saved status.",
        "schedule_text":words,"schedule":p["schedule"],"timezone":p["timezone"],"machine_id":machine.id,
    })).await);
    let a = &created["automation"];
    data(
        f.op(
            2,
            "automations.pause",
            json!({"space_id":space,"id":a["id"],"base_revision":a["revision"]}),
        )
        .await,
    );
    let input = json!({"space_id":space,"id":a["id"]});
    let start = async {
        tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                let ready = data(f.op(2, "agent.readiness", json!({"space_id":space})).await);
                if ready["machines"][0]["available"] == true {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
        })
        .await
        .expect("the machine should advertise capacity while polling");
        data(
            f.op_id(2, "automations.run_now", input.clone(), "manual-replay")
                .await,
        )
    };
    let (run, _) = tokio::join!(start, machine.sync(&f, json!([])));
    let replay = data(
        f.op_id(2, "automations.run_now", input.clone(), "manual-replay")
            .await,
    );
    assert_eq!(run, replay);
    let thread = data(f.op(1, "threads.get", json!({"id":run["thread_id"]})).await);
    assert_eq!(thread["thread"]["metadata"]["automation_id"], a["id"]);
    assert_eq!(thread["thread"]["metadata"]["title"], "Check the handoff");
    assert_eq!(thread["thread"]["messages"][0]["role"], "automation");
    assert_eq!(
        data(f.op(1, "automations.next_run", input.clone()).await)["next_run_ms"],
        Value::Null
    );
    let token = machine.next_token(&f).await;
    data(
        f.op(
            1,
            "spaces.remove_member",
            json!({"space_id":space,"account_id":f.ids[2]}),
        )
        .await,
    );
    let stopped = tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            let last = data(f.op(1, "automations.last_run", input.clone()).await);
            if last["last"]["status"] == "stopped" {
                break last;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(stopped["last"]["thread_id"], run["thread_id"]);
    assert!(
        stopped["last"]["reason"]
            .as_str()
            .unwrap()
            .contains("no longer")
    );
    let (status, _) = agent(
        &f,
        &token,
        "after-revoke",
        "notes.list",
        json!({"space_id":space}),
    )
    .await;
    assert!(matches!(status, 401 | 403 | 404));
}
