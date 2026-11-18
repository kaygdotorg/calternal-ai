//! Cursor continuation and isolation using the normal passkey operation route.
use super::*;

async fn first(f: &Fixture, op: &str, input: &Value, key: &str) -> Value {
    let page = f.op(1, op, input.clone()).await;
    page.ok();
    let page = &page.body["data"];
    assert_eq!(page[key].as_array().unwrap().len(), 100, "{op}");
    assert!(page["next_cursor"].is_string(), "{op}");
    page.clone()
}

async fn check(
    f: &Fixture,
    op: &str,
    input: &Value,
    key: &str,
    identity: &str,
    first: &Value,
    other_space: &str,
) {
    let mut more = input.clone();
    more["cursor"] = first["next_cursor"].clone();
    let next = f.op(1, op, more.clone()).await;
    next.ok();
    let rows = next.body["data"][key].as_array().unwrap();
    assert_eq!(rows.len(), if op == "tags.list" { 4 } else { 3 }, "{op}");
    for row in rows {
        assert!(
            !first[key]
                .as_array()
                .unwrap()
                .iter()
                .any(|seen| seen[identity] == row[identity])
        );
    }
    assert!(next.body["data"].get("next_cursor").is_none(), "{op}");

    let wrong_account = f.op(2, op, more.clone()).await;
    let wrong_space = f
        .op(1, op, {
            let mut input = more.clone();
            input["space_id"] = json!(other_space);
            input
        })
        .await;
    let changed = f
        .op(1, op, {
            let mut input = more.clone();
            if op == "notes.list" {
                input["tag"] = json!("cursor");
            }
            if op.ends_with(".lookup") {
                input["prefix"] = json!("Changed");
            }
            let cursor = &input["cursor"];
            if op == "tags.list" || op == "trash.list" {
                input["cursor"] = json!(format!("A{}", cursor.as_str().unwrap()));
            }
            input
        })
        .await;
    more["cursor"] = json!("invalid");
    let invalid = f.op(1, op, more).await;
    assert_eq!(invalid.status, 400, "{op}: {}", invalid.body);
    assert_eq!(invalid.body["error"]["code"], "invalid_input");
    for refused in [wrong_account, wrong_space, changed] {
        assert_eq!(refused.status, 400, "{op}: {}", refused.body);
        assert_eq!(refused.signature(), invalid.signature(), "{op}");
        assert!(refused.body.get("data").is_none());
    }
}

#[tokio::test]
async fn note_index_cursors_continue_and_bind_to_the_authorized_query() {
    let f = Fixture::new().await;
    let space = &f.spaces[2]; // Owner and Reader both have access here.
    let other = &f.spaces[0]; // A second space the issuing account can read.
    let mut ids = Vec::new();
    for i in 0..103 {
        let created = f
            .op(
                1,
                "notes.create",
                json!({"space_id":space,"title":format!("Page {i:03}"),"markdown":format!("#cursor/tag-{i:03}")}),
            )
            .await;
        created.ok();
        ids.push(created.body["data"]["note"]["id"].clone());
    }
    for (op, input, rows, identity) in [
        ("notes.list", json!({"space_id":space}), "notes", "id"),
        (
            "notes.lookup",
            json!({"space_id":space,"prefix":"pAgE"}),
            "notes",
            "id",
        ),
        ("tags.list", json!({"space_id":space}), "tags", "name"),
        (
            "tags.lookup",
            json!({"space_id":space,"prefix":"cursor/tag-"}),
            "tags",
            "name",
        ),
    ] {
        let page = first(&f, op, &input, rows).await;
        // tags.list includes the projected parent tag. Its final page has four rows.
        if op == "tags.list" {
            assert_eq!(page["tags"][0], json!({"name":"cursor","count":103}));
        }
        check(&f, op, &input, rows, identity, &page, other).await;
    }
    let input = json!({"space_id":space,"tag":"cursor"});
    let tagged = first(&f, "notes.list", &input, "notes").await;
    let inserted = f
        .op(
            1,
            "notes.create",
            json!({"space_id":space,"title":"Later","markdown":"#cursor"}),
        )
        .await;
    inserted.ok();
    let boundary = &tagged["notes"][99]["id"];
    f.op(1, "notes.delete", json!({"id":boundary})).await.ok();
    let next = f
        .op(
            1,
            "notes.list",
            json!({"space_id":space,"tag":"cursor","cursor":tagged["next_cursor"]}),
        )
        .await;
    next.ok();
    assert_eq!(next.body["data"]["notes"].as_array().unwrap().len(), 3);
    let cross_list = f
        .op(
            1,
            "trash.list",
            json!({"space_id":space,"cursor":tagged["next_cursor"]}),
        )
        .await;
    assert_eq!(cross_list.status, 400);
    for id in ids.into_iter().filter(|id| id != boundary) {
        f.op(1, "notes.delete", json!({"id":id})).await.ok();
    }
    let input = json!({"space_id":space});
    let trashed = first(&f, "trash.list", &input, "notes").await;
    check(&f, "trash.list", &input, "notes", "id", &trashed, other).await;
    // The public history continuation also binds its existing item-qualified
    // position to the requesting account.
    let note = &inserted.body["data"]["note"];
    for revision in ["first edit", "second edit"] {
        let current = f.op(1, "notes.get", json!({"id":note["id"]})).await;
        current.ok();
        f.op(
            1,
            "notes.update",
            json!({"id":note["id"],
            "base_revision":current.body["data"]["note"]["revision"],
            "markdown":revision}),
        )
        .await
        .ok();
    }
    let history = f
        .op(1, "notes.history", json!({"id":note["id"],"limit":1}))
        .await;
    history.ok();
    let mut history_input =
        json!({"id":note["id"],"limit":2,"cursor":history.body["data"]["next_cursor"]});
    let foreign = f.op(2, "notes.history", history_input.clone()).await;
    assert_eq!(foreign.status, 400, "{}", foreign.body);
    f.op(1, "notes.history", history_input.clone()).await.ok();
    history_input["cursor"] = json!("invalid");
    assert_eq!(
        foreign.signature(),
        f.op(1, "notes.history", history_input).await.signature()
    );
}
