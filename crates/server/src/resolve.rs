//! One existence-hiding address resolver over identifier-only content routing.
//! A catalog lookup selects a space; current authority is checked before any
//! content database is opened. No list or membership-count-dependent probe runs.
use crate::{
    auth::{Directory, Identity},
    store::Store,
};
use calternal_core::ResolveResult;
use calternal_types::{Capability, CapabilitySet, Error};

/// Resolves a kind and global item UUID to its owning space and current access.
/// Wrong kind, absent item and missing read access have identical refusals.
/// Takes no lock beyond the catalog read pool. Cost: one indexed global route lookup,
/// one current capability check, and one bounded capability-vector allocation;
/// no content reads, membership scans or fsyncs.
pub fn resolve(
    directory: &impl Directory,
    store: &Store,
    account: &Identity,
    kind: &str,
    id: &str,
) -> Result<ResolveResult, Error> {
    resolve_for(directory, store, account, kind, id, Capability::SpaceRead).map(|(r, _)| r)
}

/// Internal mutation lookup permits explicit write authority without implicit read grants.
/// Returns the caller's capabilities in the owning space so admission needs no second
/// lookup. Cost: one indexed global route, one authority check and a small access vector;
/// no content database or file I/O and no work proportional to membership count.
pub(crate) fn resolve_for(
    directory: &impl Directory,
    store: &Store,
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
    let space = store.locate(kind, id)?.ok_or_else(missing)?;
    let caps = directory
        .capabilities(account.id, space)
        .filter(|c| visible(*c))
        .ok_or_else(missing)?;
    let resolved = ResolveResult {
        kind: kind.into(),
        id: id.into(),
        space_id: space.to_string(),
        access: Capability::ALL
            .into_iter()
            .filter(|c| caps.contains(*c))
            .map(|c| c.name().to_owned())
            .collect(),
    };
    Ok((resolved, caps))
}
