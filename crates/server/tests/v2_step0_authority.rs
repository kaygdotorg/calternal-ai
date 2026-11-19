//! Scripted client and launch command for the disposable sealed host.
#[path = "../../../spikes/plugin-api-v2/authority/mod.rs"]
mod authority;
#[path = "../../../spikes/plugin-api-v2/fixture.rs"]
mod fixture;

use authority::Host;
use reqwest::Client;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;

struct ClientFixture {
    host: Arc<Host>,
    http: Client,
    base: String,
    owner: String,
    guest: String,
    task: tokio::task::JoinHandle<()>,
    _root: tempfile::TempDir,
}

fn token(path: &Path, name: &str) -> String {
    std::fs::read_to_string(path.join(format!("appdata/dev-tokens/{name}.token"))).unwrap()
}

impl ClientFixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let host = Host::create(&fixture::roots(root.path())).await;
        let owner = token(root.path(), "owner");
        let guest = token(root.path(), "guest");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/step0/v2", listener.local_addr().unwrap());
        let app = host.clone().router();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let http = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        Self {
            host,
            http,
            base,
            owner,
            guest,
            task,
            _root: root,
        }
    }

    async fn bootstrap(&self, token: &str) -> Value {
        self.http
            .get(format!("{}/fixture", self.base))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    async fn call(
        &self,
        token: &str,
        route: &str,
        lease: Option<&Value>,
        input: Value,
    ) -> (u16, Value) {
        let mut req = self
            .http
            .post(format!("{}/{route}", self.base))
            .bearer_auth(token)
            .json(&input);
        if let Some(lease) = lease {
            req = req.header("x-step0-view-lease", lease.as_str().unwrap());
        }
        let reply = req.send().await.unwrap();
        let status = reply.status().as_u16();
        (status, reply.json().await.unwrap())
    }

    async fn ok(&self, token: &str, route: &str, lease: Option<&Value>, input: Value) -> Value {
        let (status, result) = self.call(token, route, lease, input).await;
        assert_eq!(status, 200, "{route} {result}");
        result
    }
}
impl Drop for ClientFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn open(id: &Value) -> Value {
    json!({"type":"view_open","view_ref":id})
}
fn window() -> Value {
    json!({"type":"window","request":{"bind":"notes","from":0,"to":20}})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "disposable Step 0 authority host smoke"]
async fn sealed_view_and_window() {
    let f = ClientFixture::new().await;
    let routes = f.bootstrap(&f.owner).await;
    let own = f
        .ok(&f.owner, "view.open", None, open(&routes["views"]["home"]))
        .await;
    assert_eq!(own["audience"], "own_space_member");
    let rows = f
        .ok(
            &f.owner,
            "window",
            Some(&own["lease"]["lease_ref"]),
            window(),
        )
        .await;
    assert_eq!(rows["rows"].as_array().unwrap().len(), 4);
    assert_eq!(rows["rows"][0]["fields"]["title"], "Project brief");
    let member = f
        .ok(
            &f.guest,
            "view.open",
            None,
            open(&routes["views"]["shared"]),
        )
        .await;
    let visible = f
        .ok(
            &f.guest,
            "window",
            Some(&member["lease"]["lease_ref"]),
            window(),
        )
        .await;
    assert_eq!(visible["audience"], "member");
    assert_eq!(visible["rows"].as_array().unwrap().len(), 4);
    assert_eq!(visible["activation_contexts"], json!([]));
    assert_eq!(
        f.call(&f.guest, "view.open", None, open(&routes["views"]["home"]))
            .await,
        (404, json!({"error":"not_found"}))
    );
    let grant = f.host.sharing(true).await.unwrap();
    let guest = f
        .ok(&f.guest, "view.open", None, open(&routes["views"]["home"]))
        .await;
    let lease = &guest["lease"]["lease_ref"];
    let visible = f.ok(&f.guest, "window", Some(lease), window()).await;
    assert_eq!(visible["audience"], "guest");
    assert_eq!(visible["rows"].as_array().unwrap().len(), 1);
    assert!(visible.get("count").is_none());
    f.host.unshare(grant).await;
    assert_eq!(
        f.call(&f.guest, "window", Some(lease), window()).await,
        (404, json!({"error":"not_found"}))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "serve a disposable authority fixture for renderer integration"]
async fn serve_authority_fixture() {
    let root = tempfile::tempdir().unwrap();
    let host = Host::create(&fixture::roots(root.path())).await;
    let port = std::env::var("STEP0_AUTHORITY_PORT").map_or(43125, |s| s.parse::<u16>().unwrap());
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap();
    println!("BASE http://{}/step0/v2", listener.local_addr().unwrap());
    for name in ["owner", "guest"] {
        let path: PathBuf = root.path().join(format!("appdata/dev-tokens/{name}.token"));
        println!("{name} bearer file {}", path.display());
    }
    println!(
        "Use GET /step0/v2/fixture with a bearer to discover authorized view refs and the template."
    );
    axum::serve(listener, host.router())
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.unwrap();
        })
        .await
        .unwrap();
}

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
                    "remind",
                    &context["remind"],
                    Uuid::new_v4(),
                    json!({"dispatch":true}),
                ),
            )
            .await;
        assert!(ordinary.get("confirm").is_none());
        assert!(ordinary["resolution_ref"].is_string());

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
