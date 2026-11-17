//! Per-space recovery performed under the existing cache slot lock before admission.
use crate::{Content, ErrorCode, Result, db::Space, types::internal};
use uuid::Uuid;

impl Content {
    /// Replays committed outbox intents before materializing pending heads. Failure
    /// leaves the cache slot unopened, so a later call retries recovery. External
    /// file conflicts remain preserved and blocked; no other space is touched.
    pub(crate) fn recover_space(&self, space: &mut Space) -> Result<()> {
        space.cleanup()?;
        self.publish_pending(space)?;
        space.materialize_pending(self.fault)
    }
}

impl Space {
    /// Complete durable pending heads in this space. Called under its slot lock
    /// by lazy recovery and the existing lifecycle worker, never an async worker.
    pub(crate) fn materialize_pending(&mut self, fault: fn(crate::FaultPoint)) -> Result<()> {
        // A later failed publication can coexist with an older acknowledged head.
        // Never let its old wake publish newer file bytes before journal recovery.
        let unpublished: bool = self
            .conn
            .query_row("SELECT EXISTS(SELECT 1 FROM outbox)", [], |r| r.get(0))
            .map_err(internal)?;
        if unpublished {
            return Err(crate::Error::unavailable(
                "pending journal publication; retry the request",
            ));
        }
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM items WHERE mat_state='pending'")
            .map_err(internal)?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(internal)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(internal)?;
        drop(stmt);
        for id in ids {
            if let Err(e) = self.place(Uuid::parse_str(&id).map_err(internal)?, fault)
                && e.code != ErrorCode::Conflict
            {
                return Err(e);
            }
        }
        self.pending_since = None;
        Ok(())
    }
}
