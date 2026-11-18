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
        self.exec_scoped(account, lease, argv, env, stdin, false, false)
    }

    /// Run the selected provider's title entry point without environment
    /// credentials on its private lease. Uses the ordinary bounded exec queue.
    pub(crate) fn title_exec(
        &self,
        account: AccountId,
        lease: Uuid,
        executable: String,
        stdin: String,
    ) -> Result<Uuid, Error> {
        self.exec_scoped(
            account,
            lease,
            vec![executable, "--title".into()],
            vec![],
            stdin,
            true,
            false,
        )
    }

    /// Start only the image-owned tool-free adapter on a shared turn lease.
    /// The shared provider credentials are never provided as command data.
    pub(crate) fn shared_turn_exec(
        &self,
        account: AccountId,
        lease: Uuid,
        stdin: String,
    ) -> Result<Uuid, Error> {
        self.exec_scoped(
            account,
            lease,
            vec![crate::turns::space_execution::SHARED_ADAPTER.into()],
            Vec::new(),
            stdin,
            false,
            true,
        )
    }

    fn exec_scoped(
        &self,
        account: AccountId,
        lease: Uuid,
        argv: Vec<String>,
        env: Vec<(String, String)>,
        stdin: String,
        background_title: bool,
        shared: bool,
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
        let _steps = match st.leases.get(&lease) {
            Some(l)
                if l.row.account == account
                    && (l.row.worker
                        || l.provider_login
                        || l.row.background_title != background_title
                        || l.row.shared_sponsor.is_some() != shared) =>
            {
                return Err(Error::forbidden(
                    "internal machine execs are host-controlled",
                ));
            }
            Some(l) if l.row.account == account && l.state != LeaseState::Ended => {
                st.lease_steps(&l.row)?
            }
            Some(l) if l.row.account == account => {
                return Err(Error::conflict("run has ended"));
            }
            _ => return Err(Error::not_found("run not found")),
        };
        let id = Uuid::new_v4();
        let mut exec = Exec::new(lease, account, argv, env, ExecState::Queued);
        exec.stdin = stdin;
        st.execs.insert(id, exec);
        st.send_execs(lease);
        Ok(id)
    }
}
