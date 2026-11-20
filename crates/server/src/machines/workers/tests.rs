use super::*;

use calternal_machine::{
    identity::MachineKey,
    protocol::{EnrolRequest, PROTOCOL_VERSION, SessionFinish},
};

mod erasure;

fn ready_machine(service: &MachineService, owner: Owner, capacity: u32) -> Uuid {
    ready_machine_with_key(service, owner, capacity).0
}

fn ready_machine_with_key(
    service: &MachineService,
    owner: Owner,
    capacity: u32,
) -> (Uuid, MachineKey) {
    let (code, _) = service.issue_code(owner).unwrap();
    let key = MachineKey::generate().unwrap();
    let machine = service
        .enrol(&EnrolRequest {
            code,
            public_key: key.public_key_b64(),
            name: "fixture".into(),
            os: "linux".into(),
            arch: "x86_64".into(),
            direct: false,
        })
        .unwrap()
        .machine_id;
    connect(service, machine, &key);
    let mut state = service.lock().unwrap();
    let live = state
        .machines
        .get_mut(&machine)
        .unwrap()
        .live
        .as_mut()
        .unwrap();
    live.capacity.max_sandboxes = capacity;
    live.capacity.available_memory_bytes = Some(64 << 30);
    (machine, key)
}

fn connect(service: &MachineService, machine: Uuid, key: &MachineKey) -> String {
    let challenge = service.session_start(machine).unwrap();
    service
        .session_finish(&SessionFinish {
            machine_id: machine,
            signature: key.sign_session(machine, &challenge),
            challenge,
            boot_id: Uuid::new_v4(),
            protocol: PROTOCOL_VERSION,
            backend: "podman".into(),
            guest_memory_mib: None,
        })
        .unwrap()
        .token
}

fn small_vm_capacity(max_residents: u32) -> calternal_machine::protocol::SmallVmCapacity {
    use calternal_machine::protocol::{
        SMALL_VM_IDLE_SECONDS, SMALL_VM_MEMORY_MIB, SmallVmCapacity,
    };
    SmallVmCapacity {
        idle_seconds: SMALL_VM_IDLE_SECONDS,
        memory_limit_bytes: (u64::from(SMALL_VM_MEMORY_MIB) << 20) * u64::from(max_residents),
        max_residents,
        resident_count: 0,
        resident_memory_bytes: 0,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn space_channel_is_single_use_and_bound_to_the_enrolled_assignee() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_owned();
    let (service, owner, worker, machine, token, foreign) = blocking(move || {
        let spaces = calternal_auth::spaces::Spaces::open(&root.join("system/auth.db")).unwrap();
        let ids = spaces.seed_development().unwrap();
        let service = MachineService::open(&root, spaces.machines(), Settings::default()).unwrap();
        service.set_pool_grant(ids.owner, true).unwrap();
        let (machine, key) = ready_machine_with_key(&service, Owner::Pool, 3);
        let token = connect(&service, machine, &key);
        {
            let mut state = service.lock().unwrap();
            let live = state
                .machines
                .get_mut(&machine)
                .unwrap()
                .live
                .as_mut()
                .unwrap();
            live.capacity.max_sandboxes = 3;
            live.capacity.available_memory_bytes = Some(64 << 30);
            live.capacity.small_vm = Some(small_vm_capacity(3));
        }
        let worker = service
            .request_space_vm(
                ids.owner,
                Uuid::new_v4(),
                PluginWorker {
                    space_id: ids.personal,
                    source: "fixture".into(),
                },
            )
            .unwrap();
        let (other, foreign_key) = ready_machine_with_key(&service, Owner::Pool, 1);
        let foreign = connect(&service, other, &foreign_key);
        Ok((service, ids.owner, worker, machine, token, foreign))
    })
    .await
    .unwrap();
    let scheduler = service.clone();
    let lease = worker.lease_id;
    let opened = tokio::spawn(async move {
        scheduler
            .open_space_channel(
                owner,
                lease,
                calternal_plugin_host::space_channel::PackagePin {
                    package: "calternal-fixture".into(),
                    digest: "a".repeat(64),
                },
            )
            .await
    });
    let service2 = service.clone();
    let binding = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let service3 = service2.clone();
            let message = blocking(move || {
                let state = service3.lock()?;
                Ok(state.machines[&machine]
                    .live
                    .as_ref()
                    .unwrap()
                    .outbox
                    .iter()
                    .find_map(|envelope| match &envelope.message {
                        Down::OpenSpaceChannel { channel } => Some(channel.clone()),
                        _ => None,
                    }))
            })
            .await
            .unwrap();
            if let Some(binding) = message {
                break binding;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let service2 = service.clone();
    blocking(move || {
        assert!(
            service2
                .claim_channel(&foreign, binding.space, binding.assignment)
                .is_err()
        );
        assert!(
            service2
                .claim_channel(&token, SpaceId::new(), binding.assignment)
                .is_err()
        );
        let (_, _, closing, _, _, reply) = service2
            .claim_channel(&token, binding.space, binding.assignment)
            .unwrap();
        assert!(
            service2
                .claim_channel(&token, binding.space, binding.assignment)
                .is_err()
        );
        service2.stop_run(owner, lease).unwrap();
        assert!(*closing.borrow());
        drop(reply);
        Ok(())
    })
    .await
    .unwrap();
    assert!(opened.await.unwrap().is_err());
}

#[test]
fn worker_frames_and_leases_are_not_public_machine_resources() {
    let dir = tempfile::tempdir().unwrap();
    let spaces = calternal_auth::spaces::Spaces::open(&dir.path().join("system/auth.db")).unwrap();
    let owner = spaces.seed_development().unwrap().owner;
    let service = MachineService::open(dir.path(), spaces.machines(), Settings::default()).unwrap();
    service.set_pool_grant(owner, true).unwrap();
    ready_machine(&service, Owner::Pool, 3);
    let space = SpaceId::new();
    let worker = service
        .request_worker(
            owner,
            Uuid::new_v4(),
            PluginWorker {
                space_id: space,
                source: "verified fixture".into(),
            },
        )
        .unwrap();
    let exec = Uuid::new_v4();
    service.lock().unwrap().execs.insert(
        exec,
        Exec::new(
            worker.lease_id,
            owner,
            vec!["node".into()],
            vec![],
            ExecState::Queued,
        ),
    );
    assert!(
        service
            .request_run(space, owner, worker.thread_id, None)
            .is_err()
    );
    assert_eq!(service.run_space(worker.lease_id).unwrap(), None);
    assert_eq!(service.exec_space(exec).unwrap(), None);
    let regular = service
        .request_run(space, owner, Uuid::new_v4(), None)
        .unwrap();
    assert_eq!(service.run_space(regular.lease_id).unwrap(), Some(space));
}
#[test]
fn admitted_worker_capacity_is_the_sum_of_ready_pool_ceilings() {
    let dir = tempfile::tempdir().unwrap();
    let spaces = calternal_auth::spaces::Spaces::open(&dir.path().join("system/auth.db")).unwrap();
    let owner = spaces.seed_development().unwrap().owner;
    let service = MachineService::open(dir.path(), spaces.machines(), Settings::default()).unwrap();
    let start = || {
        service.request_worker(
            owner,
            Uuid::new_v4(),
            PluginWorker {
                space_id: SpaceId::new(),
                source: "verified fixture".into(),
            },
        )
    };
    let reason = |error: Error| {
        error.details.unwrap()["reason"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    ready_machine(&service, Owner::Account { id: owner }, 10);
    assert_eq!(reason(start().unwrap_err()), "worker_machine_required");
    let first = ready_machine(&service, Owner::Pool, 4);
    let second = ready_machine(&service, Owner::Pool, 5);
    assert_eq!(reason(start().unwrap_err()), "worker_machine_required");
    service.set_pool_grant(owner, true).unwrap();

    let placements: Vec<_> = (0..9)
        .map(|_| start().unwrap().machine_id.unwrap())
        .collect();
    assert_eq!(
        placements
            .iter()
            .filter(|&&machine| machine == first)
            .count(),
        4
    );
    assert_eq!(
        placements
            .iter()
            .filter(|&&machine| machine == second)
            .count(),
        5
    );
    assert_eq!(reason(start().unwrap_err()), "worker_capacity");
}

#[test]
fn teardown_capacity_requires_ack_and_survives_server_restart() {
    let dir = tempfile::tempdir().unwrap();
    let open = || {
        let spaces =
            calternal_auth::spaces::Spaces::open(&dir.path().join("system/auth.db")).unwrap();
        let owner = spaces.seed_development().unwrap().owner;
        (
            MachineService::open(dir.path(), spaces.machines(), Settings::default()).unwrap(),
            owner,
        )
    };
    let (service, owner) = open();
    let source = || PluginWorker {
        space_id: SpaceId::new(),
        source: "verified fixture".into(),
    };
    service.set_pool_grant(owner, true).unwrap();
    let machine = ready_machine(&service, Owner::Pool, 1);
    let run = service
        .request_worker(owner, Uuid::new_v4(), source())
        .unwrap();
    service.stop_run(owner, run.lease_id).unwrap();
    assert!(!service.worker_stopped(run.lease_id).unwrap());
    assert_eq!(service.lock().unwrap().load(machine), 1);
    assert_eq!(service.lock().unwrap().pool_load.get(&owner), Some(&1));
    assert_eq!(
        service
            .request_worker(owner, Uuid::new_v4(), source())
            .unwrap_err()
            .details
            .unwrap()["reason"],
        "worker_capacity"
    );
    let mut report = SyncRequest {
        ack: 4,
        discards: vec![],
        capacity: Default::default(),
        draining: false,
        leases: vec![],
    };
    {
        let mut state = service.lock().unwrap();
        state.worker_stops.insert(
            run.lease_id,
            WorkerStop {
                machine,
                account: Some(owner),
                session: [2; 32],
                seq: 5,
                checkpoint: false,
            },
        );
        assert_eq!(
            state
                .finish_worker_stops(machine, &[2; 32], &report)
                .unwrap(),
            0
        );
        report.ack = 5;
        report
            .leases
            .push(calternal_machine::protocol::LeaseReport {
                lease_id: run.lease_id,
                fence: 1,
                sandbox: calternal_machine::protocol::SandboxState::Running,
            });
        assert_eq!(
            state
                .finish_worker_stops(machine, &[2; 32], &report)
                .unwrap(),
            0
        );
    }
    drop(service);
    let (service, owner) = open();
    assert_eq!(service.lock().unwrap().load(machine), 1);
    assert_eq!(service.lock().unwrap().pool_load.get(&owner), Some(&1));
    assert!(!service.worker_stopped(run.lease_id).unwrap());
    let other = ready_machine(&service, Owner::Pool, 1);
    let next = service
        .request_worker(owner, Uuid::new_v4(), source())
        .unwrap();
    assert_eq!(next.machine_id, Some(other));
    assert_eq!(service.lock().unwrap().pool_load.get(&owner), Some(&2));
    report.leases.clear();
    {
        let mut state = service.lock().unwrap();
        assert_eq!(
            state
                .finish_worker_stops(Uuid::new_v4(), &[3; 32], &report)
                .unwrap(),
            0
        );
        assert_eq!(
            state
                .finish_worker_stops(machine, &[3; 32], &report)
                .unwrap(),
            1
        );
    }
    assert!(service.worker_stopped(run.lease_id).unwrap());
    assert_eq!(service.lock().unwrap().load(machine), 0);
    assert_eq!(service.lock().unwrap().pool_load.get(&owner), Some(&1));
    service.stop_run(owner, next.lease_id).unwrap();
    assert!(!service.worker_stopped(next.lease_id).unwrap());
    assert!(service.revoke_any(other).unwrap());
    assert!(service.worker_stopped(next.lease_id).unwrap());
    assert_eq!(service.lock().unwrap().load(other), 0);
    assert!(!service.lock().unwrap().pool_load.contains_key(&owner));
    drop(service);
    let (service, _) = open();
    assert!(service.worker_stopped(next.lease_id).unwrap());
}
