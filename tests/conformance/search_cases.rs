//! Palette and mention isolation, ordering and save/index crash recovery.
use super::*;

mod live;
mod rebuild_source;
mod thread_titles;

pub(super) const OPERATIONS: &[&str] = &[
    "search.query",
    "items.lookup",
    "threads.lookup",
    "people.lookup",
    "skills.list",
    "plugins.manifests",
];

#[track_caller]
fn package_cli(args: &[&str]) -> Value {
    let output = Command::new(std::env::var_os("CALTERNAL_SERVER_BIN").unwrap())
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice::<Value>(&output.stdout).unwrap()
}

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

async fn ready_search(f: &Fixture, who: usize, op: &str, input: Value) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let page = data(f.op(who, op, input.clone()).await);
            if page["partial"] == false {
                return page;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("search projection should catch up for {op}: {input}"))
}

#[tokio::test]
async fn empty_space_rebuild_leaves_userdata_unprovisioned() {
    let f = Fixture::new().await;
    let space = &f.spaces[2];
    let files = f.dir.path().join("userdata-ssd/spaces").join(space);
    assert!(!files.exists(), "fixture space starts without userdata");
    f.op(
        1,
        "plugins.enable",
        json!({"space_id":space,"plugin_id":"calternal-notes-insights"}),
    )
    .await
    .ok();
    let app = f.appdata().join("spaces").join(space);
    assert!(app.join("content.db").is_file());
    let page = ready_search(
        &f,
        2,
        "search.query",
        json!({"space_id":space,"kinds":["notes"],"query":"needle"}),
    )
    .await;
    assert_eq!(page, json!({"hits":[],"partial":false}));
    assert!(app.join("search-v2.db").is_file(), "the worker ran");
    assert!(!files.exists(), "search rebuild provisioned {files:?}");
}

#[tokio::test]
async fn cross_space_search_and_prefix_lookups_hide_other_spaces() {
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
    for (op, field, text) in [
        ("search.query", "query", "Needle"),
        ("items.lookup", "prefix", "Project"),
        ("threads.lookup", "prefix", "Project"),
    ] {
        let mut query = json!({(field):text});
        let available = ready_search(&f, 2, op, query.clone()).await;
        let hits = available["hits"].as_array().unwrap();
        if op == "threads.lookup" {
            assert!(hits.is_empty());
        } else {
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0]["space_id"], *shared);
            assert_eq!(
                hits[0]["address"],
                format!("/notes/{}", visible["id"].as_str().unwrap())
            );
        }

        query["space_id"] = json!(private);
        let hidden = data(f.op(2, op, query.clone()).await);
        query["space_id"] = json!(&missing);
        assert_eq!(hidden, data(f.op(2, op, query).await));
        assert_eq!(hidden, json!({"hits":[],"partial":false}));
    }
    data(
        f.op(
            1,
            "plugins.disable",
            json!({"space_id":shared,"plugin_id":"calternal-notes"}),
        )
        .await,
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
    let visible = ready_search(&f, 2, "search.query", json!({"query":"inherited grant"})).await;
    assert_eq!(visible["hits"][0]["space_id"], group_space);
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
    let first = ready_search(&f, 2, "search.query", input.clone()).await;
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
    let incremental = ready_search(&f, 2, "search.query", query.clone()).await;
    assert_eq!(
        incremental["hits"][0]["address"],
        second["hits"][0]["address"]
    );
    data(f.op_id(1, "notes.update", input, "search-crash").await);
    f.stop();
    let root = f.appdata().join("spaces").join(&space);
    let source = Connection::open(root.join("content.db")).unwrap();
    let inline_search: i64 = source
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name='search_docs'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(inline_search, 0);
    drop(source);
    let index_path = root.join("search-v2.db");
    let locked = Connection::open(&index_path).unwrap();
    locked
        .execute(
            "UPDATE search_configuration SET generation='older-release'",
            [],
        )
        .unwrap();
    locked.execute_batch("BEGIN IMMEDIATE").unwrap();
    f.start().await;
    data(f.op(2, "notes.list", json!({"space_id":space})).await);
    data(f.op(2, "notes.get", json!({"id":body["id"]})).await);
    data(
        f.op(
            1,
            "notes.update",
            json!({"id":title["id"],"base_revision":title["revision"],"markdown":"saved during rebuild"}),
        )
        .await,
    );
    let pending = data(f.op(2, "search.query", query.clone()).await);
    assert_eq!(pending, json!({"hits":[],"partial":true}));
    locked.execute_batch("ROLLBACK").unwrap();
    drop(locked);
    assert_eq!(
        incremental,
        ready_search(&f, 2, "search.query", query.clone()).await
    );
    data(f.op(1, "notes.delete", json!({"id":body["id"]})).await);
    assert_eq!(
        ready_search(&f, 2, "search.query", query.clone()).await["hits"],
        json!([])
    );
    data(f.op(1, "notes.restore", json!({"id":body["id"]})).await);
    assert_eq!(
        incremental,
        ready_search(&f, 2, "search.query", query).await
    );

    let before = data(f.op(1, "notes.get", json!({"id":body["id"]})).await);
    let locked = Connection::open(&index_path).unwrap();
    locked.busy_timeout(Duration::from_secs(2)).unwrap();
    locked.execute_batch("BEGIN IMMEDIATE").unwrap();
    data(
        f.op(
            1,
            "notes.update",
            json!({"id":body["id"],"base_revision":before["note"]["revision"],"markdown":"afterackcrash"}),
        )
        .await,
    );
    let q = json!({"space_id":space,"query":"afterackcrash"});
    assert_eq!(
        data(f.op(2, "search.query", q.clone()).await)["partial"],
        true
    );
    f.stop();
    locked.execute_batch("ROLLBACK").unwrap();
    drop(locked);
    f.start().await;
    assert_eq!(
        ready_search(&f, 2, "search.query", q).await["hits"][0]["address"],
        incremental["hits"][0]["address"]
    );
}

#[tokio::test]
async fn first_search_returns_partial_when_the_index_cannot_open() {
    let mut f = Fixture::new().await;
    let space = f.spaces[2].clone();
    let saved = note(&f, 1, &space, "Raceprobe", "raw body").await;
    let query = json!({"space_id":space,"query":"Raceprobe"});
    ready_search(&f, 2, "search.query", query.clone()).await;
    f.stop();

    let root = f.appdata().join("spaces").join(&space);
    for file in ["search-v2.db", "search-v2.db-wal", "search-v2.db-shm"] {
        match std::fs::remove_file(root.join(file)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("could not reset the derived fixture: {error}"),
        }
    }
    let locked = Connection::open(root.join("search-v2.db")).unwrap();
    locked.execute_batch("BEGIN EXCLUSIVE").unwrap();
    f.start().await;
    assert_eq!(
        data(f.op(2, "search.query", query.clone()).await),
        json!({"hits":[],"partial":true})
    );
    data(f.op(2, "notes.get", json!({"id":saved["id"]})).await);
    locked.execute_batch("ROLLBACK").unwrap();
    drop(locked);
    assert_eq!(
        ready_search(&f, 2, "search.query", query).await["hits"][0]["address"],
        format!("/notes/{}", saved["id"].as_str().unwrap())
    );
}

#[tokio::test]
async fn palette_results_and_picker_operations_hide_other_spaces() {
    let f = Fixture::new().await;
    let private = &f.spaces[0];
    let shared = &f.spaces[2];
    let missing = uuid::Uuid::now_v7().to_string();
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
        f.op(2, "people.lookup", json!({"space_id":group_space}))
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn palette_reflects_live_install_update_and_removal_in_the_enabled_space() {
    let f = Fixture::new().await;
    let space = &f.spaces[0];
    let id = "palette-live-example";
    let pid = f.child.as_ref().unwrap().id();
    let path = f.dir.path().join("palette-live.calplugin");
    let path = path.to_str().unwrap();
    let appdata = f.appdata();
    let root = appdata.to_str().unwrap();
    let publish = |action: &str, version: &str, label: &str| {
        let archive = json!({"manifest":{
            "api_version":1,"id":id,"name":"Palette live example","icon":"puzzle","version":version,
            "kinds":[],"operations":[],"collections":[],"views":[],"attention":[],"settings":[],
            "schedules":[],"webhooks":[],"secrets":[],"hooks":[],"themes":[],"host_calls":[],
            "palette":{"commands":[{"id":"palette-live-example.open","label":label,"group":"Example","go":"/notes"}],
                       "settings_sections":[]}
        },"worker":null});
        std::fs::write(path, archive.to_string()).unwrap();
        let review = package_cli(&["plugin", "review", path]);
        package_cli(&[
            "plugin",
            action,
            path,
            "--appdata",
            root,
            "--approve",
            review["data"]["checksum"].as_str().unwrap(),
        ]);
    };
    let command = async |who, space: &str| {
        data(
            f.op(who, "plugins.manifests", json!({"space_id":space}))
                .await,
        )["manifests"]
            .as_array()
            .unwrap()
            .iter()
            .find(|manifest| manifest["plugin"] == id)
            .map(|manifest| {
                manifest["commands"][0]["label"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
    };

    assert_eq!(command(1, space).await, None);
    publish("install", "0.1.0", "Open the example");
    assert_eq!(command(1, space).await, None);
    f.op(
        1,
        "plugins.enable",
        json!({"space_id":space,"plugin_id":id}),
    )
    .await
    .ok();
    assert_eq!(command(1, space).await.as_deref(), Some("Open the example"));
    assert_eq!(command(2, &f.spaces[1]).await, None);
    assert_eq!(
        f.op(2, "plugins.manifests", json!({"space_id":space}))
            .await
            .status,
        404
    );
    publish("update", "0.2.0", "Open the updated example");
    assert_eq!(
        command(1, space).await.as_deref(),
        Some("Open the updated example")
    );
    package_cli(&["plugin", "remove", id, "--appdata", root]);
    assert_eq!(command(1, space).await, None);
    assert_eq!(f.child.as_ref().unwrap().id(), pid);
}
