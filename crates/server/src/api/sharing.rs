//! Host-authorized sharing operations for items in the content catalogue.

use super::*;

mod collections;
mod eviction;
mod guest;
mod rescue;
use calternal_auth::sharing::{Audience, LinkEdit};
use calternal_core::{
    ShareAudience, ShareAudiencesInput, ShareGrantInput, ShareGrantTarget, ShareLinkCreateInput,
    ShareLinkPolicyInput, ShareLinkUpdateInput, ShareListInput, SharePermission, ShareSeenInput,
    SharedItemRef, SharesReceivedInput,
};
pub(super) use guest::routes;
use serde_json::json;
use uuid::Uuid;

fn missing() -> Error {
    Error::not_found("item not found")
}

fn auth_error(error: calternal_auth::Error) -> Error {
    if error.code() == calternal_auth::ErrorCode::NotFound {
        missing()
    } else {
        crate::auth::auth_error(error)
    }
}

fn eligible(kind: &str, metadata: &Value) -> bool {
    match kind {
        "notes" => true,
        "collections" => metadata["item_kind"] == "notes" && metadata["standard_key"].is_null(),
        _ => false,
    }
}

async fn item_route<D: Directory>(
    state: &Shared<D>,
    item: &SharedItemRef,
) -> Result<SpaceId, Error> {
    if !matches!(item.kind.as_str(), "notes" | "collections") {
        return Err(missing());
    }
    let (kind, id) = (item.kind.clone(), item.id);
    blocking(state, move |s| {
        s.content().locate(&kind, id)?.ok_or_else(missing)
    })
    .await
}

async fn validate_item<D: Directory>(
    state: &Shared<D>,
    space: SpaceId,
    item: &SharedItemRef,
) -> Result<(), Error> {
    let (kind, id) = (item.kind.clone(), item.id);
    blocking(state, move |s| {
        let content = s.content();
        let metadata = content.live_metadata(space, &kind, id)?;
        if !eligible(&kind, &metadata) {
            return Err(missing());
        }
        Ok(())
    })
    .await
}

async fn manage_item<D: Directory>(
    state: &Shared<D>,
    actor: Identity,
    item: &SharedItemRef,
) -> Result<SpaceId, Error> {
    let space = item_route(state, item).await?;
    let (caps, _) =
        crate::resolve::item_capabilities(state, actor.id, space, &item.kind, item.id).await?;
    let caps = caps
        .filter(|caps| caps.contains(calternal_types::Capability::SpaceRead))
        .ok_or_else(missing)?;
    if !caps.contains(calternal_types::Capability::ContentWrite)
        || !super::authorization::capabilities(state, actor.id, space)
            .await?
            .is_some_and(|caps| caps.contains(calternal_types::Capability::ContentWrite))
    {
        return Err(Error::forbidden("sharing requires a space Editor"));
    }
    Ok(space)
}

enum Mutation {
    Grant(Audience, bool),
    Revoke(Uuid),
    Link(LinkEdit),
    Policy(bool),
    Seen,
}

pub(super) async fn dispatch<D: Directory>(
    state: Shared<D>,
    actor: Identity,
    request_id: &str,
    operation: &str,
    mut input: JsonObject,
) -> Result<Value, Error> {
    if operation == "sharing.rescue" {
        return rescue::dispatch(state, actor, request_id, input).await;
    }
    if operation.starts_with("collections.") {
        return collections::dispatch(state, actor, request_id, operation, input).await;
    }
    if matches!(
        operation,
        "sharing.list" | "sharing.audiences" | "sharing.shared_with_me"
    ) {
        let (scope, input) = {
            let state = state.clone();
            let operation = operation.to_owned();
            crate::blocking::spawn(move || {
                let scope = super::cursors::CursorScope::open(
                    &state.store.cursor_key,
                    actor.id,
                    &operation,
                    &mut input,
                )?;
                Ok::<_, Error>((scope, input))
            })
            .await
            .map_err(|_| Error::internal("cursor task failed"))??
        };
        let mut value = read(&state, actor, operation, input).await?;
        let value = crate::blocking::spawn(move || {
            if let Some(cursor) = value.get_mut("next_cursor") {
                scope.seal(cursor);
            }
            value
        })
        .await
        .map_err(|_| Error::internal("cursor task failed"))?;
        return Ok(value);
    }
    if operation == "sharing.link_copy" {
        let input: ShareGrantTarget = dispatch::decode(input)?;
        let space = manage_item(&state, actor, &input.item).await?;
        validate_item(&state, space, &input.item).await?;
        let http = state
            .directory
            .http_auth()
            .ok_or_else(|| Error::unavailable("item links require stored account auth"))?;
        let url = http
            .auth
            .copy_item_link(actor.id, space, input.item.id, input.grant_id)
            .await
            .map_err(auth_error)?;
        return Ok(json!({"url":url}));
    }
    let (fingerprint, input) = super::request::fingerprint(operation, input).await?;
    let (space, item, change) = if operation == "sharing.set_link_policy" {
        let input: ShareLinkPolicyInput = dispatch::decode(input)?;
        (
            input.space_id,
            None,
            Mutation::Policy(input.editors_can_manage_links),
        )
    } else {
        let (item, change) = match operation {
            "sharing.mark_seen" => (
                dispatch::decode::<ShareSeenInput>(input)?.item,
                Mutation::Seen,
            ),
            "sharing.grant" => {
                let input: ShareGrantInput = dispatch::decode(input)?;
                let audience = match input.audience {
                    ShareAudience::Account(target) => Audience::Account(target.account_id),
                    ShareAudience::Group(target) => Audience::Group(target.group_id),
                };
                (
                    input.item,
                    Mutation::Grant(audience, matches!(input.permission, SharePermission::Edit)),
                )
            }
            "sharing.revoke" => {
                let input: ShareGrantTarget = dispatch::decode(input)?;
                (input.item, Mutation::Revoke(input.grant_id))
            }
            "sharing.link_create" => {
                let input: ShareLinkCreateInput = dispatch::decode(input)?;
                (
                    input.item,
                    Mutation::Link(LinkEdit::Create {
                        label: input.label,
                        expires_at: input.expires_at,
                    }),
                )
            }
            "sharing.link_update" => {
                let input: ShareLinkUpdateInput = dispatch::decode(input)?;
                (
                    input.item,
                    Mutation::Link(LinkEdit::Update {
                        grant: input.grant_id,
                        label: input.label,
                        expires_at: input.expires_at,
                    }),
                )
            }
            _ => return Err(Error::not_found("unknown operation")),
        };
        let space = if matches!(change, Mutation::Seen) {
            let space = item_route(&state, &item).await?;
            validate_item(&state, space, &item).await?;
            space
        } else {
            manage_item(&state, actor, &item).await?
        };
        (space, Some(item), change)
    };
    let eviction = if let (Some(item), Mutation::Revoke(grant)) = (&item, &change) {
        let authority = state
            .directory
            .protected_spaces()
            .ok_or_else(|| Error::unavailable("stored space directory unavailable"))?;
        authority
            .eviction_audience(space, item.id, *grant)
            .await
            .map_err(auth_error)?
            .map(|audience| (item.kind.clone(), item.id, audience))
    } else {
        None
    };
    let directory = state.clone();
    let rid = request_id.to_string();
    let operation = operation.to_string();
    let result = blocking(&state, move |store| {
        let spaces = directory
            .directory
            .protected_spaces()
            .ok_or_else(|| Error::unavailable("stored space directory unavailable"))?;
        let receipt = (&*rid, &*operation, &*fingerprint);
        let Some(item) = item else {
            let Mutation::Policy(enabled) = change else {
                unreachable!("space link policy")
            };
            return spaces
                .set_item_link_policy(actor.id, space, enabled, receipt)
                .map_err(auth_error);
        };
        let kind = item.kind;
        let id = item.id;
        // Revocation only removes existing authority. It must also work in Trash
        // so that restore cannot re-enable a link the issuer meant to revoke.
        if let Mutation::Revoke(grant) = change {
            return spaces
                .revoke_item(actor.id, space, id, grant, receipt)
                .map_err(auth_error);
        }
        store
            .content()
            .with_live_metadata(space, &kind, id, |metadata| {
                if !eligible(&kind, &metadata) {
                    return Err(missing());
                }
                match change {
                    Mutation::Grant(audience, edit) => spaces.grant_item(
                        actor.id,
                        space,
                        id,
                        kind.clone(),
                        audience,
                        edit,
                        receipt,
                    ),
                    Mutation::Link(edit) => directory
                        .directory
                        .http_auth()
                        .ok_or_else(|| {
                            Error::unavailable("item links require stored account auth")
                        })?
                        .auth
                        .change_item_link(actor.id, space, id, kind.clone(), edit, receipt),
                    Mutation::Seen => spaces.mark_shared_item_seen(actor.id, space, id, receipt),
                    Mutation::Policy(_) | Mutation::Revoke(_) => unreachable!("separate change"),
                }
                .map_err(auth_error)
            })
    })
    .await?;
    if let Some((kind, item, audience)) = eviction {
        eviction::after_revoke(&state, space, &kind, item, audience).await?;
    }
    Ok(result)
}

async fn read<D: Directory>(
    state: &Shared<D>,
    actor: Identity,
    operation: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let directory = state
        .directory
        .protected_spaces()
        .ok_or_else(|| Error::unavailable("stored space directory unavailable"))?;
    match operation {
        "sharing.list" => {
            let input: ShareListInput = dispatch::decode(input)?;
            let space = item_route(state, &input.item).await?;
            let (caps, _) = crate::resolve::item_capabilities(
                state,
                actor.id,
                space,
                &input.item.kind,
                input.item.id,
            )
            .await?;
            if !caps.is_some_and(|caps| caps.contains(calternal_types::Capability::SpaceRead)) {
                return Err(missing());
            }
            validate_item(state, space, &input.item).await?;
            let mut result = directory
                .item_grants_page(actor.id, space, input.item.id, input.cursor)
                .await
                .map_err(auth_error)?;
            let (caps, _) = crate::resolve::item_capabilities(
                state,
                actor.id,
                space,
                &input.item.kind,
                input.item.id,
            )
            .await?;
            let caps = caps
                .filter(|caps| caps.contains(calternal_types::Capability::SpaceRead))
                .ok_or_else(missing)?;
            result["permission"] = json!(permission(caps));
            Ok(result)
        }
        "sharing.audiences" => {
            let input: ShareAudiencesInput = dispatch::decode(input)?;
            calternal_core::validate_title(input.query.trim()).or_else(|e| {
                if input.query.trim().is_empty() {
                    Ok(())
                } else {
                    Err(e)
                }
            })?;
            let space = manage_item(state, actor, &input.item).await?;
            directory
                .item_audiences_page(actor.id, space, input.query.trim().to_owned(), input.cursor)
                .await
                .map_err(auth_error)
        }
        "sharing.shared_with_me" => {
            let input: SharesReceivedInput = dispatch::decode(input)?;
            received(state, actor, input).await
        }
        _ => Err(Error::not_found("unknown operation")),
    }
}

fn permission(caps: calternal_types::CapabilitySet) -> &'static str {
    if caps.contains(calternal_types::Capability::ContentWrite) {
        "edit"
    } else {
        "view"
    }
}

async fn received<D: Directory>(
    state: &Shared<D>,
    actor: Identity,
    input: SharesReceivedInput,
) -> Result<Value, Error> {
    if !matches!(input.kind.as_str(), "notes" | "collections") {
        return Err(Error::invalid_input("unknown shared item kind"));
    }
    let directory = state
        .directory
        .protected_spaces()
        .ok_or_else(|| Error::unavailable("stored space directory unavailable"))?;
    let candidates = directory
        .received_item_grants(actor.id, input.kind.clone(), input.cursor)
        .await
        .map_err(auth_error)?;
    let next = (candidates.len() > 50).then(|| candidates[49].0.to_string());
    let mut rows = Vec::new();
    for (id, sharer, _, unread) in candidates.into_iter().take(50) {
        let item = SharedItemRef {
            kind: input.kind.clone(),
            id,
        };
        let space = match item_route(state, &item).await {
            Ok(space) => space,
            Err(error) if error.code == ErrorCode::NotFound => continue,
            Err(error) => return Err(error),
        };
        let kind = item.kind.clone();
        let title = blocking(state, move |s| {
            match s.content().live_metadata(space, &kind, id) {
                Ok(metadata) if eligible(&kind, &metadata) => {
                    Ok(metadata["title"].as_str().map(str::to_owned))
                }
                Ok(_) => Ok(None),
                Err(e) if e.code == ErrorCode::NotFound => Ok(None),
                Err(e) => Err(e),
            }
        })
        .await?;
        let Some(title) = title else {
            continue;
        };
        let (caps, _) =
            crate::resolve::item_capabilities(state, actor.id, space, &item.kind, id).await?;
        if let Some(caps) =
            caps.filter(|caps| caps.contains(calternal_types::Capability::SpaceRead))
        {
            rows.push(json!({"item":{"kind":item.kind,"id":id},"title":title,"shared_by":sharer,"permission":permission(caps),"unread":unread}));
        }
    }
    Ok(json!({"items":rows,"next_cursor":next}))
}
