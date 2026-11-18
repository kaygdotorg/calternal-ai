use super::*;

#[tokio::test]
async fn deployment_bind_preserves_host_and_origin_checks() {
    let f = fixture_with_listener().await;
    f.req("GET", "/api/v1/auth/status", None, Value::Null, None)
        .await
        .ok();

    let denied_host = f
        .client
        .get(format!("{}/api/v1/auth/status", f.base))
        .header("host", "foreign.example")
        .header("x-forwarded-host", "localhost:18080")
        .send()
        .await
        .unwrap();
    assert_eq!(denied_host.status(), 403);

    let denied_origin = f
        .client
        .post(format!("{}/api/v1/auth/setup/start", f.base))
        .header("host", "localhost:18080")
        .header("origin", "https://foreign.example")
        .header("x-forwarded-host", "localhost:18080")
        .header("x-forwarded-proto", "http")
        .json(&json!({"token":"invalid","name":"Foreign"}))
        .send()
        .await
        .unwrap();
    assert_eq!(denied_origin.status(), 403);
}

async fn fixture_with_listener() -> Fixture {
    let mut f = Fixture::empty();
    f.bind_ip = Some("127.0.0.2");
    f.start().await;
    assert!(f.base.starts_with("http://127.0.0.2:"));
    f
}

pub(super) const OPERATIONS: &[&str] = &["machines.onboarding", "machines.install"];

#[tokio::test]
async fn colocated_machine_waits_for_first_owner_and_connects_as_pool() {
    use calternal_machine::{identity::Credentials, protocol::Owner};
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    struct MachineChild(Child);
    impl Drop for MachineChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    let mut f = Fixture::empty();
    let handoff = f.dir.path().join("machine-handoff");
    let machine = f.dir.path().join("machine");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&handoff)
        .unwrap();
    f.start().await;
    let _child = MachineChild(
        Command::new(std::env::var("CALTERNAL_SERVER_BIN").unwrap())
            .args(["machine", "run", "--podman", "/nonexistent"])
            .arg("--dir")
            .arg(&machine)
            .arg("--bootstrap-handoff")
            .arg(&handoff)
            .arg("--bootstrap-server")
            .arg(&f.base)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let anonymous = f
        .req(
            "POST",
            "/api/v1/operations",
            None,
            json!({
                "operation":"machines.onboarding", "request_id":"anonymous-onboarding", "input":{}
            }),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(anonymous.status, 401);

    let request = handoff.join("request.json");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !request.is_file() {
        assert!(
            Instant::now() < deadline,
            "machine did not publish the handoff"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        std::fs::metadata(&request).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(!handoff.join("enrolled").exists());
    assert!(!Credentials::path(&machine).exists());

    let token = std::fs::read_to_string(f.appdata().join("setup.token")).unwrap();
    f.enroll(&token).await;
    let deadline = Instant::now() + Duration::from_secs(15);
    let data = loop {
        let reply = f.op(0, "machines.onboarding", json!({})).await;
        reply.ok();
        let data = reply.body["data"].clone();
        if data["ready"] == true {
            break data;
        }
        assert!(Instant::now() < deadline, "machine did not connect: {data}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    assert_eq!(data["attached"], true);
    let credentials = Credentials::load(&machine).unwrap();
    assert_eq!(data["machine_id"], credentials.machine_id.to_string());
    assert_eq!(credentials.owner, Owner::Pool);
    assert!(!request.exists());
    assert!(!handoff.join("enrolled").exists());

    let personal = f.op(0, "spaces.list", json!({})).await;
    personal.ok();
    let space = &personal.body["data"]["spaces"][0]["id"];
    let readiness = f.op(0, "agent.readiness", json!({"space_id":space})).await;
    readiness.ok();
    assert!(
        readiness.body["data"]["machines"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(readiness.body["data"]["pool"].as_array().unwrap().len(), 1);

    f.stop();
    f.start().await;
    let restarted = f.op(0, "machines.onboarding", json!({})).await;
    restarted.ok();
    assert_eq!(restarted.body["data"]["machine_id"], data["machine_id"]);
}

#[tokio::test]
async fn onboarding_refuses_other_accounts_and_thread_tokens() {
    let f = Fixture::new().await;
    assert_eq!(f.op(1, "machines.onboarding", json!({})).await.status, 403);
    let mut machine = requests_cases::Machine::enroll(&f).await;
    f.op(1, "agent.run", json!({
        "space_id": f.spaces[0], "thread_id": uuid::Uuid::now_v7(), "prompt":"Onboarding isolation"
    })).await.ok();
    let token = machine.next_token(&f).await;
    let (status, _) = requests_cases::agent(
        &f,
        &token,
        "onboarding-thread-token",
        "machines.onboarding",
        json!({}),
    )
    .await;
    assert_eq!(status, 403);

    let owner = f.op(0, "machines.onboarding", json!({})).await;
    owner.ok();
    assert_eq!(
        owner.body["data"],
        json!({"attached":false,"ready":false,"machine_id":null})
    );
}

#[tokio::test]
async fn personal_machine_installer_uses_a_private_one_time_code() {
    let f = Fixture::new().await;
    let install = f.op(1, "machines.install", json!({})).await;
    install.ok();
    assert_eq!(
        install.headers.get("cache-control").map(String::as_str),
        Some("no-store")
    );
    let data = &install.body["data"];
    assert!(
        data["expires_ms"].as_i64().unwrap()
            > std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64
    );
    let script = data["script"].as_str().unwrap();
    assert!(script.starts_with("#!/bin/sh\n"));
    assert!(script.contains("launchctl bootstrap"));
    assert!(script.contains("systemctl --user restart"));
    let code = script
        .split(" code='")
        .nth(1)
        .unwrap()
        .split('\'')
        .next()
        .unwrap();
    let mut machine = requests_cases::Machine::enroll_with_code(&f, code).await;
    let other = f.op(2, "machines.install", json!({})).await;
    other.ok();
    assert!(!other.body.to_string().contains(code));
    let machine_id = machine.id.to_string();
    for (account, expected) in [(1, true), (2, false)] {
        let result = f
            .req(
                "GET",
                "/api/v1/machines",
                Some(&f.cookies[account]),
                json!({}),
                Some(ORIGIN),
            )
            .await;
        result.ok();
        assert_eq!(result.body.to_string().contains(&machine_id), expected);
    }
    let key = calternal_machine::identity::MachineKey::generate().unwrap();
    let replay = f.machine("/api/v1/machine/enroll", None, json!({
        "code": code, "public_key":key.public_key_b64(), "name":"replay", "os":"linux", "arch":"x86_64", "direct":false
    })).await;
    assert_eq!(replay.status, 404);
    f.op(
        1,
        "agent.run",
        json!({
            "space_id":f.spaces[0], "thread_id":uuid::Uuid::now_v7(), "prompt":"Installer isolation"
        }),
    )
    .await
    .ok();
    let token = machine.next_token(&f).await;
    let (status, body) = requests_cases::agent(
        &f,
        &token,
        "installer-thread-token",
        "machines.install",
        json!({}),
    )
    .await;
    assert_eq!(status, 403);
    assert!(!body.to_string().contains("script"));
}
