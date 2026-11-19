//! Authority and secure-input tests for the named IMAPS broker.

use super::*;
use crate::{
    api::{AppState, allowed_hosts},
    auth::StoredDirectory,
    plugins::{Jobs, Registry, TrustedEntry, WorkerPool, default_row},
    store::Store,
};
use calternal_secrets::{RequestId, SecretValue};

#[tokio::test]
async fn mail_prompt_is_account_owned_and_connection_fenced() {
    let root = tempfile::tempdir().unwrap();
    let roots = calternal_roots::Roots {
        appdata: root.path().join("appdata"),
        userdata_ssd: root.path().join("ssd"),
        userdata_hdd: root.path().join("hdd"),
    };
    let store = Store::open(
        &roots,
        Uuid::new_v4(),
        true,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
    )
    .unwrap();
    let directory = StoredDirectory::load_or_create(&roots.appdata).unwrap();
    let identity = |role: &str| {
        let token = std::fs::read(roots.appdata.join(format!("dev-tokens/{role}.token"))).unwrap();
        directory.authenticate(&token).unwrap()
    };
    let owner = identity("owner");
    let guest = identity("guest");
    let spaces = directory.spaces.list(owner.id).unwrap();
    let personal = spaces.iter().find(|space| space.personal).unwrap().id;
    let shared = spaces.iter().find(|space| !space.personal).unwrap().id;
    let manifest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../plugins/calternal-mail/manifest.json"
    )))
    .unwrap();
    let plugin = Plugin::new(
        TrustedEntry {
            id: "calternal-mail".into(),
            version: "0.1.0".into(),
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
        .provision_plugin(personal, actor.clone(), &plugin.kind_registrations(), &[])
        .unwrap();
    store
        .set_plugin_enabled(
            actor.clone(),
            "enable-mail",
            "plugins.enable",
            "enable-mail",
            personal,
            &plugin.manifest.id,
            default_row(&plugin),
            true,
            |_| json!({}),
        )
        .unwrap();
    let generation = activation(&store, personal, &plugin).unwrap().generation;
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
    let plugin = state
        .registry
        .snapshot()
        .get("calternal-mail")
        .unwrap()
        .clone();
    let broker = |identity, space| {
        SpaceHost::new(
            state.clone(),
            identity,
            Session::Static,
            space,
            plugin.clone(),
            generation,
            "mail".into(),
        )
    };
    let guest_id = guest.id;
    let owner_broker = broker(owner, personal);
    let shared_broker = broker(owner, shared);
    let foreign_broker = broker(guest, personal);
    tokio::task::spawn_blocking(move || {
        let mut request = json!({
            "connection": "inbox",
            "endpoint": {"host": "MAIL.example.test", "port": 993, "username": "reader"},
            "action": {"type": "request_password"},
        });
        assert!(matches!(foreign_broker.mail(&request), Err(Error::Refused)));
        assert!(matches!(shared_broker.mail(&request), Err(Error::Refused)));
        request["connection"] = json!("wrong");
        assert!(matches!(owner_broker.mail(&request), Err(Error::Refused)));
        request["connection"] = json!("inbox");
        request["endpoint"]["password"] = json!("unexpected field");
        assert!(matches!(owner_broker.mail(&request), Err(Error::Invalid)));
        request["endpoint"]
            .as_object_mut()
            .unwrap()
            .remove("password");

        let first = owner_broker.mail(&request).unwrap();
        assert_eq!(first["ready"], false);
        assert_eq!(first, owner_broker.mail(&request).unwrap());
        let id = Uuid::parse_str(first["secret_request_id"].as_str().unwrap()).unwrap();
        let secrets = state.store.secrets();
        let owner_scope = Owner::Account(owner.id);
        assert!(
            secrets
                .pending(&Owner::Account(guest_id))
                .unwrap()
                .is_empty()
        );
        assert!(secrets.pending(&Owner::Space(personal)).unwrap().is_empty());
        let pending = secrets.pending(&owner_scope).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, id);
        assert!(pending[0].reason.contains("mail.example.test:993 (reader)"));
        secrets
            .provide(
                &owner_scope,
                &RequestId(id),
                SecretValue::new(Uuid::new_v4().to_string().into_bytes()).unwrap(),
                &actor,
            )
            .unwrap();

        let ready = owner_broker.mail(&request).unwrap();
        assert_eq!(
            ready,
            json!({ "ready": true, "endpoint_key": first["endpoint_key"] })
        );
        request["endpoint"]["port"] = json!(1993);
        assert_eq!(owner_broker.mail(&request).unwrap()["ready"], false);
    })
    .await
    .unwrap();
}
