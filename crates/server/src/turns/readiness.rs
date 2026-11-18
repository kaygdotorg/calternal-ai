//! Authorized operation for machine and provider readiness.
use super::*;

/// A verified person's cached readiness. One account-scoped registry snapshot.
/// One indexed statement checks provider activation and any thread pin.
pub(super) async fn read<D: Directory>(
    state: Shared<D>,
    machines: Arc<MachineService>,
    who: Identity,
    input: calternal_core::ReadinessInput,
) -> Result<Value, Error> {
    let space = SpaceId::parse(&input.space_id).map_err(|_| Error::not_found("space not found"))?;
    blocking(move || {
        let policy = crate::authz::policy(calternal_core::AGENT_READINESS)
            .ok_or_else(|| Error::internal("readiness operation has no policy"))?;
        policy.authorize_space(&state.directory, &who, space)?;
        let thread = input
            .thread_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| Error::not_found("thread not found"))?;
        let (pin, controls) =
            crate::threads::index::readiness_context(&state, space, thread, who.id)?;
        let active =
            |plugin: &crate::plugins::Plugin| crate::threads::index::enabled(&controls, plugin);
        let orchestrator = state
            .registry
            .get("calternal-orchestrator")
            .map(|p| active(p))
            .transpose()?
            .unwrap_or(false);
        let providers = state
            .registry
            .plugins()
            .iter()
            .filter(|plugin| plugin.manifest.provider.is_some())
            .map(|p| Ok((p.manifest.id.as_str(), orchestrator && active(p)?)))
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(json!(
            machines.readiness(who.id, space, thread, pin, &providers)?
        ))
    })
    .await
}

/// A person-request pin and explicit check target share the placement constraint.
pub(super) fn pin(checkpoint: &Value, input: &Value) -> Option<Uuid> {
    checkpoint["request_machine"]
        .as_str()
        .or_else(|| input["check_target"].as_str())
        .and_then(|id| Uuid::parse_str(id).ok())
}

/// Resolve an explicit Check target using the same current enrollment authority
/// as placement. O(1) by personal UUID or one anonymous pool candidate lookup.
pub(super) async fn check_target(
    machines: &Arc<MachineService>,
    account: calternal_types::AccountId,
    input: &AgentInput,
) -> Result<Option<Uuid>, Error> {
    if input.machine_id.is_empty() && !input.pool {
        return Ok(None);
    }
    let id = if input.machine_id.is_empty() {
        None
    } else {
        Some(
            Uuid::parse_str(&input.machine_id)
                .map_err(|_| Error::not_found("machine not found"))?,
        )
    };
    let (machines, pool) = (machines.clone(), input.pool);
    blocking(move || machines.check_target(account, id, pool).map(Some)).await
}
