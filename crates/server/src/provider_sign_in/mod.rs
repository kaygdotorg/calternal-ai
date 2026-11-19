//! Account-only provider sign-in. Provider prompts and answers stay in memory.
mod flow;
mod inbox;
pub(crate) use inbox::Inbox;

use calternal_core::{
    JsonObject, PersonRequest, ProviderSignInAnswer, ProviderSignInResult, ProviderSignInStart,
    ProviderSignInStatus, ProviderSignInTyping,
};
use calternal_types::{AccountId, Capability, Error, SpaceId};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::machines::space_execution::SpaceExecution;
use crate::{
    api::Shared,
    auth::{Directory, Identity},
    machines::MachineService,
    plugins::{Session, jobs::Job},
};
use inbox::Target;

fn decode<T: serde::de::DeserializeOwned>(input: JsonObject) -> Result<T, Error> {
    serde_json::from_value(json!(input)).map_err(|_| Error::invalid_input("invalid sign-in input"))
}

fn missing() -> Error {
    Error::not_found("provider sign-in not found")
}

fn uuid(raw: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(raw).map_err(|_| missing())
}

fn space(raw: &str) -> Result<SpaceId, Error> {
    SpaceId::parse(raw).map_err(|_| missing())
}

fn allowed<D: Directory>(state: &Shared<D>, account: AccountId, space: SpaceId) -> bool {
    state
        .directory
        .capabilities(account, space)
        .is_some_and(|caps| caps.contains(Capability::PluginsUse))
}

async fn authorize<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    space: SpaceId,
) -> Result<(), Error> {
    let state = state.clone();
    blocking(move || {
        allowed(&state, account, space)
            .then_some(())
            .ok_or_else(missing)
    })
    .await
}

async fn blocking<T: Send + 'static>(
    task: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    crate::blocking::spawn(task)
        .await
        .map_err(|_| Error::internal("provider sign-in task failed"))?
}

pub(crate) fn list<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
) -> Result<Vec<PersonRequest>, Error> {
    let pending = state.inputs.sign_ins.access(account, |entries| {
        Ok(entries
            .iter()
            .filter_map(|entry| {
                Some((
                    entry.target.space,
                    entry.target.shared.is_some(),
                    entry.request()?,
                ))
            })
            .collect::<Vec<_>>())
    })?;
    Ok(pending
        .into_iter()
        .filter(|(space, shared, _)| {
            allowed(state, account, *space)
                && (!shared || can_manage_shared(state, account, *space))
        })
        .map(|(_, _, prompt)| prompt)
        .collect())
}

fn can_manage_shared<D: Directory>(state: &Shared<D>, account: AccountId, space: SpaceId) -> bool {
    state
        .directory
        .capabilities(account, space)
        .is_some_and(|grants| grants.contains(Capability::SpaceSettings))
}

fn shared_target<D: Directory>(
    state: &Shared<D>,
    machines: &MachineService,
    account: AccountId,
    space: SpaceId,
) -> Result<SpaceExecution, Error> {
    if !can_manage_shared(state, account, space) {
        return Err(missing());
    }
    crate::turns::space_execution::resolve(state, machines, space)?
        .ok_or_else(|| Error::conflict("configure a shared space machine first"))
}

fn result(id: Uuid, status: &str) -> Value {
    json!(ProviderSignInResult {
        id: id.to_string(),
        status: status.into()
    })
}

pub(crate) async fn dispatch<D: Directory>(
    state: Shared<D>,
    machines: Arc<MachineService>,
    who: Identity,
    session: Session,
    operation: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let account = who.id;
    match operation {
        "provider.sign_in.start" => {
            let i: ProviderSignInStart = decode(input)?;
            if (i.shared && (i.pool || i.machine_id.is_some()))
                || (i.pool && i.machine_id.is_some())
            {
                return Err(Error::invalid_input(
                    "Select a personal machine or the pool.",
                ));
            }
            let target_space = space(&i.space_id)?;
            authorize(&state, account, target_space).await?;
            let thread = i.thread_id.as_deref().map(uuid).transpose()?;
            let pin = i.machine_id.as_deref().map(uuid).transpose()?;
            let pool = if i.pool {
                Some(true)
            } else {
                pin.map(|_| false)
            };
            let m = machines.clone();
            let st = state.clone();
            let (machine, shared) = blocking(move || {
                if i.shared {
                    let binding = shared_target(&st, &m, account, target_space)?;
                    Ok((binding.machine, Some(binding)))
                } else {
                    Ok((m.sign_in_target(account, pin, pool)?, None))
                }
            })
            .await?;
            start(
                state,
                machines,
                account,
                session,
                target_space,
                thread,
                machine,
                i.provider_id,
                shared,
                false,
                None,
            )
            .await
        }
        "provider.sign_in.typing" => {
            let i: ProviderSignInTyping = decode(input)?;
            typing(
                state,
                machines,
                account,
                session,
                space(&i.space_id)?,
                uuid(&i.thread_id)?,
            )
            .await
        }
        "provider.sign_in.answer" | "provider.sign_in.status" => {
            let (id, answer) = if operation.ends_with(".answer") {
                let input: ProviderSignInAnswer = decode(input)?;
                if input.code.is_empty()
                    || input.code.len() > 2048
                    || input.code.chars().any(char::is_control)
                {
                    return Err(Error::invalid_input(
                        "Paste one provider authorization code.",
                    ));
                }
                (uuid(&input.id)?, Some(input.code))
            } else {
                let input: ProviderSignInStatus = decode(input)?;
                (uuid(&input.id)?, None)
            };
            let (target_space, shared) = state.inputs.sign_ins.access(account, |entries| {
                entries
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| (e.target.space, e.target.shared))
                    .ok_or_else(missing)
            })?;
            authorize(&state, account, target_space).await?;
            if let Some(binding) = shared {
                let state = state.clone();
                blocking(move || {
                    if shared_target(&state, &machines, account, target_space)? != binding {
                        return Err(missing());
                    }
                    Ok(())
                })
                .await?;
            }
            state.inputs.sign_ins.access(account, |entries| {
                let entry = entries
                    .iter_mut()
                    .find(|e| e.id == id)
                    .ok_or_else(missing)?;
                if let Some(code) = answer {
                    if !entry.prompt.as_ref().is_some_and(|p| p.needs_code) {
                        return Err(Error::conflict(
                            "This provider sign-in is not accepting a code.",
                        ));
                    }
                    entry.answer.try_send(code).map_err(|_| missing())?;
                    entry.prompt = None;
                }
                Ok(result(id, entry.status()))
            })
        }
        _ => Err(missing()),
    }
}

#[allow(clippy::too_many_arguments)]
async fn start<D: Directory>(
    state: Shared<D>,
    machines: Arc<MachineService>,
    account: AccountId,
    session: Session,
    space: SpaceId,
    thread: Option<Uuid>,
    machine: Uuid,
    provider: String,
    shared: Option<SpaceExecution>,
    retry: bool,
    job: Option<String>,
) -> Result<Value, Error> {
    let catalog = state.registry.snapshot();
    let plugin = catalog.get(&provider).ok_or_else(missing)?;
    let metadata = plugin
        .manifest
        .provider
        .as_ref()
        .filter(|p| p.sign_in)
        .ok_or_else(missing)?;
    let executable = if shared.is_some() {
        if provider != "calternal-claude-code" {
            return Err(Error::invalid_input(
                "shared sign-in currently uses Claude Code",
            ));
        }
        crate::turns::space_execution::SHARED_ADAPTER.into()
    } else {
        metadata.executable.clone()
    };
    let label = plugin.manifest.name.clone();
    let s = state.clone();
    let name = provider.clone();
    let policy = blocking(move || {
        if !allowed(&s, account, space) {
            return Err(missing());
        }
        if let Some(thread) = thread {
            let item = s.store.content().get(space, thread)?;
            if item.kind != "threads" || item.metadata["deleted"] == true {
                return Err(missing());
            }
        }
        let catalog = s.registry.snapshot();
        let plugin = catalog.get(&name).ok_or_else(missing)?;
        if !crate::plugins::activation(&s.store, space, plugin)?.enabled {
            return Err(missing());
        }
        crate::plugins::package_store::provider_runtime::SignInPolicy::read(
            s.store.appdata(),
            &name,
        )
        .map_err(|_| Error::unavailable("Provider sign-in registration could not be read."))?
        .ok_or_else(|| Error::conflict("Provider sign-in is not configured."))
    })
    .await?;
    let target = Target {
        space,
        thread,
        machine,
        provider,
        label,
        login_scope: calternal_machine::protocol::ProviderLoginScope::Account(account),
        policy,
        retry,
        shared,
    };
    let (id, answer) = state.inputs.sign_ins.insert(account, target.clone(), job)?;
    if let Some(answer) = answer {
        tokio::spawn(flow::run(
            state, machines, account, session, target, id, executable, answer,
        ));
    }
    Ok(result(id, "pending"))
}

async fn typing<D: Directory>(
    state: Shared<D>,
    machines: Arc<MachineService>,
    account: AccountId,
    session: Session,
    space: SpaceId,
    thread: Uuid,
) -> Result<Value, Error> {
    authorize(&state, account, space).await?;
    let s = state.clone();
    let m = machines.clone();
    let (job, pin, configured) = blocking(move || {
        let (pin, _) = crate::threads::index::readiness_context(&s, space, Some(thread), account)?;
        Ok((
            s.jobs
                .thread_latest(&space.to_string(), &thread.to_string())?
                .filter(|job| job.account == account),
            pin,
            m.space_execution(space)?.is_some(),
        ))
    })
    .await?;
    if configured
        && !job
            .as_ref()
            .is_some_and(|job| job.input["request"]["personal_machine"] == true)
    {
        return Ok(Value::Null);
    }
    if let Some(job) = &job
        && job.state == calternal_core::JobState::Failed
        && job.checkpoint["error"]["class"] == "authentication"
    {
        return offer_failed(state, machines, session, job).await;
    }
    let catalog = state.registry.snapshot();
    let provider = job
        .as_ref()
        .and_then(|job| job.input["request"]["provider_id"].as_str())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| catalog.default_provider())
        .to_owned();
    let svc = machines.clone();
    let name = provider.clone();
    let Some(machine) = blocking(move || svc.signed_out_target(account, pin, &name)).await? else {
        return Ok(Value::Null);
    };
    start(
        state,
        machines,
        account,
        session,
        space,
        Some(thread),
        machine,
        provider,
        None,
        true,
        None,
    )
    .await
}

/// Offer authentication for a failed turn using the exact machine that ran it.
pub(crate) async fn offer_failed<D: Directory>(
    state: Shared<D>,
    machines: Arc<MachineService>,
    session: Session,
    job: &Job,
) -> Result<Value, Error> {
    if job.state != calternal_core::JobState::Failed
        || !job.checkpoint["shared_sponsor"].is_null()
        || job.checkpoint["error"]["class"] != "authentication"
        || !session.is_live_in(&state).await
    {
        return Ok(Value::Null);
    }
    let provider = job.checkpoint["error"]["provider"]
        .as_str()
        .ok_or_else(missing)?
        .to_owned();
    let machine = job.checkpoint["machine_id"]
        .as_str()
        .map(uuid)
        .transpose()?
        .ok_or_else(missing)?;
    let thread = job
        .input
        .get("thread_request")
        .and_then(|_| job.input["request"]["thread_id"].as_str())
        .map(uuid)
        .transpose()?;
    let account = job.account;
    let m = machines.clone();
    blocking(move || m.sign_in_target(account, Some(machine), None)).await?;
    start(
        state,
        machines,
        account,
        session,
        space(&job.space)?,
        thread,
        machine,
        provider,
        None,
        true,
        Some(job.id.clone()),
    )
    .await
}

/// Offer provider authentication once after an official status check observes sign-out.
pub(crate) async fn offer_check<D: Directory>(
    state: Shared<D>,
    machines: Arc<MachineService>,
    job: &Job,
    machine: Uuid,
    provider: String,
) -> Result<(), Error> {
    if !job.session.is_live_in(&state).await {
        return Ok(());
    }
    let account = job.account;
    let svc = machines.clone();
    let name = provider.clone();
    if blocking(move || {
        svc.sign_in_target(account, Some(machine), None)?;
        svc.claim_sign_in_offer(account, machine, &name)
    })
    .await?
    {
        start(
            state,
            machines,
            account,
            job.session.clone(),
            space(&job.space)?,
            None,
            machine,
            provider,
            None,
            true,
            None,
        )
        .await?;
    }
    Ok(())
}
