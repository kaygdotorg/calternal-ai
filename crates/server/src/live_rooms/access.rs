//! Current session, space capability and pinned plugin checks.

use super::*;
use crate::api::authorization;
use calternal_types::Capability;

impl<D: Directory> Connection<D> {
    pub(super) fn catalog(&self, key: &Key) -> Option<Arc<Plugin>> {
        self.host
            .registry
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

    pub(super) fn capabilities(
        &self,
        key: &Key,
        write: bool,
    ) -> impl std::future::Future<Output = Result<(), Error>> + use<D> {
        let (host, session, account, space) = (
            self.host.clone(),
            self.session.clone(),
            self.account,
            key.space,
        );
        async move {
            if !session.is_live(&host.directory).await {
                return Err(Error::unauthorized("room session ended"));
            }
            let caps = authorization::capabilities(&host, account, space).await?;
            let caps = caps
                .filter(|caps| caps.contains(Capability::SpaceRead))
                .ok_or_else(hidden)?;
            if write && !caps.contains(Capability::ContentWrite) {
                return Err(Error::forbidden("room is read-only"));
            }
            Ok(())
        }
    }

    pub(super) fn authorize(
        &self,
        key: &Key,
        plugin: &Arc<Plugin>,
        generation: u32,
        write: bool,
    ) -> impl std::future::Future<Output = Result<(), Error>> + use<D> {
        let capabilities = self.capabilities(key, write);
        let current = self
            .catalog(key)
            .is_some_and(|active| Arc::ptr_eq(&active, plugin));
        let (host, space, plugin) = (self.host.clone(), key.space, plugin.clone());
        async move {
            capabilities.await?;
            if !current {
                return Err(hidden());
            }
            disk(move || {
                let row = activation(&host.store, space, &plugin)?;
                if row.enabled && row.generation == generation {
                    Ok(())
                } else {
                    Err(hidden())
                }
            })
            .await
        }
    }
}
