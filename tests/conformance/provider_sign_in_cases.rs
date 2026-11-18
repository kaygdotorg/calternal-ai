//! Real passkey transport with an account-free machine protocol fixture.
use super::*;
use crate::requests_cases::{Machine, agent};

pub(super) const OPERATIONS: &[&str] = &[
    "provider.sign_in.start",
    "provider.sign_in.answer",
    "provider.sign_in.status",
    "provider.sign_in.typing",
];
const FRAME: &str = "eyJ1cmwiOiAiaHR0cHM6Ly9jbGF1ZGUuYWkvb2F1dGgvYXV0aG9yaXplP3N0YXRlPVBSSVZBVEVfVEVTVF9MSU5LIiwgImRpc3BsYXlfY29kZSI6IG51bGwsICJuZWVkc19jb2RlIjogdHJ1ZX0K";
const ANSWER: &str = "PRIVATE_TEST_BROWSER_ANSWER";

fn assert_private_bytes_absent(path: &std::path::Path) {
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            assert_private_bytes_absent(&entry.path());
        } else if entry.file_type().unwrap().is_file() {
            let bytes = std::fs::read(entry.path()).unwrap();
            for sentinel in [ANSWER, "PRIVATE_TEST_LINK", FRAME] {
                assert!(
                    !bytes
                        .windows(sentinel.len())
                        .any(|window| window == sentinel.as_bytes()),
                    "a provider challenge or answer reached durable state"
                );
            }
        }
    }
}

async fn next(f: &Fixture, machine: &mut Machine, kind: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Some(message) = machine
                .sync(f, json!([]))
                .await
                .into_iter()
                .find(|v| v["type"] == kind)
            {
                return message;
            }
        }
    })
    .await
    .unwrap()
}

async fn fixture_start(f: &Fixture, machine: &mut Machine) -> (String, Value, Value) {
    let created = f
        .op(
            1,
            "threads.create",
            json!({"space_id":f.spaces[0],"title":"Private provider login"}),
        )
        .await;
    created.ok();
    let thread = created.body["data"]["thread"]["id"].clone();
    let input = json!({"space_id":f.spaces[0],"thread_id":thread,"provider_id":"calternal-claude-code","machine_id":machine.id});
    let reply = f.op(1, "provider.sign_in.start", input.clone()).await;
    reply.ok();
    let id = reply.body["data"]["id"].as_str().unwrap().to_owned();
    let duplicate = f.op(1, "provider.sign_in.start", input).await;
    duplicate.ok();
    assert_eq!(duplicate.body["data"]["id"], id);
    let exec = next(f, machine, "exec").await;
    assert_eq!(exec["argv"][1], "--sign-in");
    assert_eq!(exec["interactive"], true);
    machine
        .output(
            f,
            &exec,
            json!([{"kind":"data","seq":1,"stream":"stdout","data":FRAME}]),
        )
        .await;
    (id, thread, exec)
}

async fn until(f: &Fixture, op: &str, input: Value, condition: impl Fn(&Value) -> bool) -> Value {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        let reply = f.op(1, op, input.clone()).await;
        reply.ok();
        if condition(&reply.body) {
            return reply.body;
        }
        assert!(Instant::now() < until, "timed out waiting for {op}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn provider_sign_in_relays_only_the_owners_challenge_and_answer() {
    let f = Fixture::new().await;
    let mut machine = Machine::enroll(&f).await;
    let (id, thread, exec) = fixture_start(&f, &mut machine).await;
    until(&f, "requests.list", json!({}), |v| {
        v.to_string().contains("PRIVATE_TEST_LINK")
    })
    .await;
    let raw = f
        .req(
            "GET",
            &format!(
                "/api/v1/machines/execs/{}/output",
                exec["exec_id"].as_str().unwrap()
            ),
            Some(&f.cookies[1]),
            Value::Null,
            Some(ORIGIN),
        )
        .await;
    assert_eq!(raw.status, 404);
    assert_private_bytes_absent(f.dir.path());
    for account in [0, 2, 3] {
        let foreign = f.op(account, "requests.list", json!({})).await;
        foreign.ok();
        assert!(!foreign.body.to_string().contains("PRIVATE_TEST_LINK"));
        assert_eq!(
            f.op(account, "provider.sign_in.status", json!({"id":id}))
                .await
                .status,
            404
        );
        assert_eq!(
            f.op(
                account,
                "provider.sign_in.answer",
                json!({"id":id,"code":ANSWER})
            )
            .await
            .status,
            404
        );
    }
    f.op(1, "agent.run", json!({
        "space_id":f.spaces[0],"thread_id":uuid::Uuid::now_v7(),"prompt":"thread token for sign-in isolation"
    })).await.ok();
    let token = machine.next_token(&f).await;
    for (operation, input) in [
        (
            "provider.sign_in.start",
            json!({"space_id":f.spaces[0],"provider_id":"calternal-claude-code"}),
        ),
        ("provider.sign_in.answer", json!({"id":id,"code":ANSWER})),
    ] {
        let (status, body) = agent(&f, &token, operation, operation, input).await;
        assert_eq!(status, 403);
        assert!(!body.to_string().contains("PRIVATE_TEST"));
    }
    f.op(1, "provider.sign_in.answer", json!({"id":id,"code":ANSWER}))
        .await
        .ok();
    let input = next(&f, &mut machine, "input").await;
    assert_eq!(input["exec_id"], exec["exec_id"]);
    assert_eq!(input["data"], json!(format!("{ANSWER}\n").as_bytes()));
    machine
        .output(&f, &exec, json!([{"kind":"exit","seq":2,"code":0}]))
        .await;
    until(&f, "provider.sign_in.status", json!({"id":id}), |v| {
        v["data"]["status"] == "done"
    })
    .await;
    let document = until(&f, "threads.get", json!({"id":thread}), |v| {
        v.to_string()
            .contains("Sign-in was done for Claude Code at")
    })
    .await;
    let text = document.to_string();
    assert_eq!(
        text.matches("Sign-in was done for Claude Code at").count(),
        1
    );
    assert!(!text.contains("PRIVATE_TEST"));
    assert!(
        !f.op(1, "requests.list", json!({}))
            .await
            .body
            .to_string()
            .contains("PRIVATE_TEST")
    );
    assert_private_bytes_absent(f.dir.path());
}

#[tokio::test]
async fn provider_sign_in_crash_drops_the_pending_challenge() {
    let mut f = Fixture::new().await;
    let mut machine = Machine::enroll(&f).await;
    let (id, thread, _) = fixture_start(&f, &mut machine).await;
    until(&f, "requests.list", json!({}), |v| {
        v.to_string().contains("PRIVATE_TEST_LINK")
    })
    .await;
    f.stop();
    f.start().await;
    assert_eq!(
        f.op(1, "provider.sign_in.status", json!({"id":id}))
            .await
            .status,
        404
    );
    assert_eq!(
        f.op(1, "provider.sign_in.answer", json!({"id":id,"code":ANSWER}))
            .await
            .status,
        404
    );
    let inbox = f.op(1, "requests.list", json!({})).await;
    inbox.ok();
    assert!(!inbox.body.to_string().contains("PRIVATE_TEST"));
    let document = f.op(1, "threads.get", json!({"id":thread})).await;
    document.ok();
    assert_eq!(
        document
            .body
            .to_string()
            .matches("Sign-in was attempted for Claude Code at")
            .count(),
        1
    );
    assert!(!document.body.to_string().contains("PRIVATE_TEST"));
}

#[tokio::test]
async fn provider_sign_in_automatically_resumes_the_same_failed_turn() {
    let f = Fixture::new().await;
    let mut machine = Machine::enroll(&f).await;
    let created = f
        .op(
            1,
            "threads.create",
            json!({"space_id":f.spaces[0],"title":"Resume after login"}),
        )
        .await;
    created.ok();
    let thread = created.body["data"]["thread"]["id"].clone();
    let submitted = f
        .op(
            1,
            "threads.send",
            json!({"id":thread,"message":"Resume this request once."}),
        )
        .await;
    submitted.ok();
    let job = submitted.body["data"]["job"]["id"].clone();
    let turn = next(&f, &mut machine, "exec").await;
    machine.output(&f, &turn, json!([
        {"kind":"data","seq":1,"stream":"stdout","data":"eyJ2ZXJzaW9uIjogMSwgInNlcSI6IDEsICJldmVudCI6IHsidHlwZSI6ICJlcnJvciIsICJjb2RlIjogInByb3ZpZGVyX3NpZ25lZF9vdXQiLCAibWVzc2FnZSI6ICJQcm92aWRlciBzaWduLWluIGlzIHJlcXVpcmVkLiIsICJjbGFzcyI6ICJhdXRoZW50aWNhdGlvbiIsICJiaWxsaW5nIjogInVua25vd24ifX0KeyJ2ZXJzaW9uIjogMSwgInNlcSI6IDIsICJldmVudCI6IHsidHlwZSI6ICJkb25lIiwgIm91dGNvbWUiOiAiZmFpbGVkIn19Cg=="},
        {"kind":"exit","seq":2,"code":0}
    ])).await;
    let login = next(&f, &mut machine, "exec").await;
    assert_eq!(login["argv"][1], "--sign-in");
    machine
        .output(
            &f,
            &login,
            json!([{"kind":"data","seq":1,"stream":"stdout","data":FRAME}]),
        )
        .await;
    let inbox = until(&f, "requests.list", json!({}), |v| {
        v.to_string().contains("PRIVATE_TEST_LINK")
    })
    .await;
    let request = inbox["data"]["requests"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["detail"]["kind"] == "provider_sign_in")
        .unwrap();
    assert_eq!(
        request["detail"]["label"],
        "Claude Code needs you to sign in again"
    );
    let id = request["id"].clone();
    let typing = f
        .op(
            1,
            "provider.sign_in.typing",
            json!({"space_id":f.spaces[0],"thread_id":thread}),
        )
        .await;
    typing.ok();
    assert_eq!(typing.body["data"]["id"], id);
    f.op(1, "provider.sign_in.answer", json!({"id":id,"code":ANSWER}))
        .await
        .ok();
    let _ = next(&f, &mut machine, "input").await;
    machine
        .output(&f, &login, json!([{"kind":"exit","seq":2,"code":0}]))
        .await;
    let resumed = next(&f, &mut machine, "exec").await;
    assert_ne!(resumed["exec_id"], turn["exec_id"]);
    let status = f.op(1, "jobs.get", json!({"job_id":job})).await;
    status.ok();
    assert!(matches!(
        status.body["data"]["job"]["state"].as_str(),
        Some("running" | "placing")
    ));
    assert!(!status.body.to_string().contains("PRIVATE_TEST"));
}

#[tokio::test]
async fn provider_sign_in_typing_reoffers_a_readiness_challenge() {
    let f = Fixture::new().await;
    let mut machine = Machine::enroll(&f).await;
    let created = f
        .op(
            1,
            "threads.create",
            json!({"space_id":f.spaces[0],"title":"Compose after sign-in offer"}),
        )
        .await;
    created.ok();
    let thread = created.body["data"]["thread"]["id"].clone();
    f.op(
        1,
        "agent.check",
        json!({"space_id":f.spaces[0],"machine_id":machine.id}),
    )
    .await
    .ok();
    let check = next(&f, &mut machine, "exec").await;
    machine.output(&f, &check, json!([
        {"kind":"data","seq":1,"stream":"stdout","data":"eyJ2ZXJzaW9uIjogMSwgInNlcSI6IDEsICJldmVudCI6IHsidHlwZSI6ICJjaGVjayIsICJpbnN0YWxsZWQiOiB0cnVlLCAic2lnbmVkX2luIjogZmFsc2UsICJzaWduX2luX2NvbW1hbmQiOiAiY2xhdWRlIGF1dGggbG9naW4ifX0K"},
        {"kind":"exit","seq":2,"code":0}
    ])).await;
    let login = next(&f, &mut machine, "exec").await;
    machine
        .output(
            &f,
            &login,
            json!([{"kind":"data","seq":1,"stream":"stdout","data":FRAME}]),
        )
        .await;
    let inbox = until(&f, "requests.list", json!({}), |v| {
        v.to_string().contains("PRIVATE_TEST_LINK")
    })
    .await;
    let id = inbox["data"]["requests"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["detail"]["kind"] == "provider_sign_in")
        .unwrap()["id"]
        .clone();
    machine
        .output(&f, &login, json!([{"kind":"exit","seq":2,"code":1}]))
        .await;
    until(&f, "provider.sign_in.status", json!({"id":id}), |v| {
        v["data"]["status"] == "failed"
    })
    .await;
    let offered = f
        .op(
            1,
            "provider.sign_in.typing",
            json!({"space_id":f.spaces[0],"thread_id":thread}),
        )
        .await;
    offered.ok();
    assert_eq!(offered.body["data"]["status"], "pending");
    assert_ne!(offered.body["data"]["id"], id);
}
