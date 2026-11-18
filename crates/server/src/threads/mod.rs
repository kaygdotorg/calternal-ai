//! Threads product engine: manifest policy, content files and the existing turn engine.
//! All durable work runs on blocking workers; no independent store or global lock.
mod delta;
mod document;
pub(crate) mod index;
mod operations;
pub(crate) mod persistence;
pub(crate) mod provider_session;
pub(crate) mod turn_events;
mod turn_status;
use crate::turns::blocking;
use crate::{
    api::Shared,
    auth::{Directory, Identity},
    plugins::Session,
    turns::access::Authority,
};
use calternal_core::{AgentInput, AgentMode, JsonObject};
use calternal_types::{Error, SpaceId};
use serde_json::{Value, json};
use uuid::Uuid;

/// Called only after the ordinary manifest resolver, capability and token policy gate.
/// Schemas are cached at startup. Writes use content's revision guard and receipts;
/// send admission is replayable after its message commit. Cost is documented in threads.md.
pub(crate) async fn dispatch<D: Directory>(
    state: Shared<D>,
    who: Identity,
    session: Session,
    agent: Option<Authority>,
    rid: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let space = input
        .get("space_id")
        .and_then(Value::as_str)
        .and_then(|s| SpaceId::parse(s).ok())
        .ok_or_else(|| Error::not_found("space not found"))?;
    let generation =
        crate::turns::enabled_generation(&state, &space.to_string(), "calternal-orchestrator")
            .await?;
    let plugin = state
        .registry
        .get("calternal-orchestrator")
        .ok_or_else(|| Error::not_found("unknown operation"))?;
    plugin
        .schemas
        .validate_input(op, &json!(input))
        .map_err(|_| Error::invalid_input("input does not match operation schema"))?;
    let id = input
        .get("id")
        .and_then(Value::as_str)
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| Error::not_found("item not found"))?;
    if let Some(a) = &agent {
        if id.is_some_and(|id| a.thread().ok() != Some(id)) || op == "threads.create" {
            return Err(Error::not_found("item not found"));
        }
        if input.contains_key("mode") {
            return Err(Error::forbidden(
                "a person must choose the thread permission mode",
            ));
        }
    }
    let (st, operation, request_id, data) =
        (state.clone(), op.to_owned(), rid.to_owned(), input.clone());
    let caller = who.clone();
    let actor = agent
        .as_ref()
        .map(|a| a.actor.clone())
        .unwrap_or_else(|| calternal_types::Actor::account(who.id));
    let value = blocking(move || {
        operations::apply(
            &st,
            &caller,
            space,
            &operation,
            &request_id,
            &data,
            id,
            &actor,
        )
    })
    .await?;
    if op == "threads.send" {
        crate::fault::hit(crate::fault::FaultPoint::AfterThreadMessage);
        let thread = value["thread"]["id"]
            .as_str()
            .ok_or_else(|| Error::internal("saved thread identity missing"))?;
        let i = AgentInput {
            space_id: space.to_string(),
            thread_id: thread.into(),
            prompt: input["message"].as_str().unwrap_or("").into(),
            provider_id: value["thread"]["metadata"]["provider"]
                .as_str()
                .unwrap_or("")
                .into(),
            mode: serde_json::from_value(value["thread"]["metadata"]["mode"].clone())
                .map_err(|_| Error::internal("saved mode invalid"))?,
            model: input
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            source_app: "threads".into(),
            ..Default::default()
        };
        let job = crate::turns::admit_thread(
            &state,
            who,
            session,
            rid,
            i,
            generation,
            value["thread"]["metadata"]["pending"].clone(),
        )
        .await?;
        return Ok(json!({"thread":value["thread"],"job":job.view()}));
    }
    Ok(value)
}
