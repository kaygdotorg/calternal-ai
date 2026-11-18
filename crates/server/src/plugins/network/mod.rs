//! Trusted network policy and content handling for plugin invocations.

mod admission;
mod audit;
pub(crate) mod db;
mod eligibility;
pub(crate) mod frames;
pub(crate) mod http;
mod media;
mod sanitize;
#[cfg(test)]
mod tests;

use calternal_core::{ContentFrameView, ContentPrepareInput, ContentPrepareResult};
use calternal_types::{AccountId, Capability, Error, SpaceId};
use ring::hmac;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    api::Shared,
    auth::Directory,
    plugins::{Plugin, Session, activation},
    store::Store,
};

const FRAME_PREFIX: &str = "/api/v1/plugin-content";

fn missing() -> Error {
    Error::not_found("prepared content is unavailable")
}

fn frame_path(id: Uuid) -> String {
    format!("{FRAME_PREFIX}/{id}")
}

fn asset_signature(store: &Store, frame: Uuid) -> String {
    let mut mac = hmac::Context::with_key(&store.cursor_key);
    mac.update(b"plugin-content/asset/v1");
    mac.update(frame.as_bytes());
    hex::encode(mac.sign().as_ref())
}

/// Load only the package's declared live source and prepare a short-lived
/// reference. No socket I/O. Call from the admitted broker's blocking thread.
pub(super) fn prepare(
    store: &Store,
    account: AccountId,
    session: Session,
    space: SpaceId,
    plugin: &Plugin,
    activation_generation: u32,
    request: ContentPrepareInput,
) -> Result<ContentPrepareResult, Error> {
    if !plugin
        .manifest
        .network
        .as_ref()
        .and_then(|network| network.content_fetch.as_ref())
        .is_some_and(|fetch| {
            fetch
                .sources
                .iter()
                .any(|source| source.kind == request.kind && source.field == request.field)
        })
    {
        return Err(missing());
    }
    let item_id = Uuid::parse_str(&request.item_id).map_err(|_| missing())?;
    let lifecycle = store.content().lifecycle();
    let _space = lifecycle.step(space.as_uuid())?;
    let _account = lifecycle.step(account.as_uuid())?;
    let item = store.content().get_live(space, item_id)?;
    if item.kind != request.kind || item.revision != request.revision {
        return Err(missing());
    }
    let id = Uuid::new_v4();
    let assets = format!("{}/{}", frame_path(id), asset_signature(store, id));
    let html = std::str::from_utf8(&item.bytes).map_err(|_| missing())?;
    let content = sanitize::clean(html, &assets).ok_or_else(missing)?;
    store.plugin_frames.insert(
        id,
        frames::Source {
            account,
            session,
            space,
            plugin: plugin.manifest.id.clone(),
            package_revision: plugin.revision,
            activation_generation,
            kind: item.kind,
            item: item.id,
            revision: item.revision,
        },
        content,
    );
    Ok(ContentPrepareResult {
        frame_id: id.to_string(),
    })
}

fn current_frame<D: Directory>(
    state: &Shared<D>,
    id: Uuid,
    account: AccountId,
) -> Result<Arc<frames::Frame>, Error> {
    let frame = state.store.plugin_frames.get(id).ok_or_else(missing)?;
    let source = &frame.source;
    if source.account != account
        || !state
            .directory
            .capabilities(account, source.space)
            .is_some_and(|caps| {
                caps.contains(Capability::SpaceRead) && caps.contains(Capability::PluginsUse)
            })
    {
        return Err(missing());
    }
    let catalog = state.registry.snapshot();
    let plugin = catalog
        .revision(&source.plugin, source.package_revision)
        .ok_or_else(missing)?;
    let row = activation(&state.store, source.space, plugin)?;
    if !row.enabled
        || row.generation != source.activation_generation
        || state
            .store
            .content()
            .live_revision(source.space, &source.kind, source.item)?
            != source.revision
    {
        return Err(missing());
    }
    Ok(frame)
}

/// Resolve a prepared frame and its external navigation targets for the
/// authenticated owner. Costs activation and revision head reads, no body I/O.
pub(crate) fn view<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    space: SpaceId,
    id: &str,
) -> Result<ContentFrameView, Error> {
    let id = Uuid::parse_str(id).map_err(|_| missing())?;
    let frame = current_frame(state, id, account)?;
    if frame.source.space != space {
        return Err(missing());
    }
    Ok(ContentFrameView {
        frame_url: frame_path(id),
        links: frame
            .content
            .links
            .iter()
            .map(|url| url.to_string())
            .collect(),
        preference: db::preference(state.store.appdata(), space, account)?,
    })
}
