//! Worker admission and ordered stdin on the existing fenced machine exec channel.
use super::*;
use calternal_machine::{
    protocol::{Down, Owner, PluginWorker, SyncRequest},
    sandbox::worker,
};

/// Holds machine capacity until teardown. An intact lease also holds an
/// account slot. An erased lease is backed by a machine-only durable row.
#[derive(Clone, Copy)]
pub(super) struct WorkerStop {
    /// Machine that holds the worker sandbox.
    pub machine: Uuid,
    /// Pool reservation owner, cleared as soon as the lease is erased.
    pub account: Option<AccountId>,
    /// Session that was sent the stop command.
    pub session: [u8; 32],
    /// Outbox sequence the session must acknowledge.
    pub seq: u64,
}

impl WorkerStop {
    /// Wait for a fresh authenticated inventory when a prior session is gone.
    /// This creates an in-memory entry only.
    pub(super) fn recovered(machine: Uuid, account: Option<AccountId>) -> Self {
        Self {
            machine,
            account,
            session: [0; 32],
            seq: u64::MAX,
        }
    }
}

impl MachineService {
    /// Admit one ephemeral plugin sandbox on the operator pool. Refuses cold
    /// starts without a free, eligible slot before writing a lease. Admission
    /// and assignment use the existing registry lock and two commits. The
    /// bundle stays in memory and restarting fences the lease.
    pub(crate) fn request_worker(
        &self,
        account: AccountId,
        thread: Uuid,
        source: PluginWorker,
    ) -> Result<RunView, Error> {
        if source.source.len() > worker::MAX_SOURCE_BYTES {
            return Err(Error::invalid_input("plugin bundle exceeds 1 MiB"));
        }
        self.request_work(
            source.space_id,
            account,
            thread,
            Some(worker::IMAGE.into()),
            Some(source),
            None,
            false,
            false,
            false,
            None,
            None,
        )
    }

    /// Whether the authenticated machine has confirmed worker teardown. One
    /// memory lookup; absent live sessions retain their pending stop so capacity
    /// cannot be recycled while an unreachable worker may still exist.
    pub(crate) fn worker_stopped(&self, lease: Uuid) -> Result<bool, Error> {
        Ok(!self.lock()?.worker_stops.contains_key(&lease))
    }

    /// Wait without a timer for positive teardown proof; an unreachable machine
    /// retains capacity until it reconnects. One blocking memory lookup per wake.
    pub(crate) async fn wait_worker_stopped(self: Arc<Self>, lease: Uuid) -> Result<(), Error> {
        loop {
            let changed = self.worker_changes.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let service = self.clone();
            if crate::blocking::spawn(move || service.worker_stopped(lease))
                .await
                .map_err(|_| Error::internal("worker teardown check failed"))??
            {
                return Ok(());
            }
            changed.await;
        }
    }

    /// Start one interactive worker exec; only a worker lease may hold this stream.
    /// Costs one registry lock, no I/O. Agent exec admission remains one-shot.
    pub(crate) fn worker_exec(
        &self,
        account: AccountId,
        lease: Uuid,
        argv: Vec<String>,
    ) -> Result<Uuid, Error> {
        self.internal_exec(account, lease, argv, false)
    }

    /// Start the private provider adapter inside a host-owned sign-in sandbox.
    /// Holds the same in-memory registry lock as worker exec admission.
    pub(crate) fn sign_in_exec(
        &self,
        account: AccountId,
        lease: Uuid,
        argv: Vec<String>,
    ) -> Result<Uuid, Error> {
        self.internal_exec(account, lease, argv, true)
    }

    fn internal_exec(
        &self,
        account: AccountId,
        lease: Uuid,
        argv: Vec<String>,
        sign_in: bool,
    ) -> Result<Uuid, Error> {
        let mut st = self.lock()?;
        if !st.leases.get(&lease).is_some_and(|l| {
            l.row.account == account
                && (if sign_in {
                    l.provider_login
                } else {
                    l.row.worker
                })
                && matches!(l.state, LeaseState::Assigned | LeaseState::Running)
        }) {
            return Err(Error::unavailable(
                "no execution machine has capacity for this plugin worker",
            ));
        }
        let _steps = st.lease_steps(&st.leases[&lease].row)?;
        let id = Uuid::new_v4();
        let mut exec = Exec::new(lease, account, argv, vec![], ExecState::Queued);
        exec.interactive = true;
        st.execs.insert(id, exec);
        st.send_execs(lease);
        Ok(id)
    }

    /// Queue one frame in the authenticated machine outbox, preserving order.
    /// One existing registry lock, no disk I/O. Refuses stale/foreign execs and
    /// limits outstanding input to 16 frames per exec (at most 4 MiB).
    pub(crate) fn interactive_input(
        &self,
        account: AccountId,
        exec: Uuid,
        data: Vec<u8>,
    ) -> Result<(), Error> {
        if data.len() > calternal_types::MAX_FRAME_BYTES + 1 {
            return Err(Error::invalid_input("worker input exceeds frame limit"));
        }
        let mut st = self.lock()?;
        let e = st
            .execs
            .get(&exec)
            .filter(|e| e.account == account && e.interactive && e.state != ExecState::Done)
            .ok_or_else(|| Error::not_found("worker exec not found"))?;
        let lease = e.lease;
        let l = &st.leases[&lease];
        let _steps = st.lease_steps(&l.row)?;
        let machine = l
            .row
            .machine
            .ok_or_else(|| Error::unavailable("worker machine disconnected"))?;
        let fence = l.row.fence;
        let live = st.machines[&machine]
            .live
            .as_ref()
            .ok_or_else(|| Error::unavailable("worker machine disconnected"))?;
        if live
            .outbox
            .iter()
            .filter(|m| matches!(&m.message, Down::Input {exec_id, ..} if *exec_id == exec))
            .count()
            >= 16
        {
            return Err(Error::unavailable("worker input queue is full"));
        }
        st.push(
            machine,
            Down::Input {
                lease_id: lease,
                fence,
                exec_id: exec,
                data,
            },
        );
        Ok(())
    }
}

impl super::state::State {
    /// Count machine-only stops at boot. One bounded read and a cleanup commit
    /// if machine authority was durably revoked before the last shutdown.
    pub(super) fn restore_erased_worker_stops(&mut self) -> Result<(), Error> {
        let mut revoked = Vec::new();
        for (lease, machine) in super::db::erased_worker_stops(&self.db).map_err(super::sql)? {
            if self.machines.get(&machine).is_some_and(|m| !m.row.revoked) {
                self.worker_stops
                    .insert(lease, WorkerStop::recovered(machine, None));
                self.take_slot(Some(machine), None);
            } else {
                revoked.push(lease);
            }
        }
        if !revoked.is_empty() {
            let tx = self.db.transaction().map_err(super::sql)?;
            for lease in revoked {
                super::db::remove_erased_worker_stop(&tx, lease).map_err(super::sql)?;
            }
            tx.commit().map_err(super::sql)?;
        }
        Ok(())
    }

    /// Refuse a cold worker before persisting a waiting lease. Scans the pool
    /// in memory under the same lock that assigns the worker.
    pub(super) fn check_worker_capacity(
        &self,
        account: AccountId,
        settings: &Settings,
    ) -> Result<(), Error> {
        let mut enrolled = false;
        let mut online = false;
        for machine in self.machines.values().filter(|machine| {
            machine.row.owner == Owner::Pool
                && !machine.row.revoked
                && self.may_use(machine, account)
        }) {
            enrolled = true;
            if Self::worker_machine(machine, settings) {
                online = true;
                if self.free_for(machine, account) > 0 {
                    return Ok(());
                }
            }
        }
        Err(if online {
            Error::unavailable("Plugin workers are busy. Retry shortly.")
                .with_details(serde_json::json!({"reason":"worker_capacity"}))
        } else if enrolled {
            Error::unavailable("No pool machine is ready to run containers. Ask an operator to reconnect one or enroll another with Podman.")
                .with_details(serde_json::json!({"reason":"worker_machine_offline"}))
        } else {
            Error::unavailable("A plugin worker needs a container-capable pool machine. Ask an operator to enroll one and grant pool access.")
                .with_details(serde_json::json!({"reason":"worker_machine_required"}))
        })
    }

    /// Whether the authenticated pool session reported an initialized container
    /// runner with a positive sandbox ceiling. Pure in-memory check.
    pub(super) fn worker_machine(machine: &Machine, settings: &Settings) -> bool {
        machine.row.owner == Owner::Pool
            && super::readiness::connected(machine, settings)
            && machine
                .live
                .as_ref()
                .is_some_and(|live| !live.direct && live.capacity.max_sandboxes > 0)
    }

    /// Persist teardown proof before releasing an admission reservation. One
    /// commit per stopped worker under the existing registry lock; no per-frame I/O.
    pub(super) fn finish_worker_stops(
        &mut self,
        machine: Uuid,
        session: &[u8; 32],
        req: &SyncRequest,
    ) -> Result<usize, Error> {
        let done: Vec<_> = self
            .worker_stops
            .iter()
            .filter(|(lease, stop)| {
                stop.machine == machine
                    && (stop.session != *session || req.ack >= stop.seq)
                    && !req.leases.iter().any(|report| report.lease_id == **lease)
            })
            .map(|(id, _)| *id)
            .collect();
        let count = done.len();
        for id in done {
            self.finish_worker_stop(id)?;
        }
        Ok(count)
    }

    /// Release a stopping worker after proof or durable revocation. One commit
    /// ends the lease or removes its erased stop before releasing capacity.
    pub(super) fn finish_worker_stop(&mut self, id: Uuid) -> Result<(), Error> {
        let Some(stop) = self.worker_stops.get(&id).copied() else {
            return Ok(());
        };
        if stop.account.is_none() {
            super::db::remove_erased_worker_stop(&self.db, id).map_err(super::sql)?;
        } else if let Some(lease) = self.leases.get_mut(&id) {
            let mut row = lease.row.clone();
            row.state = LeaseState::Ended.as_str().into();
            super::db::put_lease(&self.db, &row).map_err(super::sql)?;
            lease.state = LeaseState::Ended;
            lease.row = row;
        }
        self.worker_stops.remove(&id);
        self.release_slot(Some(stop.machine), stop.account);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
        service
            .lock()
            .unwrap()
            .machines
            .get_mut(&machine)
            .unwrap()
            .live
            .as_mut()
            .unwrap()
            .capacity
            .max_sandboxes = capacity;
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

    #[test]
    fn worker_frames_and_leases_are_not_public_machine_resources() {
        let dir = tempfile::tempdir().unwrap();
        let spaces =
            calternal_auth::spaces::Spaces::open(&dir.path().join("system/auth.db")).unwrap();
        let owner = spaces.seed_development().unwrap().owner;
        let service =
            MachineService::open(dir.path(), spaces.machines(), Settings::default()).unwrap();
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
        let spaces =
            calternal_auth::spaces::Spaces::open(&dir.path().join("system/auth.db")).unwrap();
        let owner = spaces.seed_development().unwrap().owner;
        let service =
            MachineService::open(dir.path(), spaces.machines(), Settings::default()).unwrap();
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
            guest_resources: Vec::new(),
            guest_runtime: Vec::new(),
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
}
