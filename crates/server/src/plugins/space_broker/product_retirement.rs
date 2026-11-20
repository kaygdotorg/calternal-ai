//! Exercise live authority changes with an enrolled, resident KVM worker.
use super::*;
use crate::{
    api::{Shared, dispatch},
    auth::StoredDirectory,
};
use calternal_machine::runner::Runner;

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn authority_revocation_and_space_erasure_reap_resident_kvm_workers() {
    let Some(path) = std::env::var_os("CALTERNAL_SMALL_VM_TEST_CONFIG") else {
        eprintln!("SKIP live KVM fixture: CALTERNAL_SMALL_VM_TEST_CONFIG is unset");
        return;
    };
    exercise_enrolled_runner(Some(path.clone().into()), VmEnd::Revoke).await;
    exercise_enrolled_runner(Some(path.into()), VmEnd::Erase).await;
}

pub(super) async fn check_retirement(
    state: &Shared<StoredDirectory>,
    service: &MachineService,
    runner: &Runner,
    account: AccountId,
    spaces: [SpaceId; 2],
    items: &[Item; 2],
    end: VmEnd,
) {
    let request = |receipt, operation, index: usize| {
        dispatch(
            state.clone(),
            Identity { id: account },
            Session::Static,
            receipt,
            operation,
            serde_json::from_value(json!({"space_id": spaces[index], "item_id": items[index].id}))
                .unwrap(),
        )
    };
    let wait_inventory = |count| async move {
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                if runner.capacity().small_vm.unwrap().resident_count == count {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
    };
    if end == VmEnd::Revoke {
        wait_inventory(1).await;
        tokio::task::block_in_place(|| service.set_pool_grant(account, false)).unwrap();
        assert!(request("revoked-call", "fixture.head", 0).await.is_err());
        wait_inventory(0).await;
    } else {
        assert_eq!(
            request("before-erasure", "fixture.other", 1).await.unwrap()["revision"],
            items[1].revision
        );
        wait_inventory(2).await;
        dispatch(
            state.clone(),
            Identity { id: account },
            Session::Static,
            "erase-resident",
            "spaces.delete",
            serde_json::from_value(json!({"space_id":spaces[1]})).unwrap(),
        )
        .await
        .unwrap();
        assert!(request("erased-call", "fixture.other", 1).await.is_err());
        wait_inventory(1).await;
        let approval = state
            .registry
            .space_item_head(spaces[0], "fixture.head")
            .unwrap();
        approval
            .wake
            .awake
            .lock()
            .await
            .as_ref()
            .unwrap()
            .fence
            .close();
        wait_inventory(0).await;
    }
}
