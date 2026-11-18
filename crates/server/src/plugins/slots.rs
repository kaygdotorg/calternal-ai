//! Compile package-owned and fixed targets once with the immutable catalog.
use super::{Catalog, host_capability};
use calternal_core::{
    OwnedPluginSlotKind, PluginSlotRenderer, PluginSlotTarget, PluginState,
    ResolvedOwnedPluginSlot, ResolvedPluginSlot,
};
use calternal_types::{Capability, CapabilitySet};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub(super) struct SlotIndex {
    owners: Vec<Declaration>,
    contributions: Vec<Contribution>,
}

#[derive(Clone, Copy, Debug)]
struct Declaration {
    plugin: usize,
    definition: usize,
    required: Capability,
}

#[derive(Clone, Copy, Debug)]
struct Contribution {
    plugin: usize,
    contribution: usize,
    required: Capability,
    owner: Option<usize>,
}

impl SlotIndex {
    pub(super) fn compile(catalog: &Catalog) -> Self {
        let plugins = catalog.plugins();
        let mut declarations = BTreeMap::new();
        for (plugin, package) in plugins.iter().enumerate() {
            for (definition, slot) in package.manifest.owned_slots.iter().flatten().enumerate() {
                if let Some(required) = host_capability(slot.required_capability) {
                    declarations.insert(
                        (package.manifest.id.as_str(), slot.id.as_str()),
                        Declaration {
                            plugin,
                            definition,
                            required,
                        },
                    );
                }
            }
        }
        let owners = declarations.values().copied().collect::<Vec<_>>();
        let ids = declarations
            .keys()
            .copied()
            .enumerate()
            .map(|(index, id)| (id, index))
            .collect::<BTreeMap<_, _>>();
        let mut contributions = Vec::new();
        for (plugin, package) in plugins.iter().enumerate() {
            for (contribution, row) in package.manifest.slots.iter().flatten().enumerate() {
                let (operation, kind) = match &row.renderer {
                    PluginSlotRenderer::Generated { operation } => {
                        (operation, OwnedPluginSlotKind::Generated)
                    }
                    PluginSlotRenderer::Command { operation } => {
                        (operation, OwnedPluginSlotKind::Command)
                    }
                    PluginSlotRenderer::Sealed { .. } => continue,
                };
                let Some(policy) = catalog.policy(operation) else {
                    continue;
                };
                let owner = match &row.slot {
                    PluginSlotTarget::Host(_) => None,
                    PluginSlotTarget::Owned(target) => {
                        let Some(&index) =
                            ids.get(&(target.plugin_id.as_str(), target.id.as_str()))
                        else {
                            continue;
                        };
                        let at = owners[index];
                        let slot = &plugins[at.plugin]
                            .manifest
                            .owned_slots
                            .as_ref()
                            .expect("indexed")[at.definition];
                        if !slot.accepts.contains(&kind) {
                            continue;
                        }
                        Some(index)
                    }
                };
                let sort = (&row.slot, package.manifest.id.as_str(), row.id.as_str());
                contributions.push((
                    sort,
                    Contribution {
                        plugin,
                        contribution,
                        required: policy.required,
                        owner,
                    },
                ));
            }
        }
        contributions.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        Self {
            owners,
            contributions: contributions.into_iter().map(|(_, row)| row).collect(),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.owners.is_empty() && self.contributions.is_empty()
    }

    pub(super) fn resolve(
        &self,
        plugins: &[Option<PluginState>],
        authority: CapabilitySet,
    ) -> (Vec<ResolvedPluginSlot>, Vec<ResolvedOwnedPluginSlot>) {
        let enabled = |index: usize| plugins[index].as_ref().filter(|state| state.enabled);
        let mut mounts = Vec::new();
        let mut remaining = vec![0; self.owners.len()];
        for (index, row) in self.owners.iter().enumerate() {
            let Some(plugin) = enabled(row.plugin).filter(|_| authority.contains(row.required))
            else {
                continue;
            };
            let definition =
                &plugin.manifest.owned_slots.as_ref().expect("indexed")[row.definition];
            remaining[index] = definition.max_contributions;
            mounts.push(ResolvedOwnedPluginSlot {
                plugin_id: plugin.manifest.id.clone(),
                generation: plugin.generation,
                definition: definition.clone(),
            });
        }
        let mut slots = Vec::new();
        for row in &self.contributions {
            let Some(plugin) = enabled(row.plugin).filter(|_| authority.contains(row.required))
            else {
                continue;
            };
            if let Some(owner) = row.owner {
                if remaining[owner] == 0 {
                    continue;
                }
                remaining[owner] -= 1;
            }
            slots.push(ResolvedPluginSlot {
                plugin_id: plugin.manifest.id.clone(),
                generation: plugin.generation,
                contribution: plugin.manifest.slots.as_ref().expect("indexed")[row.contribution]
                    .clone(),
            });
        }
        (slots, mounts)
    }
}
