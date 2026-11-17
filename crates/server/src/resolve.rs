//! One address resolver over the current directory and preview content store.
//!
//! The preview store has no public global-ID index API. Resolution probes the
//! caller's current space roots without listing their content; this is linear in
//! membership count and can be replaced by the storage lane's indexed lookup.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
use crate::{
    auth::{Directory, Identity},
    authz::capabilities::Capability,
    error::HostError,
    store::Store,
};
use calternal_core::ResolveResult;

/// Resolves a kind and global item UUID to its owning space and current access.
/// Wrong kind, absent item and missing read access have identical refusals.
/// The caller holds the content store lock; every probe rechecks current grants.
pub fn resolve(
    directory: &impl Directory,
    store: &Store,
    account: &Identity,
    kind: &str,
    id: &str,
) -> Result<ResolveResult, HostError> {
    resolve_for(directory, store, account, kind, id, Capability::SpaceRead)
}

/// Internal mutation lookup also permits explicit write authority without implicit read grants.
pub(crate) fn resolve_for(
    directory: &impl Directory,
    store: &Store,
    account: &Identity,
    kind: &str,
    id: &str,
    required: Capability,
) -> Result<ResolveResult, HostError> {
    let visible = |caps: crate::authz::CapabilitySet| {
        caps.contains(Capability::SpaceRead)
            || (required == Capability::ContentWrite && caps.contains(Capability::ContentWrite))
    };
    let missing = || HostError::not_found("item not found");
    if kind != "notes" {
        return Err(missing());
    }
    for space in directory.describe(account).spaces {
        let root = directory.content_space(&space.id);
        let Some(caps) = directory.capabilities(&account.id, &space.id) else {
            continue;
        };
        if !visible(caps) {
            continue;
        }
        match store.get(&root, id) {
            Ok(_) => {
                let caps = directory
                    .capabilities(&account.id, &space.id)
                    .filter(|c| visible(*c))
                    .ok_or_else(missing)?;
                return Ok(ResolveResult {
                    kind: kind.into(),
                    id: id.into(),
                    space_id: directory.wire_space(&space.id),
                    access: Capability::ALL
                        .into_iter()
                        .filter(|c| caps.contains(*c))
                        .map(|c| c.name().to_owned())
                        .collect(),
                });
            }
            Err(e) if e.code == calternal_core::ErrorCode::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Err(missing())
}
