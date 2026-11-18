//! Cached provider declarations and the space's current activation policy.
use super::*;

/// List providers enabled for this authorized space. Cost: one indexed activation
/// snapshot and a walk of the startup-validated manifests, without contacting a machine.
pub(super) fn list<D: Directory>(
    state: &Shared<D>,
    space: SpaceId,
    who: &Identity,
) -> Result<Value, Error> {
    let (_, controls) = super::index::readiness_context(state, space, None, who.id)?;
    let mut providers = Vec::new();
    for plugin in state.registry.snapshot().plugins().iter() {
        let Some(spec) = plugin.manifest.provider.as_ref() else {
            continue;
        };
        if super::index::enabled(&controls, plugin)? {
            providers.push(json!({
                "id": plugin.manifest.id,
                "name": plugin.manifest.name,
                "default_model": spec.default_model,
                "models": spec.models,
            }));
        }
    }
    Ok(json!({"providers": providers}))
}

/// Resolve a saved send against the installed declaration before the message write.
/// Cost: a registry lookup and bounded in-memory search. Launch independently
/// checks space activation, which can change after admission.
pub(super) fn select<D: Directory>(
    state: &Shared<D>,
    provider: &str,
    model: Option<&str>,
    reasoning: Option<&str>,
) -> Result<String, Error> {
    let catalog = state.registry.snapshot();
    let spec = catalog
        .get(provider)
        .filter(|p| p.unavailable.is_none())
        .and_then(|p| p.manifest.provider.as_ref())
        .ok_or_else(|| Error::invalid_input("Unknown provider. Choose one from threads.models."))?;
    let model = model.unwrap_or(&spec.default_model);
    let declared = spec
        .models
        .iter()
        .find(|entry| entry.id == model)
        .ok_or_else(|| {
            Error::invalid_input("Unknown model for this provider. Choose one from threads.models.")
        })?;
    if reasoning.is_some_and(|level| !declared.reasoning_levels.iter().any(|id| id == level)) {
        return Err(Error::invalid_input(
            "This model does not accept that reasoning level. Choose a level from threads.models or omit reasoning.",
        ));
    }
    Ok(declared.id.clone())
}
