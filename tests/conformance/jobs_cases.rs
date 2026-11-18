//! Plugin job and plugin write conformance over TCP HTTP, using the test-only
//! `jobs-demo` plugin (`tests/plugins`, trusted only by a `test-plugins` build).
//!
//! Accounts: 1 owns `spaces[0]`; 2 owns `spaces[1]` and is no member of `spaces[0]`.
use super::*;

/// Enable `jobs-demo` in `space` as `who` (an owner). Idempotent.
pub async fn enable(f: &Fixture, who: usize, space: &str) {
    f.op(
        who,
        "plugins.enable",
        json!({"space_id":space,"plugin_id":"jobs-demo"}),
    )
    .await
    .ok();
}

/// Enable `jobs-demo`, start a job with `input` and return its ID.
pub async fn start_job(f: &Fixture, who: usize, space: &str, input: Value) -> String {
    enable(f, who, space).await;
    let mut input = input;
    input["space_id"] = json!(space);
    let r = f.op(who, "jobs-demo.start", input).await;
    r.ok();
    r.body["data"]["job_id"].as_str().unwrap().to_owned()
}

async fn job(f: &Fixture, who: usize, id: &str) -> Answer {
    f.op(who, "jobs.get", json!({"job_id":id})).await
}

/// Poll until the job satisfies `done`, returning its record.
async fn until(f: &Fixture, who: usize, id: &str, done: impl Fn(&Value) -> bool) -> Value {
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        let r = job(f, who, id).await;
        if r.status == 200 && done(&r.body["data"]["job"]) {
            return r.body["data"]["job"].clone();
        }
        assert!(Instant::now() < end, "job condition never held: {}", r.body);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

/// One server-sent event: name, id and JSON data.
pub(super) type Event = (String, Option<String>, Value);

/// A job event stream read incrementally over HTTP.
pub(super) struct Sse {
    response: reqwest::Response,
    buffer: String,
}

impl Sse {
    /// Open `/api/v1/jobs/<id>/events`; a refusal returns its status and JSON body.
    pub(super) async fn open(
        f: &Fixture,
        cookie: &str,
        id: &str,
        after: Option<&str>,
    ) -> Result<Self, (u16, Value)> {
        let query = after.map(|a| format!("?after={a}")).unwrap_or_default();
        let response = f
            .client
            .get(format!("{}/api/v1/jobs/{id}/events{query}", f.base))
            .header("host", "localhost:18080")
            .header("origin", ORIGIN)
            .header("cookie", cookie)
            .send()
            .await
            .unwrap();
        if response.status() != 200 {
            let status = response.status().as_u16();
            return Err((status, response.json().await.unwrap_or(Value::Null)));
        }
        Ok(Self {
            response,
            buffer: String::new(),
        })
    }

    /// The next event, or `None` when the stream closes or stays silent for 10 s.
    pub(super) async fn next(&mut self) -> Option<Event> {
        loop {
            if let Some(i) = self.buffer.find("\n\n") {
                let block: String = self.buffer.drain(..i + 2).collect();
                if block.starts_with(':') {
                    continue;
                }
                let (mut name, mut id, mut data) = (String::new(), None, Value::Null);
                for line in block.lines() {
                    if let Some(v) = line.strip_prefix("event: ") {
                        name = v.into();
                    } else if let Some(v) = line.strip_prefix("id: ") {
                        id = Some(v.into());
                    } else if let Some(v) = line.strip_prefix("data: ") {
                        data = serde_json::from_str(v).unwrap();
                    }
                }
                return Some((name, id, data));
            }
            match tokio::time::timeout(Duration::from_secs(10), self.response.chunk()).await {
                Ok(Ok(Some(bytes))) => self.buffer.push_str(&String::from_utf8_lossy(&bytes)),
                _ => return None,
            }
        }
    }
}

#[tokio::test]
async fn job_streams_in_order_checkpoints_and_resumes_after_kill() {
    let mut f = Fixture::new().await;
    let s = f.spaces[0].clone();
    let id = start_job(&f, 1, &s, json!({"steps":40,"delay_ms":40,"every":5})).await;
    let mut sse = Sse::open(&f, &f.cookies[1], &id, None).await.unwrap();
    let (name, _, snapshot) = sse.next().await.unwrap();
    assert_eq!(name, "snapshot");
    assert_eq!(snapshot["job"]["id"], json!(id));
    let mut last = None;
    for expected in 0..8u64 {
        let (name, event_id, data) = sse.next().await.unwrap();
        assert_eq!(name, "chunk");
        assert_eq!(data, json!({"seq":expected,"data":{"step":expected}}));
        last = event_id;
    }
    let last = last.unwrap();
    until(&f, 1, &id, |j| j["checkpoint_seq"].as_u64() >= Some(10)).await;
    drop(sse);
    // SIGKILL mid-run: chunks are lost, the record and its checkpoint are not.
    f.stop();
    f.start().await;
    let done = until(&f, 1, &id, |j| j["state"] == "done").await;
    let resumed = done["result"]["resumed_from"].as_u64().unwrap();
    assert!(
        resumed >= 10 && resumed % 5 == 0,
        "not resumed from a checkpoint: {done}"
    );
    assert_eq!(done["result"]["steps"], 40);
    // Reconnecting with the old cursor: the run epoch changed, so a reset precedes
    // the final record.
    let mut sse = Sse::open(&f, &f.cookies[1], &id, Some(&last))
        .await
        .unwrap();
    let names: Vec<String> = {
        let mut names = vec![];
        while let Some((name, _, data)) = sse.next().await {
            if name == "end" {
                assert_eq!(data["job"]["state"], "done");
            }
            names.push(name);
        }
        names
    };
    assert_eq!(names.first().map(String::as_str), Some("snapshot"));
    assert!(
        names.iter().any(|n| n == "reset"),
        "no reset after restart: {names:?}"
    );
    assert_eq!(names.last().map(String::as_str), Some("end"));
    evidence("job-restart", json!({"job":done,"events":names}));
}

#[tokio::test]
async fn jobs_are_invisible_outside_their_space_and_cancel_discards_output() {
    let f = Fixture::new().await;
    let s = f.spaces[0].clone();
    let id = start_job(&f, 1, &s, json!({"steps":1000,"delay_ms":20,"every":10})).await;
    until(&f, 1, &id, |j| j["state"] == "running").await;
    let missing = uuid::Uuid::now_v7().to_string();
    // Account 2 is no member of the job's space: every surface answers like a missing ID.
    for op in ["jobs.get", "jobs.cancel"] {
        let x = f.op_id(2, op, json!({"job_id":id}), "probe").await;
        let y = f.op_id(2, op, json!({"job_id":missing}), "probe").await;
        assert_eq!(x.status, 404, "{op}: {}", x.body);
        assert_eq!(x.signature(), y.signature(), "{op} reveals the job");
    }
    let x = Sse::open(&f, &f.cookies[2], &id, None).await.err().unwrap();
    let y = Sse::open(&f, &f.cookies[2], &missing, None)
        .await
        .err()
        .unwrap();
    assert_eq!(x.0, 404);
    assert_eq!(x, y, "the event stream reveals the job");
    // The owner watches, then cancels; the stream ends with the cancelled record.
    let mut sse = Sse::open(&f, &f.cookies[1], &id, None).await.unwrap();
    assert_eq!(sse.next().await.unwrap().0, "snapshot");
    let r = f.op(1, "jobs.cancel", json!({"job_id":id})).await;
    r.ok();
    assert_eq!(r.body["data"]["job"]["state"], "cancelled");
    let mut end = None;
    while let Some((name, _, data)) = sse.next().await {
        if name == "end" {
            end = Some(data);
        }
    }
    assert_eq!(end.unwrap()["job"]["state"], "cancelled");
    let at_cancel = job(&f, 1, &id).await.body["data"]["job"].clone();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        job(&f, 1, &id).await.body["data"]["job"],
        at_cancel,
        "late output changed the job"
    );
}

#[tokio::test]
async fn running_job_stops_when_its_starter_signs_out() {
    let mut f = Fixture::new().await;
    let s = f.spaces[0].clone();
    let other = f.login(1).await;
    other.ok();
    let other = other.cookie();
    let id = start_job(&f, 1, &s, json!({"steps":1000,"delay_ms":20,"every":5})).await;
    until(&f, 1, &id, |j| j["state"] == "running").await;
    let r = f.auth("logout", Some(&f.cookies[1]), json!({})).await;
    assert_eq!(r.status, 204);
    // The job was started on the signed-out installation: its next host call is
    // refused and it fails. The account's other session can still read the record.
    f.cookies[1] = other;
    let failed = until(&f, 1, &id, |j| j["state"] == "failed").await;
    assert!(failed["checkpoint_seq"].as_u64().unwrap() < 1000);
}

#[tokio::test]
async fn plugin_write_after_sign_out_does_not_land() {
    let mut f = Fixture::new().await;
    let s = f.spaces[0].clone();
    enable(&f, 1, &s).await;
    // Positive control: the same write lands with a live session.
    f.op(
        1,
        "jobs-demo.write",
        json!({"space_id":s,"text":"written-before"}),
    )
    .await
    .ok();
    f.stop();
    gate_runtime(&f);
    f.start().await;
    let (client, base, cookie) = (f.client.clone(), f.base.clone(), f.cookies[1].clone());
    let body = json!({"request_id":"late-write","operation":"jobs-demo.write","input":{"space_id":s,"text":"written-after-sign-out"}});
    let task = tokio::spawn(async move {
        client
            .post(format!("{base}/api/v1/operations"))
            .header("host", "localhost:18080")
            .header("origin", ORIGIN)
            .header("cookie", cookie)
            .json(&body)
            .send()
            .await
            .unwrap()
    });
    let marker = f.dir.path().join("worker-entered");
    for _ in 0..300 {
        if marker.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(marker.exists(), "worker did not reach barrier");
    let r = f.auth("logout", Some(&f.cookies[1]), json!({})).await;
    assert_eq!(r.status, 204);
    std::fs::write(f.dir.path().join("worker-release"), "").unwrap();
    let status = task.await.unwrap().status().as_u16();
    assert_eq!(status, 401);
    tokio::time::sleep(Duration::from_millis(200)).await;
    let mut files = vec![];
    walk(f.dir.path(), f.dir.path(), &mut files);
    let contents: Vec<String> = files
        .iter()
        .filter(|v| v["directory"] == false)
        .filter_map(|v| std::fs::read(f.dir.path().join(v["path"].as_str()?)).ok())
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .collect();
    assert!(
        contents.iter().any(|c| c.contains("written-before")),
        "control write missing"
    );
    assert!(
        !contents
            .iter()
            .any(|c| c.contains("written-after-sign-out")),
        "a plugin write landed after sign-out"
    );
}

fn wait_input(wait: &str, delay_ms: u64) -> Value {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    json!({"steps":0,"delay_ms":0,"every":1,"wait":wait,"deadline_ms":now+delay_ms})
}

async fn question_id(f: &Fixture, who: usize) -> String {
    let r = f.op(who, "requests.list", json!({})).await;
    r.ok();
    r.body["data"]["requests"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["detail"]["label"] == "Release selection")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn plugin_question_wait_survives_restart_and_replays_only_the_first_answer() {
    let mut f = Fixture::new().await;
    let space = f.spaces[0].clone();
    let id = start_job(&f, 1, &space, wait_input("question", 60_000)).await;
    let parked = until(&f, 1, &id, |j| j["state"] == "waiting").await;
    assert_eq!(parked["wait"]["reason"], "Waiting for a release");
    assert_eq!(parked["checkpoint"], json!({"saved":"before-signal"}));
    let request = question_id(&f, 1).await;

    f.stop();
    f.start().await;
    assert_eq!(question_id(&f, 1).await, request);
    assert_eq!(job(&f, 1, &id).await.body["data"]["job"]["state"], "waiting");
    let invalid = f.op(1, "requests.answer", json!({"id":request,"answer":{"text":7}})).await;
    assert_eq!(invalid.status, 400, "{}", invalid.body);

    let answer = json!({"id":request,"answer":{"text":"go"}});
    let foreign = f.op_id(2, "requests.answer", answer.clone(), "resolve").await;
    assert_eq!(foreign.status, 404, "{}", foreign.body);
    f.op_id(1, "requests.answer", answer.clone(), "resolve").await.ok();
    f.op_id(1, "requests.answer", answer, "resolve").await.ok();
    let changed = f.op_id(1, "requests.answer", json!({"id":request,"answer":{"text":"stop"}}), "resolve").await;
    assert_eq!(changed.status, 409, "{}", changed.body);
    let done = until(&f, 1, &id, |j| j["state"] == "done").await;
    assert_eq!(done["result"], json!({
        "checkpoint":{"saved":"before-signal"}, "signal":{"kind":"answer","value":{"text":"go"}}
    }));
}

#[tokio::test]
async fn plugin_deadline_resumes_after_restart_and_cancel_wins_once() {
    let mut f = Fixture::new().await;
    let space = f.spaces[0].clone();
    let id = start_job(&f, 1, &space, wait_input("deadline", 8_000)).await;
    until(&f, 1, &id, |j| j["state"] == "waiting").await;
    let other = start_job(&f, 1, &space, wait_input("deadline", 8_000)).await;
    until(&f, 1, &other, |j| j["state"] == "waiting").await;
    f.op(1, "jobs.cancel", json!({"job_id":other})).await.ok();

    f.stop();
    f.start().await;
    let done = until(&f, 1, &id, |j| j["state"] == "done").await;
    assert_eq!(done["result"]["signal"], json!({"kind":"deadline"}));
    assert_eq!(job(&f, 1, &other).await.body["data"]["job"]["state"], "cancelled");
}

#[tokio::test]
async fn plugin_approval_resumes_with_the_inbox_decision() {
    let f = Fixture::new().await;
    let space = f.spaces[0].clone();
    for approved in [false, true] {
        let id = start_job(&f, 1, &space, wait_input("approval", 60_000)).await;
        until(&f, 1, &id, |j| j["state"] == "waiting").await;
        let inbox = f.op(1, "requests.list", json!({})).await;
        inbox.ok();
        let pending = inbox.body["data"]["requests"].as_array().unwrap().iter()
            .find(|r| r["detail"]["kind"] == "approval" && r["detail"]["app"] == "jobs-demo").unwrap();
        let request = &pending["id"];
        if approved {
            assert_eq!(pending["detail"]["lifetimes"], json!(["once"]));
            f.op(1, "requests.approve", json!({"id":request,"lifetime":"once"})).await.ok();
        } else {
            f.op(1, "requests.deny", json!({"id":request})).await.ok();
        }
        let done = until(&f, 1, &id, |j| j["state"] == "done").await;
        assert_eq!(done["result"]["signal"], json!({"kind":"approval","approved":approved}));
    }
}

#[tokio::test]
async fn plugin_webhook_requires_the_space_signature_and_deduplicates_delivery() {
    let f = Fixture::new().await;
    let space = f.spaces[0].clone();
    let correlation = uuid::Uuid::now_v7().to_string();
    let mut input = wait_input("webhook", 60_000);
    input["correlation"] = json!(correlation);
    let id = start_job(&f, 1, &space, input).await;
    until(&f, 1, &id, |j| j["state"] == "waiting").await;
    let inbox = f.op(1, "requests.list", json!({})).await;
    inbox.ok();
    let pending = inbox.body["data"]["requests"].as_array().unwrap().iter()
        .find(|r| r["detail"]["name"] == "plugin:jobs-demo:delivery").unwrap();
    let key = "fixture-key-only-32-bytes-or-more";
    f.auth("secrets/provide", Some(&f.cookies[1]),
        json!({"space_id":space,"request_id":pending["id"],"value":key})).await.ok();

    let path = format!("/api/v1/plugin-webhooks/{space}/jobs-demo/delivery/{id}/{correlation}");
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let sign = |body: &str| {
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, key.as_bytes());
        let message = format!("{timestamp}\nPOST\n{path}\n{body}");
        ring::hmac::sign(&key, message.as_bytes()).as_ref().iter()
            .map(|byte| format!("{byte:02x}")).collect::<String>()
    };
    let send = |body: &str, signature: String| f.client.post(format!("{}{path}", f.base))
        .header("host", "localhost:18080")
        .header("x-calternal-timestamp", timestamp)
        .header("x-calternal-signature", signature)
        .body(body.to_owned()).send();
    let body = r#"{"text":"ships"}"#;
    let signature = sign(body);
    assert_eq!(send(body, "0".repeat(64)).await.unwrap().status(), 404);
    let wrong = r#"{"text":42}"#;
    assert_eq!(send(wrong, sign(wrong)).await.unwrap().status(), 400);
    assert_eq!(send(body, signature.clone()).await.unwrap().status(), 202);
    assert_eq!(send(body, signature).await.unwrap().status(), 202);
    let different = r#"{"text":"stop"}"#;
    assert_eq!(send(different, sign(different)).await.unwrap().status(), 409);
    let done = until(&f, 1, &id, |j| j["state"] == "done").await;
    assert_eq!(done["result"]["signal"], json!({"kind":"webhook","value":{"text":"ships"}}));
}
