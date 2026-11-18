//! Inbox and transport admission for the durable job signal transaction.
use crate::{
    api::Shared,
    auth::{Directory, Identity},
    plugins::{
        Plugin, activation,
        jobs::{Job, waits::{Decision, Resolution, WaitSource}},
    },
};
use calternal_core::{JsonObject, RequestResolution};
use calternal_types::{Capability, Error, SpaceId};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;
mod webhook;
pub(crate) use webhook::{PATH, PREFIX, webhook};

/// Apply a decision addressed to an Inbox request. Returns `None` for requests
/// outside the plugin wait store, allowing the native request service to resolve them.
/// Cost: one indexed search per visible space with jobs and one FULL decision commit.
#[allow(clippy::too_many_arguments)]
pub(crate) fn inbox<D: Directory>(
    state: &Shared<D>, person: &Identity, id: &str, required: Capability,
    request: &str, fingerprint: &str, status: &str, value: Option<&JsonObject>,
    runtime: &tokio::runtime::Handle,
) -> Result<Option<RequestResolution>, Error> {
    if Uuid::parse_str(id).is_err() {
        return Ok(None);
    }
    for space in state.directory.describe(person).spaces {
        let Ok(space_id) = crate::requests::authorize(state, person.id, &space.id, required) else {
            continue;
        };
        let Some((job, source)) = state.jobs.wait_source(&space.id, id, Some(person.id))? else {
            continue;
        };
        let plugin = enabled_plugin(state, &job, space_id)?;
        let (status, outcome) = match (source, status, value) {
            (WaitSource::Question { schema_operation, .. }, "answered", Some(answer)) => {
                let answer = json!(answer);
                plugin.schemas.validate_input(&schema_operation, &answer)
                    .map_err(|_| Error::invalid_input("answer does not match the question schema"))?;
                ("answered", json!({"kind": "answer", "value": answer}))
            }
            (WaitSource::Approval, "once", _) => ("approved", json!({"kind": "approval", "approved": true})),
            (WaitSource::Approval, "denied", _) => ("denied", json!({"kind": "approval", "approved": false})),
            (WaitSource::Approval, "this_thread", _) => {
                return Err(Error::invalid_input("this question allows a one-time decision"));
            }
            _ => return Ok(None),
        };
        let resolution = state.jobs.resolve_wait(&space.id, id, &job.id, Some(Decision {
            outcome,
            receipt: Some((request, fingerprint)),
        }))?;
        if let Err(error) = state.inputs.invalidate(person.id) {
            tracing::warn!(account = %person.id, reason = %error.message, "plugin Inbox invalidation failed");
        }
        if matches!(resolution, Resolution::Woken) {
            let _runtime = runtime.enter();
            super::runner::schedule(state, space.id, job.id);
        }
        return Ok(Some(RequestResolution { id: id.to_owned(), status: status.into() }));
    }
    Ok(None)
}

fn enabled_plugin<D: Directory>(state: &Shared<D>, job: &Job, space: SpaceId) -> Result<Arc<Plugin>, Error> {
    let hidden = || Error::not_found("wait not found");
    let catalog = state.registry.snapshot();
    let plugin = catalog.revision(&job.plugin, job.package_revision).ok_or_else(hidden)?;
    let required = plugin.operation(&job.operation)
        .and_then(|op| super::host_capability(op.required_capability)).ok_or_else(hidden)?;
    if !state.directory.capabilities(job.account, space).is_some_and(|caps| caps.contains(required)) {
        return Err(hidden());
    }
    let row = activation(&state.store, space, plugin)?;
    if !row.enabled || row.generation != job.generation {
        return Err(hidden());
    }
    Ok(plugin.clone())
}
