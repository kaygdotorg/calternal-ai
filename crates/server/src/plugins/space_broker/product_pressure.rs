//! Real KVM fixture for pressure sleep and the following disk wake.
use super::*;
use crate::{
    api::{Shared, dispatch},
    auth::StoredDirectory,
};
use calternal_machine::runner::Runner;

pub(super) async fn check_pressure(
    state: &Shared<StoredDirectory>,
    runner: &Runner,
    account: AccountId,
    spaces: [SpaceId; 2],
    items: &[Item; 2],
    run: &Path,
    pid: &str,
) {
    let dispatch_read = |request, space, item, operation| {
        dispatch(
            state.clone(),
            Identity { id: account },
            Session::Static,
            request,
            operation,
            serde_json::from_value(json!({"space_id":space,"item_id":item})).unwrap(),
        )
    };
    let expected = |item: &Item| json!({"metadata":item.metadata,"revision":item.revision});
    let stats = runner.capacity().small_vm.unwrap();
    assert_eq!(
        stats.idle_seconds,
        calternal_machine::protocol::SMALL_VM_IDLE_SECONDS
    );
    assert_eq!(stats.max_residents, 1);
    assert_eq!(stats.resident_count, 1);
    assert_eq!(stats.resident_memory_bytes, stats.memory_limit_bytes);
    assert_eq!(
        dispatch_read("resident", spaces[0], items[0].id, "fixture.head")
            .await
            .unwrap(),
        expected(&items[0]),
    );
    assert_eq!(firecracker_pid(run).await, pid);
    // The fixture requires a burst on the first invocation of each
    // fresh package. Coalesce the concurrent arrivals behind eviction.
    let requests: Vec<_> = (0..MAX_ACTIVE_TICKETS)
        .map(|i| format!("pressure-{i}"))
        .collect();
    let burst = |space, item, operation| {
        futures_util::future::join_all(
            requests
                .iter()
                .map(move |request| dispatch_read(request, space, item, operation)),
        )
    };
    for result in burst(spaces[1], items[1].id, "fixture.other").await {
        assert_eq!(result.unwrap(), expected(&items[1]));
    }
    assert!(!tokio::fs::try_exists(format!("/proc/{pid}")).await.unwrap());
    let second = firecracker_pid(run).await;
    assert_ne!(pid, second);
    assert_eq!(runner.capacity().small_vm.unwrap().resident_count, 1);
    for result in burst(spaces[0], items[0].id, "fixture.head").await {
        assert_eq!(result.unwrap(), expected(&items[0]));
    }
    assert!(
        !tokio::fs::try_exists(format!("/proc/{second}"))
            .await
            .unwrap()
    );
    let restored = firecracker_pid(run).await;
    assert!(!launched_cold(&restored).await);
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
    tokio::time::timeout(Duration::from_secs(5), async {
        while tokio::fs::try_exists(format!("/proc/{restored}"))
            .await
            .unwrap()
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}
