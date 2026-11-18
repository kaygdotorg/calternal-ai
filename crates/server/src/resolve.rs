//! One existence-hiding address resolver over identifier-only content routing.
//! A catalog lookup selects a space; current authority is checked before any
//! content database is opened. No list or membership-count-dependent probe runs.
use crate::{
    api::{Shared, authorization},
    auth::{Directory, Identity},
};
use calternal_core::ResolveResult;
use calternal_types::{Capability, CapabilitySet, Error};

/// Resolves a kind and global item UUID to its owning space and current access.
/// Wrong kind, absent item and missing access have identical refusals. A mutation
/// lookup (`required` = content.write) permits write authority without read.
/// Returns the caller's capabilities in the owning space so admission needs no second
/// lookup. Cost: one indexed global route, one authority check and a small access vector;
/// no content database or file I/O and no work proportional to membership count.
/// Routing runs on a blocking task; native authority awaits its existing worker
/// outside the general blocking pool, retaining the publication barrier.
pub(crate) async fn resolve_for<D: Directory>(
    state: &Shared<D>,
    account: &Identity,
    kind: &str,
    id: &str,
    required: Capability,
) -> Result<(ResolveResult, CapabilitySet), Error> {
    let visible = |caps: CapabilitySet| {
        caps.contains(Capability::SpaceRead)
            || (required == Capability::ContentWrite && caps.contains(Capability::ContentWrite))
    };
    let missing = || Error::not_found("item not found");
    let st = state.clone();
    let route_kind = kind.to_owned();
    let route_id = id.to_owned();
    let space = tokio::task::spawn_blocking(move || st.store.locate(&route_kind, &route_id))
        .await
        .map_err(|_| Error::internal("resolver task failed"))??
        .ok_or_else(missing)?;
    let caps = authorization::capabilities(state, account.id, space)
        .await?
        .filter(|c| visible(*c))
        .ok_or_else(missing)?;
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
    Ok((resolved, caps))
}
