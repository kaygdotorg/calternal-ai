//! Protected space revocation before the existing owner lifecycle drains.
use super::*;

impl Spaces {
    /// Reads an unexpired deletion reply for the same active actor and request
    /// after the target is tombstoned. An indexed read checks the original
    /// fingerprint and returns a conflict on reuse. The boolean reports
    /// whether physical erasure is still pending.
    pub fn replay_space_delete(
        &self,
        actor: AccountId,
        space: SpaceId,
        request_id: String,
        fingerprint: String,
    ) -> Result<Option<(serde_json::Value, bool)>> {
        let retention = i64::try_from(self.receipt_retention.as_millis()).unwrap_or(i64::MAX);
        self.run(move |c| {
            exists_account(c, actor)?;
            let cutoff = crate::db::now_ms().saturating_sub(retention);
            let receipt: Option<(String, String, String, bool)> = c.query_row(
                "SELECT r.operation,r.fingerprint,r.result,e.pending FROM space_receipts r JOIN owner_erasures e ON e.kind='space' AND e.id=r.scope WHERE r.actor=?1 AND r.scope=?2 AND r.request_id=?3 AND r.at_ms>=?4",
                rusqlite::params![actor.to_string(), space.to_string(), request_id, cutoff],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            ).optional().map_err(sql)?;
            receipt.map(|(op, saved_fp, value, pending)| {
                if op != "spaces.delete" || saved_fp != fingerprint {
                    return Err(Error::new(ErrorCode::Conflict, "request_id reused"));
                }
                Ok((serde_json::from_str(&value).map_err(sql)?, pending))
            }).transpose()
        })
    }

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
