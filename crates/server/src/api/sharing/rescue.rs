//! Write blocked offline edits to the caller's own personal Notes space.

use super::*;
use calternal_core::ShareRescueInput;
use calternal_types::Actor;

pub(super) async fn dispatch<D: Directory>(
    state: Shared<D>,
    identity: Identity,
    request_id: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let operation = "sharing.rescue";
    let (fingerprint, input) = super::super::request::fingerprint(operation, input).await?;
    let input: ShareRescueInput = super::super::dispatch::decode(input)?;
    calternal_core::validate_title(&input.title)?;
    if input.changed_blocks.is_empty()
        || input.changed_blocks.len() > 10_000
        || input.changed_blocks.iter().all(|s| s.trim().is_empty())
    {
        return Err(Error::invalid_input(
            "include at least one locally changed block",
        ));
    }
    let markdown = input.changed_blocks.join("\n\n");
    calternal_core::validate_markdown(&markdown)?;
    let title: String = format!(
        "Your edits to ‹{}›",
        input.title.chars().take(237).collect::<String>()
    );
    let rid = request_id.to_owned();
    let directory = state.clone();
    blocking(&state, move |store| {
        let spaces = directory
            .directory
            .protected_spaces()
            .ok_or_else(|| Error::unavailable("stored space directory unavailable"))?;
        let space = spaces
            .list(identity.id)
            .map_err(auth_error)?
            .into_iter()
            .find(|space| space.personal)
            .ok_or_else(|| Error::unavailable("no personal Notes space"))?;
        if let Some(value) = super::super::dispatch::replay_receipt(
            store,
            identity.id,
            space.id,
            &rid,
            operation,
            &fingerprint,
        )? {
            return Ok(value);
        }
        let note = store.create(
            Actor::account(identity.id),
            &rid,
            operation,
            &fingerprint,
            space.id,
            &title,
            &markdown,
        )?;
        Ok(json!({"note":note}))
    })
    .await
}
