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
