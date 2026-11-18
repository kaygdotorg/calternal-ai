//! Resolve package activation and indexed slot declarations in the selected space.
use super::*;
use calternal_types::AccountId;

pub(super) mod live;

/// List registered manifests and caller-visible contributions. Read one indexed
/// activation row per visible package, with one authority lookup for slot
/// resolution. No content reads, worker invocations or durable writes.
pub(super) async fn list<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    space: SpaceId,
) -> Result<Value, Error> {
    let catalog = state.registry.snapshot();
    let declarations = catalog.clone();
    let plugins = blocking(state, move |store| {
        plugin_states(store, &declarations, space)
    })
    .await?;
    let (slots, owned_slots) = if catalog.has_slots() {
        let authority = super::authorization::capabilities(state, account, space).await?;
        authz::policy("plugins.list")
            .expect("listing policy")
            .check(authority)?;
        catalog.resolve_slots(&plugins, authority.unwrap_or_default())
    } else {
        (Vec::new(), Vec::new())
    };
    to_value(calternal_core::PluginsListResult {
        plugins: plugins.into_iter().flatten().collect(),
        slots: (!slots.is_empty()).then_some(slots),
        owned_slots: (!owned_slots.is_empty()).then_some(owned_slots),
    })
}

fn plugin_states(
    store: &Store,
    catalog: &crate::plugins::Catalog,
    space: SpaceId,
) -> Result<Vec<Option<PluginState>>, Error> {
    catalog
        .plugins()
        .iter()
        .map(|plugin| {
            plugin
                .available_in(space)
                .then(|| plugin_state(store, space, plugin))
                .transpose()
        })
        .collect()
}
