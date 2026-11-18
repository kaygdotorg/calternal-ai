//! People operations through HTTP with isolated real passkey sessions.
use super::*;

mod invites;

pub(crate) const ADDITIONAL_OPERATIONS: &[&str] = &[
    "account.set_profile",
    "account.set_time_zone",
    "groups.add_member",
    "groups.create",
    "groups.delete",
    "groups.list",
    "groups.remove_member",
    "groups.rename",
    "invites.create",
    "invites.list",
    "invites.redeem",
    "invites.revoke",
    "people.disable",
    "people.list",
    "roles.list",
    "spaces.add_member",
];

#[tokio::test]
async fn people_catalog_and_admin_operations_preserve_account_scope() {
    let mut f = Fixture::new().await;
    let roles = f.op(3, "roles.list", json!({})).await;
    roles.ok();
    let roles = roles.body["data"]["roles"].as_array().unwrap();
    assert_eq!(roles.len(), 4);
    assert!(roles.iter().any(|r| {
        r["name"] == "owner"
            && r["capabilities"]
                .as_array()
                .unwrap()
                .contains(&json!("space.transfer"))
    }));
    assert!(roles.iter().any(|r| {
        r["name"] == "manager"
            && !r["capabilities"]
                .as_array()
                .unwrap()
                .contains(&json!("space.transfer"))
    }));
    let anonymous = f
        .req(
            "POST",
            "/api/v1/operations",
            None,
            json!({"operation":"roles.list","request_id":"no-session","input":{}}),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(anonymous.status, 401);

    let name = f
        .op(
            3,
            "account.set_profile",
            json!({"name":"  Renamed account  "}),
        )
        .await;
    name.ok();
    assert_eq!(name.body["data"]["name"], "Renamed account");
    assert_eq!(name.body["data"]["account_id"], f.ids[3]);
    let foreign = f
        .op(
            3,
            "account.set_profile",
            json!({"name":"Cross account","account_id":f.ids[0]}),
        )
        .await;
    let unknown = f
        .op(
            3,
            "account.set_profile",
            json!({"name":"Cross account","account_id":uuid::Uuid::now_v7()}),
        )
        .await;
    assert_eq!(foreign.status, 400);
    assert_eq!(foreign.signature(), unknown.signature());

    let group = f
        .op_id(0, "groups.create", json!({"name":"Plans"}), "create-group")
        .await;
    group.ok();
    let repeated = f
        .op_id(0, "groups.create", json!({"name":"Plans"}), "create-group")
        .await;
    repeated.ok();
    assert_eq!(repeated.body, group.body);
    let group_id = &group.body["data"]["group_id"];
    let absent = uuid::Uuid::now_v7();
    for (op, actual, missing) in [
        ("groups.list", json!({}), json!({})),
        (
            "groups.create",
            json!({"name":"Hidden"}),
            json!({"name":"Hidden"}),
        ),
        (
            "groups.rename",
            json!({"group_id":group_id,"name":"New plan"}),
            json!({"group_id":absent,"name":"New plan"}),
        ),
        (
            "groups.delete",
            json!({"group_id":group_id}),
            json!({"group_id":absent}),
        ),
        (
            "groups.add_member",
            json!({"group_id":group_id,"account_id":f.ids[3]}),
            json!({"group_id":absent,"account_id":f.ids[3]}),
        ),
        (
            "groups.remove_member",
            json!({"group_id":group_id,"account_id":f.ids[3]}),
            json!({"group_id":absent,"account_id":f.ids[3]}),
        ),
        ("people.list", json!({}), json!({})),
        (
            "people.disable",
            json!({"account_id":f.ids[3]}),
            json!({"account_id":absent}),
        ),
    ] {
        let a = f.op(2, op, actual).await;
        let b = f.op(2, op, missing).await;
        assert_eq!(a.status, 404, "{op}: {}", a.body);
        assert_eq!(a.signature(), b.signature(), "{op}");
    }

    f.op(
        0,
        "groups.rename",
        json!({"group_id":group_id,"name":"Contributor plan"}),
    )
    .await
    .ok();
    f.op(
        0,
        "groups.add_member",
        json!({"group_id":group_id,"account_id":f.ids[3]}),
    )
    .await
    .ok();
    let groups = f.op(0, "groups.list", json!({})).await;
    groups.ok();
    assert_eq!(groups.body["data"]["groups"][0]["name"], "Contributor plan");
    assert_eq!(
        groups.body["data"]["groups"][0]["members"],
        json!([f.ids[3]])
    );
    f.op(
        0,
        "groups.remove_member",
        json!({"group_id":group_id,"account_id":f.ids[3]}),
    )
    .await
    .ok();
    f.op(0, "groups.delete", json!({"group_id":group_id}))
        .await
        .ok();
    assert_eq!(
        f.op(0, "groups.list", json!({})).await.body["data"]["groups"],
        json!([])
    );

    let list = f.op(0, "people.list", json!({})).await;
    list.ok();
    assert!(
        list.body["data"]["people"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["account_id"] == f.ids[3] && p["name"] == "Renamed account")
    );
    assert_eq!(
        f.op(0, "people.disable", json!({"account_id":f.ids[0]}))
            .await
            .status,
        403
    );
    f.op(0, "people.disable", json!({"account_id":f.ids[3]}))
        .await
        .ok();
    assert_eq!(f.op(3, "roles.list", json!({})).await.status, 401);
    assert_eq!(f.login(3).await.status, 404);
    let list = f.op(0, "people.list", json!({})).await;
    assert!(
        list.body["data"]["people"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["account_id"] == f.ids[3] && p["disabled"] == true)
    );
}

#[tokio::test]
async fn space_people_grants_hide_targets_and_update_effective_access_immediately() {
    let f = Fixture::new().await;
    let private = f
        .op(0, "spaces.create", json!({"name":"Shared with a plan"}))
        .await;
    private.ok();
    let space = &private.body["data"]["space"]["id"];
    let absent = uuid::Uuid::now_v7();
    let group = f
        .op(0, "groups.create", json!({"name":"Shared plan"}))
        .await;
    group.ok();
    let group_id = &group.body["data"]["group_id"];

    for (op, a, b) in [
        (
            "spaces.members",
            json!({"space_id":space}),
            json!({"space_id":absent}),
        ),
        (
            "spaces.add_member",
            json!({"space_id":space,"group_id":group_id,"role":"reader"}),
            json!({"space_id":absent,"group_id":group_id,"role":"reader"}),
        ),
        (
            "spaces.set_role",
            json!({"space_id":space,"account_id":f.ids[3],"role":"reader"}),
            json!({"space_id":absent,"account_id":f.ids[3],"role":"reader"}),
        ),
        (
            "spaces.remove_member",
            json!({"space_id":space,"group_id":group_id}),
            json!({"space_id":absent,"group_id":group_id}),
        ),
        (
            "spaces.transfer",
            json!({"space_id":space,"account_id":f.ids[3]}),
            json!({"space_id":absent,"account_id":f.ids[3]}),
        ),
        (
            "invites.create",
            json!({"space_id":space,"role":"reader"}),
            json!({"space_id":absent,"role":"reader"}),
        ),
        (
            "invites.list",
            json!({"space_id":space}),
            json!({"space_id":absent}),
        ),
        (
            "invites.revoke",
            json!({"space_id":space,"invite_id":absent}),
            json!({"space_id":absent,"invite_id":absent}),
        ),
    ] {
        let a = f.op(3, op, a).await;
        let b = f.op(3, op, b).await;
        assert_eq!(a.status, 404, "{op}: {}", a.body);
        assert_eq!(a.signature(), b.signature(), "{op}");
    }

    f.op(
        0,
        "spaces.add_member",
        json!({"space_id":space,"account_id":f.ids[2],"role":"manager"}),
    )
    .await
    .ok();
    let hidden = f
        .op(
            2,
            "spaces.add_member",
            json!({"space_id":space,"account_id":f.ids[3],"role":"reader"}),
        )
        .await;
    let missing = f
        .op(
            2,
            "spaces.add_member",
            json!({"space_id":space,"account_id":absent,"role":"reader"}),
        )
        .await;
    assert_eq!(hidden.status, 404);
    assert_eq!(hidden.signature(), missing.signature());
    assert_eq!(
        f.op(
            2,
            "spaces.transfer",
            json!({"space_id":space,"account_id":f.ids[2]})
        )
        .await
        .status,
        403
    );
    assert_eq!(
        f.op(
            2,
            "spaces.remove_member",
            json!({"space_id":space,"account_id":f.ids[0]})
        )
        .await
        .status,
        403
    );
    assert_eq!(
        f.op(
            2,
            "invites.create",
            json!({"space_id":space,"role":"manager"})
        )
        .await
        .status,
        403
    );
    assert_eq!(
        f.op(
            0,
            "spaces.add_member",
            json!({"space_id":space,"group_id":group_id,"role":"owner"})
        )
        .await
        .status,
        400
    );

    f.op(
        0,
        "groups.add_member",
        json!({"group_id":group_id,"account_id":f.ids[3]}),
    )
    .await
    .ok();
    f.op(
        0,
        "spaces.add_member",
        json!({"space_id":space,"group_id":group_id,"role":"reader"}),
    )
    .await
    .ok();
    let members = f.op(3, "spaces.members", json!({"space_id":space})).await;
    members.ok();
    assert_eq!(
        members.body["data"]["groups"][0],
        json!({"group_id":group_id,"name":"Shared plan","role":"reader","member_count":1})
    );
    assert!(
        members.body["data"]["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["account_id"] == f.ids[2] && m["name"] == "Account B")
    );
    assert_eq!(
        f.op(
            3,
            "notes.create",
            json!({"space_id":space,"title":"Denied","markdown":"x"})
        )
        .await
        .status,
        403
    );

    assert_eq!(
        f.op(
            2,
            "spaces.set_role",
            json!({"space_id":space,"group_id":group_id,"role":"manager"})
        )
        .await
        .status,
        403
    );
    f.op(
        2,
        "spaces.set_role",
        json!({"space_id":space,"group_id":group_id,"role":"editor"}),
    )
    .await
    .ok();
    f.op(
        3,
        "notes.create",
        json!({"space_id":space,"title":"Inherited editor","markdown":"yes"}),
    )
    .await
    .ok();
    f.op(
        0,
        "groups.remove_member",
        json!({"group_id":group_id,"account_id":f.ids[3]}),
    )
    .await
    .ok();
    assert_eq!(
        f.op(3, "spaces.members", json!({"space_id":space}))
            .await
            .status,
        404
    );
    f.op(
        0,
        "groups.add_member",
        json!({"group_id":group_id,"account_id":f.ids[3]}),
    )
    .await
    .ok();
    f.op(
        2,
        "spaces.remove_member",
        json!({"space_id":space,"group_id":group_id}),
    )
    .await
    .ok();
    assert_eq!(
        f.op(3, "spaces.members", json!({"space_id":space}))
            .await
            .status,
        404
    );

    // Deleting a group revokes a live group grant without removing the account.
    f.op(
        0,
        "spaces.add_member",
        json!({"space_id":space,"group_id":group_id,"role":"reader"}),
    )
    .await
    .ok();
    f.op(3, "spaces.members", json!({"space_id":space}))
        .await
        .ok();
    f.op(0, "groups.delete", json!({"group_id":group_id}))
        .await
        .ok();
    assert_eq!(
        f.op(3, "spaces.members", json!({"space_id":space}))
            .await
            .status,
        404
    );

    f.op(
        0,
        "spaces.transfer",
        json!({"space_id":space,"account_id":f.ids[2]}),
    )
    .await
    .ok();
    let members = f.op(0, "spaces.members", json!({"space_id":space})).await;
    let owners: Vec<_> = members.body["data"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "owner")
        .collect();
    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0]["account_id"], f.ids[2]);
}

#[tokio::test]
async fn people_directories_bound_rows_and_continue_by_cursor() {
    let mut f = Fixture::new().await;
    f.stop();
    let db = Connection::open(f.appdata().join("system/auth.db")).unwrap();
    let large_group = uuid::Uuid::now_v7().to_string();
    db.execute(
        "INSERT INTO groups(id,name) VALUES (?1,'Large group')",
        [&large_group],
    )
    .unwrap();
    for n in 0..55 {
        let account = uuid::Uuid::now_v7().to_string();
        let group = uuid::Uuid::now_v7().to_string();
        let name = format!("Page fixture {n}");
        db.execute(
            "INSERT INTO users(id,name,username,created_at) VALUES (?1,?2,?1,0)",
            params![account, name],
        )
        .unwrap();
        db.execute(
            "INSERT INTO space_memberships VALUES (?1,?2,'reader')",
            params![f.spaces[0], account],
        )
        .unwrap();
        db.execute(
            "INSERT INTO groups(id,name) VALUES (?1,?2)",
            params![group, name],
        )
        .unwrap();
        db.execute(
            "INSERT INTO group_members(group_id,user_id) VALUES (?1,?2)",
            params![large_group, account],
        )
        .unwrap();
        db.execute(
            "INSERT INTO space_group_memberships VALUES (?1,?2,'reader')",
            params![f.spaces[0], group],
        )
        .unwrap();
    }
    drop(db);
    f.start().await;
    for (op, who, original, key) in [
        ("people.list", 0, json!({}), "people"),
        ("groups.list", 0, json!({}), "groups"),
        (
            "spaces.members",
            1,
            json!({"space_id":f.spaces[0]}),
            "members",
        ),
    ] {
        let first = f.op(who, op, original.clone()).await;
        first.ok();
        let records = first.body["data"][key].as_array().unwrap();
        assert_eq!(records.len(), 50, "{op}");
        let cursor = first.body["data"]["next_cursor"].as_str().unwrap();
        let mut continuation = original;
        continuation["cursor"] = json!(cursor);
        let second = f.op(who, op, continuation).await;
        second.ok();
        let remainder = second.body["data"][key].as_array().unwrap();
        assert!(!remainder.is_empty(), "{op}");
        assert!(remainder.iter().all(|row| !records.contains(row)), "{op}");
        if op == "spaces.members" {
            let first_groups = second.body["data"]["groups"].as_array().unwrap();
            let cursor = second.body["data"]["next_cursor"].as_str().unwrap();
            assert_eq!(remainder.len() + first_groups.len(), 50);
            let third = f
                .op(who, op, json!({"space_id":f.spaces[0],"cursor":cursor}))
                .await;
            third.ok();
            assert_eq!(third.body["data"]["members"], json!([]));
            let groups = third.body["data"]["groups"].as_array().unwrap();
            assert_eq!(first_groups.len() + groups.len(), 55);
            assert!(groups.iter().all(|row| !first_groups.contains(row)));
            assert!(third.body["data"]["next_cursor"].is_null());
            continue;
        }
        assert!(second.body["data"]["next_cursor"].is_null(), "{op}");
    }
    let first = f
        .op(0, "groups.list", json!({"group_id":large_group}))
        .await;
    first.ok();
    let group = &first.body["data"]["groups"][0];
    assert_eq!(group["members"].as_array().unwrap().len(), 50);
    let second = f
        .op(
            0,
            "groups.list",
            json!({"group_id":large_group,"cursor":group["members_next_cursor"]}),
        )
        .await;
    second.ok();
    assert_eq!(
        second.body["data"]["groups"][0]["members"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    let hidden = f
        .op(2, "groups.list", json!({"group_id":large_group}))
        .await;
    let missing = f
        .op(2, "groups.list", json!({"group_id":uuid::Uuid::now_v7()}))
        .await;
    assert_eq!(hidden.status, 404);
    assert_eq!(hidden.signature(), missing.signature());
}

#[tokio::test]
async fn account_time_zone_is_private_and_overrides_the_device_for_thread_headings() {
    let f = Fixture::new().await;
    let op = "account.set_time_zone";
    let unauthenticated = f.req("POST", "/api/v1/operations", None,
        json!({"operation":op,"request_id":"anonymous-zone","input":{"time_zone":"Asia/Tokyo"}}),
        Some(ORIGIN)).await;
    assert_eq!(unauthenticated.status, 401);
    for invalid in [
        json!({"time_zone":"Asia/Tokyo","account_id":f.ids[2]}),
        json!({"time_zone":"../localtime"}),
        json!({}),
    ] {
        assert_eq!(f.op(1, op, invalid).await.status, 400);
    }

    let input = json!({"time_zone":"asia/tokyo"});
    let saved = f.op_id(1, op, input.clone(), "set-zone").await;
    saved.ok();
    assert_eq!(
        saved.body["data"],
        json!({"account_id":f.ids[1],"time_zone":"Asia/Tokyo"})
    );
    let replay = f.op_id(1, op, input, "set-zone").await;
    replay.ok();
    assert_eq!(replay.body["data"], saved.body["data"]);
    for (actor, zone) in [(1, json!("Asia/Tokyo")), (2, Value::Null)] {
        let own = f
            .req(
                "GET",
                "/api/v1/auth/me",
                Some(&f.cookies[actor]),
                json!({}),
                Some(ORIGIN),
            )
            .await;
        own.ok();
        assert_eq!(own.body["user"]["time_zone"], zone);
    }

    let space = &f.spaces[2];
    let device = "Asia/Kolkata";
    let created = f
        .op(
            1,
            "threads.create",
            json!({"space_id":space,"title":"Time zone","client_time_zone":device}),
        )
        .await;
    created.ok();
    let id = &created.body["data"]["thread"]["id"];
    let sent = f
        .op(
            1,
            "threads.send",
            json!({"id":id,"message":"Account zone","client_time_zone":device}),
        )
        .await;
    sent.ok();
    assert!(
        sent.body["data"]["thread"]["messages"][0]["at"]
            .as_str()
            .unwrap()
            .ends_with("+09:00")
    );
    assert!(!sent.body.to_string().contains("Asia/Tokyo"));

    let peer = f.op(2, "threads.get", json!({"id":id})).await;
    peer.ok();
    assert!(!peer.body.to_string().contains("Asia/Tokyo"));
    f.op(1, op, json!({"time_zone":null})).await.ok();
    let second = f
        .op(
            1,
            "threads.create",
            json!({"space_id":space,"title":"Device fallback","client_time_zone":device}),
        )
        .await;
    second.ok();
    let sent = f
        .op(
            1,
            "threads.send",
            json!({"id":second.body["data"]["thread"]["id"],
        "message":"Device zone","client_time_zone":device,"client_offset_minutes":0}),
        )
        .await;
    sent.ok();
    assert!(
        sent.body["data"]["thread"]["messages"][0]["at"]
            .as_str()
            .unwrap()
            .ends_with("+05:30")
    );
}
