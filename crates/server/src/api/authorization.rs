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

/// Check current delivery authority after the handler finishes. A sufficient
/// space role needs one protected read. Item-grant fallback also reads current
/// collection membership and propagates lookup failures before delivery.
pub(super) async fn authorize_delivery<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    policy: &authz::Policy,
    space: SpaceId,
    item: Option<uuid::Uuid>,
) -> Result<(), Error> {
    let direct = policy.check(capabilities(state, account, space).await?);
    match (direct, item) {
        (Err(_), Some(id)) => {
            let (caps, _) =
                crate::resolve::item_capabilities(state, account, space, "notes", id).await?;
            policy.check(caps)
        }
        (result, _) => result,
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

    fn delivery_fixture() -> (
        tempfile::TempDir,
        Shared,
        calternal_auth::spaces::DevelopmentIds,
    ) {
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
        let ids = directory.spaces.seed_development().unwrap();
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
        (root, state, ids)
    }

    #[test]
    fn owner_note_delivery_checks_current_authority_while_the_content_writer_is_held() {
        let (_root, state, ids) = delivery_fixture();
        let note = state
            .store
            .create(
                ids.owner,
                "create",
                "notes.create",
                "create",
                ids.shared,
                "Delivery fixture",
                "body",
            )
            .unwrap();
        let item = uuid::Uuid::parse_str(&note.id).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (entered, ready) = tokio::sync::oneshot::channel();
            let (release, blocked) = std::sync::mpsc::channel();
            let holder_state = state.clone();
            let holder = tokio::task::spawn_blocking(move || {
                holder_state
                    .store
                    .content()
                    .with_live_metadata(ids.shared, "notes", item, |_| {
                        let _ = entered.send(());
                        let _ = blocked.recv();
                        Ok(())
                    })
                    .unwrap();
            });
            ready.await.unwrap();
            let policy = authz::policy("notes.update").unwrap();
            let delivered = tokio::time::timeout(
                Duration::from_secs(1),
                authorize_delivery(&state, ids.owner, policy, ids.shared, Some(item)),
            )
            .await;
            release.send(()).unwrap();
            holder.await.unwrap();
            delivered
                .expect("the current space role must be checked independently of placement")
                .unwrap();
        });
    }

    #[test]
    fn item_only_note_delivery_refuses_a_revoked_edit_grant() {
        let (_root, state, ids) = delivery_fixture();
        let space = ids.personal;
        let note = state
            .store
            .create(
                ids.owner,
                "create",
                "notes.create",
                "create",
                space,
                "Shared delivery fixture",
                "body",
            )
            .unwrap();
        let item = uuid::Uuid::parse_str(&note.id).unwrap();
        let directory = &state.directory.spaces;
        assert!(directory.capabilities(ids.guest, space).unwrap().is_none());
        let grant = directory
            .grant_item(
                ids.owner,
                space,
                item,
                "notes".into(),
                calternal_auth::sharing::Audience::Account(ids.guest),
                true,
                ("grant", "sharing.grant", "grant"),
            )
            .unwrap();
        let grant = uuid::Uuid::parse_str(grant["grant_id"].as_str().unwrap()).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let policy = authz::policy("notes.update").unwrap();
        runtime
            .block_on(authorize_delivery(
                &state,
                ids.guest,
                policy,
                space,
                Some(item),
            ))
            .unwrap();

        let delivery = authorize_delivery(&state, ids.guest, policy, space, Some(item));
        directory
            .revoke_item(
                ids.owner,
                space,
                item,
                grant,
                ("revoke", "sharing.revoke", "revoke"),
            )
            .unwrap();
        assert_eq!(
            runtime.block_on(delivery).unwrap_err().code,
            calternal_core::ErrorCode::NotFound
        );
    }
}
