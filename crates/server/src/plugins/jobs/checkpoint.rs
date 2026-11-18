//! Checkpoint publication fenced against cancellation before external admission.
use super::*;

impl Jobs {
    /// Prepare a native resource and publish its checkpoint while this space's
    /// job lock fences cancellation. The callback must not re-enter jobs/content;
    /// it may durably admit a machine lease so cancellation always observes its ID.
    /// The lock across that call is deliberate: checking then publishing separately
    /// could leak a late lease after stop. Caller releases its resource on failure.
    /// Cost: one indexed current-run check, the callback, and one FULL checkpoint
    /// update. Runs only on a blocking worker; no installation-wide lock is added.
    pub(crate) fn checkpoint_prepared(
        &self,
        space: &str,
        id: &str,
        run: u32,
        live: &Live,
        prepare: impl FnOnce() -> Result<Value, Error>,
    ) -> Result<Value, Error> {
        let (checkpoint, _actor) = self.tx(space, |tx| {
            let account: String = tx.query_row(
                "SELECT account FROM jobs WHERE id=?1 AND state='placing' AND runs=?2",
                params![id, run], |row| row.get(0),
            ).optional().map_err(db_err)?
                .ok_or_else(|| Error::conflict("turn is no longer placing"))?;
            let actor = self.lifecycle.step(AccountId::parse(&account)?.as_uuid())?;
            let checkpoint = prepare()?;
            let text = bounded_json(&checkpoint, MAX_CHECKPOINT_BYTES)?;
            tx.execute(
                "UPDATE jobs SET checkpoint=?1,checkpoint_seq=?2,checkpoints=checkpoints+1 WHERE id=?3 AND state='placing' AND runs=?4",
                params![text, live.next_seq() as i64, id, run],
            ).map_err(db_err)?;
            Ok((checkpoint, actor))
        })?;
        Ok(checkpoint)
    }

    /// Save queue progress while a native turn is placing. One fenced job
    /// update, preserving the separate atomic transition that starts execution.
    pub(crate) fn checkpoint_placing(
        &self,
        job: &Job,
        live: &Live,
        checkpoint: &Value,
    ) -> Result<(), Error> {
        let text = bounded_json(checkpoint, MAX_CHECKPOINT_BYTES)?;
        let seq = live.next_seq() as i64;
        let _actor = self.lifecycle.step(job.account.as_uuid())?;
        self.tx(&job.space, |tx| {
            let changed = tx
                .execute(
                    "UPDATE jobs SET checkpoint=?1,checkpoint_seq=?2,checkpoints=checkpoints+1
                 WHERE id=?3 AND state='placing' AND runs=?4",
                    params![text, seq, job.id, job.runs],
                )
                .map_err(db_err)?;
            if changed != 1 {
                return Err(Error::conflict("turn is no longer placing"));
            }
            Ok(())
        })
    }
}

impl Jobs {
    /// Publish running state and the selected machine in one fenced FULL update.
    /// A canceled or superseded run cannot publish its placement.
    pub(crate) fn start_placed_turn(
        &self,
        space: &str,
        id: &str,
        run: u32,
        live: &Live,
        checkpoint: &Value,
    ) -> Result<(), Error> {
        if checkpoint["machine_id"]
            .as_str()
            .and_then(|id| Uuid::parse_str(id).ok())
            .is_none()
        {
            return Err(Error::internal("turn checkpoint has no machine"));
        }
        let text = bounded_json(checkpoint, MAX_CHECKPOINT_BYTES)?;
        let seq = live.next_seq() as i64;
        let _actor = self.tx(space, |tx| {
            let account: String = tx.query_row(
                "UPDATE jobs SET state='running',checkpoint=?1,checkpoint_seq=?2,checkpoints=checkpoints+1
                 WHERE id=?3 AND state='placing' AND runs=?4 RETURNING account",
                params![text, seq, id, run],
                |row| row.get(0),
            ).optional().map_err(db_err)?
                .ok_or_else(|| Error::conflict("turn is no longer placing"))?;
            self.lifecycle.step(AccountId::parse(&account)?.as_uuid())
        })?;
        Ok(())
    }
}
