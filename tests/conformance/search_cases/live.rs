use super::*;

#[tokio::test]
async fn live_search_tracks_opt_in_and_removal_without_restarting() {
    let f = Fixture::new().await;
    let private = &f.spaces[0];
    let shared = &f.spaces[2];
    note(&f, 1, private, "Registry probe", "Private catalog marker").await;
    note(&f, 1, shared, "Registry probe", "Shared catalog marker").await;
    let search = || async {
        ready_search(&f, 2, "search.query", json!({"query":"registry probe"})).await["hits"].clone()
    };
    assert_eq!(search().await.as_array().unwrap().len(), 1);

    f.op(
        1,
        "plugins.enable",
        json!({"space_id":shared,"plugin_id":"calternal-notes"}),
    )
    .await
    .ok();
    let path = f.dir.path().join("notes-search-live.calplugin");
    let archive = path.to_str().unwrap();
    let appdata = f.appdata();
    let root = appdata.to_str().unwrap();
    let publish = |version: &str, searchable: bool| {
        let mut manifest: Value = serde_json::from_str(include_str!(
            "../../../plugins/calternal-notes/manifest.json"
        ))
        .unwrap();
        manifest["version"] = json!(version);
        manifest["kinds"][0]["searchable"] = json!(searchable);
        std::fs::write(
            &path,
            json!({"manifest":manifest,"worker":null}).to_string(),
        )
        .unwrap();
        let review = package_cli(&["plugin", "review", archive]);
        package_cli(&[
            "plugin",
            "update",
            archive,
            "--appdata",
            root,
            "--approve",
            review["data"]["checksum"].as_str().unwrap(),
        ]);
    };
    publish("0.1.1", false);
    assert_eq!(search().await, json!([]));

    publish("0.1.2", true);
    let hits = search().await;
    assert_eq!(hits.as_array().unwrap().len(), 1);
    assert_eq!(hits[0]["space_id"], *shared);
    assert_eq!(hits[0]["excerpt"][0]["text"], "Registry");

    package_cli(&["plugin", "remove", "calternal-notes", "--appdata", root]);
    assert_eq!(search().await, json!([]));
}
