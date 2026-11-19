//! Resolve space placement without lending the Owner's API permissions.
use super::*;
use crate::machines::space_execution::SpaceExecution;
use calternal_core::{SPACE_MACHINE_GET, SPACE_MACHINE_SET, SpaceMachineResult};
use calternal_types::AccountId;

/// Image-owned entry point that older provider images lack and cannot silently emulate.
pub(crate) const SHARED_ADAPTER: &str = "/opt/calternal-provider/bin/calternal-provider-shared";

const LICENSE_NOTICE: &str = "Personal subscriptions are licensed for use by one person.";

pub(super) async fn dispatch<D: Directory>(
    state: Shared<D>,
    machines: Arc<MachineService>,
    who: Identity,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    calternal_core::validate_operation_input(op, &input)?;
    let space = SpaceId::parse(
        input
            .get("space_id")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
    .map_err(|_| Error::not_found("space not found"))?;
    let machine = input
        .get("machine_id")
        .and_then(Value::as_str)
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| Error::not_found("sandbox machine not found"))?;
    let policy = crate::authz::policy(op)
        .ok_or_else(|| Error::internal("space execution policy missing"))?;
    let set = op == SPACE_MACHINE_SET;
    let tools_enabled = input.get("tools_enabled").and_then(Value::as_bool);
    blocking(move || {
        let lifecycle = state.store.content().lifecycle();
        let _space = lifecycle.step(space.as_uuid())?;
        let _actor = lifecycle.step(who.id.as_uuid())?;
        let grants = state.directory.capabilities(who.id, space);
        policy.check(grants)?;
        if set {
            let sponsor = owner(&state, space)?
                .ok_or_else(|| Error::invalid_input("only shared spaces have a shared machine"))?;
            if tools_enabled == Some(true) {
                if who.id != sponsor {
                    return Err(Error::forbidden(
                        "only the space Owner can enable shared tools",
                    ));
                }
                if machine.is_none() {
                    return Err(Error::invalid_input(
                        "choose a shared machine before enabling tools",
                    ));
                }
            }
            machines.set_space_execution(space, sponsor, machine, tools_enabled)?;
        }
        let binding = machines.space_execution(space)?;
        let show = grants.is_some_and(|caps| caps.contains(Capability::SpaceSettings));
        Ok(json!(SpaceMachineResult {
            configured: binding.is_some(),
            tools_enabled: binding.is_some_and(|binding| binding.tools_enabled),
            machine_id: binding
                .filter(|_| show)
                .map(|binding| binding.machine.to_string()),
            notice: LICENSE_NOTICE.into(),
        }))
    })
    .await
}

fn owner<D: Directory>(state: &Shared<D>, space: SpaceId) -> Result<Option<AccountId>, Error> {
    state
        .directory
        .protected_spaces()
        .ok_or_else(|| Error::unavailable("stored space directory unavailable"))?
        .shared_execution_owner(space)
        .map_err(crate::auth::auth_error)
}

/// Validate a stored binding against the current Owner. Route only configured
/// spaces here so existing personal and unconfigured execution stays unchanged.
pub(crate) fn resolve<D: Directory>(
    state: &Shared<D>,
    machines: &MachineService,
    space: SpaceId,
) -> Result<Option<SpaceExecution>, Error> {
    let Some(binding) = machines.space_execution(space)? else {
        return Ok(None);
    };
    if owner(state, space)? != Some(binding.sponsor) {
        return Err(Error::unavailable(
            "the space Owner must configure a shared machine",
        ));
    }
    Ok(Some(binding))
}

/// Identify the two person-only setting operations for the turn dispatcher.
pub(super) fn is_setting(op: &str) -> bool {
    matches!(op, SPACE_MACHINE_GET | SPACE_MACHINE_SET)
}
