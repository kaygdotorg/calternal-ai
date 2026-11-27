//! Regressions for the ordering of run authority and persistent host effects.
use super::*;
use crate::worker_calls::Chain;
use calternal_auth::{Auth, AuthConfig};

fn call(name: &str, input: Value) -> Call {
    Call {
        call: name.into(),
        input,
        idempotency_key: None,
        slot_selection: None,
    }
}

#[tokio::test]
async fn stop_fences_a_queued_callback_before_persistent_publication() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().to_owned();
    let (host, owner, space, item) = tokio::task::spawn_blocking(move || {
        let auth = Auth::open(
            &path.join("auth"),
            AuthConfig::new("http://localhost").unwrap(),
        )
        .unwrap();
        let owner = crate::block_on(auth.create("Owner")).unwrap().unwrap().id;
        let space = auth
            .spaces()
            .list(owner)
            .unwrap()
            .into_iter()
            .find(|space| space.personal)
            .unwrap()
            .id;
        let backend = Arc::new(PodmanBackend::new("podman", path.join("runs"), "unused"));
        let host = Arc::new(Host::new(path, auth, 7).unwrap()).with_podman_runs(backend, "unused");
        let bundle = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../plugins-v2/calternal-chat");
        host.install_local(owner, space, &bundle, true).unwrap();
        let created = host.call(Actor::account(owner), space, call("s.write", json!({
            "changes":[{"op":"add","kind":"calternal-chat.chats","fields":{"title":"Target"}}]
        }))).unwrap();
        let item: Uuid = serde_json::from_value(created["changes"][0]["id"].clone()).unwrap();
        (host, owner, space, item)
    })
    .await
    .unwrap();
    let id = Uuid::now_v7();
    let grant = RunScopeGrant { id, space, item };
    let actor = Actor::run(owner, "calternal-chat", id);
    let started_by = Actor::plugin(owner, "calternal-chat");
    host.run_storage(started_by.clone(), space, move |storage| {
        storage.runs.entries.push_back(Arc::new(Run {
            grant,
            actor: started_by,
            target_kind: "calternal-chat.chats".into(),
            cancelled: watch::channel(false).0,
            log: Log::default(),
            notify: storage.runs.logs.clone(),
            stopped: AtomicBool::new(false),
            sends: AsyncMutex::default(),
        }));
        Ok(())
    })
    .await
    .unwrap();

    let mut chain = Chain::default();
    chain.run = Some(grant);
    let queued = host.clone().dispatch(
        actor,
        space,
        call(
            "s.write",
            json!({"changes":[{"op":"append record","item":item,"record":{"body":"Late reply"}}]}),
        ),
        chain,
    );
    let owner = Actor::account(owner);
    host.clone()
        .run_stop(owner.clone(), space, json!({"id":id}), None)
        .await
        .unwrap();
    assert_eq!(queued.await.unwrap_err().code, ErrorCode::NotFound);
    let current = host
        .trusted_call(owner, space, call("s.read", json!({"item":item})))
        .await
        .unwrap();
    assert_eq!(current["records"], json!([]));
}
