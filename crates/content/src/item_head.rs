//! Read-only item heads pinned to one space at broker admission.
use super::*;
use crate::types::{internal, missing};

/// Metadata and current opaque version of one live item.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemHead {
    /// Item metadata stored with this head.
    pub metadata: Value,
    /// Current opaque version token.
    pub revision: String,
}

/// Read-only connection pool for the selected space. It retains no installation
/// catalog or shared writer. The caller must fence authority before each use
/// and again before releasing a result.
pub struct ItemHeadReader {
    space: SpaceId,
    slot: Arc<Slot>,
    lifecycle: calternal_types::OwnerLifecycle,
}

impl Content {
    /// Pin the space's existing visibility pool during carrier wake. Takes one
    /// cache lookup. If cold, recovers the space on this blocking worker before
    /// the normal content path publishes the pool.
    pub fn item_head_reader(&self, space: SpaceId) -> Result<ItemHeadReader> {
        let slot = self.pin_slot(space)?;
        if slot.visibility.get().is_none() {
            self.with_space(space, |_| Ok(()))?;
        }
        Ok(ItemHeadReader {
            space,
            slot,
            lifecycle: self.lifecycle.clone(),
        })
    }
}

impl ItemHeadReader {
    /// Read one live head in the pinned space. Wrong kind, foreign or absent
    /// item and Trash all give the same not-found. Cost: one primary-key query,
    /// with no item bytes, catalog lookup, storage writes or flushes.
    /// Call off the asynchronous executor.
    pub fn read(&self, kind: &str, id: Uuid) -> Result<ItemHead> {
        let _step = self.lifecycle.step(self.space.as_uuid())?;
        let row = self
            .slot
            .visibility
            .get()
            .ok_or_else(missing)?
            .read(|c| {
                c.prepare_cached(
                    "SELECT metadata, revision FROM items WHERE id=?1 AND kind=?2
                 AND substr(path,1,7) <> '.trash/' AND mat_state NOT IN ('purged','purging')",
                )?
                .query_row(params![id.to_string(), kind], |row| {
                    Ok((row.get::<_, String>(0)?, row.get(1)?))
                })
                .optional()
            })
            .map_err(|error| {
                if error.sqlite_error_code() == Some(rusqlite::ErrorCode::CannotOpen) {
                    missing()
                } else {
                    internal(error)
                }
            })?
            .ok_or_else(missing)?;
        Ok(ItemHead {
            metadata: serde_json::from_str(&row.0).map_err(internal)?,
            revision: row.1,
        })
    }
}
