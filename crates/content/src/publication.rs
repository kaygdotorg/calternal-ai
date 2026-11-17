//! Identifier routing and idempotent outbox publication after a content commit.
use crate::{Content, Result, db::Space, journal::lock, types::internal};
use calternal_types::SpaceId;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;
impl Content {
    /// Records the global route of item `id`, once. An existing route (every update)
    /// is a pooled read only; a new item takes the catalog writer for one FULL insert.
    pub(crate) fn route(&self, space: SpaceId, id: &str, kind: &str) -> Result<()> {
        let read = |c: &Connection| {
            c.query_row("SELECT space,kind FROM items WHERE id=?1", [id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .optional()
        };
        let existing = match self.catalog.read(read).map_err(internal)? {
            Some(existing) => existing,
            None => {
                let index = lock(&self.index);
                index
                    .execute(
                        "INSERT INTO items VALUES(?1,?2,?3) ON CONFLICT(id) DO NOTHING",
                        params![id, space.to_string(), kind],
                    )
                    .map_err(internal)?;
                read(&index)
                    .map_err(internal)?
                    .ok_or_else(|| internal("route missing"))?
            }
        };
        if existing != (space.to_string(), kind.to_owned()) {
            return Err(internal("global item collision"));
        }
        Ok(())
    }
    /// Publish durable outbox intents in order, retaining failed work for retry.
    /// Per entry: indexed intent/kind/route/receipt reads, one FULL journal commit
    /// when new, and one FULL retirement; no content-body reread or file placement.
    pub(crate) fn publish_pending(&self, space: &mut Space) -> Result<()> {
        loop {
            let row=space.conn.query_row("SELECT account,request,operation,fingerprint,item,revision,result FROM outbox ORDER BY rowid LIMIT 1",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?))).optional().map_err(internal)?;
            let Some((account, request, op, fp, id, revision, result)) = row else {
                return Ok(());
            };
            // Item writes route their item first; control changes have no item.
            let item = match id {
                Some(id) => {
                    let kind: String = space
                        .conn
                        .query_row("SELECT kind FROM items WHERE id=?1", [&id], |r| r.get(0))
                        .map_err(internal)?;
                    self.route(space.id, &id, &kind)?;
                    Some(Uuid::parse_str(&id).map_err(internal)?)
                }
                None => None,
            };
            self.journals.publish(
                &space.journal,
                account.parse().map_err(internal)?,
                &request,
                &op,
                &fp,
                item,
                &revision,
                serde_json::from_str(&result).map_err(internal)?,
            )?;
            // Retire durably: otherwise a power loss could resurrect an intent
            // after its journal receipt has expired, extending request-ID reuse.
            space
                .conn
                .execute(
                    "DELETE FROM outbox WHERE account=?1 AND request=?2",
                    params![account, request],
                )
                .map_err(internal)?;
        }
    }
}
