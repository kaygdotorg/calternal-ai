//! One existence-hiding address resolver over identifier-only content routing.
//! A catalog lookup selects a space; current authority is checked before any
//! content database is opened. No list or membership-count-dependent probe runs.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
use crate::{
    auth::{Directory, Identity},
    store::Store,
};
use calternal_core::ResolveResult;
use calternal_types::{Capability, CapabilitySet, Error};

/// Resolves a kind and global item UUID to its owning space and current access.
/// Wrong kind, absent item and missing read access have identical refusals.
/// The caller holds the host store lock. Cost: one indexed global route lookup,
/// two current capability checks, and one bounded capability-vector allocation;
/// no content reads, membership scans or fsyncs.
pub fn resolve(
    directory: &impl Directory,
    store: &Store,
    account: &Identity,
    kind: &str,
    id: &str,
) -> Result<ResolveResult, Error> {
    resolve_for(directory, store, account, kind, id, Capability::SpaceRead)
}

/// Internal mutation lookup permits explicit write authority without implicit read grants.
/// Cost: one indexed global route, two authority checks and a small access vector;
/// no content database or file I/O and no work proportional to membership count.
pub(crate) fn resolve_for(
    directory: &impl Directory,
    store: &Store,
    account: &Identity,
    kind: &str,
    id: &str,
    required: Capability,
) -> Result<ResolveResult, Error> {
    let visible = |caps: CapabilitySet| {
        caps.contains(Capability::SpaceRead)
            || (required == Capability::ContentWrite && caps.contains(Capability::ContentWrite))
    };
    let missing = || Error::not_found("item not found");
    let root = store.locate(kind, id)?.ok_or_else(missing)?;
    let public_space = directory.wire_space(&root);
    directory
        .capabilities(&account.id, &public_space)
        .filter(|c| visible(*c))
        .ok_or_else(missing)?;
    let caps = directory
        .capabilities(&account.id, &public_space)
        .filter(|c| visible(*c))
        .ok_or_else(missing)?;
    Ok(ResolveResult {
        kind: kind.into(),
        id: id.into(),
        space_id: public_space,
        access: Capability::ALL
            .into_iter()
            .filter(|c| caps.contains(*c))
            .map(|c| c.name().to_owned())
            .collect(),
    })
}
