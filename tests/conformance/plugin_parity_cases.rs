//! Compare seeded worker packages with the same public capabilities under a different ID.
//! The native service ports and publisher provenance require their public contracts first.
use super::*;

fn plugin_cli(args: &[&str]) -> Value {
    let output = Command::new(std::env::var_os("CALTERNAL_SERVER_BIN").unwrap())
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn install_copy(f: &Fixture, entry: &Value, id: &str, copy_id: &str) {
    let appdata = f.appdata();
    let root = appdata.to_str().unwrap();
    plugin_cli(&["plugin", "remove", id, "--appdata", root]);
    let source = source_root().join("plugins");
    let mut manifest: Value = serde_json::from_slice(
        &std::fs::read(source.join(entry["manifest"].as_str().unwrap())).unwrap(),
    )
    .unwrap();
    manifest["id"] = json!(copy_id);
    let worker = std::fs::read_to_string(source.join(entry["worker"].as_str().unwrap())).unwrap();
    let archive = f.dir.path().join(format!("{copy_id}.calplugin"));
    std::fs::write(
        &archive,
        json!({"manifest": manifest, "worker": worker}).to_string(),
    )
    .unwrap();
    let archive = archive.to_str().unwrap();
    let review = plugin_cli(&["plugin", "review", archive]);
    let installed = plugin_cli(&[
        "plugin",
        "install",
        archive,
        "--appdata",
        root,
        "--approve",
        review["data"]["checksum"].as_str().unwrap(),
        "--space",
        &f.spaces[0],
        "--account",
        &f.ids[1],
    ]);
    assert_eq!(installed["data"]["id"], copy_id);
}

async fn exercise(f: &Fixture, implementation: &str, installed_id: &str) -> Value {
    let space = &f.spaces[0];
    let note = f.note(1, space).await;
    let cases = match implementation {
        "calternal-notes-insights" => vec![("notes-insights.summarize", json!({"id":note["id"]}))],
        "calternal-plugin-guide" => vec![
            ("calternal-plugin-guide.explain", json!({"topic":"skill"})),
            (
                "calternal-plugin-guide.scaffold",
                json!({"name":"parity-demo"}),
            ),
            ("calternal-plugin-guide.check", json!({"manifest":{}})),
            ("calternal-plugin-guide.publish", json!({})),
        ],
        "calternal-mail" => vec![("calternal-mail.list-accounts", json!({}))],
        "calternal-plugin-submission-guide" => {
            vec![("calternal-plugin-submission-guide.explain", json!({}))]
        }
        other => panic!("add behavioral coverage for seeded worker {other}"),
    };

    for (operation, input) in &cases {
        let mut scoped = input.clone();
        scoped["space_id"] = json!(space);
        assert_eq!(f.op(1, operation, scoped.clone()).await.status, 403);
        assert_eq!(f.op(2, operation, scoped).await.status, 404);
    }
    f.op(
        1,
        "plugins.enable",
        json!({"plugin_id":installed_id,"space_id":space}),
    )
    .await
    .ok();
    let listing = f.op(1, "plugins.list", json!({"space_id":space})).await;
    listing.ok();
    let item = listing.body["data"]["plugins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["manifest"]["id"] == installed_id)
        .unwrap();
    assert_eq!(item["enabled"], true);
    let mut manifest = item["manifest"].clone();
    manifest["id"] = json!("package-under-test");
    let slots = listing.body["data"]["slots"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|slot| slot["plugin_id"] == installed_id)
        .map(|slot| slot["contribution"].clone())
        .collect::<Vec<_>>();

    let other = f
        .op(2, "plugins.list", json!({"space_id":f.spaces[1]}))
        .await;
    other.ok();
    assert!(
        other.body["data"]["slots"]
            .as_array()
            .into_iter()
            .flatten()
            .all(|slot| slot["plugin_id"] != installed_id)
    );

    let mut results = Vec::new();
    for (operation, mut input) in cases {
        input["space_id"] = json!(space);
        assert_eq!(f.op(2, operation, input.clone()).await.status, 404);
        let given = f.op(1, operation, input).await;
        given.ok();
        let data = &given.body["data"];
        if operation.ends_with(".publish") {
            let resolve = json!({"kind":"plugin-guides","id":data["item"]["id"]});
            let target = f.op(1, "resolve", resolve.clone()).await;
            target.ok();
            assert_eq!(target.body["data"]["space_id"], *space);
            assert_eq!(f.op(2, "resolve", resolve).await.status, 404);
            results.push(json!({"published_in_own_space":true}));
        } else {
            results.push(data.clone());
        }
    }
    json!({"manifest":manifest,"slots":slots,"results":results})
}

#[tokio::test]
async fn renamed_seeded_workers_have_the_same_grants_slots_and_behavior() {
    let seed: Value =
        serde_json::from_str(include_str!("../../crates/server/trusted-plugins.json")).unwrap();
    let original = Fixture::new().await;
    let copy = Fixture::new().await;
    for (index, entry) in seed["plugins"].as_array().unwrap().iter().enumerate() {
        if entry["worker"].is_null() {
            continue;
        }
        let id = entry["id"].as_str().unwrap();
        let copy_id = format!("renamed-package-{index}");
        install_copy(&copy, entry, id, &copy_id);
        assert_eq!(
            exercise(&original, id, id).await,
            exercise(&copy, id, &copy_id).await,
            "{id}"
        );
    }
}
