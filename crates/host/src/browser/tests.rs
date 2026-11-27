use super::*;

#[tokio::test]
async fn development_sign_in_seeds_both_bundles_once() {
    use crate::WorkerRuntime;
    use calternal_auth::Auth;
    use std::path::PathBuf;

    fn executable(name: &str) -> PathBuf {
        std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|dir| dir.join(name))
            .find(|path| path.is_file())
            .unwrap()
            .canonicalize()
            .unwrap()
    }

    let data = tempfile::tempdir().unwrap();
    let root = data.path().to_owned();
    let config = AuthConfig::new("http://localhost").unwrap();
    let auth_path = root.join("auth");
    let auth_config = config.clone();
    let auth = tokio::task::spawn_blocking(move || Auth::open(&auth_path, auth_config))
        .await
        .unwrap()
        .unwrap();
    let account = auth.create("Dev owner").await.unwrap().id;
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let runtime = WorkerRuntime::new(
        executable("bwrap"),
        executable("node"),
        repo.join("packages/plugin-sdk"),
    )
    .unwrap();
    let host = Host::new(root, auth, 7)
        .unwrap()
        .with_worker_runtime(runtime)
        .with_development_plugins(
            ["calternal-chat", "calternal-chat-claude-code"]
                .map(|id| (id.to_owned(), repo.join("plugins-v2").join(id))),
        );
    let browser = Browser {
        host: Arc::new(host),
        config: Arc::new(config),
    };
    let (_, space) = personal_space(&browser.host, account).await.ok().unwrap();
    for _ in 0..2 {
        let result = signed_in(
            &browser,
            account,
            "synthetic-fixture-session".into(),
            Vec::new(),
        )
        .await;
        assert_eq!(result.ok().unwrap().status(), StatusCode::OK);
    }
    let result = browser
        .host
        .clone()
        .trusted_call(
            account.into(),
            space,
            Call {
                call: "s.declarations".into(),
                input: json!({}),
                idempotency_key: None,
                slot_selection: None,
            },
        )
        .await
        .unwrap();
    let plugins = result["plugins"].as_array().unwrap();
    for id in ["calternal-chat", "calternal-chat-claude-code"] {
        assert_eq!(
            plugins.iter().filter(|plugin| plugin["id"] == id).count(),
            1
        );
    }
}

#[tokio::test]
async fn development_status_backfills_missing_bundles_and_preserves_existing_items() {
    use crate::WorkerRuntime;
    use calternal_auth::Auth;
    use std::path::PathBuf;

    fn executable(name: &str) -> PathBuf {
        std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|dir| dir.join(name))
            .find(|path| path.is_file())
            .unwrap()
            .canonicalize()
            .unwrap()
    }

    let data = tempfile::tempdir().unwrap();
    let root = data.path().to_owned();
    let config = AuthConfig::new("http://localhost").unwrap();
    let auth_path = root.join("auth");
    let auth_config = config.clone();
    let auth = tokio::task::spawn_blocking(move || Auth::open(&auth_path, auth_config))
        .await
        .unwrap()
        .unwrap();
    let account = auth.create("Dev owner").await.unwrap().id;
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let runtime = WorkerRuntime::new(
        executable("bwrap"),
        executable("node"),
        repo.join("packages/plugin-sdk"),
    )
    .unwrap();
    let original = ["calternal-chat", "calternal-chat-claude-code"];
    let added = ["calternal-reading-list", "calternal-tasks"];
    let bundle = |id: &str| (id.to_owned(), repo.join("plugins-v2").join(id));
    let host = Host::new(root.clone(), auth.clone(), 7)
        .unwrap()
        .with_worker_runtime(runtime.clone())
        .with_development_plugins(original.map(bundle));
    let browser = Browser {
        host: Arc::new(host),
        config: Arc::new(config),
    };
    let (_, space) = personal_space(&browser.host, account).await.ok().unwrap();
    let signed_in = signed_in(
        &browser,
        account,
        "synthetic-fixture-session".into(),
        Vec::new(),
    )
    .await;
    assert_eq!(signed_in.ok().unwrap().status(), StatusCode::OK);
    let call = |host: Arc<Host>, name: &str, input| {
        host.trusted_call(
            account.into(),
            space,
            Call {
                call: name.into(),
                input,
                idempotency_key: None,
                slot_selection: None,
            },
        )
    };
    let created = call(
        browser.host.clone(),
        "s.write",
        json!({"changes":[
            {"op":"add","kind":"calternal-chat.chats","fields":{"title":"Existing item"}}
        ]}),
    )
    .await
    .unwrap();
    let item = created["changes"][0]["id"].clone();
    drop(browser);

    let host = Arc::new(
        Host::new(root, auth, 7)
            .unwrap()
            .with_worker_runtime(runtime)
            .with_development_plugins(original.into_iter().chain(added).map(bundle)),
    );
    for _ in 0..2 {
        let session = session_payload(&host, account).await.ok().unwrap();
        assert_eq!(session["session"]["space"], json!(space));
    }
    let catalog = call(host.clone(), "s.declarations", json!({}))
        .await
        .unwrap();
    let plugins = catalog["plugins"].as_array().unwrap();
    assert_eq!(plugins.len(), 4);
    for id in original.into_iter().chain(added) {
        assert_eq!(
            plugins.iter().filter(|plugin| plugin["id"] == id).count(),
            1
        );
    }
    let item = call(host, "s.read", json!({"item":item})).await.unwrap();
    assert_eq!(item["fields"]["title"], "Existing item");
    assert_eq!(item["version"], 1);
}

#[tokio::test]
async fn committed_enrollment_survives_retryable_seed_failure() {
    use calternal_auth::Auth;

    let data = tempfile::tempdir().unwrap();
    let root = data.path().to_owned();
    let config = AuthConfig::new("http://localhost").unwrap();
    let auth_path = root.join("auth");
    let auth_config = config.clone();
    let auth = tokio::task::spawn_blocking(move || Auth::open(&auth_path, auth_config))
        .await
        .unwrap()
        .unwrap();
    let account = auth.create("Dev owner").await.unwrap().id;
    let bundle = root.join("delayed-bundle");
    let bundles = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins-v2");
    let browser = Browser {
        host: Arc::new(Host::new(root, auth, 7).unwrap().with_development_plugins([
            (
                "calternal-reading-list".into(),
                bundles.join("calternal-reading-list"),
            ),
            ("delayed-bundle".into(), bundle.clone()),
            ("calternal-tasks".into(), bundles.join("calternal-tasks")),
        ])),
        config: Arc::new(config),
    };
    let response = signed_in(
        &browser,
        account,
        "synthetic-fixture-session".into(),
        vec!["synthetic-recovery".into()],
    )
    .await
    .ok()
    .expect("seed failure must preserve the committed sign-in response");
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key(header::SET_COOKIE));
    let body = to_bytes(response.into_body(), AUTH_BYTES).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["recovery_codes"], json!(["synthetic-recovery"]));
    assert_eq!(body["initialization"]["status"], "pending");

    let pending = session_payload(&browser.host, account).await.ok().unwrap();
    assert_eq!(pending["session"]["account"], json!(account));
    assert_eq!(pending["initialization"]["status"], "pending");
    let space = serde_json::from_value(pending["session"]["space"].clone()).unwrap();
    let catalog = |host: Arc<Host>| {
        host.trusted_call(
            account.into(),
            space,
            Call {
                call: "s.declarations".into(),
                input: json!({}),
                idempotency_key: None,
                slot_selection: None,
            },
        )
    };
    let installed = catalog(browser.host.clone()).await.unwrap();
    assert_eq!(installed["plugins"].as_array().unwrap().len(), 2);
    let sign_in = signed_in(
        &browser,
        account,
        "synthetic-fixture-session".into(),
        Vec::new(),
    )
    .await
    .ok()
    .unwrap();
    assert_eq!(sign_in.status(), StatusCode::OK);
    let sign_in = to_bytes(sign_in.into_body(), AUTH_BYTES).await.unwrap();
    let sign_in: Value = serde_json::from_slice(&sign_in).unwrap();
    assert_eq!(sign_in["initialization"]["status"], "pending");
    assert!(sign_in.get("recovery_codes").is_none());

    tokio::fs::create_dir(&bundle).await.unwrap();
    tokio::fs::write(
        bundle.join("plugin.md"),
        "---\nplugin: delayed-bundle\nname: Delayed Bundle\nversion: 1.0.0\nformat: 1\n---\n",
    )
    .await
    .unwrap();
    let session = session_payload(&browser.host, account).await.ok().unwrap();
    assert_eq!(session["session"]["account"], json!(account));
    assert!(session.get("initialization").is_none());
    let installed = catalog(browser.host.clone()).await.unwrap();
    assert_eq!(installed["plugins"].as_array().unwrap().len(), 3);
}
