use super::*;
use std::time::{Duration, Instant};

#[test]
fn search_worker_exits_when_app_state_drops_while_content_is_retained() {
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
    let content = store.shared_content();
    let state = Arc::new(AppState {
        blocking: crate::blocking::RequestWorkers::new().unwrap(),
        inputs: store.account_inputs(),
        store,
        directory: StoredDirectory::load_or_create(&roots.appdata).unwrap(),
        registry: Registry::empty(),
        workers: WorkerPool::new(None),
        jobs: Jobs::disabled(),
        allowed_hosts: allowed_hosts(0),
    });
    let worker = crate::api::configure_search(&state).unwrap();
    drop(state);

    let deadline = Instant::now() + Duration::from_secs(2);
    while !worker.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        worker.is_finished(),
        "search worker kept running after its AppState dropped"
    );
    worker.join().unwrap();
    drop(content);
}
