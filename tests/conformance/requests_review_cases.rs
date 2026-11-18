//! Independent review expectations for person requests across turn and space
//! lifecycles (docs/reviews/requests-independent-review-2026-11-18.md). Each
//! test states one expectation from docs/requests.md or docs/agent-turns.md; a
//! failure names a gap. Uses the request lane's machine-protocol fixture.
use super::requests_cases::{Machine, agent};
use super::*;

pub(super) async fn start_turn(f: &Fixture, space: &str, thread: &Value, prompt: &str) -> String {
    let start = f
        .op(
            1,
            "agent.run",
            json!({"space_id":space,"thread_id":thread,"prompt":prompt,"mode":"least_access"}),
        )
        .await;
    start.ok();
    start.body["data"]["job"]["id"].as_str().unwrap().to_owned()
}

fn question_form() -> Value {
    json!({"form":{"genui_version":2,"source":"$release=\"\"\nForm([Input($release,\"Which release?\")], \"Release selection\")"}})
}

pub(super) async fn new_thread(f: &Fixture, space: &str) -> Value {
    let created = f
        .op(
            1,
            "threads.create",
            json!({"space_id":space,"title":"Review fixture"}),
        )
        .await;
    created.ok();
    created.body["data"]["thread"]["id"].clone()
}

#[tokio::test]
async fn thread_token_supplies_access_request_job_and_refuses_another_job() {
    let f = Fixture::new().await;
    let mut machine = Machine::enroll(&f).await;
    let space = &f.spaces[3];
    let thread = new_thread(&f, space).await;
    let job = start_turn(&f, space, &thread, "Request from the thread credential").await;
    let token = machine.next_token(&f).await;
    let wrong = json!({"job_id":uuid::Uuid::now_v7(),"app":"notes","space_id":space});
    let (status, error) = agent(&f, &token, "wrong-job", "access.request", wrong).await;
    assert_eq!(status, 404, "{error}");
    let (status, answer) = agent(
        &f,
        &token,
        "implicit-job",
        "access.request",
        json!({"app":"notes"}),
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    let id = &answer["data"]["approval_id"];
    let inbox = f.op(1, "requests.list", json!({})).await;
    inbox.ok();
    assert!(
        inbox.body["data"]["requests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == *id && r["thread_id"] == thread)
    );
    let turn = f.op(1, "agent.events", json!({"job_id":job})).await;
    turn.ok();
    assert_eq!(
        turn.body["data"]["job"]["checkpoint"]["person_request"],
        *id
    );
}

/// A cancelled approval cannot change any successor's credential.
#[tokio::test]
async fn approving_a_stopped_turns_request_does_not_strand_the_current_turn() {
    let f = Fixture::new().await;
    let mut machine = Machine::enroll(&f).await;
    let space = f.spaces[3].clone();
    let thread = new_thread(&f, &space).await;
    let first = start_turn(&f, &space, &thread, "first").await;
    let token = machine.next_token(&f).await;
    let (status, body) = agent(
        &f,
        &token,
        "app-request",
        "access.request",
        json!({"job_id":first,"app":"notes"}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let approval = body["data"]["approval_id"].clone();
    assert!(approval.is_string(), "{body}");
    f.op(1, "agent.stop", json!({"job_id":first})).await.ok();

    let second = start_turn(&f, &space, &thread, "second").await;
    let token = machine.next_token(&f).await;
    let (status, body) = agent(
        &f,
        &token,
        "events-before",
        "agent.events",
        json!({"job_id":second}),
    )
    .await;
    assert_eq!(
        status, 200,
        "precondition: current turn credential works: {body}"
    );

    let stale = f
        .op(
            1,
            "requests.approve",
            json!({"id":approval,"lifetime":"this_thread"}),
        )
        .await;
    assert_eq!(stale.status, 409, "{}", stale.body);
    let inbox = f.op(1, "requests.list", json!({})).await;
    inbox.ok();
    assert!(
        !inbox.body["data"]["requests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == approval)
    );
    let (status, _) = agent(
        &f,
        &token,
        "events-after",
        "agent.events",
        json!({"job_id":second}),
    )
    .await;
    let state = f.op(1, "agent.events", json!({"job_id":second})).await;
    state.ok();
    let state = state.body["data"]["job"]["state"].clone();
    assert_eq!(
        status, 200,
        "a cancelled request changed the successor's credential"
    );
    assert_eq!(state, "running");
}

/// docs/agent-turns.md: "The current mode remains until superseded or the
/// account or space is deleted". docs/requests.md bounds requests at 256 per
/// account, and the inbox hides rows from spaces the person cannot read, so a
/// row left behind can neither be seen, answered nor denied. Expectation:
/// deleting a space erases its protected request, grant and thread-authority
/// rows. Holds today through the `agent_access.space` foreign-key cascade; kept
/// as the passing counterpart of the membership case below.
#[tokio::test]
async fn deleting_a_space_erases_its_requests_and_thread_authority() {
    let mut f = Fixture::new().await;
    let mut machine = Machine::enroll(&f).await;
    let space = f.spaces[0].clone();
    let thread = new_thread(&f, &space).await;
    let job = start_turn(&f, &space, &thread, "ask").await;
    let token = machine.next_token(&f).await;
    let form = question_form();
    let (status, body) = agent(&f, &token, "question", "requests.ask", form).await;
    assert_eq!(status, 200, "{body}");
    f.op(1, "agent.stop", json!({"job_id":job})).await.ok();
    let deleted = f.op(1, "spaces.delete", json!({"space_id":space})).await;
    assert_eq!(deleted.status, 200, "{}", deleted.body);

    f.stop();
    let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
    let count = |table: &str| -> i64 {
        c.query_row(
            &format!("SELECT count(*) FROM {table} WHERE space=?1"),
            [&space],
            |r| r.get(0),
        )
        .unwrap()
    };
    let (requests, access) = (count("agent_approvals"), count("agent_access"));
    assert_eq!(
        (requests, access),
        (0, 0),
        "a deleted space keeps {requests} request rows and {access} thread-authority rows"
    );
}

/// Losing membership erases the member's protected requests and authority.
#[tokio::test]
async fn leaving_a_space_erases_the_persons_requests_and_thread_authority() {
    assert_membership_exit_erases_requests(false).await;
}

#[tokio::test]
async fn removing_a_member_erases_the_persons_requests_and_thread_authority() {
    assert_membership_exit_erases_requests(true).await;
}

async fn assert_membership_exit_erases_requests(removed: bool) {
    let mut f = Fixture::new().await;
    let mut machine = Machine::enroll(&f).await;
    let space = f.spaces[4].clone(); // A owns it; B is a Manager.
    let thread = new_thread(&f, &space).await;
    start_turn(&f, &space, &thread, "ask").await;
    let token = machine.next_token(&f).await;
    let form = question_form();
    let (status, body) = agent(&f, &token, "question", "requests.ask", form).await;
    assert_eq!(status, 200, "{body}");
    let question = body["data"]["id"].clone();
    f.op(
        1,
        "spaces.transfer",
        json!({"space_id":space,"account_id":f.ids[2]}),
    )
    .await
    .ok();
    if removed {
        f.op(
            2,
            "spaces.remove_member",
            json!({"space_id":space,"account_id":f.ids[1]}),
        )
        .await
        .ok();
    } else {
        f.op(1, "spaces.leave", json!({"space_id":space}))
            .await
            .ok();
    }
    let hidden = f
        .op(1, "requests.answer", json!({"id":question,"answer":{}}))
        .await;
    assert_eq!(hidden.status, 404, "{}", hidden.body);

    f.stop();
    let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
    let count = |table: &str| -> i64 {
        c.query_row(
            &format!("SELECT count(*) FROM {table} WHERE space=?1 AND account=?2"),
            params![space, f.ids[1]],
            |r| r.get(0),
        )
        .unwrap()
    };
    let (requests, access) = (count("agent_approvals"), count("agent_access"));
    assert_eq!(
        (requests, access),
        (0, 0),
        "after leaving, the space keeps {requests} hidden request rows and {access} \
         thread-authority rows for the former member"
    );
}

#[tokio::test]
async fn unreceipted_actions_replay_success_and_spend_approval_even_on_failure() {
    let f = Fixture::new().await;
    let mut machine = Machine::enroll(&f).await;
    let space = &f.spaces[0];
    let thread = new_thread(&f, space).await;
    start_turn(&f, space, &thread, "withdraw a secret prompt").await;
    let token = machine.next_token(&f).await;
    let secret = f
        .op(
            1,
            "secrets.request",
            json!({
                "space_id":space,"name":"review-secret","label":"Fixture","reason":"Review test"
            }),
        )
        .await;
    secret.ok();
    let input = json!({"space_id":space,"request_id":secret.body["data"]["request_id"]});
    let (status, denial) = agent(
        &f,
        &token,
        "withdraw-existing",
        "secrets.revoke",
        input.clone(),
    )
    .await;
    assert_eq!(status, 403, "{denial}");
    let id = &denial["error"]["details"]["approval_id"];
    let inbox = f.op(1, "requests.list", json!({})).await;
    inbox.ok();
    let pending = inbox.body["data"]["requests"]
        .as_array()
        .unwrap()
        .iter()
        .find(|request| &request["id"] == id)
        .unwrap();
    assert_eq!(pending["detail"]["lifetimes"], json!(["once"]));
    let other = f.op(2, "requests.list", json!({})).await;
    other.ok();
    assert_eq!(other.body["data"]["requests"], json!([]));

    f.op(1, "requests.approve", json!({"id":id,"lifetime":"once"}))
        .await
        .ok();
    let token = machine.next_token(&f).await;
    let (status, first) = agent(
        &f,
        &token,
        "withdraw-existing",
        "secrets.revoke",
        input.clone(),
    )
    .await;
    assert_eq!(status, 200, "{first}");
    let (status, replay) = agent(&f, &token, "withdraw-existing", "secrets.revoke", input).await;
    assert_eq!(status, 200, "{replay}");
    assert_eq!(replay, first);

    let input = json!({"space_id":space,"request_id":uuid::Uuid::now_v7()});
    let (status, denial) = agent(
        &f,
        &token,
        "withdraw-missing",
        "secrets.revoke",
        input.clone(),
    )
    .await;
    assert_eq!(status, 403, "{denial}");
    let id = denial["error"]["details"]["approval_id"].clone();
    let persistent = f
        .op(
            1,
            "requests.approve",
            json!({"id":id,"lifetime":"this_thread"}),
        )
        .await;
    assert_eq!(persistent.status, 400, "{}", persistent.body);
    assert!(
        persistent.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("once")
    );
    f.op(1, "requests.approve", json!({"id":id,"lifetime":"once"}))
        .await
        .ok();
    let token = machine.next_token(&f).await;
    let (status, failure) = agent(
        &f,
        &token,
        "withdraw-missing",
        "secrets.revoke",
        input.clone(),
    )
    .await;
    assert_eq!(status, 404, "{failure}");
    let (status, retry) = agent(&f, &token, "withdraw-missing", "secrets.revoke", input).await;
    assert_eq!(status, 403, "{retry}");
    let next = &retry["error"]["details"]["approval_id"];
    assert!(next.is_string());
    assert_ne!(next, &id);
}
