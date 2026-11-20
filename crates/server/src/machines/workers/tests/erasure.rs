use super::*;
use calternal_machine::protocol::{Capacity, LeaseReport, SandboxState};
use std::{sync::Arc, time::Duration};

fn open(path: &std::path::Path) -> (Arc<MachineService>, AccountId) {
    let spaces = calternal_auth::spaces::Spaces::open(&path.join("system/auth.db")).unwrap();
    let owner = spaces.seed_development().unwrap().owner;
    let settings = Settings {
        poll_hold: Duration::ZERO,
        ..Settings::default()
    };
    (
        MachineService::open(path, spaces.machines(), settings).unwrap(),
        owner,
    )
}

#[test]
fn erased_worker_stop_survives_restart_and_requires_authenticated_proof() {
    exercise_stop(Completion::Proof);
}

#[test]
fn erased_worker_stop_is_removed_by_durable_revocation() {
    exercise_stop(Completion::Revocation);
}

#[test]
fn erased_worker_stop_recovery_finishes_already_durable_revocation() {
    exercise_stop(Completion::RevocationBeforeRestart);
}

enum Completion {
    Proof,
    Revocation,
    RevocationBeforeRestart,
}

fn exercise_stop(completion: Completion) {
    let dir = tempfile::tempdir().unwrap();
    let (service, account) = open(dir.path());
    service.set_pool_grant(account, true).unwrap();
    let (machine, key) = ready_machine_with_key(&service, Owner::Pool, 1);
    let space = SpaceId::new();
    let thread = Uuid::new_v4();
    let run = service
        .request_worker(
            account,
            thread,
            PluginWorker {
                space_id: space,
                source: "budget fixture".into(),
            },
        )
        .unwrap();
    let lease = run.lease_id;
    service.stop_run(account, lease).unwrap();
    service
        .retire_owner(calternal_types::Owner::Account(account))
        .unwrap();
    {
        let state = service.lock().unwrap();
        assert_eq!(state.load(machine), 1);
        assert!(!state.pool_load.contains_key(&account));
        assert!(
            crate::machines::db::lease(&state.db, lease)
                .unwrap()
                .is_none()
        );
    }
    drop(service);

    let (service, _) = open(dir.path());
    {
        let state = service.lock().unwrap();
        assert_eq!(state.load(machine), 1);
        assert!(!state.pool_load.contains_key(&account));
        assert!(!state.leases.contains_key(&lease));
        let mut query = state
            .db
            .prepare("SELECT * FROM erased_worker_stops")
            .unwrap();
        assert!(query.column_names().eq(["lease", "machine", "slots"]));
        let mut rows = query.query([]).unwrap();
        let row = rows.next().unwrap().unwrap();
        let lease_bytes = row.get_ref(0).unwrap().as_bytes().unwrap();
        let machine_bytes = row.get_ref(1).unwrap().as_bytes().unwrap();
        assert_eq!(lease_bytes, lease.to_string().as_bytes());
        assert_eq!(machine_bytes, machine.to_string().as_bytes());
        assert_eq!(row.get::<_, u32>(2).unwrap(), 1);
        for id in [account.to_string(), space.to_string(), thread.to_string()] {
            assert!(![lease_bytes, machine_bytes].iter().any(|bytes| {
                bytes
                    .windows(id.len())
                    .any(|window| window == id.as_bytes())
            }));
        }
        assert!(rows.next().unwrap().is_none());
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let token = connect(&service, machine, &key);
    let mut report = SyncRequest {
        ack: 0,
        discards: vec![],
        capacity: Capacity {
            max_sandboxes: 1,
            available_memory_bytes: Some(64 << 30),
            ..Capacity::default()
        },
        draining: false,
        leases: vec![LeaseReport {
            lease_id: lease,
            fence: 1,
            sandbox: SandboxState::Running,
        }],
    };
    let messages = runtime
        .block_on(service.sync(machine, &token, report.clone()))
        .unwrap();
    report.ack = messages
        .iter()
        .find_map(|message| match &message.message {
            Down::Stop { lease_id, .. } if *lease_id == lease => Some(message.seq),
            _ => None,
        })
        .expect("the reconnecting machine receives teardown for the erased lease");
    assert!(!service.worker_stopped(lease).unwrap());
    {
        let state = service.lock().unwrap();
        assert_eq!(state.free(&state.machines[&machine]), 0);
    }
    match completion {
        Completion::Proof => {
            report.leases.clear();
            assert!(
                runtime
                    .block_on(service.sync(machine, "invalid", report.clone()))
                    .is_err()
            );
            assert!(!service.worker_stopped(lease).unwrap());
            runtime
                .block_on(service.sync(machine, &token, report))
                .unwrap();
            let state = service.lock().unwrap();
            assert_eq!(state.free(&state.machines[&machine]), 1);
        }
        Completion::Revocation => {
            assert!(service.revoke_any(machine).unwrap());
            assert!(service.worker_stopped(lease).unwrap());
            assert_eq!(service.lock().unwrap().load(machine), 0);
        }
        Completion::RevocationBeforeRestart => {
            // Simulate shutdown after the authority commit, before registry cleanup.
            assert!(service.lock().unwrap().authority.revoke(machine).unwrap());
        }
    }
    drop(service);

    let (service, _) = open(dir.path());
    assert!(service.worker_stopped(lease).unwrap());
    let state = service.lock().unwrap();
    assert_eq!(state.load(machine), 0);
    let rows: u32 = state
        .db
        .query_row("SELECT count(*) FROM erased_worker_stops", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(rows, 0);
}

#[test]
fn dormant_space_runtime_erasure_replays_for_both_owners() {
    use calternal_machine::protocol::DiscardReport;
    use calternal_types::Owner as ErasedOwner;
    use rusqlite::params;
    for erase_account in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        // Open a deployed schema-9 database through the normal service path.
        std::fs::create_dir(dir.path().join("system")).unwrap();
        let path = dir.path().join("system/machines.db");
        let legacy = super::super::super::db::fixture_schema9(&path);
        let old_machine = Uuid::new_v4().to_string();
        legacy.execute(
            "INSERT INTO machine_erasures(machine,account,thread,request_id) VALUES(?1,?2,'',?3)",
            rusqlite::params![old_machine, AccountId::new().to_string(), Uuid::new_v4().simple().to_string()],
        ).unwrap();
        drop(legacy);
        let (service, account) = open(dir.path());
        let (machine, key) = ready_machine_with_key(&service, Owner::Pool, 1);
        let spaces =
            calternal_auth::spaces::Spaces::open(&dir.path().join("system/auth.db")).unwrap();
        let space = spaces.seed_development().unwrap().personal;
        service.set_pool_grant(account, true).unwrap();
        {
            let mut state = service.lock().unwrap();
            state
                .machines
                .get_mut(&machine)
                .unwrap()
                .live
                .as_mut()
                .unwrap()
                .capacity
                .small_vm = Some(super::small_vm_capacity(1));
        }
        let run = service
            .request_space_vm(
                account,
                Uuid::new_v4(),
                PluginWorker {
                    space_id: space,
                    source: "fixture".into(),
                },
            )
            .unwrap();
        {
            let state = service.lock().unwrap();
            assert_eq!(
                state
                    .db
                    .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                    .unwrap(),
                10
            );
            assert_eq!(
                state
                    .db
                    .query_row(
                        "SELECT count(*) FROM machine_erasures WHERE machine=?1",
                        [&old_machine],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap(),
                1
            );
            let stored: bool = state.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM space_runtime_locations WHERE space=?1 AND account=?2 AND machine=?3)",
                params![space.to_string(), account.to_string(), machine.to_string()],
                |row| row.get(0),
            ).unwrap();
            assert!(stored, "assignment must persist checkpoint provenance");
        }
        service.sleep_space_run(account, run.lease_id).unwrap();
        let owner = if erase_account {
            ErasedOwner::Account(account)
        } else {
            ErasedOwner::Space(space)
        };
        service.retire_owner(owner).unwrap();
        drop(service);

        let (service, _) = open(dir.path());
        let token = connect(&service, machine, &key);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let mut report = SyncRequest {
            ack: 0,
            discards: vec![],
            capacity: Capacity::default(),
            draining: false,
            leases: vec![],
        };
        let messages = runtime
            .block_on(service.sync(machine, &token, report.clone()))
            .unwrap();
        let request = messages
            .iter()
            .find_map(|message| match message.message {
                Down::EraseSpaceRuntime {
                    request_id,
                    space_id,
                    ..
                } if space_id == space => Some(request_id),
                _ => None,
            })
            .expect("pool machine must receive the durable space checkpoint erase after restart");
        report.ack = messages.iter().map(|m| m.seq).max().unwrap();
        report.discards.push(DiscardReport {
            request_id: request,
            success: true,
        });
        runtime
            .block_on(service.sync(machine, &token, report))
            .unwrap();
        let state = service.lock().unwrap();
        assert_eq!(
            super::super::super::erasure_queue::count(&state.db, machine).unwrap(),
            0
        );
    }
}

fn space_report(ack: u64, slots: u32, draining: bool) -> SyncRequest {
    SyncRequest {
        ack,
        draining,
        leases: vec![],
        discards: vec![],
        capacity: Capacity {
            max_sandboxes: slots,
            small_vm: Some(super::small_vm_capacity(slots)),
            available_memory_bytes: Some(64 << 30),
            ..Default::default()
        },
    }
}

#[test]
fn checkpoint_order_survives_restart_and_stale_erasure_retries_are_bounded() {
    use super::super::super::{checkpoint, erasure_queue};
    use calternal_machine::protocol::DiscardReport;
    let dir = tempfile::tempdir().unwrap();
    let (service, account) = open(dir.path());
    let spaces = calternal_auth::spaces::Spaces::open(&dir.path().join("system/auth.db")).unwrap();
    let space = spaces.seed_development().unwrap().personal;
    service.set_pool_grant(account, true).unwrap();
    let (a, akey) = ready_machine_with_key(&service, Owner::Pool, 1);
    let (b, bkey) = ready_machine_with_key(&service, Owner::Pool, 1);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
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
    let (seq, one) = sleeping(&service, &first, a, &atoken, space_report(0, 1, false));
    assert_eq!(one, 1);
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
