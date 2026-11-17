//! Explicit lease resumption for provider turns; never invoked by placement or sweep.
use super::{LeaseState, MachineService};
use crate::error::HostError;
use calternal_auth::AccountId;
use uuid::Uuid;

impl MachineService {
    /// Resume an interrupted lease owned by this account. A fenced old machine is
    /// never adopted by reconciliation. Cost: one durable update plus placement.
    pub fn resume_run(&self, account: AccountId, lease: Uuid) -> Result<(), HostError> {
        let mut st = self.lock()?;
        let l = st
            .leases
            .get_mut(&lease)
            .filter(|l| l.row.account == account)
            .ok_or_else(|| HostError::not_found("run not found"))?;
        if l.state != LeaseState::Interrupted {
            return Err(HostError::conflict("only an interrupted run can resume"));
        }
        l.state = LeaseState::Waiting;
        l.row.state = "waiting".into();
        super::db::put_lease(&st.db, &st.leases[&lease].row).map_err(super::sql)?;
        st.place_waiting(&self.settings);
        Ok(())
    }
}

impl MachineService {
    /// Set execution policy for the caller's own machine, never a pool machine.
    /// Existing leases end and the session is revoked so no old backend keeps work.
    /// Cost: one FULL update plus ending active leases under the registry lock;
    /// the shared machine registry serializes this infrequent owner action.
    pub fn set_direct(
        &self,
        account: AccountId,
        machine: Uuid,
        direct: bool,
    ) -> Result<(), HostError> {
        let mut st = self.lock()?;
        Self::check_owner(&st, account, machine)?;
        st.db
            .execute(
                "UPDATE machines SET direct=?1 WHERE id=?2",
                rusqlite::params![direct, machine.to_string()],
            )
            .map_err(super::sql)?;
        let m = st.machines.get_mut(&machine).expect("checked owner");
        m.row.direct = direct;
        if let Some(live) = m.live.take() {
            st.sessions.remove(&live.session);
            live.notify.notify_one();
        }
        let leases: Vec<_> = st
            .leases
            .values()
            .filter(|l| l.row.machine == Some(machine) && l.state != LeaseState::Ended)
            .map(|l| l.row.id)
            .collect();
        for id in leases {
            st.end_with_files(
                id,
                "machine execution setting changed",
                &self.settings,
                true,
            );
        }
        Ok(())
    }
}

impl MachineService {
    /// End a provider lease and kill its processes while retaining thread files
    /// and its persistent account home. Cost: one commit and one machine message.
    pub fn release_run(&self, account: AccountId, lease: Uuid) -> Result<(), HostError> {
        let mut st = self.lock()?;
        if !st
            .leases
            .get(&lease)
            .is_some_and(|l| l.row.account == account)
        {
            return Err(HostError::not_found("run not found"));
        }
        st.end_with_files(lease, "provider turn ended", &self.settings, true);
        Ok(())
    }
}
