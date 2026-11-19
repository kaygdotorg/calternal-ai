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
                .small_plugin_vm = true;
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

#[test]
fn checkpoint_affinity_and_erasure_precede_new_placement() {
    use calternal_machine::protocol::DiscardReport;
    let dir = tempfile::tempdir().unwrap();
    let (service, account) = open(dir.path());
    let spaces = calternal_auth::spaces::Spaces::open(&dir.path().join("system/auth.db")).unwrap();
    let space = spaces.seed_development().unwrap().personal;
    service.set_pool_grant(account, true).unwrap();
    let (first, first_key) = ready_machine_with_key(&service, Owner::Pool, 1);
    let (second, second_key) = ready_machine_with_key(&service, Owner::Pool, 1);
    let first_token = connect(&service, first, &first_key);
    let second_token = connect(&service, second, &second_key);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let report = |ack, max_sandboxes, draining, leases, discards| SyncRequest {
        ack,
        draining,
        leases,
        discards,
        capacity: Capacity {
            max_sandboxes,
            small_plugin_vm: true,
            available_memory_bytes: Some(64 << 30),
            ..Default::default()
        },
    };
    let sync = |machine, token: &str, report| {
        runtime
            .block_on(service.sync(machine, token, report))
            .unwrap()
    };
    let new_run = || {
        service.request_space_vm(
            account,
            Uuid::new_v4(),
            PluginWorker {
                space_id: space,
                source: "fixture".into(),
            },
        )
    };
    let sleep =
        |run: RunView, machine, token: &str, ack, slots, draining| {
            service.sleep_space_run(account, run.lease_id).unwrap();
            let commands = sync(machine, token, report(ack, slots, draining, vec![], vec![]));
            let seq = commands.iter().find_map(|message| {
            matches!(message.message, Down::SleepSpace { lease_id } if lease_id == run.lease_id)
                .then_some(message.seq)
        }).unwrap();
            let saved = LeaseReport {
                lease_id: run.lease_id,
                fence: 1,
                sandbox: SandboxState::CheckpointSaved,
            };
            let messages = sync(
                machine,
                token,
                report(seq, slots, draining, vec![saved], vec![]),
            );
            messages
                .iter()
                .map(|message| message.seq)
                .max()
                .unwrap_or(seq)
        };

    sync(first, &first_token, report(0, 1, false, vec![], vec![]));
    sync(second, &second_token, report(0, 8, true, vec![], vec![]));
    let run = new_run().unwrap();
    assert_eq!(run.machine_id, Some(first));
    let ack_first = sleep(run, first, &first_token, 0, 1, false);
    sync(second, &second_token, report(0, 8, false, vec![], vec![]));
    let preferred = new_run().unwrap();
    assert_eq!(preferred.machine_id, Some(first));
    let ack_first = sleep(preferred, first, &first_token, ack_first, 1, true);
    let fallback = new_run().unwrap();
    assert_eq!(fallback.machine_id, Some(second));
    let ack_second = sleep(fallback, second, &second_token, 0, 8, true);

    // Simulate the old holder reconnecting before its erasure is processed.
    let first_token = connect(&service, first, &first_key);
    let commands = sync(first, &first_token, report(0, 1, false, vec![], vec![]));
    let erase = commands
        .iter()
        .find_map(|message| match message.message {
            Down::EraseSpaceRuntime {
                request_id,
                space_id,
            } if space_id == space => Some((message.seq, request_id)),
            _ => None,
        })
        .expect("older copy must be erased after the space sleeps on the other machine");
    assert!(
        !commands
            .iter()
            .any(|message| matches!(message.message, Down::Assign { .. }))
    );
    assert!(new_run().is_err());
    let commands = sync(
        first,
        &first_token,
        report(erase.0, 1, false, vec![], vec![]),
    );
    assert!(
        !commands
            .iter()
            .any(|message| matches!(message.message, Down::Assign { .. }))
    );
    let erased = DiscardReport {
        request_id: erase.1,
        success: true,
    };
    sync(
        first,
        &first_token,
        report(erase.0, 1, false, vec![], vec![erased]),
    );
    assert_eq!(new_run().unwrap().machine_id, Some(first));
    assert!(ack_first > 0 && ack_second > 0);
}
