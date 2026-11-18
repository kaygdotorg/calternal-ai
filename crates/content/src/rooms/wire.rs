//! The content boundary accepts only one plain Markdown text and closed causal state.
use super::*;
use crate::types::missing;
use yrs::{Any, Out};

/// Wire state for one room. Reopening an unchanged item yields the same
/// lineage and update, including a lazily seeded pre-live document.
pub struct RoomSnapshot {
    /// Opaque server lineage. Clients replay their local diff on a mismatch.
    pub lineage: String,
    /// Yjs v1 full state update. Clients apply it to an empty replica.
    pub update: Vec<u8>,
}

impl Content {
    /// Load one opted-in, live Markdown item. This never writes a seed to disk.
    /// First use may drain a normal content outbox. Cost: one item and
    /// checkpoint lookup, with one allocation for the saved state or seed.
    pub fn room_snapshot(&self, space: SpaceId, kind: &str, id: Uuid) -> Result<RoomSnapshot> {
        self.with_space(space, |s| {
            if !s.live_editing(kind)? {
                return Err(missing());
            }
            self.publish_pending(s)?;
            let head = s.header(id)?;
            if head.kind != kind
                || head.path.starts_with(".trash/")
                || matches!(head.state.as_str(), "purging" | "purged")
            {
                return Err(missing());
            }
            let state = match s.checkpoint(id, head.seq)? {
                Some(state) => state,
                None => Document::seed(&s.get_selected(id, true)?)?.checkpoint()?,
            };
            Ok(RoomSnapshot {
                lineage: state.lineage,
                update: state.state,
            })
        })
    }
}

pub(super) fn apply_client_update(doc: &Doc, bytes: &[u8]) -> Result<Vec<u8>> {
    let invalid = || Error::invalid_input("invalid live Markdown update");
    let incomplete = || {
        Error::conflict("live update requires a complete sync step 2")
            .with_details(serde_json::json!({"reason":"resync"}))
    };
    let update = Update::decode_v1(bytes).map_err(|_| invalid())?;
    let mut txn = doc.transact_mut();
    let have = txn.state_vector();
    if !update
        .state_vector_lower()
        .iter()
        .all(|(writer, clock)| *clock <= have.get(writer))
    {
        return Err(incomplete());
    }
    txn.apply_update(update).map_err(|_| invalid())?;
    if txn.has_missing_updates() {
        return Err(incomplete());
    }
    if txn.root_refs().any(|(name, _)| name != TEXT) {
        return Err(invalid());
    }
    let text = txn.get_text(TEXT).ok_or_else(invalid)?;
    if text
        .diff(&txn, |_| ())
        .into_iter()
        .any(|part| part.attributes.is_some() || !matches!(part.insert, Out::Any(Any::String(_))))
    {
        return Err(invalid());
    }
    Ok(txn.encode_update_v1())
}
