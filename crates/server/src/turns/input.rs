//! Bounds and canonical identifiers at manual turn admission.
use super::{AgentInput, Directory, Error, Shared, SpaceId, Uuid, blocking};

/// Normalize the bounded manual-turn inputs before they become durable job keys.
pub(super) fn validate(i: &mut AgentInput, op: &str) -> Result<(), Error> {
    if i.prompt.len() > 16 * 1024
        || i.model.len() > 128
        || i.source_app.len() > 80
        || i.source_app.contains('.')
        || i.app.len() > 80
    {
        return Err(Error::invalid_input("agent input exceeds its bounds"));
    }
    if matches!(op, "agent.run" | "agent.check") {
        let invalid = || Error::invalid_input("check/run needs space_id and thread_id UUIDs");
        let space = SpaceId::parse(&i.space_id).map_err(|_| invalid())?;
        if op == "agent.run" || !i.thread_id.is_empty() {
            i.thread_id = Uuid::parse_str(&i.thread_id)
                .map_err(|_| invalid())?
                .to_string();
        }
        if !i.job_id.is_empty() {
            return Err(invalid());
        }
        i.space_id = space.to_string();
        if op == "agent.run" && i.prompt.trim().is_empty() {
            return Err(Error::invalid_input("run needs a prompt"));
        }
    } else {
        i.job_id = Uuid::parse_str(&i.job_id)
            .map_err(|_| Error::invalid_input("job_id must be a UUID"))?
            .to_string();
    }
    Ok(())
}

/// Origin labels select context scope, never introduce an unknown or disabled app.
/// Plugin identity/kind/operation prefixes come from the registry; native groups
/// derive from the built-in catalog. One activation read per matching plugin.
pub(super) async fn validate_source_app<D: Directory>(
    state: &Shared<D>,
    space: &str,
    source: &str,
) -> Result<(), Error> {
    if source.is_empty() {
        return Ok(());
    }
    let (st, space, source) = (
        state.clone(),
        SpaceId::parse(space).map_err(|_| Error::not_found("space not found"))?,
        source.to_owned(),
    );
    blocking(move || {
        let mut found = false;
        for plugin in st.registry.plugins() {
            if plugin.manifest.id == source
                || plugin.manifest.kinds.iter().any(|k| k.id == source)
                || plugin
                    .manifest
                    .operations
                    .iter()
                    .any(|o| crate::turns::access::app(&o.id) == source)
            {
                found = true;
                if !crate::plugins::activation(&st.store, space, plugin)?.enabled {
                    return Err(Error::forbidden("source app is disabled"));
                }
            }
        }
        if found
            || calternal_core::cached_operation_catalog()
                .iter()
                .any(|o| crate::turns::access::app(&o.id) == source)
        {
            Ok(())
        } else {
            Err(Error::invalid_input(
                "source_app must name an enabled plugin or built-in app",
            ))
        }
    })
    .await
}
