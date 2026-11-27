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
