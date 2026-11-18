//! Palette, skills and mention isolation.
use super::*;

pub(super) const OPERATIONS: &[&str] = &["people.lookup", "skills.list", "plugins.manifests"];

#[track_caller]
fn data(r: Answer) -> Value {
    r.ok();
    r.body["data"].clone()
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
    let cli = |args: &[&str]| {
        let output = Command::new(std::env::var_os("CALTERNAL_SERVER_BIN").unwrap())
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let publish = |action: &str, version: &str, label: &str| {
        let archive = json!({"manifest":{
            "api_version":1,"id":id,"name":"Palette live example","icon":"puzzle","version":version,
            "kinds":[],"operations":[],"collections":[],"views":[],"attention":[],"settings":[],
            "schedules":[],"webhooks":[],"secrets":[],"hooks":[],"themes":[],"host_calls":[],
            "palette":{"commands":[{"id":"palette-live-example.open","label":label,"group":"Example","go":"/notes"}],
                       "settings_sections":[]}
        },"worker":null});
        std::fs::write(path, archive.to_string()).unwrap();
        let review = cli(&["plugin", "review", path]);
        cli(&[
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
    cli(&["plugin", "remove", id, "--appdata", root]);
    assert_eq!(command(1, space).await, None);
    assert_eq!(f.child.as_ref().unwrap().id(), pid);
}
