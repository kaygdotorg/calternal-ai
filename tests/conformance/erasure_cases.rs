//! Account and space erasure through the real HTTP authorization and restart path.
use super::requests_cases::{Machine, agent};
use super::requests_review_cases::{new_thread, start_turn};
use super::*;

const ROUTE: &str = "account/delete";

pub(super) async fn reverify(fixture: &mut Fixture, who: usize) {
    let start = fixture
        .auth("reverify/start", Some(&fixture.cookies[who]), json!({}))
        .await;
    start.ok();
    let credential = fixture.devices[who].sign(&start);
    let renewed = fixture
        .auth(
            "reverify/finish",
            Some(&fixture.cookies[who]),
            json!({"ceremony":start.body["ceremony"],"credential":credential}),
        )
        .await;
    renewed.ok();
    fixture.cookies[who] = renewed.cookie();
}

async fn personal(fixture: &Fixture, who: usize) -> String {
    let result = fixture.op(who, "spaces.list", json!({})).await;
    result.ok();
    result.body["data"]["spaces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|space| space["personal"] == true)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn erased(fixture: &Fixture, kind: &str, id: &str) {
    for root in ["appdata", "userdata-ssd", "userdata-hdd"] {
        assert!(
            !fixture.dir.path().join(root).join(kind).join(id).exists(),
            "{root}/{kind}/{id}"
        );
    }
    let c = Connection::open(fixture.appdata().join("system/auth.db")).unwrap();
    assert_eq!(
        c.query_row(
            "SELECT pending FROM owner_erasures WHERE kind=?1 AND id=?2",
            rusqlite::params![kind.trim_end_matches('s'), id],
            |r| r.get::<_, bool>(0),
        )
        .unwrap(),
        false
    );
}

#[tokio::test]
async fn account_erasure_requires_authority_fresh_proof_and_transfer() {
    const ACTOR_REPLY: &str = "erased-actor-receipt-bytes";
    const SCOPE_REPLY: &str = "erased-scope-receipt-bytes";
    let mut f = Fixture::new().await;
    let recovery = f.dir.path().join("userdata-ssd/.calternal/recovery");
    let reply_is_retained = |reply: &str| {
        std::fs::read_dir(&recovery).unwrap().any(|entry| {
            std::fs::read(entry.unwrap().path())
                .unwrap()
                .windows(reply.len())
                .any(|part| part == reply.as_bytes())
        })
    };
    let account = f.ids[1].clone();
    let private = f.spaces[0].clone();
    let shared = f.spaces[3].clone();
    let own = personal(&f, 1).await;
    let group_space = f
        .op(1, "spaces.create", json!({"name":"group-only grant"}))
        .await;
    group_space.ok();
    let group_space = group_space.body["data"]["space"]["id"].as_str().unwrap();
    let group = f
        .op(0, "groups.create", json!({"name":"Erasure group"}))
        .await;
    group.ok();
    let group = group.body["data"]["group_id"].as_str().unwrap();
    for who in [1, 2, 3] {
        f.op(
            0,
            "groups.add_member",
            json!({"group_id":group,"account_id":f.ids[who]}),
        )
        .await
        .ok();
    }
    f.op(
        1,
        "spaces.add_member",
        json!({"space_id":group_space,"group_id":group,"role":"reader"}),
    )
    .await
    .ok();
    let retained_invite = f
        .op(
            1,
            "invites.create",
            json!({"space_id":shared,"role":"reader"}),
        )
        .await;
    retained_invite.ok();
    let invite_id = retained_invite.body["data"]["invite_id"].clone();
    let links = f.op(1, "invites.list", json!({"space_id":shared})).await;
    links.ok();
    let link = links.body["data"]["invites"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["invite_id"] == invite_id)
        .unwrap()["link"]
        .as_str()
        .unwrap();
    let invite_secret = link.rsplit('/').next().unwrap();
    f.op(
        1,
        "spaces.add_member",
        json!({"space_id":shared,"account_id":f.ids[3],"role":"manager"}),
    )
    .await
    .ok();
    let mut journal = live_cases::Live::open(&f, 2, json!([{"space_id":shared,"bookmark":null}]))
        .await
        .ok()
        .unwrap();
    let bookmark = journal.next().await.unwrap()["bookmark"].clone();
    drop(journal);
    let shared_note = f.note(1, &shared).await;
    f.note(1, &own).await;
    f.note(1, &private).await;

    let refused = f
        .auth(ROUTE, Some(&f.cookies[2]), json!({"account_id":account}))
        .await;
    assert_eq!(refused.status, 403);
    f.stop();
    {
        let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
        c.execute(
            "UPDATE installations SET assertion_at=0 WHERE user_id IN (?1,?2)",
            [&account, &f.ids[0]],
        )
        .unwrap();
        for (actor, scope, reply) in [
            (&account, &shared, ACTOR_REPLY),
            (&f.ids[0], &own, SCOPE_REPLY),
        ] {
            c.execute(
                "INSERT INTO space_receipts VALUES(?1,?2,?3,'spaces.set_role','fp',json_object('marker',?3),unixepoch()*1000)",
                rusqlite::params![actor, scope, reply],
            )
            .unwrap();
        }
    }
    f.start().await;
    assert!(
        [ACTOR_REPLY, SCOPE_REPLY]
            .into_iter()
            .all(reply_is_retained)
    );
    let stale = f.auth(ROUTE, Some(&f.cookies[1]), json!({})).await;
    assert_eq!(stale.status, 403);
    assert_eq!(
        stale.body["error"]["code"],
        "passkey_reverification_required"
    );
    let stale_admin = f
        .auth(ROUTE, Some(&f.cookies[0]), json!({"account_id":account}))
        .await;
    assert_eq!(stale_admin.status, 403);
    assert_eq!(
        stale_admin.body["error"]["code"],
        "passkey_reverification_required"
    );
    reverify(&mut f, 0).await;
    reverify(&mut f, 1).await;
    for (who, selection) in [(1, json!({})), (0, json!({"account_id":account}))] {
        let refuse = f.auth(ROUTE, Some(&f.cookies[who]), selection).await;
        assert_eq!(refuse.status, 409, "{}", refuse.body);
        let blocked = refuse.body["error"]["details"]["spaces"]
            .as_array()
            .unwrap();
        assert!(
            blocked
                .iter()
                .any(|space| space["space_id"] == shared && space["name"] == "role editor")
        );
        assert!(
            blocked.iter().any(
                |space| space["space_id"] == group_space && space["name"] == "group-only grant"
            ),
            "group-only membership must also require transfer"
        );
        assert!(
            refuse.body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("calternal people transfer")
        );
    }
    let mut machine = Machine::enroll(&f).await;
    let thread = new_thread(&f, &shared).await;
    start_turn(&f, &shared, &thread, "Erase the turn's account").await;
    let token = machine.next_token(&f).await;
    let (status, approval) = agent(
        &f,
        &token,
        "before-erasure",
        "access.request",
        json!({"app":"notes"}),
    )
    .await;
    assert_eq!(status, 200, "{approval}");
    for space in &f.spaces[2..] {
        f.op(
            1,
            "spaces.transfer",
            json!({"space_id":space,"account_id":f.ids[2]}),
        )
        .await
        .ok();
    }
    f.op(
        1,
        "spaces.remove_member",
        json!({"space_id":group_space,"group_id":group}),
    )
    .await
    .ok();
    let result = f.auth(ROUTE, Some(&f.cookies[1]), json!({})).await;
    result.ok();
    assert_eq!(result.body, json!({"account_id":account,"deleted":true}));
    assert_eq!(
        f.req(
            "GET",
            "/api/v1/auth/me",
            Some(&f.cookies[1]),
            json!({}),
            Some(ORIGIN)
        )
        .await
        .status,
        401
    );
    let retained = f.op(3, "invites.list", json!({"space_id":shared})).await;
    retained.ok();
    let row = retained.body["data"]["invites"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["invite_id"] == invite_id)
        .unwrap();
    assert!(row["revoked_at"].is_number());
    assert!(row["link"].is_null());
    assert_eq!(
        f.op(4, "invites.redeem", json!({"secret":invite_secret}))
            .await
            .status,
        404
    );
    let machine_refused = f
        .machine(
            "/api/v1/machine/session/start",
            None,
            json!({"machine_id":machine.id}),
        )
        .await;
    assert_eq!(machine_refused.status, 404);
    let (status, _) = agent(
        &f,
        &token,
        "after-erasure",
        "access.request",
        json!({"app":"notes"}),
    )
    .await;
    assert_eq!(status, 401);
    {
        let auth = Connection::open(f.appdata().join("system/auth.db")).unwrap();
        for table in ["agent_approvals", "agent_access"] {
            let rows: i64 = auth
                .query_row(
                    &format!("SELECT count(*) FROM {table} WHERE account=?1"),
                    [&account],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(rows, 0, "erased account retained rows in {table}");
        }
    }
    erased(&f, "accounts", &account);
    erased(&f, "spaces", &own);
    erased(&f, "spaces", &private);
    erased(&f, "spaces", group_space);
    assert!(
        ![ACTOR_REPLY, SCOPE_REPLY]
            .into_iter()
            .any(reply_is_retained)
    );
    let kept = f.op(2, "notes.get", json!({"id":shared_note["id"]})).await;
    kept.ok();
    assert_eq!(
        kept.body["data"]["note"]["markdown"],
        shared_note["markdown"]
    );
    let mut journal =
        live_cases::Live::open(&f, 2, json!([{"space_id":shared,"bookmark":bookmark}]))
            .await
            .ok()
            .unwrap();
    loop {
        let frame = journal.next().await.expect("historical note event");
        assert_ne!(frame["type"], "ready", "shared note event missing");
        if frame["entry"]["item"] == shared_note["id"] {
            assert_eq!(frame["entry"]["author"], account);
            assert_eq!(frame["entry"]["author_label"], "a deleted account");
            break;
        }
    }
    drop(journal);
    assert!(
        machine_admin(&f, &["list"])["pending_erasures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["machine_id"] == machine.id.to_string() && row["operator_wipe"] == true)
    );
    let page = f.op(2, "spaces.members", json!({"space_id":shared})).await;
    page.ok();
    assert!(
        page.body["data"]["members"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["account_id"] != account)
    );
    // Exercise the CLI against the same route with an administrator's live
    // installation. The fixture token file is private and removed on drop.
    use std::io::Write as _;
    let mut token = tempfile::NamedTempFile::new_in(f.dir.path()).unwrap();
    token
        .write_all(f.cookies[0].split_once('=').unwrap().1.as_bytes())
        .unwrap();
    let binary = std::env::var("CALTERNAL_SERVER_BIN").unwrap();
    for (who, machine_output) in [(3, true), (4, false)] {
        let mut cmd = Command::new(&binary);
        cmd.args([
            "people",
            "delete",
            "--account",
            &f.ids[who],
            "--base-url",
            &f.base,
            "--token-file",
        ])
        .arg(token.path());
        if machine_output {
            cmd.arg("--json");
        }
        let response = cmd.output().unwrap();
        let output = String::from_utf8(response.stdout).unwrap();
        assert!(response.status.success(), "{output}");
        if machine_output {
            let parsed: Value = serde_json::from_str(&output).unwrap();
            assert_eq!(
                parsed["data"],
                json!({"account_id":f.ids[who],"deleted":true})
            );
        } else {
            assert!(output.contains("Deleted"));
            assert!(output.contains(&f.ids[who]));
        }
    }
}

#[tokio::test]
async fn account_erasure_recovers_after_authority_key_and_tree_crashes() {
    for phase in [
        "erasure_authority_committed",
        "erasure_recorded",
        "erasure_key_removed",
        "erasure_storage_removed",
    ] {
        let mut f = Fixture::new().await;
        let id = f.ids[3].clone();
        let space = personal(&f, 3).await;
        f.note(3, &space).await;
        f.op(
            3,
            "agent.settings",
            json!({"retry_settings":{"provider_errors":true,"subscription_limits":false}}),
        )
        .await
        .ok();
        let root = f.appdata().join("accounts").join(&id);
        assert!(root.join("agent-settings.db").is_file());

        reverify(&mut f, 3).await;
        let pending = f
            .op(
                3,
                "secrets.request",
                json!({"name":"erasure-test","label":"Erasure test","reason":"crash matrix"}),
            )
            .await;
        pending.ok();
        f.auth("secrets/provide", Some(&f.cookies[3]),
            json!({"request_id":pending.body["data"]["request_id"],"value":"disposable-erasure-test"})).await.ok();
        assert!(root.join("secret.key").is_file());
        f.stop();
        let crash = f.dir.path().join("content-fault");
        std::fs::write(&crash, phase).unwrap();
        f.start().await;
        let response = f
            .client
            .post(format!("{}/api/v1/auth/{ROUTE}", f.base))
            .header("host", "localhost:18080")
            .header("origin", ORIGIN)
            .header("cookie", &f.cookies[0])
            .json(&json!({"account_id":id}))
            .send()
            .await;
        assert!(response.is_err(), "{phase} did not interrupt the response");
        f.stop();
        std::fs::remove_file(crash).unwrap();
        f.start().await;
        assert_eq!(
            f.req(
                "GET",
                "/api/v1/auth/me",
                Some(&f.cookies[3]),
                json!({}),
                Some(ORIGIN)
            )
            .await
            .status,
            401
        );
        assert!(f.login(3).await.status >= 400, "deleted passkey signed in");
        erased(&f, "accounts", &id);
        let auth = Connection::open(f.appdata().join("system/auth.db")).unwrap();
        assert!(
            auth.query_row(
                "SELECT erasing FROM owner_secret_keys WHERE kind='account' AND owner_id=?1",
                [&id],
                |r| r.get::<_, bool>(0)
            )
            .unwrap()
        );
        erased(&f, "spaces", &space);
    }
}

#[tokio::test]
async fn space_erasure_removes_all_owned_roots_and_stays_absent_after_restart() {
    let mut f = Fixture::new().await;
    let space = f.spaces[0].clone();
    let note = f.note(1, &space).await;
    let hdd = f.dir.path().join("userdata-hdd/spaces").join(&space);
    std::fs::create_dir_all(&hdd).unwrap();
    std::fs::write(hdd.join("owned.md"), "owner-only file").unwrap();
    let input = json!({"space_id":space});
    let deleted = f
        .op_id(1, "spaces.delete", input.clone(), "erase-space")
        .await;
    deleted.ok();
    erased(&f, "spaces", &space);
    f.stop();
    f.start().await;
    let replay = f
        .op_id(1, "spaces.delete", input.clone(), "erase-space")
        .await;
    replay.ok();
    assert_eq!(replay.body["data"], deleted.body["data"]);
    assert_eq!(
        f.op_id(2, "spaces.delete", input, "erase-space")
            .await
            .status,
        404
    );
    assert_eq!(
        f.op(1, "notes.get", json!({"id":note["id"]})).await.status,
        404
    );
    erased(&f, "spaces", &space);
}

pub(super) fn machine_admin(f: &Fixture, args: &[&str]) -> Value {
    let result = Command::new(std::env::var_os("CALTERNAL_SERVER_BIN").unwrap())
        .args(["--json", "machines"])
        .args(args)
        .arg("--appdata")
        .arg(f.appdata())
        .arg("--userdata-ssd")
        .arg(f.dir.path().join("userdata-ssd"))
        .arg("--userdata-hdd")
        .arg(f.dir.path().join("userdata-hdd"))
        .args(["--installation-id", "00000000-0000-0000-0000-000000000001"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    serde_json::from_slice::<Value>(&result.stdout).unwrap()["data"].clone()
}

#[tokio::test]
async fn pool_erasures_survive_replay_and_require_a_machine_acknowledgement() {
    let mut f = Fixture::new().await;
    let issued = machine_admin(&f, &["code", "--pool"]);
    let mut pool = Machine::enroll_with_code(&f, issued["code"].as_str().unwrap()).await;
    machine_admin(&f, &["pool-grant", &f.ids[1]]);
    let account = f.ids[1].clone();
    let space = f.spaces[0].clone();
    let thread = new_thread(&f, &space).await;
    let job = start_turn(&f, &space, &thread, "Populate a pool workspace").await;
    let _ = pool.next_token(&f).await;
    f.op(1, "agent.stop", json!({"job_id":job})).await.ok();

    f.stop();
    {
        // Simulate expired lease history; erasure must use durable placement
        // provenance rather than an ended lease or live registry entry.
        let c = Connection::open(f.appdata().join("system/machines.db")).unwrap();
        assert!(
            c.execute(
                "DELETE FROM leases WHERE space=?1 AND state='ended'",
                [&space]
            )
            .unwrap()
                > 0
        );
    }
    let crash = f.dir.path().join("content-fault");
    std::fs::write(&crash, "erasure_recorded").unwrap();
    f.start().await;
    let response = f.client.post(format!("{}/api/v1/operations", f.base))
        .header("host", "localhost:18080").header("origin", ORIGIN)
        .header("cookie", &f.cookies[1])
        .json(&json!({"request_id":uuid::Uuid::now_v7(),"operation":"spaces.delete","input":{"space_id":space}}))
        .send().await;
    assert!(response.is_err(), "expected the live erasure fault");
    f.stop();
    std::fs::write(&crash, "erasure_machines_queued").unwrap();
    f.start_checked(true, true).await;
    f.stop();
    let request_id: String = {
        let c = Connection::open(f.appdata().join("system/machines.db")).unwrap();
        c.query_row(
            "SELECT request_id FROM machine_erasures WHERE machine=?1 AND thread=?2",
            rusqlite::params![pool.id.to_string(), thread.as_str().unwrap()],
            |row| row.get(0),
        )
        .unwrap()
    };
    std::fs::remove_file(crash).unwrap();
    f.start().await;
    erased(&f, "spaces", &space);
    assert_eq!(pool.erasure_status(&f, false).await.status, 401);
    let status = pool.erasure_status(&f, true).await;
    status.ok();
    assert_eq!(status.body["pending_erasures"], 1);
    let pending = machine_admin(&f, &["list"]);
    assert_eq!(
        pending["pending_erasures"],
        json!([
            {"machine_id":pool.id,"pending":1,"operator_wipe":false}
        ])
    );

    pool.connect(&f).await;
    let messages = pool.sync(&f, json!([])).await;
    let erase = messages
        .iter()
        .find(|message| message["type"] == "erase")
        .unwrap();
    assert_eq!(erase["account_id"], account);
    assert_eq!(erase["thread_id"], thread);
    assert_eq!(
        uuid::Uuid::parse_str(erase["request_id"].as_str().unwrap())
            .unwrap()
            .simple()
            .to_string(),
        request_id
    );
    let replayed = pool.sync(&f, json!([])).await;
    assert!(
        replayed.iter().any(|message| message == erase),
        "transport acknowledgement must not remove durable erasure"
    );
    let _ = pool
        .sync(
            &f,
            json!([{"request_id":erase["request_id"],"success":true}]),
        )
        .await;
    assert_eq!(
        pool.erasure_status(&f, true).await.body["pending_erasures"],
        0
    );

    for space in &f.spaces[2..] {
        f.op(
            1,
            "spaces.transfer",
            json!({"space_id":space,"account_id":f.ids[2]}),
        )
        .await
        .ok();
    }
    reverify(&mut f, 0).await;
    f.auth(ROUTE, Some(&f.cookies[0]), json!({"account_id":account}))
        .await
        .ok();
    erased(&f, "accounts", &account);
    let messages = pool.sync(&f, json!([])).await;
    let erase = messages
        .iter()
        .find(|message| message["type"] == "erase")
        .unwrap();
    assert_eq!(erase["account_id"], account);
    assert!(
        erase["thread_id"].is_null(),
        "the entire pool account tree must be erased"
    );
    let _ = pool
        .sync(
            &f,
            json!([{"request_id":erase["request_id"],"success":true}]),
        )
        .await;
    assert_eq!(
        pool.erasure_status(&f, true).await.body["pending_erasures"],
        0
    );
}
