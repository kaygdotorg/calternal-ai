//! One bounded authenticated stream of replacement projections for a package mount.
use super::*;
use axum::body::Body;
use axum::extract::Query;
use axum::response::{IntoResponse, Response};
use calternal_core::{OwnedPluginSlotId, OwnedSlotUpdate, PluginSlotTarget};
use calternal_store::JournalEntry;
use calternal_types::{Capability, CapabilitySet};
use futures_util::future::select_all;
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::{broadcast, watch};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::api) struct Selector {
    space_id: String,
    plugin_id: String,
    slot_id: String,
}

struct Follow<D: Directory> {
    state: Shared<D>,
    account: AccountId,
    session: Session,
    space: SpaceId,
    target: OwnedPluginSlotId,
    _store: calternal_store::SpaceStore,
    journal: broadcast::Receiver<Arc<JournalEntry>>,
    restored: watch::Receiver<u64>,
    catalog: watch::Receiver<u64>,
    authority: Vec<watch::Receiver<u64>>,
    last_capabilities: CapabilitySet,
    last: OwnedSlotUpdate,
    first: bool,
}

fn hidden() -> Error {
    Error::not_found("plugin slot not found")
}

async fn projection<D: Directory>(
    state: &Shared<D>,
    space: SpaceId,
    target: OwnedPluginSlotId,
    caps: CapabilitySet,
) -> Result<OwnedSlotUpdate, Error> {
    let catalog = state.registry.snapshot();
    blocking(state, move |store| {
        let plugins = super::plugin_states(store, &catalog, space)?;
        let (contributions, owners) = catalog.resolve_slots(&plugins, caps);
        let active = owners.iter().any(|slot| {
            slot.plugin_id == target.plugin_id && slot.definition.id == target.id
        });
        Ok(OwnedSlotUpdate {
            space_id: space.to_string(),
            contributions: contributions.into_iter()
                .filter(|row| matches!(&row.contribution.slot, PluginSlotTarget::Owned(id) if id == &target))
                .collect(),
            target,
            active,
        })
    }).await
}

async fn capabilities<D: Directory>(
    state: &Shared<D>,
    session: &Session,
    account: AccountId,
    space: SpaceId,
) -> Result<CapabilitySet, Error> {
    if !session.is_live(&state.directory).await {
        return Err(hidden());
    }
    crate::api::authorization::capabilities(state, account, space)
        .await?
        .filter(|caps| caps.contains(Capability::SpaceRead))
        .ok_or_else(hidden)
}

/// Follow one owned slot in an authenticated space. Bounded by the space's
/// existing journal listener budget. Reuses catalogue resolution on relevant
/// wakes, and rechecks authority before each update or keepalive.
pub(in crate::api) async fn events<D: Directory>(
    State(state): State<Shared<D>>,
    Query(selector): Query<Selector>,
    req: Request,
) -> Response {
    match open(state, selector, req).await {
        Ok(response) => response,
        Err(error) => crate::api::error_response(error),
    }
}

async fn open<D: Directory>(
    state: Shared<D>,
    selector: Selector,
    req: Request,
) -> Result<Response, Error> {
    let account = req.extensions().get::<Identity>().ok_or_else(hidden)?.id;
    let session = req
        .extensions()
        .get::<Session>()
        .cloned()
        .ok_or_else(hidden)?;
    let space = SpaceId::parse(&selector.space_id).map_err(|_| hidden())?;
    if [selector.plugin_id.len(), selector.slot_id.len()]
        .into_iter()
        .any(|len| !(1..=128).contains(&len))
    {
        return Err(hidden());
    }
    let target = OwnedPluginSlotId {
        plugin_id: selector.plugin_id,
        id: selector.slot_id,
    };
    let authority = state.directory.changes();
    let caps = capabilities(&state, &session, account, space).await?;
    let catalog = state.registry.changes();
    let journal = blocking(&state, move |store| {
        store
            .content()
            .live_journal(space)
            .map_err(|_| Error::internal("journal open failed"))
    })
    .await?;
    let receiver = journal
        .subscribe_bounded(state.inputs.limits().listeners)
        .map_err(|_| Error::unavailable("space live subscriber limit reached"))?;
    let restored = journal.restored();
    let last = projection(&state, space, target.clone(), caps).await?;
    if !last.active {
        return Err(hidden());
    }
    let workers = state.blocking.clone();
    let follow = Follow {
        state,
        account,
        session,
        space,
        target,
        _store: journal,
        journal: receiver,
        restored,
        catalog,
        authority,
        last_capabilities: caps,
        last,
        first: true,
    };
    let body = Body::from_stream(workers.read_stream(futures_util::stream::unfold(
        follow,
        |mut follow| async move {
            let frame = follow.next_frame().await?;
            Some((Ok::<_, std::convert::Infallible>(frame), follow))
        },
    )));
    Ok((
        axum::http::StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "text/event-stream"),
            (axum::http::header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response())
}

impl<D: Directory> Follow<D> {
    fn encode(&self) -> Option<String> {
        Some(format!(
            "data: {}\n\n",
            serde_json::to_string(&self.last).ok()?
        ))
    }

    async fn next_frame(&mut self) -> Option<String> {
        if self.first {
            self.first = false;
            let caps = capabilities(&self.state, &self.session, self.account, self.space)
                .await
                .ok()?;
            if caps != self.last_capabilities {
                self.last = projection(&self.state, self.space, self.target.clone(), caps)
                    .await
                    .ok()?;
                self.last_capabilities = caps;
            }
            return self.encode();
        }
        loop {
            let authority = async {
                if self.authority.is_empty() {
                    std::future::pending::<()>().await;
                }
                let _ = select_all(self.authority.iter_mut().map(|c| Box::pin(c.changed()))).await;
            };
            let changed = tokio::select! {
                journal = self.journal.recv() => match journal {
                    Ok(entry) if matches!(entry.kind.as_str(), "plugins.enable" | "plugins.disable") => true,
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => true,
                    Err(broadcast::error::RecvError::Closed) => return None,
                },
                value = self.catalog.changed() => value.ok().is_some(),
                value = self.restored.changed() => value.ok().is_some(),
                _ = authority => true,
                _ = tokio::time::sleep(Duration::from_secs(1)) => false,
            };
            let caps = capabilities(&self.state, &self.session, self.account, self.space)
                .await
                .ok()?;
            if changed || caps != self.last_capabilities {
                let next = projection(&self.state, self.space, self.target.clone(), caps)
                    .await
                    .ok()?;
                self.last_capabilities = caps;
                if next != self.last {
                    self.last = next;
                    return self.encode();
                }
            } else {
                return Some(": keepalive\n\n".into());
            }
        }
    }
}
