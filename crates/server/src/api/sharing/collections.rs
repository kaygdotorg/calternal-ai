//! Saved Notes collections with explicit same-space membership.

use super::*;
use calternal_content::{CollectionOwner, ItemOrder, Request, StoredCollection};
use calternal_core::{
    CollectionCreateInput, CollectionGetInput, CollectionGetResult, CollectionListInput,
    CollectionMember, CollectionMembersInput, CollectionPage, CollectionSummary,
};
use calternal_types::{Capability, CapabilitySet};

pub(super) fn live(store: &Store, space: SpaceId, id: Uuid) -> Result<StoredCollection, Error> {
    let saved = store.content().live_collection(space, id)?;
    if saved.collection.owner != CollectionOwner::Space(space)
        || saved.collection.item_kind != "notes"
        || saved.collection.standard_key.is_some()
    {
        return Err(missing());
    }
    Ok(saved)
}

fn membership_editor(caps: CapabilitySet) -> bool {
    caps.contains(Capability::ContentWrite) && caps.contains(Capability::SpaceDelete)
}

// SpaceDelete never comes from an item grant. When it is present, verify
// space-level write too, since an Edit grant may have supplied item write.
pub(super) async fn editable<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    space: SpaceId,
    effective: CapabilitySet,
) -> Result<bool, Error> {
    if !membership_editor(effective) {
        return Ok(false);
    }
    Ok(
        super::super::authorization::capabilities(state, account, space)
            .await?
            .is_some_and(membership_editor),
    )
}

async fn space_read<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    space: SpaceId,
) -> Result<CapabilitySet, Error> {
    super::super::authorization::capabilities(state, account, space)
        .await?
        .filter(|caps| caps.contains(Capability::SpaceRead))
        .ok_or_else(missing)
}

pub(super) async fn list<D: Directory>(
    state: &Shared<D>,
    identity: Identity,
    input: CollectionListInput,
) -> Result<Value, Error> {
    let space = input.space_id;
    space_read(state, identity.id, space).await?;
    let limit = input.limit.unwrap_or(50).clamp(1, 50) as usize;
    let cursor = input.cursor;
    let page = blocking(state, move |store| {
        store.content().item_page(
            space,
            "collections",
            ItemOrder::Updated { label: None },
            cursor.as_deref(),
            limit,
        )
    })
    .await?;
    let can_edit = membership_editor(space_read(state, identity.id, space).await?);
    let items = page
        .rows
        .into_iter()
        .filter_map(|row| {
            if !eligible("collections", &row.metadata) {
                return None;
            }
            Some(CollectionSummary {
                id: row.id,
                title: row.metadata["title"].as_str()?.to_owned(),
                revision: row.revision,
                can_edit,
            })
        })
        .collect();
    dispatch::to_value(CollectionPage {
        items,
        next_cursor: page.next,
    })
}

pub(super) async fn dispatch<D: Directory>(
    state: Shared<D>,
    identity: Identity,
    request_id: &str,
    operation: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    if operation == "collections.get" {
        let input: CollectionGetInput = dispatch::decode(input)?;
        let reference = SharedItemRef {
            kind: "collections".into(),
            id: input.id,
        };
        let space = item_route(&state, &reference).await?;
        require_read(&state, identity, space, input.id).await?;
        let (saved, members) = blocking(&state, move |store| {
            let saved = live(store, space, input.id)?;
            let content = store.content();
            let mut members = Vec::new();
            for &id in &saved.collection.members {
                match content.live_metadata(space, "notes", id) {
                    Ok(metadata) => {
                        if let Some(title) = metadata["title"].as_str() {
                            members.push(CollectionMember {
                                id,
                                title: title.to_owned(),
                            });
                        }
                    }
                    Err(error) if error.code == ErrorCode::NotFound => (),
                    Err(error) => return Err(error),
                }
            }
            Ok((saved, members))
        })
        .await?;
        let effective = require_read(&state, identity, space, input.id).await?;
        let can_edit = editable(&state, identity.id, space, effective).await?;
        return dispatch::to_value(CollectionGetResult {
            summary: CollectionSummary {
                id: saved.id,
                title: saved.collection.title,
                revision: saved.revision,
                can_edit,
            },
            members,
        });
    }

    let (fingerprint, input) = super::super::request::fingerprint(operation, input).await?;
    let (space, title, update, members) = if operation == "collections.create" {
        let input: CollectionCreateInput = dispatch::decode(input)?;
        calternal_core::validate_title(&input.title)?;
        (input.space_id, Some(input.title), None, input.members)
    } else {
        let input: CollectionMembersInput = dispatch::decode(input)?;
        let item = SharedItemRef {
            kind: "collections".into(),
            id: input.id,
        };
        let space = item_route(&state, &item).await?;
        (
            space,
            None,
            Some((input.id, input.base_revision)),
            input.members,
        )
    };
    if members.len() > 10_000 {
        return Err(Error::invalid_input(
            "a saved collection can contain up to 10000 notes",
        ));
    }
    let caps = space_read(&state, identity.id, space).await?;
    if !caps.contains(Capability::ContentWrite) || (update.is_some() && !membership_editor(caps)) {
        return Err(Error::forbidden(
            "only the space Owner can change collection membership",
        ));
    }
    let (rid, op) = (request_id.to_owned(), operation.to_owned());
    blocking(&state, move |s| {
        if let Some(replay) =
            super::super::dispatch::replay_receipt(s, identity.id, space, &rid, &op, &fingerprint)?
        {
            return Ok(replay);
        }
        let content = s.content();
        let mut seen = std::collections::HashSet::new();
        let mut valid = Vec::with_capacity(members.len());
        for note in members {
            if seen.insert(note) {
                content.live_metadata(space, "notes", note)?;
                valid.push(note);
            }
        }
        let request = Request {
            account: identity.id,
            via: None,
            id: &rid,
            operation: &op,
            fingerprint: &fingerprint,
        };
        if let Some((id, base)) = update {
            live(s, space, id)?;
            let saved = content.set_collection_members(space, id, &base, valid, request)?;
            Ok(json!({"id":id,"revision":saved.revision}))
        } else {
            content.create_notes_collection(space, title.expect("collection title"), valid, request)
        }
    })
    .await
}

async fn require_read<D: Directory>(
    state: &Shared<D>,
    identity: Identity,
    space: SpaceId,
    id: Uuid,
) -> Result<CapabilitySet, Error> {
    let (caps, _) =
        crate::resolve::item_capabilities(state, identity.id, space, "collections", id).await?;
    caps.filter(|caps| caps.contains(Capability::SpaceRead))
        .ok_or_else(missing)
}
