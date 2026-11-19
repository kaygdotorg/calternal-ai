//! Item authorization is checked at join and writes. A View proof can serve
//! later frames only while the authorizer still publishes the same clean revision.
use super::*;
use crate::resolve::{authorize_item_access, item_access_revision};
use calternal_core::SharePermission;
use calternal_types::ErrorCode;
use std::{
    future::Future,
    sync::atomic::{AtomicU64, Ordering},
};
use tokio::time::Instant;

async fn authorize<D: Directory>(
    host: &Shared<D>,
    key: &Key,
    account: AccountId,
    permission: SharePermission,
) -> Result<(), Error> {
    authorize_item_access(host, key.space, &key.kind, key.item, account, permission).await
}

const UNCACHED: u64 = u64::MAX;
const RETRY: Duration = Duration::from_millis(1);
const WAIT: Duration = Duration::from_secs(2);
const DIRTY_ACTIVATION: u64 = (1 << 16) - 1;

pub(super) struct Proof {
    view: AtomicU64,
    activation: AtomicU64,
}

impl Proof {
    fn new() -> Self {
        Self {
            view: AtomicU64::new(UNCACHED),
            activation: AtomicU64::new(UNCACHED),
        }
    }

    pub(super) fn refresh_needed(&self, item: Option<u64>, activation: u64) -> bool {
        (item.is_some() && !self.has_view(item))
            || activation & DIRTY_ACTIVATION != 0
            || self.activation.load(Ordering::Relaxed) != activation
    }

    fn has_view(&self, epoch: Option<u64>) -> bool {
        epoch.is_some_and(|epoch| epoch & 1 == 0 && self.view.load(Ordering::Relaxed) == epoch)
    }
}

fn active_plugin<D: Directory>(host: &Shared<D>, key: &Key) -> Option<Arc<Plugin>> {
    host.registry
        .snapshot()
        .plugins()
        .iter()
        .find(|plugin| {
            plugin.available_in(key.space)
                && plugin.unavailable.is_none()
                && plugin
                    .manifest
                    .kinds
                    .iter()
                    .any(|kind| kind.id == key.kind && kind.live_editing())
        })
        .cloned()
}

impl<D: Directory> Connection<D> {
    pub(super) fn catalog(&self, key: &Key) -> Option<Arc<Plugin>> {
        active_plugin(&self.host, key)
    }

    pub(super) async fn admit(
        &mut self,
        key: &Key,
        plugin: &Arc<Plugin>,
    ) -> Result<(Arc<Proof>, u32), Error> {
        let hub = self.host.store.live_rooms();
        if hub.item_authority_epoch().is_none() {
            hub.retain_item_revision(item_access_revision(
                &self.host,
                key.space,
                &key.kind,
                key.item,
                self.account,
            )?)?;
        }
        let proof = Arc::new(Proof::new());
        let deadline = Instant::now() + WAIT;
        loop {
            let epoch = self.host.store.live_rooms().item_authority_epoch();
            let act = self.host.store.live_rooms().activation_epoch(key.space);
            if epoch.is_none_or(|epoch| epoch & 1 == 0) && act & DIRTY_ACTIVATION == 0 {
                if !self.session.is_live(&self.host.directory).await {
                    return Err(Error::unauthorized("room session ended"));
                }
                authorize(&self.host, key, self.account, SharePermission::View).await?;
                let (host, opening, pinned) = (self.host.clone(), key.clone(), plugin.clone());
                let generation = disk(move || {
                    let row = activation(&host.store, opening.space, &pinned)?;
                    if !row.enabled {
                        return Err(hidden());
                    }
                    host.store
                        .content()
                        .enable_live_editing(opening.space, &opening.kind)
                        .map_err(|error| {
                            if error.code == ErrorCode::InvalidInput {
                                hidden()
                            } else {
                                error
                            }
                        })?;
                    Ok(row.generation)
                })
                .await?;
                if self.host.store.live_rooms().item_authority_epoch() == epoch
                    && self.host.store.live_rooms().activation_epoch(key.space) == act
                    && active_plugin(&self.host, key)
                        .is_some_and(|active| Arc::ptr_eq(&active, plugin))
                {
                    proof
                        .view
                        .store(epoch.unwrap_or(UNCACHED), Ordering::Relaxed);
                    proof.activation.store(act, Ordering::Relaxed);
                    return Ok((proof, generation));
                }
            }
            wait_for_revision(deadline).await?;
        }
    }

    pub(super) fn authorize_member(
        &self,
        member: &Member,
        write: bool,
    ) -> impl Future<Output = Result<(), Error>> + use<D> {
        let (host, session, account) = (self.host.clone(), self.session.clone(), self.account);
        let (key, plugin, generation, proof, termination) = (
            member.key.clone(),
            member.plugin.clone(),
            member.generation,
            member.proof.clone(),
            member.termination.clone(),
        );
        async move {
            let deadline = Instant::now() + WAIT;
            loop {
                if let Some(end) = *termination.borrow() {
                    return Err(end.error());
                }
                let item_epoch = host.store.live_rooms().item_authority_epoch();
                let plugin_epoch = host.store.live_rooms().activation_epoch(key.space);
                if item_epoch.is_none_or(|epoch| epoch & 1 == 0)
                    && plugin_epoch & DIRTY_ACTIVATION == 0
                {
                    if !session.is_live(&host.directory).await {
                        return Err(Error::unauthorized("room session ended"));
                    }
                    let access = async {
                        if !proof.has_view(item_epoch) {
                            authorize(&host, &key, account, SharePermission::View)
                                .await
                                .map_err(|error| match error.code {
                                    ErrorCode::Forbidden | ErrorCode::NotFound => revoked(error),
                                    _ => error,
                                })?;
                        }
                        if write {
                            authorize(&host, &key, account, SharePermission::Edit)
                                .await
                                .map_err(|error| match error.code {
                                    ErrorCode::NotFound => revoked(error),
                                    _ => error,
                                })?;
                        }
                        Ok(())
                    }
                    .await;
                    if let Err(error) = access {
                        if host.store.live_rooms().item_authority_epoch() != item_epoch {
                            wait_for_revision(deadline).await?;
                            continue;
                        }
                        return Err(error);
                    }
                    if proof.activation.load(Ordering::Relaxed) != plugin_epoch {
                        let (host, space, plugin) = (host.clone(), key.space, plugin.clone());
                        disk(move || {
                            let row = activation(&host.store, space, &plugin)?;
                            if row.enabled && row.generation == generation {
                                Ok(())
                            } else {
                                Err(hidden())
                            }
                        })
                        .await?;
                    }
                    if let Some(end) = *termination.borrow() {
                        return Err(end.error());
                    }
                    if host
                        .registry
                        .snapshot()
                        .get(&plugin.manifest.id)
                        .is_none_or(|current| !Arc::ptr_eq(current, &plugin))
                    {
                        return Err(hidden());
                    }
                    if host.store.live_rooms().item_authority_epoch() == item_epoch
                        && host.store.live_rooms().activation_epoch(key.space) == plugin_epoch
                    {
                        proof
                            .view
                            .store(item_epoch.unwrap_or(UNCACHED), Ordering::Relaxed);
                        proof.activation.store(plugin_epoch, Ordering::Relaxed);
                        return Ok(());
                    }
                }
                wait_for_revision(deadline).await?;
            }
        }
    }
}

async fn wait_for_revision(deadline: Instant) -> Result<(), Error> {
    if Instant::now() >= deadline {
        return Err(
            Error::new(ErrorCode::RetryAfter, "Room authority is changing. Retry shortly.")
                .with_details(json!({"retry_after_ms":250})),
        );
    }
    tokio::time::sleep(RETRY).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn authority_wait_expires_with_a_retry_delay() {
        let error = wait_for_revision(Instant::now()).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::RetryAfter);
        assert_eq!(error.details.unwrap()["retry_after_ms"], 250);
    }

    #[test]
    fn cached_views_require_the_same_clean_revision() {
        let revision = Arc::new(AtomicU64::new(0));
        let hub = Hub::default();
        hub.retain_item_revision(revision.clone()).unwrap();
        let proof = Proof::new();
        let other_membership = Proof::new();
        assert!(!proof.has_view(hub.item_authority_epoch()));
        proof.view.store(0, Ordering::Relaxed);
        other_membership.view.store(0, Ordering::Relaxed);
        assert!(proof.has_view(hub.item_authority_epoch()));
        assert!(other_membership.has_view(hub.item_authority_epoch()));
        revision.store(1, Ordering::Release);
        assert!(!proof.has_view(hub.item_authority_epoch()));
        assert!(!other_membership.has_view(hub.item_authority_epoch()));
        revision.store(2, Ordering::Release);
        assert!(!proof.has_view(hub.item_authority_epoch()));
        proof.view.store(2, Ordering::Relaxed);
        assert!(proof.has_view(hub.item_authority_epoch()));
        assert!(!other_membership.has_view(hub.item_authority_epoch()));
        assert!(!Proof::new().has_view(None));
    }
}
