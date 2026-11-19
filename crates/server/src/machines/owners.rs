//! Owner and operator machine administration through guarded authority.
use super::*;
impl MachineService {
    // --------------------------------------------------------------- owners

    /// Issue a one-time enrolment code for `owner`; blocks (one protected
    /// authority commit). Returns the code and its expiry in Unix milliseconds.
    pub fn issue_code(&self, owner: Owner) -> Result<(String, i64), Error> {
        let st = self.lock()?;
        st.authority
            .issue_code(db::owner_account(owner), self.settings.code_ttl)
            .map_err(authority_error)
    }

    /// Issue a code valid for `ttl` instead of the configured default (operator
    /// commands choose the lifetime); blocks (one protected authority commit).
    pub fn issue_code_for(&self, owner: Owner, ttl: Duration) -> Result<(String, i64), Error> {
        let st = self.lock()?;
        st.authority
            .issue_code(db::owner_account(owner), ttl)
            .map_err(authority_error)
    }

    /// Every enrolled machine, revoked ones included, for the operator. Blocks
    /// briefly (no file I/O).
    pub fn all_machines(&self) -> Result<Vec<db::MachineRow>, Error> {
        let st = self.lock()?;
        let mut rows: Vec<_> = st.machines.values().map(|m| m.row.clone()).collect();
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(rows)
    }

    /// Revoke any machine as the operator; false when it does not exist. Blocks
    /// (one protected authority commit); takes effect at once as in [`Self::revoke`].
    pub fn revoke_any(&self, machine: Uuid) -> Result<bool, Error> {
        let mut st = self.lock()?;
        let st = &mut *st;
        if !st.authority.revoke(machine).map_err(authority_error)? {
            return Ok(false);
        }
        let result = st.apply_revocation(machine, &self.settings);
        self.worker_changes.notify_waiters();
        result?;
        Ok(true)
    }

    /// Grant or withdraw an account's use of pool machines; blocks (one protected
    /// authority commit) and places waiting runs the grant now allows.
    pub fn set_pool_grant(&self, account: AccountId, granted: bool) -> Result<(), Error> {
        let mut st = self.lock()?;
        let st = &mut *st;
        st.authority
            .set_pool_grant(account, granted)
            .map_err(authority_error)?;
        if granted {
            st.pool_grants.insert(account);
        } else {
            st.pool_grants.remove(&account);
            st.close_account_channels(account);
        }
        st.readiness.change(account);
        st.place_waiting(&self.settings);
        Ok(())
    }

    /// Machines owned by `account`. Blocks briefly (no file I/O).
    pub fn machines_of(&self, account: AccountId) -> Result<Vec<MachineView>, Error> {
        let st = self.lock()?;
        let owner = Owner::Account { id: account };
        Ok(st
            .machines
            .values()
            .filter(|m| m.row.owner == owner)
            .map(|m| st.machine_view(m, &self.settings))
            .collect())
    }
}
