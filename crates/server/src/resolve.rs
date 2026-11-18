//! One existence-hiding address resolver over identifier-only content routing.
//! The catalogue selects a space and the protected directory checks current authority.
//! Note grants can inherit through an indexed reverse collection lookup.
use crate::{
    api::{Shared, authorization},
    auth::{Directory, Identity},
};
use calternal_core::ResolveResult;
use calternal_types::{AccountId, Capability, CapabilitySet, Error, SpaceId};
use uuid::Uuid;

/// Resolves a kind and global item UUID to its owning space and current access.
/// Wrong kind, absent item and missing access have identical refusals. A mutation
/// lookup (`required` = content.write) permits write authority without read.
/// Returns scoped capabilities and whether access is item-only so admission
/// can reuse both. Cost: one indexed global route and one protected authority
/// check, plus one indexed note-membership lookup for share-enabled notes.
/// Routing runs on a blocking task; native authority awaits its existing worker
/// outside the general blocking pool, retaining the publication barrier.
pub(crate) async fn resolve_for<D: Directory>(
    state: &Shared<D>,
    account: &Identity,
    kind: &str,
    id: &str,
    required: Capability,
    allow_item_grants: bool,
) -> Result<(ResolveResult, CapabilitySet, bool), Error> {
    let visible = |caps: CapabilitySet| {
        caps.contains(Capability::SpaceRead)
            || (required == Capability::ContentWrite && caps.contains(Capability::ContentWrite))
    };
    let missing = || Error::not_found("item not found");
    let st = state.clone();
    let route_kind = kind.to_owned();
    let route_id = id.to_owned();
    let space = crate::blocking::spawn(move || st.store.locate(&route_kind, &route_id))
        .await
        .map_err(|_| Error::internal("resolver task failed"))??
        .ok_or_else(missing)?;
    let (caps, item_only) = if allow_item_grants && matches!(kind, "notes" | "collections") {
        let id = Uuid::parse_str(id).map_err(|_| missing())?;
        item_capabilities(state, account.id, space, kind, id).await?
    } else {
        (
            authorization::capabilities(state, account.id, space).await?,
            false,
        )
    };
    let caps = caps.filter(|c| visible(*c)).ok_or_else(missing)?;
    if item_only && kind == "collections" {
        let st = state.clone();
        let id = Uuid::parse_str(id).map_err(|_| missing())?;
        if crate::blocking::spawn(move || st.store.content().item_state(space, id))
            .await
            .map_err(|_| Error::internal("resolver task failed"))??
            != "active"
        {
            return Err(missing());
        }
    }
    let resolved = ResolveResult {
        kind: kind.into(),
        id: id.into(),
        space_id: space.to_string(),
        state: None,
        access: Capability::ALL
            .into_iter()
            .filter(|c| caps.contains(*c))
            .map(|c| c.name().to_owned())
            .collect(),
    };
    Ok((resolved, caps, item_only))
}

/// Item grants apply only to host operations whose effects remain on the item.
pub(crate) fn allows_item_grants(operation: &str) -> bool {
    matches!(operation, "resolve" | "notes.get" | "notes.update")
}

/// Derive item-scoped capabilities for this admission or delivery and report
/// whether the caller lacks space read. One current auth statement when stored.
pub(crate) async fn item_capabilities<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    space: SpaceId,
    kind: &str,
    id: Uuid,
) -> Result<(Option<CapabilitySet>, bool), Error> {
    let Some(directory) = state.directory.protected_spaces() else {
        return Ok((
            authorization::capabilities(state, account, space).await?,
            false,
        ));
    };
    let collections = if kind == "notes" {
        let state = state.clone();
        crate::blocking::spawn(move || state.store.content().sharing_collections(space, id))
            .await
            .map_err(|_| Error::internal("collection membership task failed"))??
    } else {
        Vec::new()
    };
    let (caps, permission) = directory
        .item_authority(account, space, id, collections)
        .await
        .map_err(crate::auth::auth_error)?;
    if permission == 0 {
        return Ok((caps, false));
    }
    let current = caps.unwrap_or_else(CapabilitySet::empty);
    let item_only = !current.contains(Capability::SpaceRead);
    let mut scoped = current.with(Capability::SpaceRead);
    if permission == 2 {
        scoped = scoped.with(Capability::ContentWrite);
    }
    Ok((Some(scoped), item_only))
}

/// Authorize one live item for a room join, write or delivery refresh. The
/// caller authenticates the account and supplies the known space and kind.
/// Missing, trashed and invisible items share one refusal.
///
/// A stored Notes check makes one live-head lookup, one indexed reverse
/// collection lookup and one protected auth statement. It does not read the
/// global route, load a body or flush. The space lock holds lifecycle and
/// membership stable through the worker's authority check.
pub async fn authorize_item_access<D: Directory>(
    state: &Shared<D>,
    space: SpaceId,
    kind: &str,
    item: Uuid,
    account: AccountId,
    permission: calternal_core::SharePermission,
) -> Result<(), Error> {
    let state = state.clone();
    let kind = kind.to_owned();
    crate::blocking::spawn(move || {
        state
            .store
            .content()
            .with_live_share_scope(space, &kind, item, |collections| {
                let (caps, granted) = if let Some(directory) = state.directory.protected_spaces() {
                    directory
                        .item_authority_blocking(account, space, item, collections)
                        .map_err(crate::auth::auth_error)?
                } else {
                    (state.directory.capabilities(account, space), 0)
                };
                let can_view =
                    granted != 0 || caps.is_some_and(|caps| caps.contains(Capability::SpaceRead));
                let can_edit = granted == 2
                    || caps.is_some_and(|caps| caps.contains(Capability::ContentWrite));
                match permission {
                    calternal_core::SharePermission::View if can_view => Ok(()),
                    calternal_core::SharePermission::Edit if can_edit => Ok(()),
                    calternal_core::SharePermission::Edit if can_view => {
                        Err(Error::forbidden("item is read-only"))
                    }
                    _ => Err(Error::not_found("item not found")),
                }
            })
    })
    .await
    .map_err(|_| Error::internal("item authorization task failed"))?
}

/// Get the room cache invalidator for an item. All items share one process-
/// local atomic, so changes to group or recovery state need no room scan.
/// Getting or sampling the handle costs zero database reads. The handle
/// authorizes nothing. Check its unchanged even value around full authority
/// checks, using Acquire, and pause delivery while it is dirty or changed.
pub fn item_access_revision<D: Directory>(
    _state: &Shared<D>,
    _space: SpaceId,
    _kind: &str,
    _item: Uuid,
    _account: AccountId,
) -> Result<std::sync::Arc<std::sync::atomic::AtomicU64>, Error> {
    Ok(calternal_types::item_access_revision_handle())
}
