//! Item sharing over real cookie sessions and the anonymous link transport.

use super::*;

pub(super) const OPERATIONS: &[&str] = &[
    "collections.create",
    "collections.get",
    "collections.list",
    "collections.set_members",
    "sharing.audiences",
    "sharing.grant",
    "sharing.link_copy",
    "sharing.link_create",
    "sharing.link_update",
    "sharing.list",
    "sharing.mark_seen",
    "sharing.rescue",
    "sharing.revoke",
    "sharing.set_link_policy",
    "sharing.shared_with_me",
];

fn data(answer: Answer) -> Value {
    answer.ok();
    answer.body["data"].clone()
}

#[tokio::test]
async fn bearer_open_discloses_item_address_only_to_an_independently_authorized_session() {
    let f = Fixture::new().await;
    let space = &f.spaces[0];
    let original = data(
        f.op(
            1,
            "notes.create",
            json!({
                "space_id":space, "title":"Sharing probe", "markdown":"Unique bearing compass"
            }),
        )
        .await,
    );
    let item = json!({"kind":"notes","id":original["note"]["id"]});
    let link = data(
        f.op(
            1,
            "sharing.link_create",
            json!({"item":item,"label":"Guest"}),
        )
        .await,
    );
    let copied = data(
        f.op(
            1,
            "sharing.link_copy",
            json!({"item":item,"grant_id":link["grant_id"]}),
        )
        .await,
    );
    let path = url::Url::parse(copied["url"].as_str().unwrap())
        .unwrap()
        .path()
        .to_owned();
    let get = |who: Option<usize>| {
        let mut request = f
            .client
            .get(format!("{}{}", f.base, path))
            .header("host", "localhost:18080")
            .header("accept", "application/json");
        if let Some(who) = who {
            request = request.header("cookie", &f.cookies[who]);
        }
        async move {
            let response = request.send().await.unwrap();
            let status = response.status().as_u16();
            (status, response.json::<Value>().await.unwrap())
        }
    };
    let (status, anonymous) = get(None).await;
    assert_eq!(status, 200);
    assert_eq!(
        anonymous,
        json!({"title":"Sharing probe","markdown":"Unique bearing compass"})
    );
    assert_eq!(get(Some(2)).await, (200, anonymous));
    assert_eq!(get(Some(1)).await.1["item"], item);
}

#[tokio::test]
async fn collection_lists_are_space_scoped_and_membership_editing_is_owner_only() {
    let f = Fixture::new().await;
    let space = &f.spaces[0];
    let first = data(
        f.op(
            1,
            "collections.create",
            json!({
                "space_id":space,"title":"First saved set","members":[]
            }),
        )
        .await,
    );
    let second = data(
        f.op(
            1,
            "collections.create",
            json!({
                "space_id":space,"title":"Second saved set","members":[]
            }),
        )
        .await,
    );
    let one = data(
        f.op(1, "collections.list", json!({"space_id":space,"limit":1}))
            .await,
    );
    assert_eq!(one["items"].as_array().unwrap().len(), 1);
    assert_eq!(one["items"][0]["can_edit"], true);
    let cursor = one["next_cursor"].as_str().unwrap();
    let two = data(
        f.op(
            1,
            "collections.list",
            json!({
                "space_id":space,"limit":1,"cursor":cursor
            }),
        )
        .await,
    );
    assert!(two.get("next_cursor").is_none());
    let ids = [&one["items"][0]["id"], &two["items"][0]["id"]];
    assert_ne!(ids[0], ids[1]);
    assert!(ids.contains(&&first["id"]) && ids.contains(&&second["id"]));
    let wrong_scope = f
        .op(
            1,
            "collections.list",
            json!({
                "space_id":f.spaces[2],"limit":1,"cursor":cursor
            }),
        )
        .await;
    assert_eq!(wrong_scope.status, 400);

    let group = data(
        f.op(
            0,
            "groups.create",
            json!({"name":"Collection test recipients"}),
        )
        .await,
    );
    for account in [&f.ids[1], &f.ids[2]] {
        f.op(
            0,
            "groups.add_member",
            json!({"group_id":group["group_id"],"account_id":account}),
        )
        .await
        .ok();
    }
    let id = &first["id"];
    f.op(
        1,
        "sharing.grant",
        json!({
            "item":{"kind":"collections","id":id},
            "audience":{"account_id":f.ids[2]},
            "permission":"edit"
        }),
    )
    .await
    .ok();
    let owner = data(f.op(1, "collections.get", json!({"id":id})).await);
    let recipient = data(f.op(2, "collections.get", json!({"id":id})).await);
    assert_eq!(owner["can_edit"], true);
    assert_eq!(recipient["can_edit"], false);
    let shared = data(
        f.op(2, "sharing.shared_with_me", json!({"kind":"collections"}))
            .await,
    );
    assert_eq!(shared["items"][0]["item"]["id"], *id);
    assert_eq!(shared["items"][0]["permission"], "edit");
    assert_eq!(shared["items"][0]["can_edit"], false);
    let private = f.op(2, "collections.list", json!({"space_id":space})).await;
    assert_eq!(private.status, 404);
}

#[tokio::test]
async fn collection_guest_member_addresses_never_retarget_and_revoke_with_the_root() {
    let f = Fixture::new().await;
    let space = &f.spaces[0];
    let make_note = |title| {
        let f = &f;
        async move {
            data(
                f.op(
                    1,
                    "notes.create",
                    json!({
                        "space_id":space, "title":title, "markdown":"Shared collection member"
                    }),
                )
                .await,
            )["note"]["id"]
                .clone()
        }
    };
    let first = make_note("Original member").await;
    let second = make_note("Replacement member").await;
    let collection = data(
        f.op(
            1,
            "collections.create",
            json!({
                "space_id":space,"title":"A guest collection","members":[first,second]
            }),
        )
        .await,
    );
    let item = json!({"kind":"collections","id":collection["id"]});
    let link = data(
        f.op(
            1,
            "sharing.link_create",
            json!({"item":item,"label":"Guest"}),
        )
        .await,
    );
    let copy = data(
        f.op(
            1,
            "sharing.link_copy",
            json!({
                "item":item, "grant_id":link["grant_id"]
            }),
        )
        .await,
    );
    let root = url::Url::parse(copy["url"].as_str().unwrap())
        .unwrap()
        .path()
        .to_owned();
    let get = |path: &str, signed_in: bool| {
        let mut req = f
            .client
            .get(format!("{}{path}", f.base))
            .header("host", "localhost:18080")
            .header("accept", "application/json");
        if signed_in {
            req = req.header("cookie", &f.cookies[1]);
        }
        async move {
            let reply = req.send().await.unwrap();
            if reply.status().is_success() {
                Ok(reply.json::<Value>().await.unwrap())
            } else {
                Err(reply.status().as_u16())
            }
        }
    };
    let guest = get(&root, false).await.unwrap();
    assert_eq!(guest["title"], "A guest collection");
    assert!(guest.get("item").is_none());
    assert_eq!(get(&root, true).await.unwrap()["item"], item);
    let original = guest["members"][0]["href"].as_str().unwrap();
    assert_eq!(
        get(original, false).await.unwrap(),
        json!({
            "title":"Original member", "markdown":"Shared collection member","back":root
        })
    );
    assert_eq!(
        get(original, true).await.unwrap()["item"],
        json!({"kind":"notes","id":first})
    );
    assert!(!original.contains(first.as_str().unwrap()));

    f.op(
        1,
        "collections.set_members",
        json!({
            "id":collection["id"],"base_revision":collection["revision"],"members":[second]
        }),
    )
    .await
    .ok();
    assert_eq!(get(original, false).await, Err(404));
    let current = get(&root, false).await.unwrap();
    let replacement = current["members"][0]["href"].as_str().unwrap();
    assert_eq!(
        get(replacement, false).await.unwrap()["title"],
        "Replacement member"
    );

    f.op(
        1,
        "sharing.revoke",
        json!({"item":item, "grant_id":link["grant_id"]}),
    )
    .await
    .ok();
    assert_eq!(get(&root, false).await, Err(404));
    assert_eq!(get(replacement, false).await, Err(404));
}

#[tokio::test]
async fn shared_notes_are_searchable_only_while_a_named_grant_applies() {
    let f = Fixture::new().await;
    let space = &f.spaces[0];
    let group = data(
        f.op(0, "groups.create", json!({"name":"Sharing recipients"}))
            .await,
    );
    for account in [&f.ids[1], &f.ids[2]] {
        f.op(
            0,
            "groups.add_member",
            json!({"group_id":group["group_id"],"account_id":account}),
        )
        .await
        .ok();
    }
    let note = data(
        f.op(
            1,
            "notes.create",
            json!({
                "space_id":space,"title":"Compass", "markdown":"A distinctive dodecahedron"
            }),
        )
        .await,
    );
    let sibling = data(
        f.op(
            1,
            "notes.create",
            json!({
                "space_id":space,"title":"Unshared dodecahedron","markdown":"private"
            }),
        )
        .await,
    );
    let item = json!({"kind":"notes","id":note["note"]["id"]});
    let grant = data(
        f.op(
            1,
            "sharing.grant",
            json!({
                "item":item,"audience":{"account_id":f.ids[2]},"permission":"view"
            }),
        )
        .await,
    );
    let query = json!({"query":"dodecahedron","kinds":["notes"]});
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let result = data(f.op(2, "search.query", query.clone()).await);
            if result["partial"] == false {
                return result;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    let hits = result["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(
        hits[0]["address"],
        format!("/notes/{}", note["note"]["id"].as_str().unwrap())
    );
    assert_ne!(
        hits[0]["address"],
        format!("/notes/{}", sibling["note"]["id"].as_str().unwrap())
    );
    f.op(
        1,
        "sharing.revoke",
        json!({"item":item,"grant_id":grant["grant_id"]}),
    )
    .await
    .ok();
    assert_eq!(
        data(f.op(2, "search.query", query).await)["hits"],
        json!([])
    );
    let rescued = data(
        f.op(
            2,
            "sharing.rescue",
            json!({
                "title":"Compass","changed_blocks":["Only my edit"]
            }),
        )
        .await,
    );
    assert_eq!(rescued["note"]["markdown"], "Only my edit");
}
