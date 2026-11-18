//! Recipient search reuses owner projections with an explicit allowed ID set.

use super::*;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub(super) type Root = (SpaceId, String, Uuid);

pub(super) fn collect_ids(
    store: &Store,
    member_spaces: &[SpaceId],
    roots: &[Root],
) -> Result<(BTreeMap<SpaceId, String>, bool), Error> {
    let mut partial = roots.len() > 500;
    let mut space_ids: BTreeMap<SpaceId, BTreeSet<String>> = BTreeMap::new();
    let mut left: usize = 10_000;
    for (space, kind, id) in roots.iter().take(500) {
        if member_spaces.contains(space) {
            continue;
        }
        if left == 0 {
            partial = true;
            break;
        }
        let ids = if kind == "notes" {
            vec![id.to_string()]
        } else {
            store
                .content()
                .search_collection_members(*space, *id, left + 1)?
        };
        partial |= ids.len() > left;
        let space_ids = space_ids.entry(*space).or_default();
        for id in ids.into_iter().take(left) {
            if space_ids.insert(id) {
                left -= 1;
            }
        }
    }
    let pages = space_ids
        .into_iter()
        .filter(|(_, ids)| !ids.is_empty())
        .map(|(space, ids)| (space, serde_json::to_string(&ids).expect("UUID strings")))
        .collect();
    Ok((pages, partial))
}

pub(super) async fn recheck<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    spaces: &[SpaceId],
    result: &mut SearchResult,
) -> Result<(), Error> {
    let mut hits = Vec::with_capacity(result.hits.len());
    for hit in std::mem::take(&mut result.hits) {
        let space = SpaceId::parse(&hit.space_id)?;
        if spaces.contains(&space) {
            hits.push(hit);
            continue;
        }
        let Some(id) = hit
            .address
            .strip_prefix("/notes/")
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            continue;
        };
        let (caps, _) =
            crate::resolve::item_capabilities(state, account, space, "notes", id).await?;
        if !caps.is_some_and(|caps| caps.contains(calternal_types::Capability::SpaceRead)) {
            continue;
        }
        let live = blocking(state, move |store| {
            match store.content().live_metadata(space, "notes", id) {
                Ok(_) => Ok(true),
                Err(error) if error.code == ErrorCode::NotFound => Ok(false),
                Err(error) => Err(error),
            }
        })
        .await?;
        if live {
            hits.push(hit);
        }
    }
    result.hits = hits;
    Ok(())
}
