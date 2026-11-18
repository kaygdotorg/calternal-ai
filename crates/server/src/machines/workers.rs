//! Worker admission and ordered stdin on the existing fenced machine exec channel.
use super::*;
use calternal_machine::{
    protocol::{Down, PluginWorker, SyncRequest},
    sandbox::worker,
};

impl MachineService {
    /// Admit one ephemeral plugin sandbox using ordinary account/pool placement.
    /// No source persists: restarting the server ends these leases. A cold start
    /// costs one lease commit under the existing machine registry lock; no new lock
    /// is introduced. The caller verifies the bundle and bounds live workers.
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
        let mut st = self.lock()?;
        if !st.leases.get(&lease).is_some_and(|l| {
            l.row.account == account
                && l.row.worker
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
    pub(crate) fn worker_input(
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
            .filter(|(lease, (owner, previous, seq))| {
                *owner == machine
                    && (*previous != *session || req.ack >= *seq)
                    && !req.leases.iter().any(|report| report.lease_id == **lease)
            })
            .map(|(id, _)| *id)
            .collect();
        let count = done.len();
        for id in done {
            if let Some(lease) = self.leases.get_mut(&id) {
                let mut row = lease.row.clone();
                row.state = "ended".into();
                super::db::put_lease(&self.db, &row).map_err(super::sql)?;
                lease.state = LeaseState::Ended;
                lease.row = row;
            }
            self.worker_stops.remove(&id);
            self.recovery_stops.remove(&id);
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_frames_and_leases_are_not_public_machine_resources() {
        let dir = tempfile::tempdir().unwrap();
        let spaces =
            calternal_auth::spaces::Spaces::open(&dir.path().join("system/auth.db")).unwrap();
        let owner = spaces.seed_development().unwrap().owner;
        let service =
            MachineService::open(dir.path(), spaces.machines(), Settings::default()).unwrap();
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
        let run = service
            .request_worker(owner, Uuid::new_v4(), source())
            .unwrap();
        let machine = Uuid::new_v4();
        {
            let mut state = service.lock().unwrap();
            let lease = state.leases.get_mut(&run.lease_id).unwrap();
            lease.row.machine = Some(machine);
            lease.state = LeaseState::Assigned;
        }
        service.stop_run(owner, run.lease_id).unwrap();
        assert!(!service.worker_stopped(run.lease_id).unwrap());
        let mut report = SyncRequest {
            ack: 4,
            discards: vec![],
            capacity: Default::default(),
            draining: false,
            leases: vec![],
        };
        {
            let mut state = service.lock().unwrap();
            state
                .worker_stops
                .insert(run.lease_id, (machine, [2; 32], 5));
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
        assert!(
            service
                .request_worker(owner, Uuid::new_v4(), source())
                .is_err()
        );
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
        assert!(
            service
                .request_worker(owner, Uuid::new_v4(), source())
                .is_ok()
        );
    }
}
