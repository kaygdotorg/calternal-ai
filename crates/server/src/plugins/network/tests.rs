use super::*;
use crate::{
    api::{AppState, allowed_hosts, router},
    auth::StoredDirectory,
    plugins::{Jobs, Registry, TrustedEntry, WorkerPool, default_row},
    store::PluginItemWrite,
};
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use calternal_core::ContentFetchField;
use calternal_types::Actor;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

#[test]
fn sealed_frame_is_per_person_and_dies_when_the_source_head_changes() {
    let temp = tempfile::tempdir().unwrap();
    let roots = calternal_roots::Roots {
        appdata: temp.path().join("appdata"),
        userdata_ssd: temp.path().join("ssd"),
        userdata_hdd: temp.path().join("hdd"),
    };
    let store = Store::open(
        &roots,
        Uuid::new_v4(),
        true,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
    )
    .unwrap();
    let directory = StoredDirectory::load_or_create(&roots.appdata).unwrap();
    let owner_token = std::fs::read(roots.appdata.join("dev-tokens/owner.token")).unwrap();
    let guest_token = std::fs::read(roots.appdata.join("dev-tokens/guest.token")).unwrap();
    let owner = directory.authenticate(&owner_token).unwrap();
    let shared = directory
        .spaces
        .list(owner.id)
        .unwrap()
        .into_iter()
        .find(|space| !space.personal)
        .unwrap()
        .id;
    let manifest = serde_json::from_value(json!({
        "id": "mail-fixture", "name": "Mail fixture", "version": "1", "icon": "mail", "api_version": 1,
        "operations": [], "views": [], "attention": [], "collections": [],
        "settings": [], "schedules": [], "webhooks": [], "secrets": [], "hooks": [], "themes": [],
        "kinds": [{"id": "mail-fixture", "schema": true, "storage": {"hidden": true}, "claimed_text": true}],
        "host_calls": ["content_prepare_v1"],
        "network": {"content_fetch": {"profile": "images_fonts", "sources": [{"kind": "mail-fixture", "field": "text"}]}}
    })).unwrap();
    let plugin = Plugin::new(
        TrustedEntry {
            id: "mail-fixture".into(),
            version: "1".into(),
            manifest: "manifest.json".into(),
            worker: None,
            worker_sha256: None,
            default_enabled: true,
        },
        manifest,
        None,
    )
    .unwrap();
    assert!(!default_row(&plugin).enabled);
    let actor = Actor::account(owner.id);
    store
        .provision_plugin(shared, actor.clone(), &plugin.kind_registrations(), &[])
        .unwrap();
    store
        .set_plugin_enabled(
            actor.clone(),
            "enable",
            "plugins.enable",
            "enable",
            shared,
            &plugin.manifest.id,
            default_row(&plugin),
            true,
            |_| json!({}),
        )
        .unwrap();
    let saved = store
        .write_item(
            actor.clone(),
            "write-1",
            "write-1",
            shared,
            "mail-fixture",
            None,
            None,
            PluginItemWrite::Replace {
                metadata: json!({}),
                text: r#"<p>hello</p><img src="https://example.com/image.png?key=hidden">"#,
            },
            |_| {},
        )
        .unwrap();
    let id = saved["id"].as_str().unwrap();
    let revision = saved["revision"].as_str().unwrap();
    let frame = prepare(
        &store,
        owner.id,
        Session::Static,
        shared,
        &plugin,
        activation(&store, shared, &plugin).unwrap().generation,
        ContentPrepareInput {
            kind: "mail-fixture".into(),
            item_id: id.into(),
            revision: revision.into(),
            field: ContentFetchField::Text,
        },
    )
    .unwrap();
    let state = Arc::new(AppState {
        inputs: store.account_inputs(),
        store,
        directory,
        registry: Registry::from_plugins(vec![plugin]),
        workers: WorkerPool::new(None),
        jobs: Jobs::disabled(),
        allowed_hosts: allowed_hosts(0),
        blocking: crate::blocking::RequestWorkers::new().unwrap(),
    });
    let path = view(&state, owner.id, shared, &frame.frame_id)
        .unwrap()
        .frame_url;
    let app = router(state.clone(), None);
    let get = |token: &[u8]| {
        let token = std::str::from_utf8(token).unwrap().trim();
        app.clone().oneshot(
            Request::get(&path)
                .header(header::HOST, "localhost:0")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let denied = get(&guest_token).await.unwrap();
        assert_eq!(denied.status(), StatusCode::NOT_FOUND);
        let allowed = get(&owner_token).await.unwrap();
        assert_eq!(allowed.status(), StatusCode::OK);
        assert_eq!(allowed.headers()[header::X_FRAME_OPTIONS], "SAMEORIGIN");
        assert!(
            allowed.headers()[header::CONTENT_SECURITY_POLICY]
                .to_str()
                .unwrap()
                .contains("sandbox;")
        );
        let html = allowed.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&html).unwrap();
        assert!(html.contains("<p>hello</p>"));
        assert!(!html.contains("example.com"));
        assert!(!html.contains("key=hidden"));
    });

    state
        .store
        .write_item(
            actor,
            "write-2",
            "write-2",
            shared,
            "mail-fixture",
            Some(Uuid::parse_str(id).unwrap()),
            Some(revision),
            PluginItemWrite::Replace {
                metadata: json!({}),
                text: "<p>replacement</p>",
            },
            |_| {},
        )
        .unwrap();
    assert_eq!(
        runtime.block_on(get(&owner_token)).unwrap().status(),
        StatusCode::NOT_FOUND
    );
}

#[test]
fn asset_signature_binds_index_and_expiry() {
    let key = hmac::Key::new(hmac::HMAC_SHA256, &[7; 32]);
    let frame = Uuid::new_v4();
    let token = asset_signature(&key, frame, 0, 100);
    assert!(asset_authorized(&key, frame, 0, 100, &token, 99));
    assert!(!asset_authorized(&key, frame, 1, 100, &token, 99));
    assert!(!asset_authorized(&key, frame, 0, 101, &token, 99));
    assert!(!asset_authorized(&key, frame, 0, 100, &token, 100));
}
