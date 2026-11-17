//! Atomic job claims bind the interpreter and optional submitting account.
use super::*;

impl Jobs {
    /// Move an admitted job to running and start a fresh stream epoch whose sequence
    /// continues from the checkpoint. `None` when the job is no longer admitted.
    /// Cost: one FULL transaction.
    pub fn claim(&self, space: &str, id: &str) -> Result<Option<Claimed>, Error> {
        self.claim_filtered(space, id, None, None, None)
    }
    /// Claim only a native agent turn, optionally bound to the account that just
    /// submitted it. This prevents a plugin-shaped result from selecting the
    /// native interpreter for an ordinary job. Cost: the existing FULL claim
    /// transaction/read, with predicates in its update; no additional head load.
    pub(crate) fn claim_turn(
        &self,
        space: &str,
        id: &str,
        account: Option<AccountId>,
    ) -> Result<Option<Claimed>, Error> {
        self.claim_filtered(
            space,
            id,
            Some("agent.turn"),
            Some("calternal-orchestrator"),
            account,
        )
    }
    fn claim_filtered(
        &self,
        space: &str,
        id: &str,
        operation: Option<&str>,
        plugin: Option<&str>,
        account: Option<AccountId>,
    ) -> Result<Option<Claimed>, Error> {
        let job = self.tx(space, |tx| {
            let n = tx
                .execute(
                    "UPDATE jobs SET state='running',runs=runs+1 WHERE id=?1 AND state='admitted' AND (?2 IS NULL OR operation=?2) AND (?3 IS NULL OR plugin=?3) AND (?4 IS NULL OR account=?4)",
                    params![id,operation,plugin,account.map(|a|a.to_string())],
                )
                .map_err(db_err)?;
            if n == 0 {
                return Ok(None);
            }
            tx.query_row(
                &format!("SELECT {COLUMNS} FROM jobs WHERE id=?1"),
                [id],
                |r| row(space, r),
            )
            .optional()
            .map_err(db_err)
        })?;
        let Some(job) = job else { return Ok(None) };
        let live = self.live(id);
        let cancel = CancelFlag::new();
        {
            let mut b = live.lock();
            b.epoch = Uuid::now_v7().to_string();
            b.run = job.runs;
            b.next_seq = job.checkpoint_seq;
            b.chunks.clear();
            b.bytes = 0;
            b.cancel = Some(cancel.clone());
            b.running = true;
        }
        live.bump();
        Ok(Some(Claimed { job, live, cancel }))
    }
}
