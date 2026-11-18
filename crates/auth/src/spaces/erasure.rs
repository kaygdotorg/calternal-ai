//! Protected space revocation before the existing owner lifecycle drains.
use super::*;

impl Spaces {
    /// Revoke a space and queue its erasure in one protected commit. The
    /// callback closes owner admissions immediately after the SQLite commit.
    /// Drain the owner only after this call returns, outside the auth worker.
    pub fn delete_fenced(
        &self,
        actor: AccountId,
        space: SpaceId,
        receipt: (&str, &str, &str),
        close: impl FnOnce(()) + Send + 'static,
    ) -> Result<serde_json::Value> {
        self.mutate_with(
            actor,
            Some(space),
            Some(receipt),
            move |tx| {
                check(tx, actor, Some(space), &SpaceChange::Delete)?;
                Ok(move |tx: &rusqlite::Transaction<'_>| {
                    tx.execute(
                        "DELETE FROM development_directory WHERE id=?1",
                        [space.to_string()],
                    )
                    .map_err(sql)?;
                    tx.execute(
                        "DELETE FROM space_receipts WHERE scope=?1",
                        [space.to_string()],
                    )
                    .map_err(sql)?;
                    Ok((apply(tx, actor, Some(space), &SpaceChange::Delete)?, ()))
                })
            },
            close,
        )
    }
}
