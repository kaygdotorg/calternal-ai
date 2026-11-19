//! The actual loopback routes used by both renderers.
use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "disposable renderer chrome fixture"]
async fn sealed_chrome_catalogue() {
    let f = ClientFixture::new().await;
    let bootstrap = f.bootstrap(&f.owner).await;
    for key in ["notes", "threads", "reading"] {
        assert!(bootstrap["views"][key].is_string(), "missing {key} view");
        for region in ["sidebar", "header", "footer"] {
            assert!(bootstrap["templates"][key]["chrome"][region].is_object());
        }
    }
    for key in ["notes", "threads", "reading"] {
        assert!(
            bootstrap["templates"][key]["binds"]
                .as_array()
                .unwrap()
                .iter()
                .all(|bind| bind["source"].get("mode").is_none())
        );
    }
    let note_bind = bootstrap["templates"]["notes"]["binds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|bind| bind["id"] == "notes")
        .unwrap();
    assert_eq!(
        note_bind["row"]["secondary"],
        json!([
            {"field":"updated","format":"relative_date"},{"field":"snippet","format":"text"}
        ])
    );
    assert_eq!(
        note_bind["row"]["icon"]["image"],
        json!({"field":"thumbnail","fallback":"snippet"})
    );
    let win = |bind: &str, parent: Option<&Value>| {
        let mut request = json!({"bind":bind,"from":0,"to":20});
        if let Some(parent) = parent {
            request["parent"] = parent.clone();
        }
        json!({"type":"window","request":request})
    };
    let open_threads = f
        .ok(
            &f.owner,
            "view.open",
            None,
            open(&bootstrap["views"]["threads"]),
        )
        .await;
    let lease = &open_threads["lease"]["lease_ref"];
    let roots = f
        .ok(&f.owner, "window", Some(lease), win("all", None))
        .await;
    assert_eq!(roots["count"]["value"], bootstrap["fixture"]["threads"]);
    let parent = &roots["rows"][0]["parent_refs"]["children"];
    assert!(parent.is_string());
    let child = f
        .ok(
            &f.owner,
            "window",
            Some(lease),
            win("children", Some(parent)),
        )
        .await;
    assert!(child["count"]["value"].as_u64().unwrap() > 0);
    assert_eq!(child["parent"], *parent);
    assert_eq!(
        f.call(
            &f.owner,
            "window",
            Some(lease),
            win("in-section", Some(parent))
        )
        .await
        .0,
        404
    );
    let layout = f
        .ok(&f.owner, "window", Some(lease), win("layout", None))
        .await;
    assert_eq!(layout["count"]["value"], 2);
    let section = &layout["rows"][0]["parent_refs"]["in-section"];
    let placed = f
        .ok(
            &f.owner,
            "window",
            Some(lease),
            win("in-section", Some(section)),
        )
        .await;
    assert!(placed["count"]["value"].as_u64().unwrap() > 0);

    let notes = f
        .ok(
            &f.owner,
            "view.open",
            None,
            open(&bootstrap["views"]["notes"]),
        )
        .await;
    let lease = &notes["lease"]["lease_ref"];
    let page = f
        .ok(&f.owner, "window", Some(lease), win("notes", None))
        .await;
    assert_eq!(page["count"]["value"], bootstrap["fixture"]["notes"]);
    let fields = &page["rows"][0]["fields"];
    assert!(fields["updated"].is_string());
    assert!(
        fields["snippet"]
            .as_str()
            .is_some_and(|text| !text.is_empty())
    );
    assert!(fields.get("thumbnail").is_none());
    let trash = f
        .ok(&f.owner, "window", Some(lease), win("trash-count", None))
        .await;
    assert!(trash["count"]["value"].as_u64().unwrap() > 0);

    let reading = f
        .ok(
            &f.owner,
            "view.open",
            None,
            open(&bootstrap["views"]["reading"]),
        )
        .await;
    let lease = &reading["lease"]["lease_ref"];
    let request = json!({"type":"invoke","action_id":"add","activation_id":uuid::Uuid::new_v4(),
        "activation_context":reading["activation_contexts"]["add"],
        "interaction":{"form":{"title":"New reading item","url":"https://example.test/read"}}});
    let resolved = f.ok(&f.owner, "activation.resolve", None, request).await;
    let dispatch = json!({"type":"invoke_dispatch","activation_id":resolved["activation_id"],
        "resolution_ref":resolved["resolution_ref"]});
    let added = f
        .ok(&f.owner, "activation.dispatch", None, dispatch.clone())
        .await;
    assert_eq!(added["state"], "ok");
    assert_eq!(
        f.ok(&f.owner, "activation.dispatch", None, dispatch).await,
        added
    );
    let rows = f
        .ok(&f.owner, "window", Some(lease), win("items", None))
        .await;
    assert_eq!(
        rows["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["fields"]["title"] == "New reading item")
            .count(),
        1
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "disposable 5,000-row renderer fixture"]
async fn sealed_chrome_scale() {
    let f = ClientFixture::with_rows(5000).await;
    let bootstrap = f.bootstrap(&f.owner).await;
    let mut opened = Vec::new();
    for (app, bind) in [("notes", "notes"), ("threads", "all")] {
        let view = f
            .ok(&f.owner, "view.open", None, open(&bootstrap["views"][app]))
            .await;
        let lease = view["lease"]["lease_ref"].clone();
        let tail = f
            .ok(
                &f.owner,
                "window",
                Some(&lease),
                json!({"type":"window","request":{
                    "bind":bind,"from":4980,"to":5000
                }}),
            )
            .await;
        assert_eq!(tail["count"]["value"], 5000);
        assert_eq!(tail["rows"].as_array().unwrap().len(), 20);
        opened.push((lease, bind));
    }
    let churn = f.host.start_churn(3).unwrap();
    let (lease, bind) = &opened[0];
    let mut saw = (false, false, false);
    let until = tokio::time::Instant::now() + Duration::from_secs(3);
    while saw != (true, true, true) && tokio::time::Instant::now() < until {
        let row = f
            .ok(
                &f.owner,
                "window",
                Some(lease),
                json!({"type":"window","request":{"bind":bind,"from":0,"to":10}}),
            )
            .await;
        let trash = f
            .ok(
                &f.owner,
                "window",
                Some(lease),
                json!({"type":"window","request":{"bind":"trash-count","from":0,"to":1}}),
            )
            .await;
        saw.0 |= row["count"]["value"] == 5001;
        saw.1 |= row["rows"][0]["fields"]["title"]
            .as_str()
            .unwrap()
            .starts_with("Renamed fixture");
        saw.2 |= trash["count"]["value"].as_u64().unwrap() > 3;
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    churn.abort();
    assert_eq!(saw, (true, true, true));
}
