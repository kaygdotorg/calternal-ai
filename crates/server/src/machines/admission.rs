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
        self.request_run_on(space, account, thread, image, None)
    }

    /// Admit a provider lease optionally pinned to the machine that ran it
    /// before a person request. A missing or busy pin waits for that machine.
    /// Uses the shared lease and placement commits.
    pub(crate) fn request_run_on(
        &self,
        space: SpaceId,
        account: AccountId,
        thread: Uuid,
        image: Option<String>,
        pinned_machine: Option<Uuid>,
    ) -> Result<RunView, Error> {
        self.request_work(
            space,
            account,
            thread,
            image,
            None,
            pinned_machine,
            false,
            false,
        )
    }

    /// Lease for a diagnostic on one explicitly authorized pool machine. This
    /// exception is local to the pinned scratch run and grants no pool access.
    pub(crate) fn request_pool_diagnostic(
        &self,
        space: SpaceId,
        account: AccountId,
        scratch: Uuid,
        machine: Uuid,
    ) -> Result<RunView, Error> {
        self.request_work(
            space,
            account,
            scratch,
            None,
            None,
            Some(machine),
            true,
            false,
        )
    }

    /// Reserve a fenced scratch sandbox with write access to the machine-local login.
    /// The host authorizes the target and never exposes the lease to clients.
    pub(crate) fn request_sign_in(
        &self,
        space: SpaceId,
        account: AccountId,
        machine: Uuid,
    ) -> Result<RunView, Error> {
        self.request_work(
            space,
            account,
            Uuid::new_v4(),
            None,
            None,
            Some(machine),
            false,
            true,
        )
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
        pinned_machine: Option<Uuid>,
        pool_diagnostic: bool,
        provider_login: bool,
    ) -> Result<RunView, Error> {
        let _account = self
            .lifecycle
            .get()
            .map(|l| l.step(account.as_uuid()))
            .transpose()?;
        let _space = self
            .lifecycle
            .get()
            .map(|l| l.step(space.as_uuid()))
            .transpose()?;
        let mut st = self.lock()?;
        let st = &mut *st;
        if worker.is_some() && !st.recovery_stops.is_empty() {
            return Err(Error::unavailable(
                "plugin workers are waiting for machine teardown confirmation after restart",
            ));
        }
        if let Some(id) = st.threads.get(&thread).copied() {
            let l = &st.leases[&id];
            if l.row.worker != worker.is_some() || l.provider_login != provider_login {
                return Err(Error::not_found("run not found"));
            }
            if l.row.account != account
                || l.row.space != space
                || l.row.pinned_machine != pinned_machine
            {
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
            pinned_machine,
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
                pool_diagnostic,
                provider_login,
            },
        );
        if let Some(worker) = worker {
            st.workers.insert(id, worker);
        }
        st.place_waiting(&self.settings);
        Ok(st.view(id))
    }
}
