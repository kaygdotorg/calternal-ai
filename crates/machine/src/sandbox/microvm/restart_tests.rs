//! An abrupt role restart must reconcile its own resident VMM and device files.
use super::*;

#[tokio::test]
async fn resident_kvm_is_reaped_when_the_machine_role_restarts() {
    let Some(config) = std::env::var_os("CALTERNAL_SMALL_VM_TEST_CONFIG").map(PathBuf::from) else {
        eprintln!("SKIP live KVM fixture: CALTERNAL_SMALL_VM_TEST_CONFIG is unset");
        return;
    };
    let child_root = std::env::var_os("CALTERNAL_KVM_RESTART_CHILD").map(PathBuf::from);
    let initialize = |path: PathBuf| async {
        let runtime = Arc::new(SmallRuntime::load(path, &config).await.unwrap());
        runtime.initialize().await.unwrap();
        runtime
    };
    let space = SpaceId::new();
    let spec = SandboxSpec {
        account_id: Uuid::new_v4(),
        thread_id: Uuid::new_v4(),
        space_id: Some(space),
        image: Some(SPACE_VM_IMAGE.into()),
        worker: Some(PluginWorker {
            space_id: space,
            source: ENTROPY_PACKAGE.into(),
        }),
        limits: Limits::default(),
        code_directory: None,
        provider_login: false,
        shared_provider_sponsor: None,
        shared_tools: None,
    };
    if let Some(path) = child_root {
        let runtime = initialize(path.clone()).await;
        let placement = runtime.start(&spec, None).await.unwrap();
        entropy(&placement).await;
        let pid = placement.slot.lock().await.vm.as_ref().unwrap().pid();
        tokio::fs::write(path.join("resident"), format!("{pid}\n"))
            .await
            .unwrap();
        std::future::pending::<()>().await;
        return;
    }
    let root = tempfile::Builder::new()
        .prefix("rv")
        .rand_bytes(5)
        .tempdir_in(config.parent().unwrap())
        .unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap());
    child.args(["--exact", "sandbox::microvm::space::tests::restart_tests::resident_kvm_is_reaped_when_the_machine_role_restarts", "--test-threads=1"])
        .env("CALTERNAL_KVM_RESTART_CHILD", root.path())
        .kill_on_drop(true);
    let mut child = child.spawn().unwrap();
    let pid = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if let Ok(value) = tokio::fs::read_to_string(root.path().join("resident")).await
                && let Ok(pid) = value.trim().parse::<u32>()
            {
                break pid;
            }
            assert!(
                child.try_wait().unwrap().is_none(),
                "child role exited before loading the package"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    assert!(std::path::Path::new(&format!("/proc/{pid}")).exists());
    child.kill().await.unwrap();
    let recovered = initialize(root.path().to_owned()).await;
    assert_eq!(recovered.capacity().unwrap().resident_count, 0);
    assert!(
        !tokio::fs::try_exists(format!("/proc/{pid}/cmdline"))
            .await
            .unwrap()
            || tokio::fs::read(format!("/proc/{pid}/cmdline"))
                .await
                .unwrap_or_default()
                .is_empty()
    );
    let placement = recovered.start(&spec, None).await.unwrap();
    entropy(&placement).await;
    assert_ne!(placement.slot.lock().await.vm.as_ref().unwrap().pid(), pid);
    placement.stop().await.unwrap();
}
