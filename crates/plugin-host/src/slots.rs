//! Validate contribution references and the renderer allowed at each mount point.
use crate::registry::{identifier, unique};
use calternal_core::{
    ManifestCapability, ManifestV1, PluginSlot, PluginSlotRenderer, PluginSlotTarget,
};

pub(crate) fn validate(manifest: &ManifestV1) -> Result<(), String> {
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
        return Err(
            "/owned_slots: check unique IDs, required capabilities and contribution bounds".into(),
        );
    }
    let Some(slots) = &manifest.slots else {
        return Ok(());
    };
    if slots.len() > 32 || !unique(slots.iter().map(|slot| slot.id.as_str())) {
        return Err("/slots: use at most 32 unique valid slot IDs".into());
    }
    for (index, slot) in slots.iter().enumerate() {
        let invalid = |field, rule| Err(format!("/slots/{index}/{field}: {rule}"));
        if slot.label.trim().is_empty()
            || slot.label.len() > 128
            || slot.label.chars().any(char::is_control)
        {
            return invalid(
                "label",
                "expected a nonempty label of at most 128 bytes without control characters",
            );
        }
        let host = match &slot.slot {
            PluginSlotTarget::Host(host) => Some(*host),
            PluginSlotTarget::Owned(target) => {
                if !identifier(&target.plugin_id) || !identifier(&target.id) {
                    return invalid("slot", "expected a valid slot owner and slot ID");
                }
                None
            }
        };
        let valid = match &slot.renderer {
            PluginSlotRenderer::Generated { operation } => {
                host != Some(PluginSlot::PaletteCommands)
                    && manifest.operations.iter().any(|op| {
                        op.id == *operation
                            && !op.mutates
                            && op.output_schema["properties"]["view"].is_object()
                            && op.output_schema["required"]
                                .as_array()
                                .is_some_and(|fields| fields.iter().any(|field| field == "view"))
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
        };
        if !valid {
            let rule = match &slot.renderer {
                PluginSlotRenderer::Generated { .. } => {
                    "expected a read-only operation with an output schema declaring and requiring view, in a slot that accepts generated views"
                }
                PluginSlotRenderer::Sealed { .. } => {
                    "expected a safe relative HTML asset path in a host slot that accepts sealed frames"
                }
                PluginSlotRenderer::Command { .. } => {
                    "expected an existing operation in a slot that accepts commands"
                }
            };
            return invalid("renderer", rule);
        }
    }
    Ok(())
}
