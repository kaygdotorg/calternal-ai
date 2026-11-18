//! Cross-package slot resolution over the live catalog and real space authority.
use super::*;

fn package(f: &Fixture, id: &str, owned: Value, slots: Value, capability: &str) -> PathBuf {
    let operation = format!("{id}.view");
    let has_renderer = slots.as_array().is_some_and(|slots| !slots.is_empty());
    let operations = if has_renderer {
        json!([{"id":operation,"required_capability":capability,"mutates":false,"risk":"read",
            "idempotency":"read_only","input_schema":{"type":"object"},
            "output_schema":{"type":"object","properties":{"view":{"type":"object"}},"required":["view"]}}])
    } else {
        json!([])
    };
    let worker = has_renderer.then_some(
        r#"require('node:readline').createInterface({input:process.stdin}).on('line', line => {
        if (JSON.parse(line).type === 'ready') console.log(line);
    });"#,
    );
    let manifest = json!({"id":id,"name":id,"icon":"puzzle","version":"0.1.0","api_version":1,
        "kinds":[],"collections":[],"views":[],"attention":[],"settings":[],"schedules":[],"webhooks":[],
        "secrets":[],"hooks":[],"themes":[],"host_calls":[],"owned_slots":owned,"slots":slots,"operations":operations});
    let archive = f.dir.path().join(format!("{id}.calplugin"));
    std::fs::write(
        &archive,
        json!({"manifest":manifest,"worker":worker}).to_string(),
    )
    .unwrap();
    archive
}

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(std::env::var_os("CALTERNAL_SERVER_BIN").unwrap())
        .args(args)
        .arg("--json")
        .output()
        .unwrap()
}

fn install(f: &Fixture, path: &Path, space: &str) {
    let source = path.to_str().unwrap();
    let review = cli(&["plugin", "review", source]);
    assert!(review.status.success(), "{review:?}");
    let review: Value = serde_json::from_slice(&review.stdout).unwrap();
    let installed = cli(&[
        "plugin",
        "install",
        source,
        "--appdata",
        f.appdata().to_str().unwrap(),
        "--approve",
        review["data"]["checksum"].as_str().unwrap(),
        "--space",
        space,
        "--account",
        &f.ids[1],
    ]);
    assert!(installed.status.success(), "{installed:?}");
}

fn mount(id: &str, capability: &str) -> Value {
    json!({"id":id,"required_capability":capability,"accepts":["generated"],"max_contributions":1})
}

fn contribution(package: &str, id: &str, owner: &str, slot: &str, kind: &str) -> Value {
    json!({"id":id,"label":id,"slot":{"plugin_id":owner,"id":slot},
        "renderer":{"type":kind,"operation":format!("{package}.view")}})
}

async fn set(f: &Fixture, space: &str, package: &str, enable: bool) {
    f.op(
        1,
        if enable {
            "plugins.enable"
        } else {
            "plugins.disable"
        },
        json!({"space_id":space,"plugin_id":package}),
    )
    .await
    .ok();
}

async fn available(f: &Fixture, who: usize, space: &str) -> (Vec<String>, Vec<String>) {
    let listing = f.op(who, "plugins.list", json!({"space_id":space})).await;
    listing.ok();
    let data = &listing.body["data"];
    let mounts = data["owned_slots"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|slot| slot["definition"]["id"].as_str().unwrap().to_string())
        .collect();
    let entries = data["slots"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|slot| slot["contribution"]["slot"].is_object())
        .map(|slot| slot["contribution"]["id"].as_str().unwrap().to_string())
        .collect();
    (mounts, entries)
}

#[tokio::test]
async fn owned_slots_check_both_grants_capacity_and_live_catalog() {
    let f = Fixture::new().await;
    let space = &f.spaces[2]; // Account 2 is a reader here.
    let owner = "renamed-slot-owner";
    let first = "renamed-a-contributor";
    let second = "renamed-b-contributor";
    let special = "renamed-z-contributor";
    let early = package(
        &f,
        first,
        json!([]),
        json!([contribution(first, "first", owner, "public", "generated")]),
        "content.write",
    );
    install(&f, &early, space);
    set(&f, space, first, true).await;
    assert_eq!(available(&f, 1, space).await, (vec![], vec![]));

    let definition = package(
        &f,
        owner,
        json!([
            mount("public", "space.read"),
            mount("protected-by-role", "content.write")
        ]),
        json!([]),
        "space.read",
    );
    install(&f, &definition, space);
    let open = package(
        &f,
        second,
        json!([]),
        json!([
            contribution(second, "fallback", owner, "public", "generated"),
            contribution(second, "role-only", owner, "protected-by-role", "generated"),
            contribution(second, "undefined", owner, "missing", "generated")
        ]),
        "space.read",
    );
    install(&f, &open, space);
    let mismatch = package(
        &f,
        special,
        json!([]),
        json!([contribution(
            special,
            "wrong-renderer",
            owner,
            "public",
            "command"
        )]),
        "space.read",
    );
    install(&f, &mismatch, space);
    set(&f, space, second, true).await;
    set(&f, space, special, true).await;
    assert_eq!(available(&f, 1, space).await, (vec![], vec![]));
    set(&f, space, owner, true).await;
    assert_eq!(
        available(&f, 1, space).await,
        (
            vec!["protected-by-role".into(), "public".into()],
            vec!["role-only".into(), "first".into()]
        )
    );
    assert_eq!(
        available(&f, 2, space).await,
        (vec!["public".into()], vec!["fallback".into()])
    );
    assert_eq!(available(&f, 2, &f.spaces[1]).await, (vec![], vec![]));
    assert_eq!(
        f.op(3, "plugins.list", json!({"space_id":space}))
            .await
            .status,
        404
    );

    set(&f, space, first, false).await;
    assert_eq!(
        available(&f, 1, space).await.1,
        vec!["role-only", "fallback"]
    );
    set(&f, space, first, true).await;
    let removed = cli(&[
        "plugin",
        "remove",
        first,
        "--appdata",
        f.appdata().to_str().unwrap(),
    ]);
    assert!(removed.status.success(), "{removed:?}");
    assert_eq!(
        available(&f, 1, space).await.1,
        vec!["role-only", "fallback"]
    );
    set(&f, space, owner, false).await;
    assert_eq!(available(&f, 1, space).await, (vec![], vec![]));
    set(&f, space, owner, true).await;
    let removed = cli(&[
        "plugin",
        "remove",
        owner,
        "--appdata",
        f.appdata().to_str().unwrap(),
    ]);
    assert!(removed.status.success(), "{removed:?}");
    assert_eq!(available(&f, 1, space).await, (vec![], vec![]));
}

#[test]
fn protected_host_names_cannot_be_declared_as_targets() {
    let id = "attempted-foundation-target";
    let draft = |name| {
        json!({"id":"probe","label":"Probe","slot":name,
        "renderer":{"type":"generated","operation":format!("{id}.view")}})
    };
    // Validation occurs before server contact. Use the archive form of the same
    // CLI gate without a running server.
    let dir = tempfile::tempdir().unwrap();
    for name in ["sign_in", "approvals", "secrets", "recovery"] {
        let path = dir.path().join("protected.calplugin");
        std::fs::write(&path, json!({"manifest":{
            "id": id,"name":id,"icon":"puzzle","version":"0.1.0","api_version":1,
            "kinds":[],"collections":[],"views":[],"attention":[],"settings":[],"schedules":[],
            "webhooks":[],"secrets":[],"hooks":[],"themes":[],"host_calls":[],"slots":[draft(name)],
            "operations":[{"id":format!("{id}.view"),"required_capability":"space.read","mutates":false,
                "risk":"read","idempotency":"read_only","input_schema":{"type":"object"},
                "output_schema":{"type":"object","properties":{"view":{"type":"object"}},"required":["view"]}}]
        },"worker":null}).to_string()).unwrap();
        assert!(
            !cli(&["plugin", "review", path.to_str().unwrap()])
                .status
                .success(),
            "{name}"
        );
    }
}

#[tokio::test]
async fn measure_fixed_and_owned_slot_listings() {
    if std::env::var_os("CALTERNAL_MEASURE_OWNED_SLOTS").is_none() {
        return;
    }
    let seed: Value =
        serde_json::from_str(include_str!("../../crates/server/trusted-plugins.json")).unwrap();
    let full = 128 - seed["plugins"].as_array().unwrap().len();
    for owned in [false, true] {
        let f = Fixture::new().await;
        let space = &f.spaces[0];
        let mut installed = 0;
        for size in [2, full] {
            for index in installed..size {
                let id = format!("slot-measurement-{index:03}");
                let definition = if owned {
                    let mut slot = mount("sample", "space.read");
                    slot["max_contributions"] = json!(32);
                    json!([slot])
                } else {
                    json!([])
                };
                let contributions = (0..32)
                    .map(|row| {
                        let mut slot =
                            contribution(&id, &format!("row-{row:02}"), &id, "sample", "generated");
                        if !owned {
                            slot["slot"] = json!("card_window");
                        }
                        slot
                    })
                    .collect::<Vec<_>>();
                let archive = package(&f, &id, definition, json!(contributions), "space.read");
                install(&f, &archive, space);
                set(&f, space, &id, true).await;
            }
            installed = size;

            let mut samples = Vec::new();
            let mut response_bytes = 0;
            for index in 0..68 {
                let start = Instant::now();
                let listing = f.op(1, "plugins.list", json!({"space_id":space})).await;
                let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                listing.ok();
                let data = &listing.body["data"];
                assert_eq!(data["slots"].as_array().unwrap().len(), size * 32);
                assert_eq!(
                    data["owned_slots"].as_array().map_or(0, Vec::len),
                    if owned { size } else { 0 }
                );
                if index == 0 {
                    response_bytes = listing.body.to_string().len();
                }
                if index >= 8 {
                    samples.push(elapsed_ms);
                }
            }
            samples.sort_by(f64::total_cmp);
            eprintln!(
                "OWNED_SLOT_MEASUREMENT {}",
                json!({
                    "owned":owned,"synthetic_packages":size,"contributions":size*32,"response_bytes":response_bytes,
                    "p50_ms":samples[29],"p95_ms":samples[56],"samples_ms":samples
                })
            );
        }
    }
}
