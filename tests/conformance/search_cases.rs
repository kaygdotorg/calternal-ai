//! Palette and mention isolation, ordering and save/index crash recovery.
use super::*;

pub(super) const OPERATIONS: &[&str] = &[
    "search.query",
    "items.lookup",
    "threads.lookup",
    "people.lookup",
    "skills.list",
    "plugins.manifests",
];

#[track_caller]
fn data(r: Answer) -> Value {
    r.ok();
    r.body["data"].clone()
}

async fn note(f: &Fixture, who: usize, space: &str, title: &str, markdown: &str) -> Value {
    data(
        f.op(
            who,
            "notes.create",
            json!({"space_id":space,"title":title,"markdown":markdown}),
        )
        .await,
    )["note"]
        .clone()
}

#[tokio::test]
async fn palette_results_and_picker_operations_hide_other_spaces() {
    let f = Fixture::new().await;
    let private = &f.spaces[0];
    let shared = &f.spaces[2];
    let missing = uuid::Uuid::now_v7().to_string();
    let _ = note(
        &f,
        1,
        private,
        "Project Needle",
        "Needle from an unreadable space",
    )
    .await;
    let visible = note(&f, 1, shared, "Project Needle", "Shared overview").await;
    let thread = data(
        f.op(
            1,
            "threads.create",
            json!({"space_id":shared,"title":"Project Needle thread"}),
        )
        .await,
    );
    let _ = data(
        f.op(
            1,
            "threads.create",
            json!({"space_id":private,"title":"Project Needle private thread"}),
        )
        .await,
    );
    let path = f
        .dir
        .path()
        .join("userdata-ssd/spaces")
        .join(shared)
        .join("AI/Skills/review");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
        path.join("SKILL.md"),
        "---\nname: review\ndescription: Review a draft.\n---\nA private skill body.",
    )
    .unwrap();
    std::os::unix::fs::symlink(&path, path.parent().unwrap().join("alias")).unwrap();

    for (op, field, text) in [
        ("search.query", "query", "Needle"),
        ("items.lookup", "prefix", "Project"),
        ("threads.lookup", "prefix", "Project"),
    ] {
        let mut query = json!({(field):text});
        let available = data(f.op(2, op, query.clone()).await);
        let expected = if op == "threads.lookup" { 1 } else { 2 };
        assert_eq!(available["hits"].as_array().unwrap().len(), expected);
        assert!(
            available["hits"]
                .as_array()
                .unwrap()
                .iter()
                .all(|hit| hit["space_id"] == *shared)
        );
        assert!(
            available["hits"]
                .as_array()
                .unwrap()
                .iter()
                .any(|hit| hit["address"] == thread["thread"]["address"])
        );
        if op != "threads.lookup" {
            assert!(
                available["hits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|hit| hit["address"]
                        == format!("/notes/{}", visible["id"].as_str().unwrap()))
            );
        }

        query["space_id"] = json!(private);
        let hidden = data(f.op(2, op, query.clone()).await);
        query["space_id"] = json!(&missing);
        assert_eq!(hidden, data(f.op(2, op, query).await));
        assert_eq!(hidden, json!({"hits":[],"partial":false}));
    }
    for op in ["people.lookup", "skills.list", "plugins.manifests"] {
        let hidden = f.op(2, op, json!({"space_id":private})).await;
        let absent = f.op(2, op, json!({"space_id":missing})).await;
        assert_eq!(hidden.status, 404);
        assert_eq!(hidden.signature(), absent.signature());
    }
    let people = data(
        f.op(
            2,
            "people.lookup",
            json!({"space_id":shared,"prefix":"acc"}),
        )
        .await,
    );
    assert_eq!(people["people"].as_array().unwrap().len(), 2);
    assert_eq!(people["people"][0]["name"], "Account A");
    let skills = data(f.op(2, "skills.list", json!({"space_id":shared})).await);
    assert_eq!(
        skills,
        json!({"skills":[{"name":"review","description":"Review a draft."}]})
    );
    let catalogue = data(
        f.op(2, "plugins.manifests", json!({"space_id":shared}))
            .await,
    );
    assert!(
        catalogue["manifests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["plugin"] == "calternal-notes" && m["commands"][0]["id"] == "notes.open")
    );
    let agent_section = catalogue["manifests"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["plugin"] == "calternal-orchestrator")
        .unwrap();
    assert!(
        agent_section["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == "settings.agents"
                && c["group"] == "Settings"
                && c["go"] == "/settings/agents")
    );
    data(
        f.op(
            1,
            "plugins.disable",
            json!({"space_id":shared,"plugin_id":"calternal-notes"}),
        )
        .await,
    );
    let catalogue = data(
        f.op(2, "plugins.manifests", json!({"space_id":shared}))
            .await,
    );
    assert!(
        catalogue["manifests"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["plugin"] != "calternal-notes")
    );
    let results = data(
        f.op(
            2,
            "search.query",
            json!({"query":"Needle","kinds":["notes"]}),
        )
        .await,
    );
    assert_eq!(results["hits"], json!([]));
    let through_group = data(
        f.op(0, "spaces.create", json!({"name":"Group search"}))
            .await,
    );
    let group_space = through_group["space"]["id"].as_str().unwrap();
    let group = data(
        f.op(0, "groups.create", json!({"name":"Palette plan"}))
            .await,
    );
    data(
        f.op(
            0,
            "groups.add_member",
            json!({"group_id":group["group_id"],"account_id":f.ids[2]}),
        )
        .await,
    );
    data(
        f.op(
            0,
            "spaces.add_member",
            json!({"space_id":group_space,"group_id":group["group_id"],"role":"reader"}),
        )
        .await,
    );
    let _ = note(&f, 0, group_space, "Inherited grant probe", "").await;
    let visible = data(
        f.op(2, "search.query", json!({"query":"inherited grant"}))
            .await,
    );
    assert_eq!(visible["hits"][0]["space_id"], group_space);
    let groups = data(
        f.op(
            2,
            "people.lookup",
            json!({"space_id":group_space,"prefix":"Palette"}),
        )
        .await,
    );
    assert_eq!(
        groups,
        json!({"people":[{"id":group["group_id"],"name":"Palette plan","kind":"group"}]})
    );
    data(
        f.op(
            0,
            "groups.remove_member",
            json!({"group_id":group["group_id"],"account_id":f.ids[2]}),
        )
        .await,
    );
    assert_eq!(
        data(
            f.op(2, "search.query", json!({"query":"inherited grant"}))
                .await
        )["hits"],
        json!([])
    );
}

#[tokio::test]
async fn search_orders_titles_first_then_bodies_and_rebuilds_identically_after_a_save_crash() {
    let mut f = Fixture::new().await;
    let space = f.spaces[2].clone();
    let body = note(
        &f,
        1,
        &space,
        "Minutes",
        "---\nmeta: frontmatterword\n---\nmeeting in the garden",
    )
    .await;
    let title = note(&f, 1, &space, "Meeting agenda", "prepare").await;
    let input = json!({"space_id":space,"query":"meeti","limit":1});
    let first = data(f.op(2, "search.query", input.clone()).await);
    assert_eq!(
        first["hits"][0]["address"],
        format!("/notes/{}", title["id"].as_str().unwrap())
    );
    assert!(
        first["hits"][0]["excerpt"]
            .as_array()
            .unwrap()
            .iter()
            .any(|part| part["matched"] == true)
    );
    let mut next = input.clone();
    next["cursor"] = first["next_cursor"].clone();
    let second = data(f.op(2, "search.query", next.clone()).await);
    assert_eq!(
        second["hits"][0]["address"],
        format!("/notes/{}", body["id"].as_str().unwrap())
    );
    assert!(
        second["hits"][0]["excerpt"]
            .as_array()
            .unwrap()
            .iter()
            .any(|part| part["matched"] == true)
    );
    assert_eq!(f.op(1, "search.query", next.clone()).await.status, 400);
    next["query"] = json!("minutes");
    assert_eq!(f.op(2, "search.query", next).await.status, 400);
    let typo = data(
        f.op(
            2,
            "search.query",
            json!({"space_id":space,"query":"meetng"}),
        )
        .await,
    );
    assert_eq!(typo["hits"][0]["address"], first["hits"][0]["address"]);
    assert_eq!(
        data(
            f.op(
                2,
                "search.query",
                json!({"space_id":space,"query":"frontmatterword"})
            )
            .await
        )["hits"],
        json!([])
    );

    f.stop();
    let fault = f.dir.path().join("content-fault");
    std::fs::write(&fault, "after_commit").unwrap();
    f.start().await;
    let input = json!({"id":body["id"], "base_revision":body["revision"], "markdown":"Recovered bodytoken"});
    let lost = f
        .client
        .post(format!("{}/api/v1/operations", f.base))
        .header("host", "localhost:18080")
        .header("origin", ORIGIN)
        .header("cookie", &f.cookies[1])
        .json(&json!({"operation":"notes.update","request_id":"search-crash","input":input}))
        .send()
        .await;
    assert!(lost.is_err());
    f.stop();
    std::fs::remove_file(fault).unwrap();
    f.start().await;
    let query = json!({"space_id":space,"query":"bodytoken"});
    let incremental = data(f.op(2, "search.query", query.clone()).await);
    assert_eq!(
        incremental["hits"][0]["address"],
        second["hits"][0]["address"]
    );
    data(f.op_id(1, "notes.update", input, "search-crash").await);
    f.stop();
    let conn =
        Connection::open(f.appdata().join("spaces").join(&space).join("content.db")).unwrap();
    conn.execute_batch("DELETE FROM search_configuration; DROP TABLE search_docs; DROP TABLE search_text; DROP TABLE search_title_grams;").unwrap();
    drop(conn);
    f.start().await;
    assert_eq!(
        incremental,
        data(f.op(2, "search.query", query.clone()).await)
    );
    data(f.op(1, "notes.delete", json!({"id":body["id"]})).await);
    assert_eq!(
        data(f.op(2, "search.query", query.clone()).await)["hits"],
        json!([])
    );
    data(f.op(1, "notes.restore", json!({"id":body["id"]})).await);
    assert_eq!(incremental, data(f.op(2, "search.query", query).await));
}
