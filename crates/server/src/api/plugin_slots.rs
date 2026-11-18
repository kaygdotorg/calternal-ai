//! Resolve bounded slot declarations against live space authority and activation.
use super::*;
use calternal_core::{PluginSlotRenderer, ResolvedPluginSlot};

/// Resolve only enabled contributions whose operation is visible to the caller.
/// Cost: at most one extra authority lookup, no content reads or durable writes,
/// and one bounded scan of each enabled manifest with contributions.
pub(super) async fn resolve<D: Directory>(
    state: &Shared<D>,
    catalog: &crate::plugins::Catalog,
    account: calternal_types::AccountId,
    space: SpaceId,
    plugins: &[PluginState],
) -> Result<Option<Vec<ResolvedPluginSlot>>, Error> {
    let enabled = plugins
        .iter()
        .filter(|plugin| plugin.enabled && plugin.manifest.slots.is_some());
    if enabled.clone().next().is_none() {
        return Ok(None);
    }
    let authority = super::authorization::capabilities(state, account, space).await?;
    authz::policy("plugins.list")
        .expect("plugin listing policy")
        .check(authority)?;
    let mut slots = Vec::new();
    for plugin in enabled {
        for contribution in plugin.manifest.slots.iter().flatten() {
            let operation = match &contribution.renderer {
                PluginSlotRenderer::Generated { operation }
                | PluginSlotRenderer::Command { operation } => operation,
                PluginSlotRenderer::Sealed { .. } => continue,
            };
            let reachable = catalog
                .policy(operation)
                .is_some_and(|policy| policy.check(authority).is_ok());
            if reachable {
                slots.push(ResolvedPluginSlot {
                    contribution: contribution.clone(),
                    plugin_id: plugin.manifest.id.clone(),
                    generation: plugin.generation,
                });
            }
        }
    }
    slots.sort_unstable_by(|a, b| {
        (a.contribution.slot, &a.plugin_id, &a.contribution.id).cmp(&(
            b.contribution.slot,
            &b.plugin_id,
            &b.contribution.id,
        ))
    });
    Ok(Some(slots))
}
