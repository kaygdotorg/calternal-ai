//! Trusted network policy and content handling for plugin invocations.

mod admission;
mod audit;
mod cache;
pub(crate) mod db;
mod eligibility;
pub(crate) mod fetch;
pub(crate) mod frames;
pub(crate) mod http;
mod media;
mod quota;
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

fn asset_signature(key: &hmac::Key, frame: Uuid, index: u64, expiry: i64) -> String {
    let mut mac = hmac::Context::with_key(key);
    mac.update(b"plugin-content/asset/v2");
    mac.update(frame.as_bytes());
    mac.update(&index.to_be_bytes());
    mac.update(&expiry.to_be_bytes());
    hex::encode(mac.sign().as_ref())
}

fn asset_authorized(key: &hmac::Key, frame: Uuid, index: u64, expiry: i64, signature: &str, now: i64) -> bool {
    use subtle::ConstantTimeEq;
    expiry > now && bool::from(asset_signature(key, frame, index, expiry).as_bytes().ct_eq(signature.as_bytes()))
}

fn asset_now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

fn declares(plugin: &Plugin, kind: &str, field: calternal_core::ContentFetchField) -> bool {
    plugin
        .manifest
        .network
        .as_ref()
        .and_then(|network| network.content_fetch.as_ref())
        .is_some_and(|fetch| {
            fetch
                .sources
                .iter()
                .any(|source| source.kind == kind && source.field == field)
        })
}

fn source_age(item: &calternal_content::Item) -> eligibility::SourceAge {
    use eligibility::SourceAge;
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};
    if item
        .metadata
        .get("junk")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        return SourceAge::Junk;
    }
    match OffsetDateTime::parse(&item.updated_at, &Rfc3339) {
        Ok(at) if OffsetDateTime::now_utc() - at <= time::Duration::days(90) => {
            SourceAge::WithinWindow
        }
        Ok(_) => SourceAge::Older,
        Err(_) => SourceAge::Junk,
    }
}

/// Queue media only from a newly committed, declared text source.
/// The background task rereads the revision and its current authorizations.
pub(super) fn ingest<D: Directory>(
    runtime: &tokio::runtime::Handle,
    state: Shared<D>,
    account: AccountId,
    session: Session,
    space: SpaceId,
    plugin: &Plugin,
    generation: u32,
    kind: &str,
    item: calternal_content::Item,
) {
    if !declares(plugin, kind, calternal_core::ContentFetchField::Text) {
        return;
    }
    let age = source_age(&item);
    if !db::preference(state.store.appdata(), space, account).is_ok_and(|preference| {
        eligibility::may_enqueue(preference, age, eligibility::Trigger::Ingest)
    }) {
        return;
    }
    let Ok(html) = std::str::from_utf8(&item.bytes) else {
        return;
    };
    let Some(content) = sanitize::clean(html, &|_| String::new()) else {
        return;
    };
    let source = Arc::new(frames::Source {
        account,
        session,
        space,
        plugin: plugin.manifest.id.clone(),
        package_revision: plugin.revision,
        activation_generation: generation,
        kind: item.kind,
        item: item.id,
        revision: item.revision,
        age,
    });
    fetch::enqueue(
        runtime,
        &state,
        source,
        &content.assets,
        eligibility::Trigger::Ingest,
    );
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
    if !declares(plugin, &request.kind, request.field) {
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
    let expiry = asset_now() + frames::LIFETIME.as_secs() as i64;
    let html = std::str::from_utf8(&item.bytes).map_err(|_| missing())?;
    let content = sanitize::clean(html, &|index| {
        let signature = asset_signature(&store.cursor_key, id, index as u64, expiry);
        format!("{}/{index}/{expiry}/{signature}", frame_path(id))
    }).ok_or_else(missing)?;
    store.plugin_frames.insert(
        id,
        frames::Source {
            account,
            session,
            space,
            plugin: plugin.manifest.id.clone(),
            package_revision: plugin.revision,
            activation_generation,
            age: source_age(&item),
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
    if frame.source.account != account {
        return Err(missing());
    }
    check_source(state, &frame.source)?;
    Ok(frame)
}

fn check_source<D: Directory>(state: &Shared<D>, source: &frames::Source) -> Result<(), Error> {
    if !state
        .directory
        .capabilities(source.account, source.space)
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
    Ok(())
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
