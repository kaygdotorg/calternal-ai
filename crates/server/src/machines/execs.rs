//! Bounded admission and output delivery for machine exec streams.
use super::*;
use calternal_machine::protocol::ChunkEvent;
use tokio::sync::Notify;

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
        self.queued_exec(account, lease, argv, env, stdin, false, false, false, false)
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
        self.queued_exec(
            account,
            lease,
            vec![executable, "--title".into()],
            vec![],
            stdin,
            true,
            false,
            false,
            false,
        )
    }

    /// Start only an image-owned adapter on a shared turn lease.
    /// The shared provider credentials are never provided as command data.
    pub(crate) fn shared_turn_exec(
        &self,
        account: AccountId,
        lease: Uuid,
        env: Vec<(String, String)>,
        stdin: String,
        provider: Option<calternal_machine::protocol::SharedToolsProvider>,
    ) -> Result<Uuid, Error> {
        let executable = match provider {
            Some(calternal_machine::protocol::SharedToolsProvider::Codex) => {
                "/opt/calternal-provider/bin/calternal-provider-codex"
            }
            _ => crate::turns::space_execution::SHARED_ADAPTER,
        };
        self.queued_exec(
            account,
            lease,
            vec![executable.into()],
            env,
            stdin,
            false,
            false,
            false,
            true,
        )
    }

    /// Start a host-controlled provider process in its assigned harness sandbox.
    /// Interactive input is permitted only for the negotiated adapter.
    pub(crate) fn harness_exec(
        &self,
        account: AccountId,
        lease: Uuid,
        argv: Vec<String>,
        env: Vec<(String, String)>,
        stdin: String,
        interactive: bool,
    ) -> Result<Uuid, Error> {
        self.queued_exec(
            account,
            lease,
            argv,
            env,
            stdin,
            false,
            true,
            interactive,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn queued_exec(
        &self,
        account: AccountId,
        lease: Uuid,
        argv: Vec<String>,
        env: Vec<(String, String)>,
        stdin: String,
        background_title: bool,
        harness: bool,
        interactive: bool,
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
                        || l.row.harness.is_some() != harness
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
        exec.interactive = interactive;
        st.execs.insert(id, exec);
        st.send_execs(lease);
        Ok(id)
    }

    /// Output of an exec after `after`, waiting up to `wait` (at most 30 s) for
    /// something new.
    ///
    /// Reading with `after = n` releases chunks up to `n` from the buffer, which
    /// is what lets a machine that was told to wait continue: one reader per exec.
    pub async fn read_output(
        self: &Arc<Self>,
        account: AccountId,
        exec_id: Uuid,
        after: u64,
        wait: Duration,
    ) -> Result<OutputView, Error> {
        let deadline = tokio::time::Instant::now() + wait.min(Duration::from_secs(30));
        loop {
            let svc = self.clone();
            let step = blocking(move || svc.take_output(account, exec_id, after)).await?;
            let notify = match step {
                Ok(view) => return Ok(view),
                Err(notify) => notify,
            };
            if tokio::time::timeout_at(deadline, notify.notified())
                .await
                .is_err()
            {
                return Ok(OutputView {
                    chunks: Vec::new(),
                    next: after,
                    exit: None,
                    lost_before: None,
                });
            }
        }
    }

    /// Release chunks up to `after` and return the rest, or the exec's `Notify`
    /// when there is nothing yet.
    fn take_output(
        &self,
        account: AccountId,
        exec_id: Uuid,
        after: u64,
    ) -> Result<Result<OutputView, Arc<Notify>>, Error> {
        let mut st = self.lock()?;
        let exec = match st.execs.get_mut(&exec_id) {
            Some(e) if e.account == account => e,
            _ => return Err(Error::not_found("exec not found")),
        };
        while exec.chunks.front().is_some_and(|(c, _)| c.seq <= after) {
            exec.held -= exec.chunks.pop_front().expect("front").1;
        }
        if exec.chunks.is_empty() && exec.state != ExecState::Done {
            return Ok(Err(exec.notify.clone()));
        }
        let chunks: Vec<Chunk> = exec.chunks.iter().map(|(c, _)| c.clone()).collect();
        let exit = chunks.iter().find_map(|c| match &c.event {
            ChunkEvent::Exit(e) => Some(e.clone()),
            ChunkEvent::Data { .. } => None,
        });
        Ok(Ok(OutputView {
            next: chunks.last().map_or(after.max(exec.last_seq), |c| c.seq),
            exit,
            lost_before: exec.lost_before,
            chunks,
        }))
    }

    /// The space of a public agent exec; internal worker streams stay hidden.
    /// Cost: one registry lock and two memory lookups, no I/O.
    pub fn exec_space(&self, exec: Uuid) -> Result<Option<SpaceId>, Error> {
        let st = self.lock()?;
        Ok(st
            .execs
            .get(&exec)
            .and_then(|e| st.leases.get(&e.lease))
            .filter(|l| !l.row.worker && !l.provider_login && !l.row.background_title)
            .map(|l| l.row.space))
    }
}
