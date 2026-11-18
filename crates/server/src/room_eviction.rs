//! The collaboration service's post-commit item eviction boundary.

use calternal_types::{AccountId, SpaceId};
use futures_util::future::BoxFuture;
use uuid::Uuid;

/// An idempotent close for one account's memberships and queued deliveries.
/// Sharing calls this only after committed loss of the last read grant.
pub trait RoomEviction: Send + Sync {
    /// Close the account's rooms for this item. Implementations may await the
    /// room worker and must handle repeated requests. No auth transaction is held.
    fn evict_item_account<'a>(
        &'a self,
        space: SpaceId,
        kind: &'a str,
        item: Uuid,
        account: AccountId,
    ) -> BoxFuture<'a, ()>;
}

pub(crate) struct NoRooms;

impl RoomEviction for NoRooms {
    fn evict_item_account<'a>(
        &'a self,
        _space: SpaceId,
        _kind: &'a str,
        _item: Uuid,
        _account: AccountId,
    ) -> BoxFuture<'a, ()> {
        Box::pin(std::future::ready(()))
    }
}
