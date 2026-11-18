//! Identifier routing and idempotent outbox publication after a content commit.
use crate::{Content, Result, db::Space, instructions, journal::lock, types::internal};
use calternal_types::{SpaceId, Via};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use uuid::Uuid;
pub(crate) const OUTBOX_VIA: &str = "_calternal_outbox_via";

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
    /// Publish ordered intents, finishing linked placements and purge intents
    /// first. Cost: indexed intent/kind/route/receipt reads and one FULL journal
    /// commit plus FULL retirement per new event; failures retain replayable work.
    pub(crate) fn publish_pending(&self, space: &mut Space) -> Result<()> {
        loop {
            let row=space.conn.query_row(
                "SELECT o.account,o.request,o.operation,o.fingerprint,o.item,o.revision,o.result,i.kind,i.mat_state,
                    EXISTS(SELECT 1 FROM outbox_placement p WHERE p.account=o.account AND p.request=o.request)
                 FROM outbox o LEFT JOIN items i ON i.id=o.item ORDER BY o.rowid LIMIT 1",
                [], |r| Ok((
                    r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,
                    r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,String>(5)?,
                    r.get::<_,String>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,Option<String>>(8)?,r.get::<_,bool>(9)?,
                )),
            ).optional().map_err(internal)?;
            let Some((account, request, op, fp, id, revision, result, kind, state, place_related)) =
                row
            else {
                return Ok(());
            };
            if place_related {
                space.place_related(&account, &request, self.fault)?;
            }
            let item = if let Some(id) = id {
                if state.as_deref() == Some("purging")
                    && !space.finish_purge(&id, &account, &request, self.fault)?
                {
                    continue;
                }
                self.route(
                    space.id,
                    &id,
                    &kind.ok_or_else(|| internal("outbox item missing"))?,
                )?;
                Some(Uuid::parse_str(&id).map_err(internal)?)
            } else {
                None
            };
            let mut payload: Value = serde_json::from_str(&result).map_err(internal)?;
            let via: Option<Via> =
                if let Some(marker) = payload.as_object_mut().and_then(|o| o.remove(OUTBOX_VIA)) {
                    let via = serde_json::from_value(marker).map_err(internal)?;
                    payload = payload
                        .as_object_mut()
                        .and_then(|o| o.remove("payload"))
                        .ok_or_else(|| internal("attributed outbox lacks result"))?;
                    via
                } else {
                    None
                };
            let payload = if revision == instructions::INTENT {
                self.place_instructions(space.id, &payload)?;
                Value::Null
            } else {
                payload
            };
            self.journals.publish(
                &space.journal,
                account.parse().map_err(internal)?,
                via.as_ref(),
                &request,
                &op,
                &fp,
                item,
                &revision,
                payload,
            )?;
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
