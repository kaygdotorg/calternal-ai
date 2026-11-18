//! Shared provider turns: durable jobs, machine exec streams and explicit resume.
//! The Threads engine persists conversations through content and calls this admission path.
pub mod access;
mod admission;
mod input;
mod instructions;
mod launch;
mod lease;
pub(crate) mod phases;
mod progress;
mod readiness;
pub(crate) mod reason;
mod redact;
pub mod retry;
mod run;
pub(crate) mod space_execution;
mod thread_admission;
mod tool_steps;
mod verify;
pub(crate) use run::schedule_for_account as schedule_thread;
pub(crate) use thread_admission::{admit as admit_thread, finish_stop as finish_thread_stop};

use crate::{
    api::Shared,
    auth::{Directory, Identity},
    machines::MachineService,
    plugins::{
        Session,
        jobs::{Admission, Job},
    },
};
use access::Authority;
use calternal_core::{AgentInput, JobState, JsonObject};
use calternal_types::SpaceId;
use calternal_types::{Capability, Error};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

pub(super) async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    crate::blocking::spawn(f)
        .await
        .map_err(|_| Error::internal("turn task failed"))?
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
    mut input: JsonObject,
) -> Result<Value, Error> {
    if op == "access.request" {
        if let Some(a) = &agent {
            match input.get("job_id") {
                None => {
                    input.insert("job_id".into(), json!(a.job));
                }
                Some(id) if id.as_str() == Some(a.job.as_str()) => {}
                Some(_) => return Err(Error::not_found("turn not found")),
            }
        } else if !input
            .get("job_id")
            .and_then(Value::as_str)
            .is_some_and(|id| !id.is_empty())
        {
            return Err(Error::invalid_input(
                "Supply the turn's job_id for a person access request.",
            ));
        }
    }
    if op == calternal_core::MACHINES_INSTALL {
        if agent.is_some() || matches!(session, Session::Agent { .. }) {
            return Err(Error::forbidden("a person must enroll a machine"));
        }
        if !session.is_live(&state.directory).await {
            return Err(Error::unauthorized("sign in again first"));
        }
        return crate::machines::install::script(state, machines, who.id).await;
    }
    if op == calternal_core::MACHINES_ONBOARDING {
        if agent.is_some() {
            return Err(Error::forbidden("a person must check machine onboarding"));
        }
        return crate::machines::colocated::onboarding(state, machines, who.id).await;
    }
    if space_execution::is_setting(op) {
        if agent.is_some() {
            return Err(Error::forbidden("a person must manage space execution"));
        }
        if !session.is_live(&state.directory).await {
            return Err(Error::unauthorized("sign in again first"));
        }
        return space_execution::dispatch(state, machines, who, op, input).await;
    }
    let verify = op == calternal_core::MACHINES_VERIFY;
    if verify {
        if agent.is_some() {
            return Err(Error::forbidden("a person must verify a machine"));
        }
        calternal_core::validate_operation_input(op, &input)?;
    }
    if let Some(a) = &agent {
        a.check(op, &input)?;
    }
    if op == calternal_core::AGENT_READINESS {
        if agent.is_some() {
            return Err(Error::forbidden("a person must read machine readiness"));
        }
        let target = serde_json::from_value(json!(input))
            .map_err(|_| Error::invalid_input("input does not match readiness schema"))?;
        return readiness::read(state, machines, who, target).await;
    }
    let actor = agent
        .as_ref()
        .map(|scope| scope.actor.clone())
        .unwrap_or_else(|| calternal_types::Actor::account(who.id));
    let mut i: AgentInput = serde_json::from_value(json!(input))
        .map_err(|_| Error::invalid_input("input does not match agent schema"))?;
    if op == "agent.check" && agent.is_some() {
        return Err(Error::forbidden("a person must check provider sign-in"));
    }
    if op == "agent.settings" {
        if agent.is_some() {
            return Err(Error::forbidden(
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
        if job.account != who.id {
            return Err(Error::not_found("turn not found"));
        }
        let value = i
            .retry_settings
            .ok_or_else(|| Error::invalid_input("thread settings require retry_settings"))?;
        let canonical: JsonObject =
            serde_json::from_value(json!(&i)).map_err(|_| Error::internal("settings encoding"))?;
        let space = SpaceId::parse(&job.space).map_err(|_| Error::not_found("turn not found"))?;
        let admission = admission::ManualAdmission::new(
            state.store.shared_content(),
            actor,
            space,
            rid,
            op,
            &canonical,
        );
        let (st, sp, id) = (state.clone(), job.space, job.id);
        return blocking(move || {
            admission.apply(|| {
                st.jobs.agent_thread_settings(&sp, &id, value)?;
                Ok(json!({"retry_settings":value}))
            })
        })
        .await;
    }
    if op == "machines.resume" {
        if agent.is_some() {
            return Err(Error::forbidden("a person must resume interrupted work"));
        }
        let account = who.id;
        let lease = Uuid::parse_str(&i.lease_id)
            .map_err(|_| Error::invalid_input("lease_id must be a UUID"))?;
        let svc = machines.clone();
        let space = blocking(move || {
            svc.run_space(lease)?
                .ok_or_else(|| Error::not_found("run not found"))
        })
        .await?;
        authorize(&state, &who, &space.to_string(), Capability::PluginsUse).await?;
        blocking(move || machines.resume_run(account, lease)).await?;
        return Ok(json!({"lease_id":i.lease_id,"state":"waiting"}));
    }
    if op == "machines.set" {
        if agent.is_some() {
            return Err(Error::forbidden(
                "a person must change machine execution settings",
            ));
        }
        let account = who.id;
        let machine = Uuid::parse_str(&i.machine_id)
            .map_err(|_| Error::invalid_input("machine_id must be a UUID"))?;
        blocking(move || machines.set_direct(account, machine, i.direct)).await?;
        return Ok(json!({"machine_id":i.machine_id,"direct":i.direct}));
    }
    input::validate(&mut i, if verify { "agent.check" } else { op })?;
    let canonical: JsonObject =
        serde_json::from_value(json!(&i)).map_err(|_| Error::internal("turn input encoding"))?;
    if verify || matches!(op, "agent.run" | "agent.check") {
        authorize(&state, &who, &i.space_id, Capability::PluginsUse).await?;
        input::validate_source_app(&state, &i.space_id, &i.source_app).await?;
        let (st, account, request, scope) = (
            state.clone(),
            who.id.to_string(),
            rid.to_owned(),
            i.space_id.clone(),
        );
        let generation = enabled_generation(&state, &i.space_id, "calternal-orchestrator").await?;
        let (st_settings, account_settings) = (state.clone(), who.id.to_string());
        let settings = if verify {
            calternal_core::AgentRetrySettings {
                provider_errors: false,
                subscription_limits: false,
            }
        } else if let Some(settings) = i.retry_settings {
            settings
        } else {
            let defaults = retry::manifest_defaults(&state)?;
            blocking(move || {
                st_settings
                    .jobs
                    .agent_settings(&account_settings, None, defaults)
            })
            .await?
        };
        let check = verify || op == "agent.check";
        let (target, pool_diagnostic) = if verify {
            let (id, pool) = verify::target(&state, &machines, who.id, &i.machine_id).await?;
            (Some(id), pool)
        } else if check {
            (readiness::check_target(&machines, who.id, &i).await?, false)
        } else {
            (None, false)
        };
        let data = json!({"request":i,"check":check,"check_target":target,"verify":verify,"pool_diagnostic":pool_diagnostic,"retry_settings":settings});
        let compare = data["request"].clone();
        let space = SpaceId::parse(&scope).map_err(|_| Error::not_found("space not found"))?;
        let admission = admission::ManualAdmission::new(
            state.store.shared_content(),
            actor,
            space,
            rid,
            op,
            &canonical,
        );
        let id = blocking(move || {
            admission.apply(|| {
                let id = st.jobs.admit(Admission {
                    space: scope,
                    plugin: "calternal-orchestrator".into(),
                    account: who.id,
                    session,
                    operation: "agent.turn".into(),
                    input: data,
                    parent: None,
                    generation,
                    request_key: format!("agent/{account}/{request}"),
                })?;
                Ok(json!({"job_id":id}))
            })
        })
        .await?["job_id"]
            .as_str()
            .ok_or_else(|| Error::internal("turn admission result"))?
            .to_owned();
        let job = load(&state, &id).await?;
        if job.input["request"] != compare {
            return Err(Error::conflict(
                "request_id already used with different input",
            ));
        }
        run::schedule(&state, &machines, job.space.clone(), id.clone());
        if verify {
            return verify::outcome(&state, &machines, &who, job).await;
        }
        return Ok(json!({"job":job.view_for(who.id)}));
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
    if job.operation != "agent.turn"
        || job.input["verify"] == true
        || (job.input["check"] == true && job.account != who.id)
    {
        return Err(Error::not_found("turn not found"));
    }
    let is_owner = job.account == who.id;
    if op != "agent.events" && !is_owner {
        let (st, account, space) = (
            state.clone(),
            who.id,
            SpaceId::parse(&job.space).map_err(|_| Error::not_found("turn not found"))?,
        );
        let manager = blocking(move || {
            Ok(st
                .directory
                .capabilities(account, space)
                .is_some_and(|c| c.contains(Capability::PluginsManage)))
        })
        .await?;
        if op != "agent.stop" || !manager {
            return Err(Error::not_found("turn not found"));
        }
    }
    let space = SpaceId::parse(&job.space).map_err(|_| Error::not_found("turn not found"))?;
    let admission = admission::ManualAdmission::new(
        state.store.shared_content(),
        actor,
        space,
        rid,
        op,
        &canonical,
    );
    match op {
        "agent.events" => {
            let batch = state.jobs.live(&job.id).read(i.after);
            let reset = (!i.epoch.is_empty() && i.epoch != batch.epoch) || i.after < batch.oldest;
            let events:Vec<Value> = batch.chunks.iter().map(|c| {
                let data = serde_json::from_str(&c.data).unwrap_or(Value::Null);
                json!({"seq":c.seq,"data":if is_owner { data } else { reason::public_event(data) }})
            }).collect();
            let next = batch.chunks.last().map_or(i.after, |c| c.seq + 1);
            Ok(
                json!({"job":job.view_for(who.id),"epoch":batch.epoch,"events":events,"next":next,"reset":reset}),
            )
        }
        "agent.discard" => {
            if !job.state.is_final() && job.state != JobState::Interrupted {
                return Err(Error::conflict(
                    "stop this turn before discarding its workspace",
                ));
            }
            let (st, sp, id) = (state.clone(), job.space.clone(), job.id.clone());
            blocking(move || {
                admission.apply(|| {
                    st.jobs.cancel(&sp, &id)?;
                    st.jobs.agent_revoke(&sp, &id)?;
                    Ok(Value::Null)
                })
            })
            .await?;
            let account = job.account;
            let lease = job.checkpoint["lease_id"]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok());
            let status = if let Some(lease) = lease {
                blocking(move || machines.discard_run(account, lease, i.retry_failed)).await?
            } else {
                "done".into()
            };
            Ok(json!({"job":load(&state,&job.id).await?.view_for(who.id),"discard":status}))
        }
        "agent.stop" => {
            let (st, sp, id) = (state.clone(), job.space.clone(), job.id.clone());
            let (svc, ending) = (machines.clone(), job.clone());
            blocking(move || {
                admission.apply(|| {
                    st.jobs.cancel(&sp, &id)?;
                    st.jobs.agent_revoke(&sp, &id)?;
                    st.inputs.invalidate(ending.account)?;
                    release(&svc, &ending)?;
                    Ok(Value::Null)
                })
            })
            .await?;
            Ok(json!({"job":load(&state,&job.id).await?.view_for(who.id)}))
        }
        "agent.resume" => {
            let (st, sp, id, svc) = (
                state.clone(),
                job.space.clone(),
                job.id.clone(),
                machines.clone(),
            );
            blocking(move || {
                admission.apply(|| {
                    let current = st
                        .jobs
                        .get(&sp, &id)?
                        .ok_or_else(|| Error::not_found("turn not found"))?;
                    if current.state != JobState::Interrupted {
                        return Err(Error::conflict(
                            "only an interrupted turn can resume; inspect its checkpoint first",
                        ));
                    }
                    release(&svc, &current)?;
                    st.jobs.agent_resume(&sp, &id)?;
                    Ok(Value::Null)
                })
            })
            .await?;
            run::schedule(&state, &machines, job.space.clone(), job.id.clone());
            Ok(json!({"job":load(&state,&job.id).await?.view_for(who.id)}))
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
                admission.apply(|| {
                    let a = st.jobs.agent_scope(&sp, &id)?;
                    let create = || st.jobs.agent_request(&a, &app, write);
                    let id = crate::requests::park(&st, &machines, &a, create)?;
                    st.inputs.invalidate(a.actor.account)?;
                    Ok(json!({"approval_id":id}))
                })
            })
            .await?["approval_id"]
                .as_str()
                .ok_or_else(|| Error::internal("approval result"))?
                .to_owned();
            Ok(json!({"approval_id":id,"status":"pending","scope":"pending person decision"}))
        }
        "agent.mode" => {
            if agent.is_some() {
                return Err(Error::forbidden(
                    "a freshly verified person must change agent access",
                ));
            }
            fresh(&state, &session).await?;
            if job.state.is_final() {
                return Err(Error::conflict("finished turns cannot change access"));
            }
            // Fence the job before changing its protected credential.
            let (st, sp, id) = (state.clone(), job.space.clone(), job.id.clone());
            let (svc, ending) = (machines.clone(), job.clone());
            blocking(move || {
                admission.apply(|| {
                    let mut a = st.jobs.agent_scope(&sp, &id)?;
                    a.mode = i.mode;
                    st.jobs.agent_interrupt(&sp, &id)?;
                    st.jobs.agent_scope_change(&a)?;
                    release(&svc, &ending)?;
                    Ok(Value::Null)
                })
            })
            .await?;
            Ok(
                json!({"job":load(&state,&job.id).await?.view_for(who.id),"hint":"Resume explicitly to deliver the replacement token."}),
            )
        }
        _ => Err(Error::not_found("unknown agent operation")),
    }
}

async fn load<D: Directory>(state: &Shared<D>, id: &str) -> Result<Job, Error> {
    let space = state
        .jobs
        .route(id)
        .ok_or_else(|| Error::not_found("turn not found"))?;
    let (st, id) = (state.clone(), id.to_owned());
    blocking(move || {
        st.jobs
            .get(&space, &id)?
            .ok_or_else(|| Error::not_found("turn not found"))
    })
    .await
}

pub(super) async fn authorize<D: Directory>(
    state: &Shared<D>,
    who: &Identity,
    space: &str,
    required: Capability,
) -> Result<(), Error> {
    let (st, id, space) = (
        state.clone(),
        who.id,
        SpaceId::parse(space).map_err(|_| Error::not_found("turn not found"))?,
    );
    blocking(move || {
        if st
            .directory
            .capabilities(id, space)
            .is_some_and(|c| c.contains(required))
        {
            Ok(())
        } else {
            Err(Error::not_found("turn not found"))
        }
    })
    .await
}

pub(crate) async fn fresh<D: Directory>(state: &Shared<D>, session: &Session) -> Result<(), Error> {
    let Session::Installation {
        account,
        installation,
    } = session
    else {
        return Err(Error::passkey_reverification_required());
    };
    let service = state
        .directory
        .http_auth()
        .ok_or_else(|| Error::forbidden("person session required"))?;
    service
        .auth
        .require_fresh(*account, *installation, false)
        .await
        .map_err(crate::account_http::fresh_error)
}

pub(crate) fn release(machines: &MachineService, job: &Job) -> Result<(), Error> {
    if job.state == JobState::Waiting && job.checkpoint["person_request"].is_string() {
        return Ok(());
    }
    if let Some(lease) = job.checkpoint["lease_id"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        if job.input["check"] == true {
            machines.release_probe(job.account, lease)?;
        } else {
            machines.release_run(job.account, lease)?;
        }
    }
    Ok(())
}

async fn stop_machine(machines: &Arc<MachineService>, job: &Job) -> Result<(), Error> {
    if let Some(id) = job
        .checkpoint
        .get("lease_id")
        .and_then(Value::as_str)
        .and_then(|v| Uuid::parse_str(v).ok())
    {
        let (svc, account) = (machines.clone(), job.account);
        let check = job.input["check"] == true;
        blocking(move || {
            if check {
                svc.release_probe(account, id)
            } else {
                svc.release_run(account, id)
            }
        })
        .await?;
    }
    Ok(())
}

pub(crate) async fn enabled_generation<D: Directory>(
    state: &Shared<D>,
    space: &str,
    plugin: &str,
) -> Result<u32, Error> {
    let (st, key, plugin) = (
        state.clone(),
        SpaceId::parse(space).map_err(|_| Error::not_found("turn not found"))?,
        plugin.to_owned(),
    );
    blocking(move || {
        let catalog = st.registry.snapshot();
        let plugin = catalog
            .get(&plugin)
            .ok_or_else(|| Error::unavailable("required plugin is not installed"))?;
        let store = &st.store;
        let row = crate::plugins::activation(store, key, plugin)?;
        if !row.enabled {
            return Err(Error::forbidden("required plugin is disabled"));
        }
        Ok(row.generation)
    })
    .await
}

/// The job keeps its admitting proof; token use and execution share this check.
pub(super) async fn session_live<D: Directory>(
    state: &Shared<D>,
    job: &Job,
) -> Result<bool, Error> {
    let (st, account, session) = (state.clone(), job.account, job.session.clone());
    blocking(move || st.jobs.admitting_session_live(account, &session)).await
}
