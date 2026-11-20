//! Execution receipts on the existing per-space content publication boundary.
use crate::{Content, FaultPoint, Request, Result, types::internal};
use calternal_types::{Error, SpaceId};
use rusqlite::{Transaction, TransactionBehavior, params};
use serde_json::Value;

impl Content {
    /// Commit a checked execution effect and its receipt on the selected
    /// space's FULL content transaction. Checks run on both fresh and replay
    /// paths under the space's writer lock. Neither callback may re-enter
    /// content or dispatch remote work. A crash after commit replays the
    /// outbox before looking up the receipt or admitting any duplicate work.
    /// Cost when warm: the normal outbox check, one indexed receipt read and,
    /// for fresh work, one content commit plus journal commit and retirement.
    pub fn execution_command(
        &self,
        space: SpaceId,
        request: Request<'_>,
        authorize: impl FnOnce() -> Result<()>,
        run: impl FnOnce(&Transaction<'_>) -> Result<Value>,
    ) -> Result<Value> {
        let _actor = self.lifecycle.step(request.account.as_uuid())?;
        self.with_space(space, |s| {
            self.publish_pending(s)?;
            authorize()?;
            if let Some(saved) = self
                .journals
                .receipt(&s.journal, request.account, request.id)?
            {
                if saved.operation != request.operation || saved.fingerprint != request.fingerprint
                {
                    return Err(Error::request_reused());
                }
                return Ok(saved.result);
            }
            let tx = s
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(internal)?;
            let response = run(&tx)?;
            tx.execute(
                "INSERT INTO outbox VALUES(?1,?2,?3,?4,NULL,'control',?5)",
                params![
                    request.account.to_string(),
                    request.id,
                    request.operation,
                    request.fingerprint,
                    request.outbox_result(&response)
                ],
            )
            .map_err(internal)?;
            tx.commit().map_err(internal)?;
            (self.fault)(FaultPoint::AfterCommit);
            self.publish_pending(s)?;
            Ok(response)
        })
    }
}
