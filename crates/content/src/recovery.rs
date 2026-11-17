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
        let mut stmt = space
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
            if let Err(e) = space.place(Uuid::parse_str(&id).map_err(internal)?, self.fault)
                && e.code != ErrorCode::Conflict
            {
                return Err(e);
            }
        }
        Ok(())
    }
}
