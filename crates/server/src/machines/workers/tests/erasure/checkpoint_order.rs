use super::super::super::super::{checkpoint, erasure_queue};
use super::*;

fn space_report(ack: u64, slots: u32, draining: bool) -> SyncRequest {
    SyncRequest {
        ack,
        draining,
        leases: vec![],
        discards: vec![],
        capacity: Capacity {
            max_sandboxes: slots,
            small_vm: Some(super::super::small_vm_capacity(slots)),
            available_memory_bytes: Some(64 << 30),
            ..Default::default()
        },
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
}

#[test]
fn erasing_the_current_lease_machine_invalidates_split_checkpoint_provenance() {
    let dir = tempfile::tempdir().unwrap();
    let (service, account) = open(dir.path());
    let spaces = calternal_auth::spaces::Spaces::open(&dir.path().join("system/auth.db")).unwrap();
    let ids = spaces.seed_development().unwrap();
    let space = ids.shared;
    service.set_pool_grant(account, true).unwrap();
    let (acknowledged, _) = ready_machine_with_key(&service, Owner::Pool, 1);
    let (current, key) = ready_machine_with_key(&service, Owner::Pool, 1);
    let token = connect(&service, current, &key);
    runtime()
        .block_on(service.sync(current, &token, space_report(0, 1, false)))
        .unwrap();
    let lease = service
        .request_space_vm(
            account,
            Uuid::new_v4(),
            PluginWorker {
                space_id: space,
                source: "fixture".into(),
            },
        )
        .unwrap();
    assert_eq!(lease.machine_id, Some(current));
    let state = service.lock().unwrap();
    // New checkpoint assignment normally clears the previous acknowledgement.
    // Exercise both provenance fields in one row, as raised in issue #90.
    state
        .db
        .execute(
            "INSERT INTO space_runtime_checkpoints VALUES(?1,2,?2,?3)",
            rusqlite::params![
                space.to_string(),
                lease.lease_id.to_string(),
                acknowledged.to_string()
            ],
        )
        .unwrap();
    state
        .db
        .execute(
            "INSERT OR IGNORE INTO space_runtime_locations VALUES(?1,?2,?3)",
            rusqlite::params![space.to_string(), account.to_string(), current.to_string()],
        )
        .unwrap();
    assert_eq!(
        checkpoint::proof(&state.db, space, acknowledged).unwrap(),
        Some(2)
    );
    erasure_queue::queue(&state.db, calternal_types::Owner::Account(account), &[]).unwrap();
    assert_eq!(
        checkpoint::proof(&state.db, space, acknowledged).unwrap(),
        None
    );
    assert_eq!(
        checkpoint::placement(&state.db, space).unwrap().1,
        std::collections::HashSet::from([current])
    );
}

#[test]
fn machine_reported_space_idle_window_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _) = open(dir.path());
    let (machine, key) = ready_machine_with_key(&service, Owner::Pool, 1);
    let token = connect(&service, machine, &key);
    let mut report = space_report(0, 1, false);
    report.capacity.small_vm.as_mut().unwrap().idle_seconds = u32::MAX;
    runtime()
        .block_on(service.sync(machine, &token, report))
        .unwrap();
    assert_eq!(
        service.lock().unwrap().machines[&machine]
            .live
            .as_ref()
            .unwrap()
            .capacity
            .small_vm
            .unwrap()
            .idle_seconds,
        calternal_machine::protocol::SMALL_VM_MAX_IDLE_SECONDS
    );
}

#[test]
fn checkpoint_order_survives_restart_and_stale_erasure_retries_are_bounded() {
    use calternal_machine::protocol::DiscardReport;
    let dir = tempfile::tempdir().unwrap();
    let (service, account) = open(dir.path());
    let spaces = calternal_auth::spaces::Spaces::open(&dir.path().join("system/auth.db")).unwrap();
    let space = spaces.seed_development().unwrap().personal;
    service.set_pool_grant(account, true).unwrap();
    let (a, akey) = ready_machine_with_key(&service, Owner::Pool, 1);
    let (b, bkey) = ready_machine_with_key(&service, Owner::Pool, 1);
    let runtime = runtime();
    let sync = |service: &Arc<MachineService>, machine, token: &str, report| {
        runtime
            .block_on(service.sync(machine, token, report))
            .unwrap()
    };
    let wake = |service: &Arc<MachineService>| {
        service.request_space_vm(
            account,
            Uuid::new_v4(),
            PluginWorker {
                space_id: space,
                source: "fixture".into(),
            },
        )
    };
    let sleeping = |service: &Arc<MachineService>,
                    run: &RunView,
                    machine,
                    token: &str,
                    mut report: SyncRequest| {
        service.sleep_space_run(account, run.lease_id).unwrap();
        // Inventory was taken before the role received SleepSpace.
        report.leases.push(LeaseReport {
            lease_id: run.lease_id,
            fence: 1,
            sandbox: SandboxState::Running,
        });
        let commands = sync(service, machine, token, report);
        assert!(!commands.iter().any(|message| matches!(
            message.message, Down::Stop { lease_id, .. } if lease_id == run.lease_id
        )));
        commands
            .iter()
            .find_map(|message| match message.message {
                Down::SleepSpace { lease_id, counter } if lease_id == run.lease_id => {
                    Some((message.seq, counter))
                }
                _ => None,
            })
            .unwrap()
    };
    let proof = |service: &Arc<MachineService>, machine| {
        checkpoint::proof(&service.lock().unwrap().db, space, machine).unwrap()
    };
    let atoken = connect(&service, a, &akey);
    let btoken = connect(&service, b, &bkey);
    sync(&service, a, &atoken, space_report(0, 1, false));
    sync(&service, b, &btoken, space_report(0, 8, true));
    let first = wake(&service).unwrap();
    assert_eq!(first.machine_id, Some(a));
    let (_, one) = sleeping(&service, &first, a, &atoken, space_report(0, 1, false));
    assert_eq!(one, 1);
    let atoken = connect(&service, a, &akey);
    let mut pending = space_report(0, 1, false);
    pending.leases.push(LeaseReport {
        lease_id: first.lease_id,
        fence: 1,
        sandbox: SandboxState::SleepPending,
    });
    let commands = sync(&service, a, &atoken, pending);
    assert!(!commands.iter().any(|command| matches!(
        command.message, Down::Stop { lease_id, .. } if lease_id == first.lease_id
    )));
    let seq = commands
        .iter()
        .find_map(|command| match command.message {
            Down::SleepSpace { lease_id, counter }
                if lease_id == first.lease_id && counter == one =>
            {
                Some(command.seq)
            }
            _ => None,
        })
        .expect("reconnect must replay the same authorized sleep");
    let mut saved = space_report(seq, 1, false);
    saved.leases.push(LeaseReport {
        lease_id: first.lease_id,
        fence: 1,
        sandbox: SandboxState::CheckpointSaved { counter: one },
    });
    let ack = sync(&service, a, &atoken, saved)
        .iter()
        .map(|e| e.seq)
        .max()
        .unwrap_or(seq);
    assert_eq!(proof(&service, a), Some(1));
    let failed = wake(&service).unwrap();
    assert_eq!(failed.machine_id, Some(a));
    let service2 = service.clone();
    let binding = runtime.block_on(async {
        let opening = tokio::spawn(async move {
            service2
                .open_space_channel(
                    account,
                    failed.lease_id,
                    calternal_plugin_host::space_channel::PackagePin {
                        package: "calternal-fixture".into(),
                        digest: "0".repeat(64),
                    },
                )
                .await
        });
        let binding = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                for envelope in service
                    .sync(a, &atoken, space_report(ack, 1, false))
                    .await
                    .unwrap()
                {
                    if let Down::OpenSpaceChannel { channel } = envelope.message {
                        return (envelope.seq, channel);
                    }
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let mut report = space_report(binding.0, 1, false);
        report.leases.push(LeaseReport {
            lease_id: binding.1.lease_id,
            fence: binding.1.fence,
            sandbox: SandboxState::Failed {
                reason: "restore failed before Running".into(),
            },
        });
        let commands = service.sync(a, &atoken, report).await.unwrap();
        let seq = commands.iter().map(|m| m.seq).max().unwrap();
        service
            .sync(a, &atoken, space_report(seq, 1, false))
            .await
            .unwrap();
        assert!(opening.await.unwrap().is_err());
        (seq, binding.1)
    });
    assert_eq!(binding.1.checkpoint, Some(1));
    assert_eq!(
        proof(&service, a),
        Some(1),
        "a failed restore keeps its second try"
    );
    let ack = binding.0;
    sync(&service, b, &btoken, space_report(0, 8, false));
    let prior = wake(&service).unwrap();
    assert_eq!(prior.machine_id, Some(a));
    let (_, two) = sleeping(&service, &prior, a, &atoken, space_report(ack, 1, true));
    assert_eq!(two, 2);
    drop(service); // A still holds an unreported checkpoint.

    let (service, _) = open(dir.path());
    let btoken = connect(&service, b, &bkey);
    sync(&service, b, &btoken, space_report(0, 8, false));
    let current = wake(&service).unwrap();
    assert_eq!(current.machine_id, Some(b));
    assert_eq!(proof(&service, b), None, "fallback must cold boot");
    let (seq, three) = sleeping(&service, &current, b, &btoken, space_report(0, 8, true));
    assert_eq!(three, 3);
    let mut saved = space_report(seq, 8, true);
    saved.leases.push(LeaseReport {
        lease_id: current.lease_id,
        fence: 1,
        sandbox: SandboxState::CheckpointSaved { counter: three },
    });
    sync(&service, b, &btoken, saved);

    let atoken = connect(&service, a, &akey);
    let mut late = space_report(0, 1, false);
    late.leases.push(LeaseReport {
        lease_id: prior.lease_id,
        fence: 1,
        sandbox: SandboxState::CheckpointSaved { counter: two },
    });
    let commands = sync(&service, a, &atoken, late);
    let erased = commands
        .iter()
        .find_map(|e| match e.message {
            Down::EraseSpaceRuntime {
                request_id,
                space_id,
                through: Some(3),
                ..
            } if space_id == space => Some((e.seq, request_id)),
            _ => None,
        })
        .expect("erase the superseded copy before assigning");
    assert_eq!(proof(&service, b), Some(3));
    assert_eq!(proof(&service, a), None);
    assert!(wake(&service).is_err());

    let mut ack = commands.iter().map(|e| e.seq).max().unwrap();
    for failure in 1..=erasure_queue::RUNTIME_RETRIES {
        let mut failed = space_report(ack, 1, false);
        failed.discards.push(DiscardReport {
            request_id: erased.1,
            success: false,
        });
        sync(&service, a, &atoken, failed);
        // Age the retry deadline without sleeping the real control plane.
        service
            .lock()
            .unwrap()
            .db
            .execute(
                "UPDATE space_runtime_erasures SET retry_after_ms=0 WHERE request_id=?1",
                [erased.1.simple().to_string()],
            )
            .unwrap();
        let commands = sync(&service, a, &atoken, space_report(ack, 1, false));
        assert!(commands.iter().any(|envelope| matches!(
            envelope.message,
            Down::EraseSpaceRuntime { request_id, background, .. } if request_id == erased.1
                && background == (failure == erasure_queue::RUNTIME_RETRIES)
        )));
        ack = commands.iter().map(|e| e.seq).max().unwrap();
    }
    assert_eq!(proof(&service, a), None);
    assert_eq!(
        service.pending_erasures().unwrap()[0].unusable_checkpoints,
        1
    );
    // The final physical deletion remains pending, but the only available
    // machine can start cold. A successful receipt retires idle provenance.
    let cold = wake(&service).unwrap();
    assert_eq!(cold.machine_id, Some(a));
    assert_eq!(proof(&service, a), None);
    service.stop_run(account, cold.lease_id).unwrap();
    let commands = sync(&service, a, &atoken, space_report(ack, 1, false));
    let ack = commands.iter().map(|e| e.seq).max().unwrap_or(ack);
    sync(&service, a, &atoken, space_report(ack, 1, false));
    let mut receipt = space_report(ack, 1, false);
    receipt.discards.push(DiscardReport {
        request_id: erased.1,
        success: true,
    });
    sync(&service, a, &atoken, receipt);
    let state = service.lock().unwrap();
    assert_eq!(erasure_queue::count(&state.db, a).unwrap(), 0);
    assert_eq!(
        state
            .db
            .query_row(
                "SELECT count(*) FROM space_runtime_locations WHERE space=?1 AND machine=?2",
                rusqlite::params![space.to_string(), a.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
}
