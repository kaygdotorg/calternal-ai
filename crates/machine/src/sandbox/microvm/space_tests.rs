use super::*;
use crate::{
    protocol::{Limits, PluginWorker, Stream},
    sandbox::{ExecRequest, OutputEvent, worker::SPACE_VM_IMAGE},
};

async fn entropy(vm: &Placement) -> serde_json::Value {
    // Draw from the guest kernel and the freshly launched Node process.
    let js = r#"
        const fd = require('node:fs').openSync('/dev/urandom','r');
        const kernel = Buffer.alloc(32);
        if (require('node:fs').readSync(fd, kernel) !== 32) process.exit(1);
        const node = require('node:crypto').randomBytes(32);
        console.log(JSON.stringify({kernel:kernel.toString('hex'), node:node.toString('hex')}));
    "#;
    let mut exec = vm
        .exec(ExecRequest {
            argv: vec!["node".into(), "-e".into(), js.into()],
            ..Default::default()
        })
        .await
        .unwrap();
    let mut stdout = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = exec.events.recv().await {
            match event {
                OutputEvent::Data(Stream::Stdout, chunk) => stdout.extend(chunk),
                OutputEvent::Exit(info) => {
                    assert_eq!(info.code, Some(0), "{info:?}");
                    return;
                }
                _ => {}
            }
        }
        panic!("entropy child produced no exit");
    })
    .await
    .unwrap();
    serde_json::from_slice(&stdout).unwrap()
}

#[tokio::test]
async fn small_snapshot_reseeds_recovers_after_interruption_and_rejects_mixed_files() {
    let Some(path) = std::env::var_os("CALTERNAL_SMALL_VM_TEST_CONFIG").map(PathBuf::from) else {
        eprintln!("SKIP live KVM fixture: CALTERNAL_SMALL_VM_TEST_CONFIG is unset");
        return;
    };
    let root = tempfile::Builder::new()
        .prefix("vm")
        .rand_bytes(5)
        .tempdir_in(path.parent().unwrap())
        .unwrap();
    let runtime = Arc::new(
        SmallRuntime::load(root.path().to_path_buf(), &path)
            .await
            .unwrap(),
    );
    runtime.initialize().await.unwrap();
    let space = calternal_types::SpaceId::new();
    let spec = SandboxSpec {
        account_id: Uuid::new_v4(),
        thread_id: Uuid::new_v4(),
        space_id: Some(space),
        image: Some(SPACE_VM_IMAGE.into()),
        worker: Some(PluginWorker {
            space_id: space,
            source: "/* fixture */".into(),
        }),
        limits: Limits::default(),
        code_directory: None,
        provider_login: false,
        shared_provider_sponsor: None,
        shared_tools: None,
    };
    let initial = runtime.start(&spec).await.unwrap();
    assert!(
        initial
            .slot
            .lock()
            .await
            .vm
            .as_ref()
            .unwrap()
            .restored
            .is_none()
    );
    let blocked = tokio::time::timeout(Duration::from_secs(4), runtime.start(&spec))
        .await
        .expect("same-space gate must have a deadline");
    assert!(blocked.err().unwrap().0.contains("still shutting down"));
    let _ = entropy(&initial).await;
    let saved = initial.saved.clone();
    let binding = initial.binding.clone();
    initial.release().await.unwrap();
    drop(initial);
    let base = super::super::snapshot::Generation::current(&saved, &binding)
        .unwrap()
        .unwrap();

    // Resume the same checkpoint without the explicit reseed exchange.
    // VMGenID may update entropy, but it cannot acknowledge the host's request
    // or authorize either guest readiness or package startup.
    {
        let _gate = runtime.gate(space).lock_owned().await;
        let run = runtime.state.root.join("run");
        let disk = run.join(&base.disk_name);
        let control = run.join(&base.control_name);
        for (name, path) in [("disk", &disk), ("control", &control)] {
            super::super::snapshot::clone_or_copy(&base.path.join(name), path).unwrap();
        }
        let vm = Guest::load_small(
            &runtime.state.config,
            &run,
            &base,
            disk.clone(),
            control,
            runtime.state.owned.get().unwrap().clone(),
            Arc::new(runtime.state.capacity.clone().try_acquire_owned().unwrap()),
        )
        .await
        .unwrap();
        assert!(vm.control(&Request::SmallReady).await.is_err());
        assert!(
            vm.control(&Request::Start(space, spec.clone()))
                .await
                .is_err()
        );
        let mut unmarked = spec.clone();
        unmarked.image = None;
        assert!(vm.control(&Request::Start(space, unmarked)).await.is_err());
        let mut seed = [0; 32];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut seed).unwrap();
        vm.control(&Request::ReseedSmall(seed)).await.unwrap();
        vm.control(&Request::SmallReady).await.unwrap();
        vm.control(&Request::Start(space, spec.clone()))
            .await
            .unwrap();
        vm.stop().await.unwrap();
        std::fs::remove_file(disk).unwrap();
    }

    let first = runtime.start(&spec).await.unwrap();
    assert_eq!(
        first.slot.lock().await.vm.as_ref().unwrap().restored,
        Some(base.id)
    );
    let first_draw = entropy(&first).await;
    first.stop().await.unwrap();
    drop(first);
    let second = runtime.start(&spec).await.unwrap();
    assert_eq!(
        second.slot.lock().await.vm.as_ref().unwrap().restored,
        Some(base.id)
    );
    let second_draw = entropy(&second).await;
    assert_ne!(first_draw["kernel"], second_draw["kernel"]);
    assert_ne!(first_draw["node"], second_draw["node"]);
    second.release().await.unwrap();
    drop(second);
    let updated = super::super::snapshot::Generation::current(&saved, &binding)
        .unwrap()
        .unwrap();
    assert_ne!(
        updated.id, base.id,
        "saving a restored guest must publish a new generation"
    );
    let base = updated;

    // Power loss between Firecracker's component writes and manifest
    // publication leaves a complete draft on disk and the old pointer intact.
    let interrupted = runtime.start(&spec).await.unwrap();
    interrupted.thread.stop().await.unwrap();
    {
        use super::super::{snapshot::Generation, snapshot_api};
        let mut slot = interrupted.slot.lock().await;
        let vm = slot.vm.take().unwrap();
        let control = vm.control_disk.as_ref().unwrap();
        let draft = Generation::create(&saved, &vm.disk, control).unwrap();
        let api = vm.api_socket.as_ref().unwrap();
        snapshot_api::pause(api).await.unwrap();
        snapshot_api::create(
            api,
            &draft.path.join("state"),
            &draft.path.join("memory"),
            false,
        )
        .await
        .unwrap();
        vm.stop().await.unwrap();
    }
    drop(interrupted);
    assert_eq!(
        super::super::snapshot::Generation::current(&saved, &binding)
            .unwrap()
            .unwrap()
            .id,
        base.id,
    );
    let recovered = runtime.start(&spec).await.unwrap();
    assert_eq!(
        recovered.slot.lock().await.vm.as_ref().unwrap().restored,
        Some(base.id)
    );
    let _ = entropy(&recovered).await;
    recovered.stop().await.unwrap();
    drop(recovered);

    // Corrupt a published state file while updating the fixture's manifest
    // metadata so it reaches the actual Firecracker restore failure.
    std::fs::write(base.path.join("state"), b"invalid Firecracker state").unwrap();
    base.publish(&saved, &binding).unwrap();
    assert!(
        super::super::snapshot::Generation::current(&saved, &binding)
            .unwrap()
            .is_some()
    );
    let cold = runtime.start(&spec).await.unwrap();
    assert!(
        cold.slot
            .lock()
            .await
            .vm
            .as_ref()
            .unwrap()
            .restored
            .is_none()
    );
    assert!(!base.path.exists());
    let run = runtime.state.root.join("run");
    assert!(!run.join(&base.disk_name).exists());
    assert!(!run.join(&base.control_name).exists());
    let _ = entropy(&cold).await;

    // The old Placement Arc remains live after teardown and must release the
    // space gate for both erasure and the next start.
    cold.release().await.unwrap();
    assert!(saved.exists());
    let other = runtime.saved(SpaceId::new());
    std::fs::create_dir(&other).unwrap();
    runtime.erase(space).await.unwrap();
    assert!(!saved.exists());
    assert!(other.exists());
    let newer = runtime.start(&spec).await.unwrap();
    newer.release().await.unwrap();
    let expired = std::time::SystemTime::now() - RETENTION - Duration::from_secs(1);
    std::fs::File::open(saved.join("current"))
        .unwrap()
        .set_modified(expired)
        .unwrap();
    runtime.sweep_expired().await.unwrap();
    assert!(!saved.exists());
    assert!(other.exists());
}
