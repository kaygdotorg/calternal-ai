//! Account isolation, real passkey decisions and a machine-protocol turn fixture.
use super::*;
use calternal_machine::identity::MachineKey;

pub(super) const OPERATIONS: &[&str] = &[
    "requests.list",
    "requests.ask",
    "requests.answer",
    "requests.approve",
    "requests.deny",
    "space.instructions",
];

pub(super) struct Machine {
    pub(super) id: uuid::Uuid,
    key: MachineKey,
    boot: uuid::Uuid,
    token: String,
    ack: u64,
    leases: Vec<Value>,
}

impl Machine {
    pub(super) async fn enroll(f: &Fixture) -> Self {
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
        Self::enroll_with_code(f, code.body["code"].as_str().unwrap()).await
    }

    pub(super) async fn enroll_with_code(f: &Fixture, code: &str) -> Self {
        let key = MachineKey::generate().unwrap();
        let machine = f
            .machine(
                "/api/v1/machine/enroll",
                None,
                json!({
                    "code":code, "public_key":key.public_key_b64(),
                    "name":"request conformance", "os":"linux", "arch":"x86_64", "direct":false
                }),
            )
            .await;
        machine.ok();
        let mut m = Self {
            id: serde_json::from_value(machine.body["machine_id"].clone()).unwrap(),
            key,
            boot: uuid::Uuid::now_v7(),
            token: String::new(),
            ack: 0,
            leases: vec![],
        };
        m.connect(f).await;
        m
    }

    pub(super) async fn connect(&mut self, f: &Fixture) {
        self.connect_as(f, "podman").await;
    }

    pub(super) async fn connect_as(&mut self, f: &Fixture, backend: &str) -> String {
        self.connect_sized(f, backend, None).await
    }

    pub(super) async fn connect_sized(
        &mut self,
        f: &Fixture,
        backend: &str,
        memory_mib: Option<u32>,
    ) -> String {
        let challenge = f
            .machine(
                "/api/v1/machine/session/start",
                None,
                json!({"machine_id":self.id}),
            )
            .await;
        challenge.ok();
        let ch = challenge.body["challenge"].as_str().unwrap();
        let r = f.machine("/api/v1/machine/session/finish",None,json!({
            "machine_id":self.id,"challenge":ch,"signature":self.key.sign_session(self.id,ch),
            "boot_id":self.boot,"protocol":calternal_machine::protocol::PROTOCOL_VERSION,"backend":backend,"guest_memory_mib":memory_mib,
        })).await;
        r.ok();
        self.token = r.body["token"].as_str().unwrap().to_owned();
        self.ack = 0;
        self.leases.clear();
        r.body["backend"].as_str().unwrap().to_owned()
    }

    pub(super) async fn sync(&mut self, f: &Fixture, discards: Value) -> Vec<Value> {
        self.sync_with_slots(f, discards, 2).await
    }

    pub(super) async fn sync_with_slots(
        &mut self,
        f: &Fixture,
        discards: Value,
        slots: u32,
    ) -> Vec<Value> {
        self.try_sync(f, discards, slots).await.unwrap()
    }

    pub(super) async fn try_sync(
        &mut self,
        f: &Fixture,
        discards: Value,
        slots: u32,
    ) -> Result<Vec<Value>, reqwest::Error> {
        let r = f.try_machine("/api/v1/machine/sync",Some(&self.token),json!({
            "ack":self.ack,"capacity":{"cpus":1,"memory_bytes":0,"max_sandboxes":slots,"running_sandboxes":0},
            "draining":false,"leases":self.leases,"discards":discards,
        })).await?;
        r.ok();
        let mut messages = Vec::new();
        for envelope in r.body["messages"].as_array().unwrap() {
            self.ack = envelope["seq"].as_u64().unwrap();
            let data = &envelope["message"];
            match data["type"].as_str().unwrap() {
                "assign" => self.leases.push(json!({"lease_id":data["lease_id"],"fence":data["fence"],"sandbox":{"state":"running"}})),
                "stop" => self.leases.retain(|lease| lease["lease_id"] != data["lease_id"]),
                _ => (),
            }
            messages.push(data.clone());
        }
        Ok(messages)
    }

    pub(super) async fn erasure_status(&self, f: &Fixture, valid: bool) -> Answer {
        let challenge = f
            .machine(
                "/api/v1/machine/session/start",
                None,
                json!({"machine_id":self.id}),
            )
            .await;
        challenge.ok();
        let challenge = challenge.body["challenge"].as_str().unwrap();
        let signature = if valid {
            self.key.sign_erasure_status(self.id, challenge)
        } else {
            self.key.sign_session(self.id, challenge)
        };
        f.machine(
            "/api/v1/machine/status",
            None,
            json!({
                "machine_id":self.id,"challenge":challenge,"signature":signature,
            }),
        )
        .await
    }

    pub(super) async fn output(&self, f: &Fixture, exec: &Value, chunks: Value) {
        self.try_output(f, exec, chunks).await.unwrap().ok();
    }

    pub(super) async fn try_output(
        &self,
        f: &Fixture,
        exec: &Value,
        chunks: Value,
    ) -> Result<Answer, reqwest::Error> {
        f.try_machine(
            "/api/v1/machine/output",
            Some(&self.token),
            json!({
                "lease_id":exec["lease_id"], "fence":exec["fence"], "exec_id":exec["exec_id"],
                "chunks":chunks,
            }),
        )
        .await
    }

    pub(super) async fn next_token(&mut self, f: &Fixture) -> String {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            assert!(
                Instant::now() < deadline,
                "machine never received provider exec"
            );
            let token = self
                .sync(f, json!([]))
                .await
                .iter()
                .filter(|data| data["type"] == "exec")
                .filter_map(|data| data["env"].as_array())
                .flatten()
                .find(|env| env[0] == "CALTERNAL_AGENT_TOKEN")
                .and_then(|env| env[1].as_str())
                .map(str::to_owned);
            if let Some(token) = token {
                return token;
            }
        }
    }
}

pub(super) async fn agent(
    f: &Fixture,
    token: &str,
    rid: &str,
    op: &str,
    input: Value,
) -> (u16, Value) {
    let r = f
        .client
        .post(format!("{}/api/v1/operations", f.base))
        .header("host", "localhost:18080")
        .bearer_auth(token)
        .json(&json!({"request_id":rid,"operation":op,"input":input}))
        .send()
        .await
        .unwrap();
    (r.status().as_u16(), r.json().await.unwrap())
}

#[tokio::test]
async fn request_operations_keep_questions_approvals_and_secrets_account_private() {
    let mut f = Fixture::new().await;
    let mut machine = Machine::enroll(&f).await;
    let space = f.spaces[3].clone(); // B can edit this space and still cannot see A's inbox.
    let created = f
        .op(
            1,
            "threads.create",
            json!({"space_id":space,"title":"Request fixture"}),
        )
        .await;
    created.ok();
    let thread = created.body["data"]["thread"]["id"].clone();
    let mut feed = live_cases::Live {
        response: drafts_cases::account_live(&f, 1, None).await,
        buffer: Vec::new(),
    };
    assert_eq!(feed.next().await.unwrap()["type"], "ready");
    let start = f
        .op(
            1,
            "agent.run",
            json!({
                "space_id":space,"thread_id":thread,"prompt":"fixture","mode":"least_access",
            }),
        )
        .await;
    start.ok();
    let job = start.body["data"]["job"]["id"].as_str().unwrap().to_owned();
    let token = machine.next_token(&f).await;
    for (op, input) in [
        ("requests.list", json!({})),
        (
            "requests.answer",
            json!({"id":uuid::Uuid::now_v7(),"answer":{}}),
        ),
        (
            "requests.approve",
            json!({"id":uuid::Uuid::now_v7(),"lifetime":"once"}),
        ),
        ("requests.deny", json!({"id":uuid::Uuid::now_v7()})),
    ] {
        assert_eq!(
            agent(&f, &token, "forbidden", op, input).await.0,
            403,
            "{op}"
        );
    }
    let form = json!({"form":{"genui_version":2,"source":"$release=\"\"\nForm([Input($release,\"Which release?\")], \"Release selection\")"}});
    assert_eq!(f.op(1, "requests.ask", form.clone()).await.status, 403);
    let (status, body) = agent(&f, &token, "question", "requests.ask", form).await;
    assert_eq!(status, 200, "{body}");
    let question = body["data"]["id"].clone();
    let frame = feed.next().await.unwrap();
    assert_eq!(frame["type"], "changed");
    assert!(frame.get("id").is_none());
    let waiting = f.op(1, "agent.events", json!({"job_id":job})).await;
    assert_eq!(waiting.body["data"]["job"]["state"], "waiting");
    assert_eq!(
        agent(&f, &token, "ended", "requests.list", json!({}))
            .await
            .0,
        401
    );
    let lease = waiting.body["data"]["job"]["checkpoint"]["lease_id"]
        .as_str()
        .unwrap();
    let released = f
        .req(
            "GET",
            &format!("/api/v1/machines/runs/{lease}"),
            Some(&f.cookies[1]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    released.ok();
    assert_eq!(released.body["state"], "ended");
    assert_eq!(
        f.op(2, "requests.answer", json!({"id":question,"answer":{}}))
            .await
            .status,
        404
    );
    let secret = f.op(1,"secrets.request",json!({"space_id":space,"name":"lane-secret","label":"Fixture","reason":"Needed for test"})).await;
    secret.ok();
    let mut frame = feed.next().await.unwrap();
    if frame["type"] == "ready" {
        frame = feed.next().await.unwrap();
    }
    assert_eq!(frame["type"], "changed");
    drop(feed);
    let ours = f.op(1, "requests.list", json!({})).await;
    ours.ok();
    let rows = ours.body["data"]["requests"].as_array().unwrap();
    assert!(
        rows.iter()
            .any(|row| row["id"] == question && row["detail"]["kind"] == "question")
    );
    assert!(
        rows.iter()
            .any(|row| row["id"] == secret.body["data"]["request_id"]
                && row["detail"]["kind"] == "secret")
    );
    for who in [0, 2, 3] {
        let theirs = f.op(who, "requests.list", json!({})).await;
        theirs.ok();
        assert_eq!(theirs.body["data"]["requests"], json!([]));
    }
    // Leave a different machine available while the original is drained.
    let spare = Machine::enroll(&f).await;
    let drain = format!("/api/v1/machines/{}/drain", machine.id);
    f.req(
        "POST",
        &drain,
        Some(&f.cookies[1]),
        json!({"draining":true}),
        Some(ORIGIN),
    )
    .await
    .ok();
    let warm = f
        .req(
            "POST",
            "/api/v1/machines/runs",
            Some(&f.cookies[1]),
            json!({"space_id":space,"thread_id":uuid::Uuid::now_v7()}),
            Some(ORIGIN),
        )
        .await;
    warm.ok();
    let assigned = f.machine("/api/v1/machine/sync",Some(&spare.token),json!({
        "ack":0,"capacity":{"cpus":1,"memory_bytes":0,"max_sandboxes":2,"running_sandboxes":0},
        "draining":false,"leases":[],
    })).await;
    assigned.ok();
    assert_eq!(
        assigned.body["messages"][0]["message"]["lease_id"],
        warm.body["lease_id"]
    );
    let other_lease = warm.body["lease_id"].as_str().unwrap();
    f.req(
        "POST",
        &format!("/api/v1/machines/runs/{other_lease}/stop"),
        Some(&f.cookies[1]),
        json!({}),
        Some(ORIGIN),
    )
    .await
    .ok();

    let answer = json!({"id":question,"answer":{"release":"1.0"}});
    let answer_first = f
        .op_id(1, "requests.answer", answer.clone(), "answer-once")
        .await;
    answer_first.ok();
    let answer_replay = f.op_id(1, "requests.answer", answer, "answer-once").await;
    answer_replay.ok();
    assert_eq!(answer_first.body["data"], answer_replay.body["data"]);

    let deadline = Instant::now() + Duration::from_secs(5);
    let queued_lease = loop {
        let current = f.op(1, "agent.events", json!({"job_id":job})).await;
        current.ok();
        if let Some(new) = current.body["data"]["job"]["checkpoint"]["lease_id"].as_str()
            && new != lease
        {
            break new.to_owned();
        }
        assert!(
            Instant::now() < deadline,
            "a new pinned lease was not prepared"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    let queued = f
        .req(
            "GET",
            &format!("/api/v1/machines/runs/{queued_lease}"),
            Some(&f.cookies[1]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    queued.ok();
    assert_eq!(
        queued.body["state"], "waiting",
        "should not move to the spare"
    );
    f.req(
        "POST",
        &drain,
        Some(&f.cookies[1]),
        json!({"draining":false}),
        Some(ORIGIN),
    )
    .await
    .ok();

    let token = machine.next_token(&f).await;
    let note = json!({"space_id":space,"title":"Approved fixture","markdown":"Approved body"});
    let (status, refusal) = agent(&f, &token, "approved-note", "notes.create", note.clone()).await;
    assert_eq!(status, 403, "{refusal}");
    let approval = refusal["error"]["details"]["approval_id"].clone();
    assert!(approval.is_string(), "{refusal}");
    for (op, input) in [
        ("requests.deny", json!({"id":approval})),
        ("requests.approve", json!({"id":approval,"lifetime":"once"})),
    ] {
        assert_eq!(f.op(2, op, input).await.status, 404, "{op}");
    }
    f.stop();
    let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
    c.execute(
        "UPDATE installations SET assertion_at=assertion_at-86400000 WHERE user_id=?1",
        params![f.ids[1]],
    )
    .unwrap();
    drop(c);
    f.start().await;
    let stale = f
        .op(
            1,
            "requests.approve",
            json!({"id":approval,"lifetime":"once"}),
        )
        .await;
    assert_eq!(stale.status, 403, "{}", stale.body);
    assert_eq!(
        stale.body["error"]["code"],
        "passkey_reverification_required"
    );
    let mode = f
        .op(1, "agent.mode", json!({"job_id":job,"mode":"full_access"}))
        .await;
    assert_eq!(mode.status, 403, "{}", mode.body);
    assert_eq!(
        mode.body["error"]["code"],
        "passkey_reverification_required"
    );
    erasure_cases::reverify(&mut f, 1).await;
    f.op(
        1,
        "requests.approve",
        json!({"id":approval,"lifetime":"once"}),
    )
    .await
    .ok();

    // Restart after the approval commit and before any matching action.
    f.stop();
    f.start().await;
    machine.connect(&f).await;
    f.op(1, "agent.resume", json!({"job_id":job})).await.ok();
    let token = machine.next_token(&f).await;
    let (first, retry) = tokio::join!(
        agent(&f, &token, "approved-note", "notes.create", note.clone()),
        agent(&f, &token, "approved-note", "notes.create", note.clone()),
    );
    assert_eq!(first.0, 200, "{}", first.1);
    assert_eq!(retry, first);
    let notes = f.op(1, "notes.list", json!({"space_id":space})).await;
    notes.ok();
    assert_eq!(
        notes.body["data"]["notes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|note| note["title"] == "Approved fixture")
            .count(),
        1
    );

    let changed = agent(
        &f,
        &token,
        "approved-note",
        "notes.create",
        json!({"space_id":space,"title":"Changed","markdown":"different"}),
    )
    .await;
    assert_eq!(changed.0, 409, "{}", changed.1);
    let (status, refusal) = agent(&f, &token, "other-note", "notes.create", note).await;
    assert_eq!(status, 403, "{refusal}");
    let next = refusal["error"]["details"]["approval_id"].clone();
    assert_ne!(next, approval);
    f.op(1, "requests.deny", json!({"id":next})).await.ok();
    let token = machine.next_token(&f).await;
    let (status, app) = agent(
        &f,
        &token,
        "app-approval",
        "access.request",
        json!({"job_id":job,"app":"notes"}),
    )
    .await;
    assert_eq!(status, 200, "{app}");
    let app_approval = app["data"]["approval_id"].clone();
    let inbox = f.op(1, "requests.list", json!({})).await;
    inbox.ok();
    let pending = inbox.body["data"]["requests"]
        .as_array()
        .unwrap()
        .iter()
        .find(|request| request["id"] == app_approval)
        .unwrap();
    assert_eq!(
        pending["detail"]["lifetimes"],
        json!(["once", "this_thread"])
    );
    f.op(
        1,
        "requests.approve",
        json!({"id":app_approval,"lifetime":"this_thread"}),
    )
    .await
    .ok();
    let token = machine.next_token(&f).await;
    assert_eq!(
        agent(
            &f,
            &token,
            "read-notes",
            "notes.list",
            json!({"space_id":space})
        )
        .await
        .0,
        200
    );
    f.op(1, "agent.stop", json!({"job_id":job})).await.ok();
    let follow = f
        .op(
            1,
            "agent.run",
            json!({
                "space_id":space,"thread_id":thread,"prompt":"next turn","mode":"least_access",
            }),
        )
        .await;
    follow.ok();
    let token = machine.next_token(&f).await;
    assert_eq!(
        agent(
            &f,
            &token,
            "read-notes-again",
            "notes.list",
            json!({"space_id":space})
        )
        .await
        .0,
        200
    );
    f.op(
        1,
        "agent.stop",
        json!({"job_id":follow.body["data"]["job"]["id"]}),
    )
    .await
    .ok();
    f.op(1, "threads.archive", json!({"id":thread})).await.ok();
    assert_eq!(
        f.op(1, "requests.deny", json!({"id":app_approval}))
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn standing_instructions_use_space_settings_capability_and_journaled_replacement() {
    let f = Fixture::new().await;
    let space = &f.spaces[4]; // B is a Manager here.
    let path = f
        .dir
        .path()
        .join("userdata-ssd/spaces")
        .join(space)
        .join("AI/AGENTS.md");
    let outside = f.dir.path().join("instructions-canary");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&outside, "outside").unwrap();
    std::os::unix::fs::symlink(&outside, &path).unwrap();
    assert_eq!(
        f.op(
            2,
            "space.instructions",
            json!({"space_id":space,"markdown":"refused"})
        )
        .await
        .status,
        500
    );
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "outside");
    std::fs::remove_file(&path).unwrap();
    let mut journal = live_cases::Live::open(&f, 1, json!([{"space_id":space,"bookmark":null}]))
        .await
        .unwrap();
    assert_eq!(journal.next().await.unwrap()["type"], "ready");
    let input = json!({"space_id":space,"markdown":"# Standing instructions\nUse the fixture."});
    let written = f
        .op_id(2, "space.instructions", input.clone(), "standing-write")
        .await;
    written.ok();
    let event = journal.next().await.unwrap();
    assert_eq!(event["entry"]["kind"], "space.instructions");
    assert!(!event.to_string().contains("Use the fixture."));

    let replay = f
        .op_id(2, "space.instructions", input.clone(), "standing-write")
        .await;
    replay.ok();
    assert_eq!(written.body["data"], replay.body["data"]);
    let got = f
        .op(1, "space.instructions", json!({"space_id":space}))
        .await;
    got.ok();
    assert_eq!(got.body["data"]["markdown"], input["markdown"]);
    assert_eq!(
        std::fs::read_to_string(
            f.dir
                .path()
                .join("userdata-ssd/spaces")
                .join(space)
                .join("AI/AGENTS.md")
        )
        .unwrap(),
        input["markdown"].as_str().unwrap()
    );
    assert_eq!(
        f.op(2, "space.instructions", json!({"space_id":f.spaces[3]}))
            .await
            .status,
        403
    );
    for who in [0, 3] {
        let hidden = f.op(who, "space.instructions", input.clone()).await;
        let absent = f
            .op(
                who,
                "space.instructions",
                json!({"space_id":uuid::Uuid::now_v7()}),
            )
            .await;
        assert_eq!(hidden.status, 404, "{}", hidden.body);
        assert_eq!(hidden.signature(), absent.signature());
    }
    let reused = f
        .op_id(
            2,
            "space.instructions",
            json!({"space_id":space,"markdown":"Changed"}),
            "standing-write",
        )
        .await;
    assert_eq!(reused.status, 409);
}
