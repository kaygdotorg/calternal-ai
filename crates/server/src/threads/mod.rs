//! Threads product engine: manifest policy, content files and the existing turn engine.
//! All durable work runs on blocking workers; no independent store or global lock.
#[cfg(target_os = "linux")]
mod append_checkpoint;
pub(crate) mod automation;
mod delta;
mod document;
mod headings;
pub(crate) mod index;
mod models;
mod operations;
pub(crate) mod persistence;
pub(crate) mod provider_notice;
pub(crate) mod provider_session;
mod records;
pub(crate) mod search;
mod snooze;
pub(crate) mod title;
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
    actor: calternal_types::Actor,
    rid: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let space = input
        .get("space_id")
        .and_then(Value::as_str)
        .and_then(|s| SpaceId::parse(s).ok())
        .ok_or_else(|| Error::not_found("space not found"))?;
    let space_key = space.to_string();
    let generation = retry_verifying(|| {
        crate::turns::enabled_generation(&state, &space_key, "calternal-orchestrator")
    }).await?;
    let catalog = state.registry.snapshot();
    let plugin = catalog
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
        if input.contains_key("mode") || input.contains_key("personal_machine") {
            return Err(Error::forbidden(
                "a person must choose the thread permission mode or execution machine",
            ));
        }
    }
    if op == "threads.models" {
        return blocking(move || models::list(&state, space, &who)).await;
    }
    #[cfg(feature = "perf-timings")]
    if op == "threads.send" {
        crate::api::thread_timing::mark(rid, "thread_save_queued");
    }
    let value = retry_verifying(|| {
        let (st, operation, request_id, data) =
            (state.clone(), op.to_owned(), rid.to_owned(), input.clone());
        let caller = who.clone();
        let message_actor = actor.clone();
        let session = session.clone();
        blocking(move || {
            let apply = || {
                operations::apply(
                    &st,
                    &caller,
                    space,
                    &operation,
                    &request_id,
                    &data,
                    id,
                    &message_actor,
                    Some(SendContext {
                        session,
                        generation,
                        machine: None,
                    }),
                )
            };
            #[cfg(feature = "perf-timings")]
            if operation == "threads.send" {
                return calternal_store::timings::observe_thread_storage(
                    &request_id,
                    "thread_save",
                    apply,
                );
            }
            apply()
        })
    })
    .await?;
    if op == "threads.send" {
        #[cfg(feature = "perf-timings")]
        crate::api::thread_timing::mark(rid, "thread_saved");
        crate::fault::hit(crate::fault::FaultPoint::AfterThreadMessage);
        let thread = value["thread"]["id"]
            .as_str()
            .ok_or_else(|| Error::internal("saved thread identity missing"))?;
        let i = send_input(space, thread, &input, &value["thread"]["metadata"], None)?;
        let job = crate::turns::admit_thread(
            &state,
            who,
            session,
            actor,
            rid,
            i,
            generation,
            value["intent"].clone(),
        )
        .await?;
        #[cfg(feature = "perf-timings")]
        crate::api::thread_timing::mark(rid, "job_admitted");
        return Ok(json!({"thread":value["thread"],"job":job.view()}));
    }
    Ok(value)
}

/// Build the same effective input for the durable admission and a replay.
fn send_input(
    space: SpaceId,
    thread: &str,
    input: &JsonObject,
    metadata: &Value,
    machine: Option<&str>,
) -> Result<AgentInput, Error> {
    Ok(AgentInput {
        space_id: space.to_string(),
        thread_id: thread.into(),
        prompt: input["message"].as_str().unwrap_or("").into(),
        provider_id: metadata["provider"].as_str().unwrap_or("").into(),
        mode: serde_json::from_value(metadata["mode"].clone())
            .map_err(|_| Error::internal("saved mode invalid"))?,
        model: metadata["model"]
            .as_str()
            .ok_or_else(|| Error::internal("saved model missing"))?
            .into(),
        options: input
            .get("reasoning")
            .cloned()
            .map(|level| [("effort".into(), level)].into_iter().collect())
            .unwrap_or_default(),
        personal_machine: input
            .get("personal_machine")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        machine_id: machine.unwrap_or_default().into(),
        source_app: "threads".into(),
        ..Default::default()
    })
}

struct SendContext<'a> {
    session: Session,
    generation: u32,
    machine: Option<&'a str>,
}

/// Let background file verification complete without occupying a blocking worker.
async fn retry_verifying<T, Fut>(mut attempt: impl FnMut() -> Fut) -> Result<T, Error>
where
    Fut: std::future::Future<Output = Result<T, Error>>,
{
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let result = attempt().await;
        let pending = result.as_ref().is_err_and(|error| {
            error
                .details
                .as_ref()
                .is_some_and(|d| d["reason"] == "thread_verifying")
        });
        if !pending || std::time::Instant::now() >= deadline {
            return result;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
