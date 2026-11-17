//! Atomic directory request receipts shared by single and batched writes.
use super::*;
use serde_json::Value;

pub(super) struct Receipt {
    actor: String,
    scope: String,
    id: String,
    operation: String,
    fingerprint: String,
    retention_ms: i64,
}

impl Receipt {
    pub(super) fn new(
        actor: AccountId,
        space: Option<SpaceId>,
        retention: std::time::Duration,
        (id, operation, fingerprint): (&str, &str, &str),
    ) -> Self {
        Self {
            actor: actor.to_string(),
            scope: space.map_or_else(|| actor.to_string(), |space| space.to_string()),
            id: id.to_owned(),
            operation: operation.to_owned(),
            fingerprint: fingerprint.to_owned(),
            retention_ms: retention.as_millis() as i64,
        }
    }
}

/// Apply the already-authorized change or replay its receipt. Pruning and receipt
/// creation use the same transaction or savepoint as the authority mutation.
pub(super) fn apply_receipted<E>(
    connection: &Connection,
    receipt: Option<&Receipt>,
    apply: impl FnOnce() -> Result<(Value, E)>,
) -> Result<(Value, Option<E>)> {
    let now = crate::db::now_ms();
    if let Some(r) = receipt {
        connection
            .execute(
                "DELETE FROM space_receipts WHERE actor=?1 AND at_ms<?2",
                params![r.actor, now.saturating_sub(r.retention_ms)],
            )
            .map_err(sql)?;
        let saved: Option<(String, String, String)> = connection
            .query_row(
                "SELECT operation,fingerprint,result FROM space_receipts
             WHERE actor=?1 AND scope=?2 AND request_id=?3",
                params![r.actor, r.scope, r.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(sql)?;
        if let Some((operation, fingerprint, value)) = saved {
            if operation != r.operation || fingerprint != r.fingerprint {
                return Err(Error::new(ErrorCode::Conflict, "request_id reused"));
            }
            return Ok((serde_json::from_str(&value).map_err(sql)?, None));
        }
    }
    let (value, effect) = apply()?;
    if let Some(r) = receipt {
        connection
            .execute(
                "INSERT INTO space_receipts VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    r.actor,
                    r.scope,
                    r.id,
                    r.operation,
                    r.fingerprint,
                    value.to_string(),
                    now
                ],
            )
            .map_err(sql)?;
    }
    Ok((value, Some(effect)))
}
