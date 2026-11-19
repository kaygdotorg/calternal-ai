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
    // Observe actual cold Firecracker launches without changing the product
    // runner or guest. The wrapper delegates every invocation to Firecracker.
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    let mut config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    symlink(
        config["firecracker"].as_str().unwrap(),
        root.path().join("real-firecracker"),
    )
    .unwrap();
    let launcher = root.path().join("firecracker");
    std::fs::write(
        &launcher,
        r#"#!/bin/sh
profile=${0%/*}
for arg do
    if [ "$arg" = "--config-file" ]; then
        printf 'boot\n' >> "$profile/boot-count"
        break
    fi
done
exec "$profile/real-firecracker" "$@"
"#,
    )
    .unwrap();
    std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o700)).unwrap();
    config["firecracker"] = serde_json::json!(launcher);
    let path = root.path().join("fixture.json");
    std::fs::write(&path, config.to_string()).unwrap();
    let cold_boots = || {
        std::fs::read_to_string(root.path().join("boot-count"))
            .unwrap()
            .lines()
            .count()
    };
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
    let initial = runtime.start(&spec, None).await.unwrap();
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
    let blocked = tokio::time::timeout(Duration::from_secs(4), runtime.start(&spec, Some(1)))
        .await
        .expect("same-space gate must have a deadline");
    assert!(blocked.err().unwrap().0.contains("still shutting down"));
    let _ = entropy(&initial).await;
    let saved = initial.saved.clone();
    let binding = initial.binding.clone();
    assert!(initial.checkpoint(1).await.unwrap());
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

    // A host filesystem error on the production restore path preserves current.
    let run = runtime.state.root.join("run");
    let blocked_disk = run.join(&base.disk_name);
    std::fs::create_dir(&blocked_disk).unwrap();
    assert!(runtime.start(&spec, Some(1)).await.is_err());
    assert_eq!(
        Generation::current(&saved, &binding).unwrap().unwrap().id,
        base.id
    );
    assert_eq!(
        std::fs::read_to_string(saved.join("current.failures")).unwrap(),
        "1 1\n"
    );
    std::fs::remove_dir(blocked_disk).unwrap();

    let first = runtime.start(&spec, Some(1)).await.unwrap();
    assert_eq!(
        first.slot.lock().await.vm.as_ref().unwrap().restored,
        Some(base.id)
    );
    assert!(!saved.join("current.failures").exists());
    let first_draw = entropy(&first).await;
    first.stop().await.unwrap();
    drop(first);
    let second = runtime.start(&spec, Some(1)).await.unwrap();
    assert_eq!(
        second.slot.lock().await.vm.as_ref().unwrap().restored,
        Some(base.id)
    );
    let second_draw = entropy(&second).await;
    assert_ne!(first_draw["kernel"], second_draw["kernel"]);
    assert_ne!(first_draw["node"], second_draw["node"]);
    assert!(second.checkpoint(2).await.unwrap());
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
    let interrupted = runtime.start(&spec, Some(2)).await.unwrap();
    interrupted.thread.stop().await.unwrap();
    {
        use super::super::{snapshot::Generation, snapshot_api};
        let mut slot = interrupted.slot.lock().await;
        let vm = slot.vm.take().unwrap();
        let control = vm.control_disk.as_ref().unwrap();
        let draft = Generation::create(&saved, &vm.disk, control, 3).unwrap();
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
    let recovered = runtime.start(&spec, Some(2)).await.unwrap();
    assert_eq!(
        recovered.slot.lock().await.vm.as_ref().unwrap().restored,
        Some(base.id)
    );
    let _ = entropy(&recovered).await;
    recovered.stop().await.unwrap();
    drop(recovered);

    // Firecracker can load these components, but the restored guest still
    // holds the original control key in memory and rejects the new host key.
    // A failure after load must consume the same restore budget.
    let control = base.path.join("control");
    let original = std::fs::read(&control).unwrap();
    let mut wrong_key = original.clone();
    wrong_key[0] ^= 1;
    std::fs::write(&control, wrong_key).unwrap();
    base.publish(&saved, &binding).unwrap();
    assert!(runtime.start(&spec, Some(2)).await.is_err());
    assert_eq!(
        std::fs::read_to_string(saved.join("current.failures")).unwrap(),
        "2 1\n"
    );
    std::fs::write(&control, original).unwrap();
    base.publish(&saved, &binding).unwrap();
    let recovered = runtime.start(&spec, Some(2)).await.unwrap();
    assert_eq!(
        recovered.slot.lock().await.vm.as_ref().unwrap().restored,
        Some(base.id)
    );
    recovered.stop().await.unwrap();
    assert!(!saved.join("current.failures").exists());

    // Corrupt a published state file while updating the fixture's manifest
    // metadata so it reaches the actual Firecracker restore failure.
    std::fs::write(base.path.join("state"), b"invalid Firecracker state").unwrap();
    base.publish(&saved, &binding).unwrap();
    assert!(
        super::super::snapshot::Generation::current(&saved, &binding)
            .unwrap()
            .is_some()
    );
    let before = cold_boots();
    assert!(runtime.start(&spec, Some(2)).await.is_err());
    assert!(base.path.exists());
    assert_eq!(
        std::fs::read_to_string(saved.join("current.failures")).unwrap(),
        "2 1\n"
    );
    let cold = runtime.start(&spec, Some(2)).await.unwrap();
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
    assert_eq!(cold_boots(), before + 1);
    assert!(!base.path.exists());
    let run = runtime.state.root.join("run");
    assert!(!run.join(&base.disk_name).exists());
    assert!(!run.join(&base.control_name).exists());
    let _ = entropy(&cold).await;

    // The old Placement Arc remains live after teardown and must release the
    // space gate for both erasure and the next start.
    assert!(cold.checkpoint(3).await.unwrap());
    assert!(saved.exists());
    let other = runtime.saved(SpaceId::new());
    std::fs::create_dir(&other).unwrap();
    runtime.erase(space, Some(2)).await.unwrap();
    assert!(saved.exists(), "a late erase must retain a newer copy");
    runtime.erase(space, None).await.unwrap();
    assert!(!saved.exists());
    assert!(other.exists());
    let newer = runtime.start(&spec, None).await.unwrap();
    assert!(newer.checkpoint(4).await.unwrap());
    // A genuine Start refusal attempts one cold boot. When that Start also
    // fails, it preserves the checkpoint for a subsequent valid package.
    let mut oversized = spec.clone();
    oversized.worker.as_mut().unwrap().source =
        "x".repeat(crate::sandbox::worker::MAX_SOURCE_BYTES + 1);
    let before = cold_boots();
    assert!(runtime.start(&oversized, Some(4)).await.is_err());
    assert_eq!(cold_boots(), before + 1);
    assert_eq!(
        Generation::current(&saved, &binding)
            .unwrap()
            .unwrap()
            .counter,
        4
    );
    let proven = runtime.start(&spec, Some(4)).await.unwrap();
    assert!(
        proven
            .slot
            .lock()
            .await
            .vm
            .as_ref()
            .unwrap()
            .restored
            .is_some()
    );
    proven.stop().await.unwrap();
    let rebuilt = runtime.start(&spec, Some(3)).await.unwrap();

    assert!(
        rebuilt
            .slot
            .lock()
            .await
            .vm
            .as_ref()
            .unwrap()
            .restored
            .is_none()
    );
    assert!(rebuilt.checkpoint(5).await.unwrap());
    let expired = std::time::SystemTime::now() - RETENTION - Duration::from_secs(1);
    std::fs::File::open(saved.join("current"))
        .unwrap()
        .set_modified(expired)
        .unwrap();
    runtime.sweep_expired().await;
    assert!(!saved.exists());
    assert!(other.exists());
}
