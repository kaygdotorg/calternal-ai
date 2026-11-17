//! Independent review expectations for person requests across turn and space
//! lifecycles (docs/reviews/requests-independent-review-2026-11-18.md). Each
//! test states one expectation from docs/requests.md or docs/agent-turns.md; a
//! failure names a gap. Uses the request lane's machine-protocol fixture.
use super::requests_cases::{Machine, agent};
use super::*;

async fn start_turn(f: &Fixture, space: &str, thread: &Value, prompt: &str) -> String {
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

async fn new_thread(f: &Fixture, space: &str) -> Value {
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

/// A stopped turn's pending approval stays in the inbox. Approving it records a
/// thread grant and rotates the thread credential (docs/requests.md: "The
/// protected commit for a person's approval rotates the thread credential").
/// The rule in docs/agent-turns.md is that token changes revoke the old
/// credential and pause the turn so an explicit Resume delivers a replacement.
/// Expectation: the thread's current running turn either keeps a working
/// credential or is paused, never left running with a revoked one.
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

    f.op(
        1,
        "requests.approve",
        json!({"id":approval,"lifetime":"this_thread"}),
    )
    .await
    .ok();
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
    assert!(
        status == 200 || state != "running",
        "approving the stopped turn's request revoked the running turn's credential \
         (status {status}) while the turn stays {state}; no Resume can deliver a replacement"
    );
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
    let form = json!({"form":{"genui_version":2,"source":"$release=\"\"\nForm([Input($release,\"Which release?\")], \"Release selection\")"}});
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

/// Same expectation for membership: a person who leaves a space (or is
/// removed) can no longer list, answer or deny its requests, because every
/// inbox entry rechecks current space access. The rows still count toward the
/// 32-per-thread and 256-per-account bounds, and pending rows never age out.
/// Expectation: leaving a space erases the person's request, grant and
/// thread-authority rows there, as deleting the space already does.
#[tokio::test]
async fn leaving_a_space_erases_the_persons_requests_and_thread_authority() {
    let mut f = Fixture::new().await;
    let mut machine = Machine::enroll(&f).await;
    let space = f.spaces[4].clone(); // A owns it; B is a Manager.
    let thread = new_thread(&f, &space).await;
    let job = start_turn(&f, &space, &thread, "ask").await;
    let token = machine.next_token(&f).await;
    let form = json!({"form":{"genui_version":2,"source":"$release=\"\"\nForm([Input($release,\"Which release?\")], \"Release selection\")"}});
    let (status, body) = agent(&f, &token, "question", "requests.ask", form).await;
    assert_eq!(status, 200, "{body}");
    let question = body["data"]["id"].clone();
    f.op(1, "agent.stop", json!({"job_id":job})).await.ok();
    f.op(
        1,
        "spaces.transfer",
        json!({"space_id":space,"account_id":f.ids[2]}),
    )
    .await
    .ok();
    f.op(1, "spaces.leave", json!({"space_id":space}))
        .await
        .ok();
    let hidden = f
        .op(1, "requests.answer", json!({"id":question,"answer":{}}))
        .await;
    assert_eq!(hidden.status, 404, "precondition: {}", hidden.body);

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
