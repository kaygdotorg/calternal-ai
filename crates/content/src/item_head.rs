//! Read-only item heads pinned to one space at broker admission.
use super::*;
use crate::types::{internal, missing};
use std::sync::Weak;

/// Metadata and current opaque version of one live item.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemHead {
    /// Item metadata stored with this head.
    pub metadata: Value,
    /// Current opaque version token.
    pub revision: String,
}

/// A space-specific view of the cached visibility pool. It retains no slot
/// between callbacks, so ordinary cache eviction and erasure can close SQLite.
pub struct ItemHeadReader {
    space: SpaceId,
    content: Arc<Content>,
    slot: Mutex<Weak<Slot>>,
}

impl Content {
    /// Find or recover the space's visibility pool during carrier wake. Retains
    /// only a weak slot reference. Closed spaces cannot allocate cache entries.
    /// Call on a blocking worker.
    pub fn item_head_reader(self: &Arc<Self>, space: SpaceId) -> Result<ItemHeadReader> {
        let slot = self.item_head_slot(space)?;
        Ok(ItemHeadReader {
            space,
            content: self.clone(),
            slot: Mutex::new(Arc::downgrade(&slot)),
        })
    }

    fn item_head_slot(&self, space: SpaceId) -> Result<Arc<Slot>> {
        let _step = self.lifecycle.step(space.as_uuid())?;
        let slot = self.pin_slot(space)?;
        if slot.visibility.get().is_none() {
            self.with_space(space, |_| Ok(()))?;
        }
        Ok(slot)
    }
}

impl ItemHeadReader {
    /// Read one live head in the pinned space. Wrong kind, foreign or absent
    /// item and Trash all give the same not-found. Cost: one primary-key query,
    /// with no item bytes, catalog lookup, storage writes or flushes when warm.
    /// Cache eviction triggers normal cold space recovery. A lifecycle permit
    /// covers the read and the release of the temporary slot reference.
    /// Call off the asynchronous executor.
    pub fn read(&self, kind: &str, id: Uuid) -> Result<ItemHead> {
        self.read_inner(kind, id, false).map(|(head, _)| head)
    }

    /// Read the current live head and its body byte length, without loading the
    /// body. The caller must authorize that exact revision before delivery.
    /// Cost: two indexed probes in one query and no writes or file I/O.
    pub fn read_with_size(&self, kind: &str, id: Uuid) -> Result<(ItemHead, u64)> {
        self.read_inner(kind, id, true).and_then(|(head, size)| {
            let size = u64::try_from(size.ok_or_else(missing)?).map_err(internal)?;
            Ok((head, size))
        })
    }

    fn read_inner(&self, kind: &str, id: Uuid, size: bool) -> Result<(ItemHead, Option<i64>)> {
        let _step = self.content.lifecycle.step(self.space.as_uuid())?;
        let cached = lock(&self.slot).upgrade();
        let slot = match cached {
            Some(slot) => slot,
            None => {
                let slot = self.content.item_head_slot(self.space)?;
                *lock(&self.slot) = Arc::downgrade(&slot);
                slot
            }
        };
        let row = slot
            .visibility
            .get()
            .ok_or_else(missing)?
            .read(|c| {
                c.prepare_cached(
                    "SELECT metadata, revision, CASE WHEN ?3 THEN
                   (SELECT length(bytes) FROM versions WHERE item=items.id AND seq=items.seq)
                 END FROM items WHERE id=?1 AND kind=?2
                 AND substr(path,1,7) <> '.trash/' AND mat_state NOT IN ('purged','purging')",
                )?
                .query_row(params![id.to_string(), kind, size], |row| {
                    Ok((row.get::<_, String>(0)?, row.get(1)?, row.get(2)?))
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
        Ok((
            ItemHead {
                metadata: serde_json::from_str(&row.0).map_err(internal)?,
                revision: row.1,
            },
            row.2,
        ))
    }
}
