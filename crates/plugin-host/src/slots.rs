//! Validate contribution references and the renderer allowed at each mount point.
use crate::registry::{identifier, unique};
use calternal_core::{
    ManifestCapability, ManifestV1, PluginSlot, PluginSlotRenderer, PluginSlotTarget,
};

pub(crate) fn valid(manifest: &ManifestV1) -> bool {
    if manifest.owned_slots.as_ref().is_some_and(|slots| {
        slots.len() > 32
            || !unique(slots.iter().map(|slot| slot.id.as_str()))
            || slots.iter().any(|slot| {
                matches!(
                    slot.required_capability,
                    ManifestCapability::SpacesList | ManifestCapability::SpacesCreate
                ) || !(1..=32).contains(&slot.max_contributions)
                    || slot.accepts.is_empty()
                    || slot.accepts.len() > 2
                    || (slot.accepts.len() == 2 && slot.accepts[0] == slot.accepts[1])
            })
    }) {
        return false;
    }
    let Some(slots) = &manifest.slots else {
        return true;
    };
    slots.len() <= 32
        && unique(slots.iter().map(|slot| slot.id.as_str()))
        && slots.iter().all(|slot| {
            if slot.label.trim().is_empty()
                || slot.label.len() > 128
                || slot.label.chars().any(char::is_control)
            {
                return false;
            }
            let host = match &slot.slot {
                PluginSlotTarget::Host(host) => Some(*host),
                PluginSlotTarget::Owned(target) => {
                    if !identifier(&target.plugin_id) || !identifier(&target.id) {
                        return false;
                    }
                    None
                }
            };
            match &slot.renderer {
                PluginSlotRenderer::Generated { operation } => {
                    host != Some(PluginSlot::PaletteCommands)
                        && manifest.operations.iter().any(|op| {
                            op.id == *operation
                                && !op.mutates
                                && op.output_schema["properties"]["view"].is_object()
                                && op.output_schema["required"]
                                    .as_array()
                                    .is_some_and(|fields| {
                                        fields.iter().any(|field| field == "view")
                                    })
                        })
                }
                PluginSlotRenderer::Sealed { asset } => {
                    host.is_some_and(|host| {
                        !matches!(
                            host,
                            PluginSlot::PaletteCommands
                                | PluginSlot::MessageDecoration
                                | PluginSlot::Toasts
                        )
                    }) && asset.len() <= 256
                        && asset.ends_with(".html")
                        && asset.split('/').all(crate::registry::identifier)
                        && !asset.split('/').any(|part| matches!(part, "." | ".."))
                }
                PluginSlotRenderer::Command { operation } => {
                    host.is_none_or(|slot| slot == PluginSlot::PaletteCommands)
                        && manifest.operations.iter().any(|op| op.id == *operation)
                }
            }
        })
}
