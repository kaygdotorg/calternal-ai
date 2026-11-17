//! Bounded command and stdin admission for machine exec streams.
use super::*;
impl MachineService {
    /// Queue a command for a run; it is sent as soon as the run is assigned.
    /// Blocks briefly (no file I/O).
    ///
    /// `env` contains host-supplied configuration and the scoped Calternal
    /// credential. It is memory-only and delivered over the authenticated machine
    /// connection; provider logins are never read or forwarded. Future secret
    /// handles must resolve at delivery through the existing audited secret port.
    pub fn exec(
        &self,
        account: AccountId,
        lease: Uuid,
        argv: Vec<String>,
        env: Vec<(String, String)>,
    ) -> Result<Uuid, Error> {
        self.exec_input(account, lease, argv, env, String::new())
    }

    /// Queue bounded stdin with an exec; input is memory-only and delivered once.
    /// Cost: one registry lock; no I/O. At most 128 KiB of UTF-8 input per exec.
    pub fn exec_input(
        &self,
        account: AccountId,
        lease: Uuid,
        argv: Vec<String>,
        env: Vec<(String, String)>,
        stdin: String,
    ) -> Result<Uuid, Error> {
        if stdin.len() > 128 * 1024 {
            return Err(Error::invalid_input("exec input exceeds 128 KiB"));
        }
        if argv.is_empty() || argv.len() > 4096 || argv.iter().any(|a| a.len() > 65_536) {
            return Err(Error::invalid_input(
                "argv must have 1 to 4096 arguments of at most 64 KiB",
            ));
        }
        let mut st = self.lock()?;
        let st = &mut *st;
        match st.leases.get(&lease) {
            Some(l) if l.row.account == account && l.state != LeaseState::Ended => {}
            Some(l) if l.row.account == account => {
                return Err(Error::conflict("run has ended"));
            }
            _ => return Err(Error::not_found("run not found")),
        }
        let id = Uuid::new_v4();
        let mut exec = Exec::new(lease, account, argv, env, ExecState::Queued);
        exec.stdin = stdin;
        st.execs.insert(id, exec);
        st.send_execs(lease);
        Ok(id)
    }
}
