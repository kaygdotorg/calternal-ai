//! Item authorization is checked at join and writes. A View proof can serve
//! later frames only while the authorizer still publishes the same clean revision.
use super::*;
use crate::api::authorization;
use calternal_types::Capability;
use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
};
use tokio::time::Instant;

type Check = Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>;

trait ItemAuthority<D: Directory> {
    fn revision(
        host: &Shared<D>,
        key: &Key,
        account: AccountId,
    ) -> Result<Option<Arc<AtomicU64>>, Error>;
    fn view(host: Shared<D>, key: Key, account: AccountId) -> Check;
    fn edit(host: Shared<D>, key: Key, account: AccountId) -> Check;
}

// Existing space grants keep the protocol usable on this branch. This adapter
// deliberately provides no revision proof. The sharing adapter will use the
// resolver's item access check and process-wide revision for both methods.
struct CurrentAuthority;

impl<D: Directory> ItemAuthority<D> for CurrentAuthority {
    fn revision(
        _host: &Shared<D>,
        _key: &Key,
        _account: AccountId,
    ) -> Result<Option<Arc<AtomicU64>>, Error> {
        Ok(None)
    }

    fn view(host: Shared<D>, key: Key, account: AccountId) -> Check {
        space_capability(host, key, account, Capability::SpaceRead)
    }

    fn edit(host: Shared<D>, key: Key, account: AccountId) -> Check {
        space_capability(host, key, account, Capability::ContentWrite)
    }
}

fn space_capability<D: Directory>(
    host: Shared<D>,
    key: Key,
    account: AccountId,
    required: Capability,
) -> Check {
    Box::pin(async move {
        if authorization::capabilities(&host, account, key.space)
            .await?
            .is_some_and(|caps| caps.contains(Capability::SpaceRead) && caps.contains(required))
        {
            Ok(())
        } else if required == Capability::SpaceRead {
            Err(hidden())
        } else {
            Err(Error::forbidden("room is read-only"))
        }
    })
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
        if hub.item_authority_epoch().is_none()
            && let Some(revision) =
                <CurrentAuthority as ItemAuthority<D>>::revision(&self.host, key, self.account)?
        {
            hub.retain_item_revision(revision)?;
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
                CurrentAuthority::view(self.host.clone(), key.clone(), self.account).await?;
                let (host, opening, pinned) = (self.host.clone(), key.clone(), plugin.clone());
                let generation = disk(move || {
                    let row = activation(&host.store, opening.space, &pinned)?;
                    if !row.enabled {
                        return Err(hidden());
                    }
                    host.store
                        .content()
                        .enable_live_editing(opening.space, &opening.kind)?;
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
        let (key, plugin, generation, proof, kicked) = (
            member.key.clone(),
            member.plugin.clone(),
            member.generation,
            member.proof.clone(),
            member.kicked.clone(),
        );
        async move {
            let deadline = Instant::now() + WAIT;
            loop {
                if *kicked.borrow() {
                    return Err(hidden());
                }
                let item_epoch = host.store.live_rooms().item_authority_epoch();
                let plugin_epoch = host.store.live_rooms().activation_epoch(key.space);
                if item_epoch.is_none_or(|epoch| epoch & 1 == 0)
                    && plugin_epoch & DIRTY_ACTIVATION == 0
                {
                    if !session.is_live(&host.directory).await {
                        return Err(Error::unauthorized("room session ended"));
                    }
                    if write {
                        CurrentAuthority::edit(host.clone(), key.clone(), account).await?;
                    } else if !proof.has_view(item_epoch) {
                        CurrentAuthority::view(host.clone(), key.clone(), account).await?;
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
                    if *kicked.borrow()
                        || host
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
                        if !write {
                            proof
                                .view
                                .store(item_epoch.unwrap_or(UNCACHED), Ordering::Relaxed);
                        }
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
        return Err(Error::unavailable(
            "room authority is changing; reconnect to retry",
        ));
    }
    tokio::time::sleep(RETRY).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
