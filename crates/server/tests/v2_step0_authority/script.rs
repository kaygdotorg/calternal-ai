//! Scripted sealed activation matrix.
use super::*;
use uuid::Uuid;

fn denied(reply: (u16, Value)) {
    assert!(matches!(reply.0, 403 | 404 | 409 | 422), "{reply:?}");
    assert_eq!(reply.1.as_object().unwrap().len(), 1);
    assert!(reply.1["error"].is_string());
}

fn resolve(action: &str, context: &Value, id: Uuid, extra: Value) -> Value {
    let mut input =
        json!({"type":"invoke","action_id":action,"activation_id":id,"activation_context":context});
    input
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    input
}
fn dispatch(resolution: &Value) -> Value {
    json!({"type":"invoke_dispatch","activation_id":resolution["activation_id"],"resolution_ref":resolution["resolution_ref"]})
}
fn confirm(resolution: &Value) -> Value {
    json!({"type":"invoke_confirm","activation_id":resolution["activation_id"],
           "resolution_ref":resolution["resolution_ref"],"confirmation_challenge":resolution["confirmation_challenge"]})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "disposable Step 0 host authority perf matrix"]
async fn sealed_authority_script() {
    let f = ClientFixture::new().await;
    let routes = f.bootstrap(&f.owner).await;
    let shared = &routes["views"]["shared"];
    let home = f
        .ok(&f.owner, "view.open", None, open(&routes["views"]["home"]))
        .await;
    let foreign = f
        .ok(
            &f.owner,
            "window",
            Some(&home["lease"]["lease_ref"]),
            window(),
        )
        .await["rows"][0]["id"]
        .clone();
    f.host.end_lease(&home);
    f.host.member_role(Some("editor")).await;
    let runs: usize = std::env::var("STEP0_RUNS").map_or(100, |v| v.parse().unwrap());
    let mut times = Vec::new();
    for i in 0..runs {
        f.host.change_fixture_row(0, i).await;
        f.host.change_fixture_row(1, i).await;
        let member = f.ok(&f.guest, "view.open", None, open(shared)).await;
        let lease = &member["lease"]["lease_ref"];
        let rows = f.ok(&f.guest, "window", Some(lease), window()).await;
        let first = &rows["rows"][0]["id"];
        let other = &rows["rows"][1]["id"];
        let row_event = |row: &Value| json!({"type":"selection","bind":"notes","row":row});
        let selected = f
            .ok(&f.guest, "selection.settled", Some(lease), row_event(first))
            .await;
        assert_eq!(selected["row"], *first);
        let contexts = &selected["activation_contexts"];
        assert!(contexts["pin"].is_string());
        for wrong in [json!(Uuid::new_v4()), foreign.clone()] {
            denied(
                f.call(
                    &f.guest,
                    "selection.settled",
                    Some(lease),
                    row_event(&wrong),
                )
                .await,
            );
        }
        denied(
            f.call(
                &f.guest,
                "window",
                Some(lease),
                json!({"type":"window","request":{"bind":"foreign","from":0,"to":20}}),
            )
            .await,
        );
        denied(
            f.call(&f.guest, "window", Some(&json!(Uuid::new_v4())), window())
                .await,
        );
        let id = Uuid::new_v4();
        let revision = f.host.revision(0).await;
        for field in ["row", "route", "bind"] {
            let mut forged = resolve("pin", &contexts["pin"], id, json!({}));
            forged[field] = foreign.clone();
            denied(f.call(&f.guest, "activation.resolve", None, forged).await);
        }
        denied(
            f.call(
                &f.guest,
                "activation.resolve",
                None,
                resolve("pin", &json!(Uuid::new_v4()), id, json!({})),
            )
            .await,
        );
        denied(
            f.call(
                &f.owner,
                "activation.resolve",
                None,
                resolve("pin", &contexts["pin"], id, json!({})),
            )
            .await,
        );
        denied(
            f.call(
                &f.guest,
                "activation.resolve",
                None,
                resolve("rename", &contexts["pin"], id, json!({})),
            )
            .await,
        );
        assert_eq!(f.host.revision(0).await, revision);
        assert_eq!(f.host.committed_effects(id).await, 0);

        let request = resolve("pin", &contexts["pin"], id, json!({}));
        let prepared = f
            .ok(&f.guest, "activation.resolve", None, request.clone())
            .await;
        assert_eq!(prepared["confirm"], "none");
        assert_eq!(
            f.ok(&f.guest, "activation.resolve", None, request).await,
            prepared
        );
        f.host.change_fixture_row(3, i).await;
        let start = std::time::Instant::now();
        let first_result = f
            .ok(&f.guest, "activation.dispatch", None, dispatch(&prepared))
            .await;
        let second_result = f
            .ok(&f.guest, "activation.dispatch", None, dispatch(&prepared))
            .await;
        times.push(start.elapsed()); // Two exchanges, including replay.
        assert_eq!(first_result, second_result);
        assert!(first_result["undo"]["ref"].is_string());
        assert_ne!(f.host.revision(0).await, revision);
        assert_eq!(f.host.committed_effects(id).await, 1);
        denied(
            f.call(
                &f.guest,
                "activation.resolve",
                None,
                resolve("pin", &contexts["pin"], Uuid::new_v4(), json!({})),
            )
            .await,
        );

        let undo = json!({"type":"undo","undo_ref":first_result["undo"]["ref"]});
        denied(f.call(&f.owner, "undo", Some(lease), undo.clone()).await);
        denied(
            f.call(
                &f.guest,
                "undo",
                Some(lease),
                json!({"type":"undo","undo_ref":Uuid::new_v4()}),
            )
            .await,
        );
        if i % 2 == 0 {
            let applied = f.ok(&f.guest, "undo", Some(lease), undo.clone()).await;
            let once = f.host.revision(0).await;
            assert_eq!(f.ok(&f.guest, "undo", Some(lease), undo).await, applied);
            assert_eq!(f.host.revision(0).await, once);
        } else {
            f.host.change_fixture_row(0, i + runs).await;
            let new = f.host.revision(0).await;
            denied(f.call(&f.guest, "undo", Some(lease), undo).await);
            assert_eq!(f.host.revision(0).await, new);
        }

        let current = f.ok(&f.guest, "window", Some(lease), window()).await;
        let context = &current["activation_contexts"][1]["contexts"];
        let previous = f.host.revision(1).await;
        let id = Uuid::new_v4();
        let prefill = f
            .ok(
                &f.guest,
                "activation.resolve",
                None,
                resolve("rename", &context["rename"], id, json!({"dispatch":true})),
            )
            .await;
        assert_eq!(prefill["confirm"], "challenge");
        assert_eq!(prefill["material"][0]["value"], "Untouched fixture prefill");
        assert_eq!(f.host.revision(1).await, previous);
        denied(
            f.call(&f.guest, "activation.dispatch", None, dispatch(&prefill))
                .await,
        );
        let mut forged = confirm(&prefill);
        forged["confirmation_challenge"] = json!(Uuid::new_v4());
        denied(f.call(&f.guest, "activation.confirm", None, forged).await);
        let proof = f
            .ok(&f.guest, "activation.confirm", None, confirm(&prefill))
            .await;
        let mut sent = dispatch(&prefill);
        sent["confirmation_proof"] = proof["confirmation_proof"].clone();
        assert_eq!(
            f.ok(&f.guest, "activation.dispatch", None, sent.clone())
                .await["state"],
            "ok"
        );
        assert_eq!(
            f.ok(&f.guest, "activation.dispatch", None, sent).await["state"],
            "ok"
        );
        assert_eq!(f.host.committed_effects(id).await, 1);
        assert_ne!(f.host.revision(1).await, previous);

        let current = f.ok(&f.guest, "window", Some(lease), window()).await;
        let context = &current["activation_contexts"][2]["contexts"];
        let choice_ctx = &context["choose-destination"];
        let choice_event = |ctx: &Value, slot: &str| json!({"type":"choice_options","activation_context":ctx,"slot":slot});
        let choices = f
            .ok(
                &f.guest,
                "choice.options",
                None,
                choice_event(choice_ctx, "collection"),
            )
            .await;
        assert_eq!(choices["options"].as_array().unwrap().len(), 2);
        denied(
            f.call(
                &f.guest,
                "choice.options",
                None,
                choice_event(choice_ctx, "foreign"),
            )
            .await,
        );
        let attempt = |ctx: &Value, choice: &Value| {
            resolve(
                "choose-destination",
                ctx,
                Uuid::new_v4(),
                json!({"interaction":{"choices":{"collection":choice}}}),
            )
        };
        let picked = &choices["options"][0]["ref"];
        denied(
            f.call(
                &f.guest,
                "activation.resolve",
                None,
                attempt(choice_ctx, &foreign),
            )
            .await,
        );
        denied(
            f.call(
                &f.guest,
                "activation.resolve",
                None,
                attempt(
                    &current["activation_contexts"][0]["contexts"]["choose-destination"],
                    picked,
                ),
            )
            .await,
        );
        let choice = f
            .ok(
                &f.guest,
                "activation.resolve",
                None,
                attempt(choice_ctx, picked),
            )
            .await;
        assert_eq!(choice["confirm"], "challenge");
        assert_eq!(choice["material"][0]["value"], "Review");
        let choice_proof = f
            .ok(&f.guest, "activation.confirm", None, confirm(&choice))
            .await;
        let mut picked = dispatch(&choice);
        picked["confirmation_proof"] = choice_proof["confirmation_proof"].clone();
        assert_eq!(
            f.ok(&f.guest, "activation.dispatch", None, picked.clone())
                .await["state"],
            "ok"
        );
        assert_eq!(
            f.ok(&f.guest, "activation.dispatch", None, picked).await["state"],
            "ok"
        );
        assert_eq!(
            f.host
                .committed_effects(
                    Uuid::parse_str(choice["activation_id"].as_str().unwrap()).unwrap()
                )
                .await,
            1
        );
        denied(
            f.call(
                &f.guest,
                "activation.resolve",
                None,
                attempt(choice_ctx, &choices["options"][0]["ref"]),
            )
            .await,
        );

        let bulk_context = &member["activation_contexts"]["trash-selected"];
        let bulk = |rows: Value| {
            resolve(
                "trash-selected",
                bulk_context,
                Uuid::new_v4(),
                json!({"interaction":{"rows":rows},"dispatch":true}),
            )
        };
        denied(
            f.call(
                &f.guest,
                "activation.resolve",
                None,
                bulk(json!([other, foreign])),
            )
            .await,
        );
        denied(
            f.call(
                &f.guest,
                "activation.resolve",
                None,
                bulk(json!([other, other])),
            )
            .await,
        );
        let accepted = f
            .ok(
                &f.guest,
                "activation.resolve",
                None,
                bulk(json!([other, first])),
            )
            .await;
        assert_eq!(accepted["confirm"], "challenge");
        assert!(accepted["selection_ref"].is_string());
        denied(
            f.call(&f.guest, "activation.dispatch", None, dispatch(&accepted))
                .await,
        );

        let ordinary = f
            .ok(
                &f.guest,
                "activation.resolve",
                None,
                resolve(
                    "touch",
                    &context["touch"],
                    Uuid::new_v4(),
                    json!({"dispatch":true}),
                ),
            )
            .await;
        assert!(ordinary.get("confirm").is_none());
        assert!(ordinary["resolution_ref"].is_string());
        let forged_cache = json!({"type":"window","request":{"bind":"notes","from":0,"to":20},"cached_rows":rows["rows"]});
        denied(f.call(&f.guest, "window", Some(lease), forged_cache).await);

        let mut versions = serde_json::Map::new();
        versions.insert(
            current["window_ref"].as_str().unwrap().into(),
            current["result_version"].clone(),
        );
        versions.insert(
            rows["window_ref"].as_str().unwrap().into(),
            json!(Uuid::new_v4()),
        );
        let renew_id = Uuid::new_v4();
        let renew=f.ok(&f.guest,"view.renew",None,json!({
            "type":"lease_renew","lease_ref":lease,"grant_revision":member["lease"]["grant_revision"],
            "cache_partition":member["lease"]["cache_partition"],"renewal_id":renew_id,"result_versions":versions
        })).await;
        assert_eq!(
            renew["retained_results"],
            json!([{"window_ref":current["window_ref"],"result_version":current["result_version"]}])
        );
        assert_eq!(renew["renewal_id"], json!(renew_id));

        for stage in [1, 2, 3] {
            let opened = f.ok(&f.guest, "view.open", None, open(shared)).await;
            let lease = &opened["lease"]["lease_ref"];
            let rows = f.ok(&f.guest, "window", Some(lease), window()).await;
            let id = Uuid::new_v4();
            let (route, body) = match stage {
                1 => ("window", window()),
                2 => {
                    let r = f
                        .ok(
                            &f.guest,
                            "activation.resolve",
                            None,
                            resolve(
                                "pin",
                                &rows["activation_contexts"][0]["contexts"]["pin"],
                                id,
                                json!({}),
                            ),
                        )
                        .await;
                    ("activation.dispatch", dispatch(&r))
                }
                3 => {
                    let r = f
                        .ok(
                            &f.guest,
                            "activation.resolve",
                            None,
                            resolve(
                                "rename",
                                &rows["activation_contexts"][0]["contexts"]["rename"],
                                id,
                                json!({}),
                            ),
                        )
                        .await;
                    ("activation.confirm", confirm(&r))
                }
                _ => unreachable!(),
            };
            let before = f.host.revision(0).await;
            f.host.arm(stage);
            let (reply, ()) = tokio::join!(f.call(&f.guest, route, Some(lease), body), async {
                f.host.wait_for_race().await;
                f.host.member_role(None).await;
                f.host.finish_race();
            });
            denied(reply);
            assert_eq!(f.host.revision(0).await, before);
            assert_eq!(f.host.committed_effects(id).await, 0);
            denied(f.call(&f.guest, "view.open", None, open(shared)).await);
            f.host.end_lease(&opened);
            f.host.member_role(Some("editor")).await;
        }
        f.host.end_lease(&member);
    }
    fixture::summary("unrelated churn dispatch plus exact replay", &mut times);
    assert!(times[(runs * 95).div_ceil(100) - 1] <= Duration::from_secs(2));
    eprintln!(
        "authority repeats={runs} forged_effects=0 offscope_bytes=0 duplicate_commits=0 revoke_races={}",
        runs * 3
    );
}
