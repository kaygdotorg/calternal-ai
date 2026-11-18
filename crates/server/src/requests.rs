//! The account's unified request inbox and its trusted resolution operations.
use crate::{
    api::{Shared, fingerprint},
    auth::{Directory, Identity},
    machines::MachineService,
    plugins::Session,
    turns::access::Authority,
};
use calternal_core::{
    ApprovalLifetime, JsonObject, PersonRequest, RequestAnswerInput, RequestApproveInput,
    RequestAskInput, RequestDenyInput, RequestDetail, RequestOperation, RequestResolution,
    RequestTarget, RequestsListResult, SpaceInstructionsInput, SpaceInstructionsResult,
};
use calternal_secrets::{Owner, SecretStore};
use calternal_types::{AccountId, Capability, Error, SpaceId};
use serde_json::{Value, json};
use std::sync::Arc;

fn decode<T: serde::de::DeserializeOwned>(input: JsonObject) -> Result<T, Error> {
    serde_json::from_value(Value::Object(input.into_iter().collect()))
        .map_err(|_| Error::invalid_input("input does not match operation schema"))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn dispatch<D: Directory>(
    state: Shared<D>,
    machines: Option<Arc<MachineService>>,
    person: Identity,
    session: Session,
    agent: Option<Authority>,
    rid: &str,
    row: &'static RequestOperation,
    input: JsonObject,
) -> Result<Value, Error> {
    if !matches!(row.target, RequestTarget::Turn)
        && (agent.is_some() || matches!(session, Session::Agent { .. }))
    {
        return Err(Error::forbidden(
            "thread credentials cannot manage personal requests or standing instructions",
        ));
    }
    if matches!(session, Session::Installation { account, .. } if account != person.id) {
        return Err(Error::unauthorized("sign in again first"));
    }
    if row.id == "requests.approve" {
        crate::turns::fresh(&state, &session).await?;
    }
    if !matches!(session, Session::Agent { .. }) && !session.is_live(&state.directory).await {
        return Err(Error::unauthorized("sign in again first"));
    }
    if row.id.starts_with("provider.sign_in.") {
        let machines = machines.ok_or_else(|| Error::unavailable("machines are not enabled"))?;
        return crate::provider_sign_in::dispatch(state, machines, person, session, row.id, input)
            .await;
    }
    let rid = rid.to_owned();
    let scheduler = machines.clone();
    let st = state.clone();
    let runtime = tokio::runtime::Handle::current();
    let (result, resume) = crate::blocking::spawn(move || {
        let mut resume = None;
        let result = match row.id {
            "requests.list" => {
                let _: calternal_core::AccountInputList = decode(input)?;
                json!(list(&st, &person)?)
            }
            "requests.ask" => {
                let a = agent
                    .ok_or_else(|| Error::forbidden("a current thread credential is required"))?;
                let Session::Agent { token_hash, .. } = session else {
                    return Err(Error::forbidden("a current thread credential is required"));
                };
                authorize(&st, person.id, &a.space, row.required)?;
                st.jobs
                    .agent_current(&a.space, &a.job, &token_hash)?
                    .ok_or_else(|| Error::forbidden("thread credential has ended"))?;
                let i: RequestAskInput = decode(input)?;
                let label = question_label(&i.form)?;
                let machines =
                    machines.ok_or_else(|| Error::unavailable("machines are not enabled"))?;
                let id = park(&st, &machines, &a, || {
                    st.jobs.agent_wait_request(
                        &a,
                        "question",
                        "",
                        false,
                        Some(crate::store::sha256_hex(rid.as_bytes())),
                        &json!(RequestDetail::Question {
                            label,
                            form: i.form
                        }),
                    )
                })?;
                st.inputs.invalidate(person.id)?;
                json!(RequestResolution {
                    id,
                    status: "pending".into()
                })
            }
            "plugins.diagnostics" => {
                json!(crate::plugins::installed_diagnostics::read(
                    &st,
                    person.id,
                    decode(input)?
                )?)
            }
            "plugins.network.content.get" => {
                let i: calternal_core::ContentFrameAddress = decode(input)?;
                let space = authorize(&st, person.id, &i.space_id, row.required)?;
                json!(crate::plugins::network::view(
                    &st,
                    person.id,
                    space,
                    &i.frame_id
                )?)
            }
            "plugins.network.content.preference.get" => {
                let i: calternal_core::ContentFetchPreferenceAddress = decode(input)?;
                let space = authorize(&st, person.id, &i.space_id, row.required)?;
                let lifecycle = st.store.content().lifecycle();
                let _space = lifecycle.step(space.as_uuid())?;
                let _account = lifecycle.step(person.id.as_uuid())?;
                json!(crate::plugins::network::db::preference(
                    st.store.appdata(),
                    space,
                    person.id,
                )?)
            }
            "plugins.network.content.preference.set" => {
                let i: calternal_core::ContentFetchPreferenceEdit = decode(input)?;
                let space = authorize(&st, person.id, &i.space_id, row.required)?;
                let lifecycle = st.store.content().lifecycle();
                let _space = lifecycle.step(space.as_uuid())?;
                let _account = lifecycle.step(person.id.as_uuid())?;
                crate::plugins::network::db::set_preference(
                    st.store.appdata(),
                    space,
                    person.id,
                    i.preference,
                )?;
                json!(i.preference)
            }
            "plugins.personal_settings.get" | "plugins.personal_settings.set" => {
                crate::plugins::settings::personal(&st, person.id, &rid, row, input)?
            }
            "plugins.settings.get" | "plugins.settings.set" => {
                crate::plugins::settings::dispatch(&st, person.id, &rid, row.id, input)?
            }
            "space.instructions" => {
                let fp = fingerprint(row.id, &input);
                let i: SpaceInstructionsInput = decode(input)?;
                let space = authorize(&st, person.id, &i.space_id, row.required)?;
                let content = st.store.content();
                let markdown = match i.markdown {
                    Some(markdown) => {
                        content.replace_standing_instructions(
                            space,
                            calternal_content::Request {
                                account: person.id,
                                via: None,
                                id: &rid,
                                operation: row.id,
                                fingerprint: &fp,
                            },
                            &markdown,
                        )?;
                        markdown
                    }
                    None => content.standing_instructions(space)?,
                };
                authorize(&st, person.id, &i.space_id, row.required)?;
                json!(SpaceInstructionsResult { markdown })
            }
            _ => {
                let decision_fingerprint = fingerprint(row.id, &input);
                let (id, status, value) = match row.id {
                    "requests.answer" => {
                        let i: RequestAnswerInput = decode(input)?;
                        (i.id, "answered", Some(i.answer))
                    }
                    "requests.approve" => {
                        let i: RequestApproveInput = decode(input)?;
                        let status = match i.lifetime {
                            ApprovalLifetime::Once => "once",
                            ApprovalLifetime::ThisThread => "this_thread",
                        };
                        (i.id, status, None)
                    }
                    "requests.deny" => {
                        let i: RequestDenyInput = decode(input)?;
                        (i.id, "denied", None)
                    }
                    _ => return Err(Error::not_found("operation not found")),
                };
                if let Some(result) = crate::plugins::signals::inbox(
                    &st,
                    &person,
                    &id,
                    row.required,
                    &rid,
                    &decision_fingerprint,
                    status,
                    value.as_ref(),
                    &runtime,
                )? {
                    return Ok((json!(result), None));
                }
                let (space, job_id) =
                    st.jobs
                        .agent_decide(person.id, &id, &rid, status, value, |space| {
                            authorize(&st, person.id, space, row.required)
                                .map(|_| ())
                                .map_err(|_| Error::not_found("request not found"))
                        })?;
                st.inputs.invalidate(person.id)?;
                if machines.is_some()
                    && let Some(job) = st.jobs.get(&space, &job_id)?
                {
                    if job.state == calternal_core::JobState::Waiting
                        && job.checkpoint["person_request"].as_str() == Some(&id)
                    {
                        st.jobs.agent_wake_request(&space, &job_id, &id)?;
                    }
                    if st
                        .jobs
                        .get(&space, &job_id)?
                        .is_some_and(|j| j.state == calternal_core::JobState::Admitted)
                    {
                        resume = Some((space, job_id));
                    }
                }
                let status = if value_is_approval(status) {
                    "approved"
                } else {
                    status
                };
                json!(RequestResolution {
                    id,
                    status: status.into()
                })
            }
        };
        Ok::<_, Error>((result, resume))
    })
    .await
    .map_err(|_| Error::internal("request task failed"))??;
    if let (Some(machines), Some((space, job))) = (scheduler, resume) {
        crate::turns::schedule_thread(&state, &machines, space, job, person.id);
    }
    Ok(result)
}

/// Validate the Inbox's action-free question form and return its display label.
pub(crate) fn question_label(form: &calternal_core::GenUiView) -> Result<String, Error> {
    calternal_core::validate_genui(form)?;
    let limits = calternal_genui::Budgets::default();
    let program = calternal_genui::compile(&form.source, limits)
        .program
        .ok_or_else(|| Error::invalid_input("question form has GenUI diagnostics"))?;
    let evaluation = calternal_genui::evaluate(&program, None, limits);
    let [root] = evaluation.roots.as_slice() else {
        return Err(Error::invalid_input("question must contain one Form"));
    };
    if root["type"] != "Form" || !evaluation.diagnostics.is_empty() {
        return Err(Error::invalid_input("question must contain one valid Form"));
    }
    let mut pending = vec![root];
    while let Some(value) = pending.pop() {
        match value {
            Value::Array(items) => pending.extend(items),
            Value::Object(fields) => {
                if value["kind"] == "action" {
                    return Err(Error::invalid_input("question form cannot contain actions"));
                }
                pending.extend(fields.values());
            }
            _ => (),
        }
    }
    root["props"]["label"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| Error::invalid_input("question form needs a label"))
}

fn value_is_approval(status: &str) -> bool {
    matches!(status, "once" | "this_thread")
}

/// Check a live capability in the caller-selected UUID space and hide foreign spaces.
pub(crate) fn authorize<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    space: &str,
    required: Capability,
) -> Result<SpaceId, Error> {
    let space = SpaceId::parse(space).map_err(|_| Error::not_found("space not found"))?;
    let caps = state
        .directory
        .capabilities(account, space)
        .ok_or_else(|| Error::not_found("space not found"))?;
    if !caps.contains(required) {
        return Err(Error::forbidden(
            "this action requires the space capability",
        ));
    }
    Ok(space)
}

fn list<D: Directory>(state: &Shared<D>, person: &Identity) -> Result<RequestsListResult, Error> {
    let mut requests = state.jobs.personal_requests(person.id)?;
    requests.retain(|row| {
        row.space_id
            .as_deref()
            .is_some_and(|space| authorize(state, person.id, space, Capability::SpaceRead).is_ok())
    });
    let secrets = state.store.secrets();
    let collect = |requests: &mut Vec<PersonRequest>,
                   owner: Owner,
                   space_id: Option<String>|
     -> Result<(), Error> {
        for r in secrets
            .pending(&owner)
            .map_err(|_| Error::unavailable("secret requests unavailable"))?
        {
            let Some((recipient, thread)) = crate::secrets::request_actor(&r.requested_by) else {
                continue;
            };
            if recipient != person.id {
                continue;
            }
            requests.push(PersonRequest {
                id: r.id.to_string(),
                space_id: space_id.clone(),
                thread_id: thread.map(|id| id.to_string()),
                job_id: None,
                detail: RequestDetail::Secret {
                    name: r.name,
                    label: r.label,
                    reason: r.reason,
                    expires_at_ms: r.created_at + calternal_secrets::REQUEST_TTL_MS,
                },
            });
        }
        Ok(())
    };
    collect(&mut requests, Owner::Account(person.id), None)?;
    for space in state.directory.describe(person).spaces {
        if authorize(state, person.id, &space.id, Capability::SpaceRead).is_ok() {
            requests.extend(state.jobs.wait_requests(&space.id, person.id)?);
        }
        if let Ok(id) = authorize(state, person.id, &space.id, Capability::SecretsManage) {
            collect(&mut requests, Owner::Space(id), Some(space.id))?;
        }
    }
    requests.extend(crate::provider_sign_in::list(state, person.id)?);
    requests.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    Ok(RequestsListResult { requests })
}

/// Durably park the turn and fence its current machine lease. The wait stores no
/// process claim. One per-space job transaction spans the protected request
/// insert and lease release to keep concurrent decision/cancellation ordered.
pub(crate) fn park<D: Directory>(
    state: &Shared<D>,
    machines: &MachineService,
    a: &Authority,
    create: impl FnOnce() -> Result<String, Error>,
) -> Result<String, Error> {
    let job = state
        .jobs
        .get(&a.space, &a.job)?
        .ok_or_else(|| Error::not_found("turn not found"))?;
    state.jobs.agent_park_request(&job, || {
        let id = create()?;
        crate::turns::release(machines, &job)?;
        Ok(id)
    })
}
