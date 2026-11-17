//! Manual provider turns: durable jobs, machine exec streams and explicit resume.
//! No conversation file is stored here; the Threads storage lane owns that step.
pub mod access;
mod instructions;
pub mod retry;
mod run;

use crate::{
    api::Shared,
    auth::{Directory, Identity},
    authz::Capability,
    error::HostError,
    machines::MachineService,
    plugins::{
        Session,
        jobs::{Admission, Job},
    },
};
use access::Authority;
use calternal_auth::{AccountId, InstallationId, SpaceId};
use calternal_core::{AgentInput, JobState, JsonObject};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

pub(super) async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, HostError> + Send + 'static,
) -> Result<T, HostError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| HostError::internal("turn task failed"))?
}

/// Dispatch a catalogued manual-turn operation. Every call rechecks current space
/// access before returning data; control and approval operations require ownership.
/// Cost: one job read and directory read; writes use existing per-space job transactions.
pub async fn dispatch<D: Directory>(
    state: Shared<D>,
    machines: Arc<MachineService>,
    who: Identity,
    session: Session,
    agent: Option<Authority>,
    rid: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, HostError> {
    if let Some(a) = &agent {
        a.check(op, &input)?;
    }
    let mut i: AgentInput = serde_json::from_value(json!(input))
        .map_err(|_| HostError::invalid_input("input does not match agent schema"))?;
    if op == "agent.settings" {
        if agent.is_some() {
            return Err(HostError::forbidden(
                "a person must configure automatic retries",
            ));
        }
        let defaults = retry::manifest_defaults(&state)?;
        if i.job_id.is_empty() {
            let (st, account, value) = (state.clone(), who.id.to_string(), i.retry_settings);
            return blocking(move || {
                Ok(json!({"retry_settings":st.jobs.agent_settings(&account,value,defaults)?}))
            })
            .await;
        }
        let job = load(&state, &i.job_id).await?;
        authorize(&state, &who, &job.space, Capability::PluginsUse).await?;
        if job.account != who.id.as_ref() {
            return Err(HostError::not_found("turn not found"));
        }
        let value = i
            .retry_settings
            .ok_or_else(|| HostError::invalid_input("thread settings require retry_settings"))?;
        let (st, sp, id) = (state.clone(), job.space, job.id);
        blocking(move || st.jobs.agent_thread_settings(&sp, &id, value)).await?;
        return Ok(json!({"retry_settings":value}));
    }
    if op == "machines.resume" {
        if agent.is_some() {
            return Err(HostError::forbidden(
                "a person must resume interrupted work",
            ));
        }
        let account =
            AccountId::parse(&who.id).ok_or_else(|| HostError::not_found("run not found"))?;
        let lease = Uuid::parse_str(&i.lease_id)
            .map_err(|_| HostError::invalid_input("lease_id must be a UUID"))?;
        let svc = machines.clone();
        let space = blocking(move || {
            svc.run_space(lease)?
                .ok_or_else(|| HostError::not_found("run not found"))
        })
        .await?;
        authorize(&state, &who, &space.to_string(), Capability::PluginsUse).await?;
        blocking(move || machines.resume_run(account, lease)).await?;
        return Ok(json!({"lease_id":i.lease_id,"state":"waiting"}));
    }
    if op == "machines.set" {
        if agent.is_some() {
            return Err(HostError::forbidden(
                "a person must change machine execution settings",
            ));
        }
        let account =
            AccountId::parse(&who.id).ok_or_else(|| HostError::not_found("machine not found"))?;
        let machine = Uuid::parse_str(&i.machine_id)
            .map_err(|_| HostError::invalid_input("machine_id must be a UUID"))?;
        blocking(move || machines.set_direct(account, machine, i.direct)).await?;
        return Ok(json!({"machine_id":i.machine_id,"direct":i.direct}));
    }
    validate(&mut i, op)?;
    if matches!(op, "agent.run" | "agent.check") {
        authorize(&state, &who, &i.space_id, Capability::PluginsUse).await?;
        let (st, account, request, scope) = (
            state.clone(),
            who.id.to_string(),
            rid.to_owned(),
            i.space_id.clone(),
        );
        let generation = enabled_generation(&state, &i.space_id, "calternal-orchestrator").await?;
        let defaults = retry::manifest_defaults(&state)?;
        let (st_settings, account_settings) = (state.clone(), who.id.to_string());
        let settings = match i.retry_settings {
            Some(v) => v,
            None => {
                blocking(move || {
                    st_settings
                        .jobs
                        .agent_settings(&account_settings, None, defaults)
                })
                .await?
            }
        };
        let data = json!({"request":i,"check":op=="agent.check","retry_settings":settings});
        let compare = data["request"].clone();
        let id = blocking(move || {
            st.jobs.admit(Admission {
                space: scope,
                plugin: "calternal-orchestrator".into(),
                account: account.clone(),
                session,
                operation: "agent.turn".into(),
                input: data,
                parent: None,
                generation,
                request_key: format!("agent/{account}/{request}"),
            })
        })
        .await?;
        let job = load(&state, &id).await?;
        if job.input["request"] != compare {
            return Err(HostError::conflict(
                "request_id already used with different input",
            ));
        }
        run::schedule(&state, &machines, job.space.clone(), id.clone());
        return Ok(json!({"job":job.view()}));
    }
    let job = load(&state, &i.job_id).await?;
    authorize(
        &state,
        &who,
        &job.space,
        if op == "agent.events" {
            Capability::SpaceRead
        } else {
            Capability::PluginsUse
        },
    )
    .await?;
    if job.operation != "agent.turn" {
        return Err(HostError::not_found("turn not found"));
    }
    let is_owner = job.account == who.id.as_ref();
    if op != "agent.events" && !is_owner {
        let (st, account, space) = (state.clone(), who.id.clone(), job.space.clone());
        let manager = blocking(move || {
            Ok(st
                .tokens
                .capabilities(&account, &space)
                .is_some_and(|c| c.contains(Capability::PluginsManage)))
        })
        .await?;
        if op != "agent.stop" || !manager {
            return Err(HostError::not_found("turn not found"));
        }
    }
    match op {
        "agent.events" => {
            let batch = state.jobs.live(&job.id).read(i.after);
            let reset = (!i.epoch.is_empty() && i.epoch != batch.epoch) || i.after < batch.oldest;
            let events:Vec<Value> = batch.chunks.iter().map(|c| json!({"seq":c.seq,"data":serde_json::from_str::<Value>(&c.data).unwrap_or(Value::Null)})).collect();
            let next = batch.chunks.last().map_or(i.after, |c| c.seq + 1);
            let (st, sp, id) = (state.clone(), job.space.clone(), job.id.clone());
            let requests = blocking(move || st.jobs.agent_requests(&sp, &id)).await?;
            Ok(
                json!({"job":job.view(),"epoch":batch.epoch,"events":events,"next":next,"reset":reset,"requests":requests}),
            )
        }
        "agent.discard" => {
            if !job.state.is_final() && job.state != JobState::Interrupted {
                return Err(HostError::conflict(
                    "stop this turn before discarding its workspace",
                ));
            }
            let (st, sp, id) = (state.clone(), job.space.clone(), job.id.clone());
            blocking(move || {
                st.jobs.cancel(&sp, &id)?;
                st.jobs.agent_revoke(&sp, &id)
            })
            .await?;
            let account = AccountId::parse(&job.account)
                .ok_or_else(|| HostError::not_found("turn not found"))?;
            let lease = job.checkpoint["lease_id"]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok());
            let status = if let Some(lease) = lease {
                blocking(move || machines.discard_run(account, lease, i.retry_failed)).await?
            } else {
                "done".into()
            };
            Ok(json!({"job":load(&state,&job.id).await?.view(),"discard":status}))
        }
        "agent.stop" => {
            let (st, sp, id) = (state.clone(), job.space.clone(), job.id.clone());
            blocking(move || {
                st.jobs.cancel(&sp, &id)?;
                st.jobs.agent_revoke(&sp, &id)
            })
            .await?;
            stop_machine(&machines, &job).await?;
            Ok(json!({"job":load(&state,&job.id).await?.view()}))
        }
        "agent.resume" => {
            if job.state != JobState::Interrupted {
                return Err(HostError::conflict(
                    "only an interrupted turn can resume; inspect its checkpoint first",
                ));
            }
            // Fence the previous lease before readmission, including after server recovery.
            stop_machine(&machines, &job).await?;
            let (st, sp, id) = (state.clone(), job.space.clone(), job.id.clone());
            blocking(move || st.jobs.agent_resume(&sp, &id)).await?;
            run::schedule(&state, &machines, job.space.clone(), job.id.clone());
            Ok(json!({"job":load(&state,&job.id).await?.view()}))
        }
        "access.request" => {
            let (st, sp, id, app, write) = (
                state.clone(),
                job.space.clone(),
                job.id.clone(),
                i.app,
                i.write,
            );
            let id = blocking(move || {
                let a = st.jobs.agent_scope(&sp, &id)?;
                st.jobs.agent_request(&a, &app, write)
            })
            .await?;
            Ok(json!({"approval_id":id,"status":"pending","scope":"this turn"}))
        }
        "agent.mode" | "access.approve" => {
            if agent.is_some() {
                return Err(HostError::forbidden(
                    "a freshly verified person must change agent access",
                ));
            }
            fresh(&state, &session).await?;
            if job.state.is_final() {
                return Err(HostError::conflict("finished turns cannot change access"));
            }
            // Revoke authority before stopping processes so no old token can act.
            let (st, sp, id, action) = (
                state.clone(),
                job.space.clone(),
                job.id.clone(),
                op.to_owned(),
            );
            blocking(move || {
                let mut a = st.jobs.agent_scope(&sp, &id)?;
                if action == "agent.mode" {
                    a.mode = i.mode;
                    st.jobs.agent_scope_change(&a)?;
                } else {
                    st.jobs.agent_approve(&mut a, &i.approval_id)?;
                }
                st.jobs.agent_interrupt(&sp, &id)?;
                Ok(())
            })
            .await?;
            stop_machine(&machines, &job).await?;
            Ok(
                json!({"job":load(&state,&job.id).await?.view(),"hint":"Resume explicitly to deliver the replacement token."}),
            )
        }
        _ => Err(HostError::not_found("unknown agent operation")),
    }
}

async fn load<D: Directory>(state: &Shared<D>, id: &str) -> Result<Job, HostError> {
    let space = state
        .jobs
        .route(id)
        .ok_or_else(|| HostError::not_found("turn not found"))?;
    let (st, id) = (state.clone(), id.to_owned());
    blocking(move || {
        st.jobs
            .get(&space, &id)?
            .ok_or_else(|| HostError::not_found("turn not found"))
    })
    .await
}

pub(super) async fn authorize<D: Directory>(
    state: &Shared<D>,
    who: &Identity,
    space: &str,
    required: Capability,
) -> Result<(), HostError> {
    if SpaceId::parse(space).is_none() {
        return Err(HostError::not_found("turn not found"));
    }
    let (st, id, space) = (state.clone(), who.id.clone(), space.to_owned());
    blocking(move || {
        if st
            .tokens
            .capabilities(&id, &space)
            .is_some_and(|c| c.contains(required))
        {
            Ok(())
        } else {
            Err(HostError::not_found("turn not found"))
        }
    })
    .await
}

async fn fresh<D: Directory>(state: &Shared<D>, session: &Session) -> Result<(), HostError> {
    let Session::Installation {
        account,
        installation,
    } = session
    else {
        return Err(HostError::forbidden(
            "sign in again with a passkey to approve access",
        ));
    };
    let service = state
        .tokens
        .http_auth()
        .ok_or_else(|| HostError::forbidden("person session required"))?;
    let account =
        AccountId::parse(account).ok_or_else(|| HostError::forbidden("person session required"))?;
    let installation = InstallationId(
        Uuid::parse_str(installation)
            .map_err(|_| HostError::forbidden("person session required"))?,
    );
    service
        .auth
        .require_fresh(account, installation, false)
        .await
        .map_err(|_| HostError::forbidden("sign in again with a passkey to approve access"))
}

async fn stop_machine(machines: &Arc<MachineService>, job: &Job) -> Result<(), HostError> {
    if let Some(id) = job
        .checkpoint
        .get("lease_id")
        .and_then(Value::as_str)
        .and_then(|v| Uuid::parse_str(v).ok())
    {
        let (svc, account) = (
            machines.clone(),
            AccountId::parse(&job.account).ok_or_else(|| HostError::not_found("turn not found"))?,
        );
        blocking(move || svc.release_run(account, id).map(|_| ())).await?;
    }
    Ok(())
}

fn validate(i: &mut AgentInput, op: &str) -> Result<(), HostError> {
    if i.prompt.len() > 16 * 1024
        || i.model.len() > 128
        || i.source_app.len() > 80
        || i.source_app.contains('.')
        || i.app.len() > 80
    {
        return Err(HostError::invalid_input("agent input exceeds its bounds"));
    }
    if matches!(op, "agent.run" | "agent.check") {
        let invalid = || HostError::invalid_input("check/run needs space_id and thread_id UUIDs");
        let space = SpaceId::parse(&i.space_id).ok_or_else(invalid)?;
        let thread = Uuid::parse_str(&i.thread_id).map_err(|_| invalid())?;
        if !i.job_id.is_empty() {
            return Err(invalid());
        }
        i.space_id = space.to_string();
        i.thread_id = thread.to_string();
        if op == "agent.run" && i.prompt.trim().is_empty() {
            return Err(HostError::invalid_input("run needs a prompt"));
        }
    } else {
        i.job_id = Uuid::parse_str(&i.job_id)
            .map_err(|_| HostError::invalid_input("job_id must be a UUID"))?
            .to_string();
    }
    Ok(())
}

pub(super) async fn enabled_generation<D: Directory>(
    state: &Shared<D>,
    space: &str,
    plugin: &str,
) -> Result<u32, HostError> {
    let (st, key, plugin) = (
        state.clone(),
        state.tokens.content_space(space),
        plugin.to_owned(),
    );
    blocking(move || {
        let plugin = st
            .registry
            .get(&plugin)
            .ok_or_else(|| HostError::unavailable("required plugin is not installed"))?;
        let store = st
            .store
            .lock()
            .map_err(|_| HostError::internal("store poisoned"))?;
        let row = crate::plugins::activation(&store, &key, plugin)?;
        if !row.enabled {
            return Err(HostError::forbidden("required plugin is disabled"));
        }
        Ok(row.generation)
    })
    .await
}
