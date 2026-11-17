//! Shared fenced lease admission for agents and plugin workers.
use super::*;

impl MachineService {
    /// Ask to run work for (space, account, thread); blocks (one commit). Returns
    /// the existing run when the thread already has an active lease for this
    /// account and space. The caller must have checked the account's authority.
    pub fn request_run(
        &self,
        space: SpaceId,
        account: AccountId,
        thread: Uuid,
        image: Option<String>,
    ) -> Result<RunView, Error> {
        self.request_work(space, account, thread, image, None)
    }

    /// Admit agent or worker work atomically with placement. Rejects context
    /// collisions and pending worker recovery; costs one lease commit plus an
    /// assignment commit under the existing machine registry lock.
    pub(super) fn request_work(
        &self,
        space: SpaceId,
        account: AccountId,
        thread: Uuid,
        image: Option<String>,
        worker: Option<calternal_machine::protocol::PluginWorker>,
    ) -> Result<RunView, Error> {
        let mut st = self.lock()?;
        let st = &mut *st;
        if worker.is_some() && !st.recovery_stops.is_empty() {
            return Err(Error::unavailable(
                "plugin workers are waiting for machine teardown confirmation after restart",
            ));
        }
        if let Some(id) = st.threads.get(&thread).copied() {
            let l = &st.leases[&id];
            if l.row.worker != worker.is_some() {
                return Err(Error::not_found("run not found"));
            }
            if l.row.account != account || l.row.space != space {
                return Err(Error::conflict("thread is running for another context"));
            }
            return Ok(st.view(id));
        }
        let row = db::LeaseRow {
            id: Uuid::new_v4(),
            space,
            account,
            thread,
            image,
            worker: worker.is_some(),
            machine: None,
            last_machine: db::last_thread_machine(&st.db, account, thread).map_err(sql)?,
            fence: 0,
            state: LeaseState::Waiting.as_str().into(),
            reason: None,
            created_ms: db::now_ms(),
        };
        db::put_lease(&st.db, &row).map_err(sql)?;
        st.next_order += 1;
        let id = row.id;
        st.threads.insert(thread, id);
        let order = st.next_order;
        st.leases.insert(
            id,
            Lease {
                row,
                state: LeaseState::Waiting,
                order,
            },
        );
        if let Some(worker) = worker {
            st.workers.insert(id, worker);
        }
        st.place_waiting(&self.settings);
        Ok(st.view(id))
    }
}
