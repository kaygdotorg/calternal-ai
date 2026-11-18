//! Validate contribution references and the renderer allowed at each mount point.
use crate::registry::unique;
use calternal_core::{ManifestV1, PluginSlot, PluginSlotRenderer};

pub(crate) fn valid(manifest: &ManifestV1) -> bool {
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
            match &slot.renderer {
                PluginSlotRenderer::Generated { operation } => {
                    slot.slot != PluginSlot::PaletteCommands
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
                    !matches!(
                        slot.slot,
                        PluginSlot::PaletteCommands
                            | PluginSlot::MessageDecoration
                            | PluginSlot::Toasts
                    ) && asset.len() <= 256
                        && asset.ends_with(".html")
                        && asset.split('/').all(crate::registry::identifier)
                        && !asset.split('/').any(|part| matches!(part, "." | ".."))
                }
                PluginSlotRenderer::Command { operation } => {
                    slot.slot == PluginSlot::PaletteCommands
                        && manifest.operations.iter().any(|op| op.id == *operation)
                }
            }
        })
}
