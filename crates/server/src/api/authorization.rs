//! Request policy admission, separate from storage-handler scheduling.
use super::*;

/// Read current authority without occupying a general blocking slot on native
/// storage. Synchronous-only providers retain their explicit blocking adapter.
/// Cost: one backend query and fixed-size capability set; no cross-request cache.
pub(crate) async fn capabilities<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    space: SpaceId,
) -> Result<Option<calternal_types::CapabilitySet>, Error> {
    if let Some(read) = state.directory.capabilities_async(account, space) {
        return Ok(read.await);
    }
    let state = state.clone();
    tokio::task::spawn_blocking(move || state.directory.capabilities(account, space))
        .await
        .map_err(|_| Error::internal("directory task failed"))
}

/// Check one freshly read capability set before handlers or receipt replay.
/// Scope/schema errors match the synchronous policy path. Native checks await the
/// existing authority mailbox without a general blocking slot or body copy.
pub(super) async fn authorize<D: Directory>(
    state: &Shared<D>,
    person: &Identity,
    policy: &authz::Policy,
    input: &JsonObject,
) -> Result<(), Error> {
    match policy.scope_space(input)? {
        Some(space) => policy.check(capabilities(state, person.id, space).await?),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn asynchronous_grants_observe_revocation_before_poll() {
        let root = tempfile::tempdir().unwrap();
        let directory = StoredDirectory::load_or_create(root.path()).unwrap();
        let ids = directory.spaces.seed_development().unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert!(
            runtime
                .block_on(directory.capabilities_async(ids.guest, ids.shared).unwrap())
                .is_some()
        );
        let pending = directory.capabilities_async(ids.guest, ids.shared).unwrap();
        directory
            .spaces
            .remove(ids.owner, ids.shared, ids.guest)
            .unwrap();
        assert!(runtime.block_on(pending).is_none());
    }

    #[test]
    fn stored_capability_read_does_not_wait_for_a_general_blocking_slot() {
        // Create actual stored authority before entering the runtime. This tests
        // scheduling against the real auth worker, not a cached fake grant.
        let root = tempfile::tempdir().unwrap();
        let roots = calternal_roots::Roots {
            appdata: root.path().join("appdata"),
            userdata_ssd: root.path().join("ssd"),
            userdata_hdd: root.path().join("hdd"),
        };
        let store = Store::open(
            &roots,
            uuid::Uuid::from_u128(1),
            true,
            calternal_content::DEFAULT_RECEIPT_RETENTION,
        )
        .unwrap();
        let directory = StoredDirectory::load_or_create(&roots.appdata).unwrap();
        let token = std::fs::read(roots.appdata.join("dev-tokens/guest.token")).unwrap();
        let person = directory.authenticate(&token).unwrap();
        let space = directory
            .spaces
            .list(person.id)
            .unwrap()
            .into_iter()
            .find(|s| !s.personal)
            .unwrap()
            .id;
        let state = Arc::new(AppState {
            blocking: crate::blocking::RequestWorkers::new().unwrap(),
            inputs: store.account_inputs(),
            store,
            directory,
            registry: Registry::empty(),
            workers: WorkerPool::new(None),
            jobs: Jobs::disabled(),
            allowed_hosts: allowed_hosts(0),
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let completed = runtime.block_on(async {
            let (entered, ready) = tokio::sync::oneshot::channel();
            let (release, blocked) = std::sync::mpsc::channel();
            let holder = tokio::task::spawn_blocking(move || {
                let _ = entered.send(());
                blocked.recv().unwrap();
            });
            ready.await.unwrap();
            let policy = authz::policy("plugins.list").unwrap();
            let input = serde_json::from_value(serde_json::json!({"space_id":space})).unwrap();
            let read = tokio::time::timeout(
                Duration::from_secs(1),
                authorize(&state, &person, policy, &input),
            )
            .await;
            release.send(()).unwrap();
            holder.await.unwrap();
            read.is_ok_and(|result| result.is_ok())
        });
        drop(runtime);
        assert!(
            completed,
            "a current capability read queued behind unrelated blocking work"
        );
    }
}
