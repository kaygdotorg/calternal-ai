//! Journal receipts and reads for durable plugin content effects.
use super::*;

impl Store {
    /// Records the result of a mutating plugin operation under the caller's request ID,
    /// so a retry replays it through [`Self::receipt`] instead of running the plugin
    /// again. The receipt lives in the space's journal like every other write receipt.
    /// Cost: one per-space journal append with FULL durability.
    pub fn record_plugin_receipt(
        &self,
        actor: impl Into<calternal_types::Actor>,
        request_id: &str,
        operation: &str,
        fingerprint: &str,
        space_id: SpaceId,
        result: serde_json::Value,
    ) -> Result<serde_json::Value, Error> {
        let actor = actor.into();
        let request = Request {
            account: actor.account,
            via: actor.via.as_ref(),
            id: request_id,
            operation,
            fingerprint,
        };
        self.content.admit(space_id, request, || Ok(result))
    }
    /// Reads one item of `kind` for a plugin broker. The global route must place it in
    /// `space_id`; a foreign, absent or wrong-kind ID is the same not-found.
    /// Cost: catalog route lookup plus one indexed item read and body allocation.
    pub fn read_item(&self, space_id: SpaceId, kind: &str, id: &str) -> Result<Item, Error> {
        let missing = || Error::not_found("item not found");
        let (space, id) = (space_id, item_id(id)?);
        if self.content.locate(kind, id)? != Some(space) {
            return Err(missing());
        }
        let item = if kind == "notes" {
            self.content.get_live(space, id)?
        } else {
            self.content.get(space, id)?
        };
        if item.kind != kind {
            return Err(missing());
        }
        Ok(item)
    }
    /// Creates (no `id`) or updates (against `base`) one plugin-kind item with a
    /// per-account receipt, so a retry returns the original ID and revision.
    /// The kind must already be registered in the space (plugin enable does that).
    /// Cost: one generic content write: journal and FULL SQL commits, file and
    /// directory fsync.
    #[allow(clippy::too_many_arguments)]
    pub fn write_item(
        &self,
        actor: impl Into<calternal_types::Actor>,
        request_id: &str,
        fingerprint: &str,
        space_id: SpaceId,
        kind: &str,
        id: Option<Uuid>,
        base: Option<&str>,
        metadata: serde_json::Value,
        text: &str,
    ) -> Result<serde_json::Value, Error> {
        let actor = actor.into();
        if let Some(id) = id {
            self.read_item(space_id, kind, &id.to_string())?;
        }
        let request = Request {
            account: actor.account,
            via: actor.via.as_ref(),
            id: request_id,
            operation: "plugin.write_item",
            fingerprint,
        };
        let encode = |i: &Item| serde_json::json!({"id": i.id, "revision": i.revision});
        match self.content.write(
            space_id,
            id,
            kind,
            base,
            metadata,
            text.as_bytes(),
            request,
            encode,
        ) {
            Ok(item) => Ok(encode(&item)),
            Err(error) => {
                let Some(saved) = self.receipt(actor.account, space_id, request_id)? else {
                    return Err(error);
                };
                if saved.operation != request.operation || saved.fingerprint != fingerprint {
                    return Err(Error::request_reused());
                }
                Ok(saved.result)
            }
        }
    }
}
