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
    assert!(fields["thumbnail"].is_string());
    assert!(page["rows"][1]["fields"].get("thumbnail").is_none());
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "disposable renderer activate-and-redraw path"]
async fn sealed_chrome_actions_at_scale() {
    let f = ClientFixture::with_rows(5000).await;
    let boot = f.bootstrap(&f.owner).await;
    let notes = f
        .ok(&f.owner, "view.open", None, open(&boot["views"]["notes"]))
        .await;
    let lease = &notes["lease"]["lease_ref"];
    let page = |bind: &str, from: u64| json!({"type":"window","request":{"bind":bind,"from":from,"to":from+4}});
    let rows = f
        .ok(&f.owner, "window", Some(lease), page("notes", 4996))
        .await;
    let selected = &rows["rows"][0]["id"];
    let contexts = &rows["activation_contexts"][0]["contexts"];
    let invoke = |action: &str, context: &Value, interaction: Value| {
        json!({"type":"invoke","action_id":action,"activation_id":uuid::Uuid::new_v4(),
            "activation_context":context,"interaction":interaction})
    };
    let mut pin = invoke("pin", &contexts["pin"], json!({}));
    pin["dispatch"] = json!(true);
    let pinned = f
        .ok(&f.owner, "activation.resolve", None, pin.clone())
        .await;
    assert_eq!(pinned["state"], "ok");
    assert_eq!(
        f.ok(&f.owner, "activation.resolve", None, pin).await,
        pinned
    );
    let updated = f
        .ok(&f.owner, "window", Some(lease), page("notes", 4996))
        .await;
    assert_eq!(updated["rows"][0]["fields"]["pinned"], true);
    f.ok(
        &f.owner,
        "undo",
        Some(lease),
        json!({"type":"undo","undo_ref":pinned["undo"]["ref"]}),
    )
    .await;
    let old = invoke(
        "rename",
        &contexts["rename"],
        json!({"prompt":{"title":"stale"}}),
    );
    assert_ne!(
        f.call(&f.owner, "activation.resolve", None, old).await.0,
        200
    );
    let rows = f
        .ok(&f.owner, "window", Some(lease), page("notes", 4996))
        .await;
    let settled = f
        .ok(
            &f.owner,
            "selection.settled",
            Some(lease),
            json!({"type":"selection","bind":"notes","row":selected}),
        )
        .await;
    let rename = invoke(
        "rename",
        &settled["activation_contexts"]["rename"],
        json!({"prompt":{"title":"Renamed deep row"}}),
    );
    let r = f.ok(&f.owner, "activation.resolve", None, rename).await;
    let proof = f
        .ok(
            &f.owner,
            "activation.confirm",
            None,
            json!({"type":"invoke_confirm","activation_id":r["activation_id"],
        "resolution_ref":r["resolution_ref"],"confirmation_challenge":r["confirmation_challenge"]}),
        )
        .await;
    f.ok(
        &f.owner,
        "activation.dispatch",
        None,
        json!({"type":"invoke_dispatch","activation_id":r["activation_id"],
        "resolution_ref":r["resolution_ref"],"confirmation_proof":proof["confirmation_proof"]}),
    )
    .await;
    let fresh = f
        .ok(&f.owner, "window", Some(lease), page("notes", 4996))
        .await;
    assert_eq!(fresh["rows"][0]["fields"]["title"], "Renamed deep row");
    // An unrelated prefetch fitting the lease budget must leave this row usable.
    f.ok(&f.owner, "window", Some(lease), page("notes", 4500))
        .await;
    let collection = f.ok(&f.owner,"choice.options",None,json!({"type":"choice_options",
        "activation_context":fresh["activation_contexts"][0]["contexts"]["move"],"slot":"collection"})).await;
    assert_eq!(collection["bind"], "collections");

    let set = f
        .ok(
            &f.owner,
            "selection.set",
            Some(lease),
            json!({"type":"selection_set","bind":"notes","rows":[selected,rows["rows"][1]["id"]]}),
        )
        .await;
    let bulk = invoke(
        "trash-selected",
        &notes["activation_contexts"]["trash-selected"],
        json!({"selection_ref":set["selection_ref"]}),
    );
    let r = f
        .ok(&f.owner, "activation.resolve", None, bulk.clone())
        .await;
    let proof = f
        .ok(
            &f.owner,
            "activation.confirm",
            None,
            json!({"type":"invoke_confirm","activation_id":r["activation_id"],
        "resolution_ref":r["resolution_ref"],"confirmation_challenge":r["confirmation_challenge"]}),
        )
        .await;
    f.ok(
        &f.owner,
        "activation.dispatch",
        None,
        json!({"type":"invoke_dispatch","activation_id":r["activation_id"],
        "resolution_ref":r["resolution_ref"],"confirmation_proof":proof["confirmation_proof"]}),
    )
    .await;
    let trash = f
        .ok(&f.owner, "window", Some(lease), page("trash", 0))
        .await;
    assert_eq!(trash["count"]["value"], 5);
    assert_ne!(
        f.call(&f.guest, "activation.resolve", None, bulk).await.0,
        200
    );

    let threads = f
        .ok(&f.owner, "view.open", None, open(&boot["views"]["threads"]))
        .await;
    let lease = &threads["lease"]["lease_ref"];
    let roots = f
        .ok(&f.owner, "window", Some(lease), page("all", 4996))
        .await;
    let actions = &roots["activation_contexts"][0]["contexts"];
    let sections = f
        .ok(
            &f.owner,
            "choice.options",
            None,
            json!({"type":"choice_options",
        "activation_context":actions["move-to-all"],"slot":"section"}),
        )
        .await;
    assert_eq!(sections["bind"], "layout");
    let move_to = invoke(
        "move-to-all",
        &actions["move-to-all"],
        json!({"choices":{"section":sections["options"][0]["ref"]}}),
    );
    let r = f.ok(&f.owner, "activation.resolve", None, move_to).await;
    let proof = f
        .ok(
            &f.owner,
            "activation.confirm",
            None,
            json!({"type":"invoke_confirm","activation_id":r["activation_id"],
        "resolution_ref":r["resolution_ref"],"confirmation_challenge":r["confirmation_challenge"]}),
        )
        .await;
    f.ok(
        &f.owner,
        "activation.dispatch",
        None,
        json!({"type":"invoke_dispatch","activation_id":r["activation_id"],
        "resolution_ref":r["resolution_ref"],"confirmation_proof":proof["confirmation_proof"]}),
    )
    .await;
    let layout = f
        .ok(&f.owner, "window", Some(lease), page("layout", 0))
        .await;
    let mut nested = page("in-section", 0);
    nested["request"]["parent"] = layout["rows"][0]["parent_refs"]["in-section"].clone();
    let placed = f.ok(&f.owner, "window", Some(lease), nested).await;
    assert_eq!(placed["count"]["value"], 7);
    assert!(placed["activation_contexts"][0]["contexts"]["move-up-in-section"].is_string());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "disposable revision 13.18 end window and image read"]
async fn sealed_chrome_end_and_images() {
    let f = ClientFixture::with_rows(5000).await;
    let fixtures = f.bootstrap(&f.owner).await;
    assert_eq!(fixtures["contract_revision"], "13.18");
    let open = f
        .ok(
            &f.owner,
            "view.open",
            None,
            open(&fixtures["views"]["notes"]),
        )
        .await;
    let lease = &open["lease"]["lease_ref"];
    let payload = |anchor, from, to| json!({"type":"window","request":{"bind":"notes","from":from,"to":to,"anchor":anchor}});
    let first = f
        .ok(&f.owner, "window", Some(lease), payload("start", 0, 4))
        .await;
    let last = f
        .ok(
            &f.owner,
            "window",
            Some(lease),
            payload("start", 4997, 5000),
        )
        .await;
    for _ in 0..10 {
        let end = f
            .ok(&f.owner, "window", Some(lease), payload("end", 0, 3))
            .await;
        assert_eq!(end["count"], json!({"mode":"exact","value":5000}));
        assert_eq!(end["anchor"], "end");
        assert_eq!(end["rows"], last["rows"]);
    }
    assert_eq!(
        f.call(
            &f.owner,
            "window",
            Some(&json!(uuid::Uuid::new_v4())),
            payload("end", 0, 3)
        )
        .await
        .0,
        404
    );
    let root = f.base.strip_suffix("/step0/v2").unwrap();
    let image = first["rows"][0]["fields"]["thumbnail"].as_str().unwrap();
    let url = format!("{root}/api/v2/row-images/{image}");
    let get = |account: &str, lease: &Value| {
        f.http
            .get(&url)
            .bearer_auth(account)
            .header("x-step0-view-lease", lease.as_str().unwrap())
    };
    let picture = get(&f.owner, lease).send().await.unwrap();
    assert_eq!(picture.status().as_u16(), 200);
    assert_eq!(picture.headers()["content-type"], "image/png");
    assert_eq!(picture.headers()["cache-control"], "no-store");
    assert!(
        picture
            .bytes()
            .await
            .unwrap()
            .starts_with(b"\x89PNG\r\n\x1a\n")
    );
    assert_eq!(
        get(&f.guest, lease).send().await.unwrap().status().as_u16(),
        404
    );
    assert_eq!(
        f.http
            .get(&url)
            .bearer_auth(&f.owner)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        404
    );

    let context = &first["activation_contexts"][0]["contexts"]["pin"];
    f.ok(
        &f.owner,
        "activation.resolve",
        None,
        json!({"type":"invoke","action_id":"pin",
        "activation_id":uuid::Uuid::new_v4(),"activation_context":context,"dispatch":true}),
    )
    .await;
    assert_eq!(
        get(&f.owner, lease).send().await.unwrap().status().as_u16(),
        404
    );
    let after = f
        .ok(&f.owner, "window", Some(lease), payload("start", 0, 1))
        .await;
    assert!(after["rows"][0]["fields"].get("thumbnail").is_none());
    assert!(after["rows"][0]["fields"]["snippet"].is_string());
}
