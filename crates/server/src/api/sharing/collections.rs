//! Saved Notes collections with explicit same-space membership.

use super::*;
use calternal_content::Request;
use calternal_core::{CollectionCreateInput, CollectionGetInput, CollectionMembersInput};
use calternal_types::Capability;

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
        let result = blocking(&state, move |store| {
            let content = store.content();
            let collection = content.live_metadata(space, "collections", input.id)?;
            if !eligible("collections", &collection) {
                return Err(missing());
            }
            let revision = content.collection(space, input.id)?.revision;
            let members = collection["members"].as_array().ok_or_else(missing)?;
            let notes = members.iter().filter_map(|id| Uuid::parse_str(id.as_str()?).ok());
            let mut members = Vec::new();
            for id in notes {
                match content.live_metadata(space, "notes", id) {
                    Ok(metadata) => members.push(json!({"id":id,"title":metadata["title"]})),
                    Err(error) if error.code == ErrorCode::NotFound => (),
                    Err(error) => return Err(error),
                }
            }
            Ok(json!({"id":input.id,"title":collection["title"],"revision":revision,"members":members}))
        }).await?;
        require_read(&state, identity, space, input.id).await?;
        return Ok(result);
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
    let caps = super::super::authorization::capabilities(&state, identity.id, space)
        .await?
        .filter(|caps| caps.contains(Capability::SpaceRead))
        .ok_or_else(missing)?;
    if !caps.contains(Capability::ContentWrite)
        || (update.is_some() && !caps.contains(Capability::SpaceDelete))
    {
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
            let metadata = content.live_metadata(space, "collections", id)?;
            if !eligible("collections", &metadata) {
                return Err(missing());
            }
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
) -> Result<(), Error> {
    let (caps, _) =
        crate::resolve::item_capabilities(state, identity.id, space, "collections", id).await?;
    if !caps.is_some_and(|caps| caps.contains(Capability::SpaceRead)) {
        return Err(missing());
    }
    Ok(())
}
