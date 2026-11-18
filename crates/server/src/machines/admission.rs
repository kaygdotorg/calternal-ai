//! Shared fenced lease admission for agents and plugin workers.
use super::*;
use space_execution::SpaceExecution;

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
            false,
            None,
            None,
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
            false,
            None,
            None,
        )
    }

    /// Request a disposable run on the last personal sandbox machine. The
    /// lease is admitted only with current spare capacity.
    pub(crate) fn request_title(
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
            false,
            true,
            None,
            None,
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
            false,
            None,
            None,
        )
    }

    /// Admit a fenced shared turn or private sign-in scratch lease.
    /// Requires the current binding and uses the ordinary lease admission commit.
    pub(crate) fn request_shared(
        &self,
        space: SpaceId,
        actor: AccountId,
        thread: Uuid,
        target: SpaceExecution,
        sign_in: bool,
        sandbox: Option<(Option<String>, calternal_machine::protocol::SharedTools)>,
    ) -> Result<RunView, Error> {
        self.request_work(
            space,
            actor,
            thread,
            sandbox.as_ref().and_then(|(image, _)| image.clone()),
            None,
            Some(target.machine),
            false,
            sign_in,
            false,
            None,
            Some((target, sandbox.map(|(_, tools)| tools))),
        )
    }

    /// Admit a harness on a machine that supports its isolated profile and policy.
    /// Uses the same fenced placement and durability as a legacy provider lease.
    pub(crate) fn request_harness(
        &self,
        space: SpaceId,
        account: AccountId,
        thread: Uuid,
        target: Option<Uuid>,
        image: String,
        harness: calternal_machine::protocol::HarnessSandbox,
        sign_in: bool,
        pool_diagnostic: bool,
    ) -> Result<RunView, Error> {
        self.request_work(
            space,
            account,
            thread,
            Some(image),
            None,
            target,
            pool_diagnostic,
            sign_in,
            false,
            Some(harness),
            None,
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
        background_title: bool,
        harness: Option<calternal_machine::protocol::HarnessSandbox>,
        shared: Option<(
            SpaceExecution,
            Option<calternal_machine::protocol::SharedTools>,
        )>,
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
        let _sponsor = shared
            .map(|(target, _)| target)
            .filter(|target| target.sponsor != account)
            .and_then(|target| {
                self.lifecycle
                    .get()
                    .map(|l| l.step(target.sponsor.as_uuid()))
            })
            .transpose()?;
        let mut st = self.lock()?;
        let st = &mut *st;
        if let Some((mut target, tools)) = shared {
            target.tools_enabled = tools.is_some();
            st.require_execution(space, target)?;
        }
        let shared_sponsor = shared.map(|(target, _)| target.sponsor);
        let shared_tools = shared.and_then(|(_, tools)| tools);
        if background_title
            && !pinned_machine
                .and_then(|id| st.machines.get(&id))
                .is_some_and(|m| {
                    m.row.owner == (calternal_machine::protocol::Owner::Account { id: account })
                        && m.live.as_ref().is_some_and(|live| !live.direct)
                        && st.free(m) > 0
                        && !st.leases.values().any(|l| {
                            l.row.background_title && l.assigned_machine() == Some(m.row.id)
                        })
                })
        {
            return Err(Error::unavailable(
                "no spare personal sandbox for title generation",
            ));
        }
        if let Some(id) = st.threads.get(&thread).copied() {
            let l = &st.leases[&id];
            if l.row.worker != worker.is_some()
                || l.provider_login != provider_login
                || l.row.background_title != background_title
            {
                return Err(Error::not_found("run not found"));
            }
            if l.row.account != account
                || l.row.space != space
                || l.row.pinned_machine != pinned_machine
                || l.row.shared_sponsor != shared_sponsor
                || l.row.shared_tools != shared_tools
                || l.row.harness != harness
                || l.row.image != image
            {
                return Err(Error::conflict("thread is running for another context"));
            }
            return Ok(st.view(id));
        }
        let is_worker = worker.is_some();
        if is_worker {
            st.place_waiting(&self.settings);
            st.check_worker_capacity(account, &self.settings)?;
        }
        let row = db::LeaseRow {
            id: Uuid::new_v4(),
            space,
            account,
            shared_sponsor,
            shared_tools,
            thread,
            image,
            harness,
            worker: is_worker,
            background_title,
            machine: None,
            last_machine: db::last_thread_machine(&st.db, account, thread).map_err(sql)?,
            pinned_machine,
            fence: 0,
            state: LeaseState::Waiting.as_str().into(),
            reason: None,
            created_ms: db::now_ms(),
        };
        if background_title {
            db::put_title(&mut st.db, &row).map_err(sql)?;
        } else {
            db::put_lease(&st.db, &row).map_err(sql)?;
        }
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
        if is_worker && st.leases[&id].state == LeaseState::Waiting {
            st.end_lease(id, "worker placement failed", &self.settings);
            return Err(Error::unavailable(
                "Plugin worker placement failed. Retry shortly.",
            ));
        }
        Ok(st.view(id))
    }
}
