//! Authored automation definitions, using the normal guarded content writer.
mod definitions;
pub(crate) mod scheduling;

use crate::{
    api::Shared,
    auth::{Directory, Identity},
    plugins::Session,
};
use calternal_core::{
    AutomationPreview, AutomationSchedule, JsonObject, automation_schedule::Schedule,
};
use calternal_types::{AccountId, Actor, Error, SpaceId, now_ms};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

const KIND: &str = "automations";
const FORMAT: &str = "calternal-automation/v1";

/// Read the creator's current zone on a blocking worker. UTC is the default.
fn account_zone<D: Directory>(state: &Shared<D>, account: AccountId) -> Result<String, Error> {
    Ok(state
        .directory
        .protected_spaces()
        .map(|spaces| {
            spaces
                .own_heading_profile(account)
                .map_err(crate::auth::auth_error)
        })
        .transpose()?
        .flatten()
        .and_then(|(_, zone)| zone)
        .unwrap_or_else(|| "UTC".into()))
}

fn preview(schedule_text: String, timezone: String, now: i64) -> Result<AutomationSchedule, Error> {
    let schedule = Schedule::parse(&schedule_text)?;
    Ok(AutomationSchedule {
        schedule_text,
        schedule: schedule.expression(),
        human: schedule.describe(),
        next_run_ms: schedule.next(&timezone, now),
        timezone,
    })
}

fn decode<T: DeserializeOwned>(input: JsonObject) -> Result<T, Error> {
    serde_json::from_value(json!(input))
        .map_err(|_| Error::invalid_input("invalid automation input"))
}

/// Dispatch within the space that passed the normal host policy checks.
pub(crate) async fn dispatch<D: Directory>(
    state: Shared<D>,
    who: Identity,
    session: Session,
    actor: Actor,
    request_id: &str,
    operation: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let space = input
        .get("space_id")
        .and_then(Value::as_str)
        .and_then(|raw| SpaceId::parse(raw).ok())
        .ok_or_else(|| Error::not_found("space not found"))?;
    if operation == "automations.run_now" {
        if actor.via.is_some() {
            return Err(Error::forbidden("a person must start the automation"));
        }
        return scheduling::manual(state, who, session, space, request_id, decode(input)?).await;
    }
    let operation = operation.to_owned();
    let request_id = request_id.to_owned();
    crate::blocking::spawn(move || {
        if operation == "automations.preview_schedule" {
            let i: AutomationPreview = decode(input)?;
            return Ok(json!(preview(
                i.schedule_text,
                account_zone(&state, who.id)?,
                now_ms()
            )?));
        }
        if operation.starts_with("skills.") {
            return crate::api::search_skills::mutate_or_read(
                &state.store,
                space,
                &actor,
                &request_id,
                &operation,
                input,
            );
        }
        if actor.via.is_some()
            && calternal_core::automation_operation(&operation)
                .is_some_and(|op| op.risk != calternal_core::OperationRisk::Read)
        {
            return Err(Error::forbidden("a person must configure automations"));
        }
        definitions::dispatch(&state, who, actor, space, &request_id, &operation, input)
    })
    .await
    .map_err(|_| Error::internal("automation operation worker stopped"))?
}
