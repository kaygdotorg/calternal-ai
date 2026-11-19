//! Thread-file admission reuses manual turn validation and durable jobs.
//! Machine memory admission follows durable job creation. A queued job retains
//! its committed message, and its reason comes from the fenced placement loop.
use super::*;
use crate::threads::provider_session::{KEY, ProviderSession};
const ARCHIVED_SEND: &str = "thread_archived";

/// Finish a committed message intent under the content space lock that fences
/// archive and Trash. A matching intent on an archived head gets a durable
/// refusal receipt, so replay cannot start a job after unarchive. No callback
/// re-enters content. Lock order is content then jobs. On a warm path, read
/// settings, the outbox, the head and the receipt. A new decision publishes
/// one FULL journal commit. A fresh accepted send also admits and loads a job.
pub(crate) async fn admit<D: Directory>(
    state: &Shared<D>,
    who: Identity,
    session: Session,
    actor: calternal_types::Actor,
    rid: &str,
    mut input: AgentInput,
    generation: u32,
    intent: Value,
) -> Result<Job, Error> {
    super::input::validate(&mut input, "agent.run")?;
    let defaults = retry::manifest_defaults(state)?;
    let (st, account) = (state.clone(), who.id.to_string());
    let settings = blocking(move || st.jobs.agent_settings(&account, None, defaults)).await?;
    let (st, space, request_key) = (
        state.clone(),
        input.space_id.clone(),
        format!("threads/{}/{rid}", who.id),
    );
    let expected = json!(input);
    let request_id = format!("thread-admit/{rid}");
    let fp = intent["fingerprint"]
        .as_str()
        .ok_or_else(|| Error::internal("missing message fingerprint"))?
        .to_owned();
    let result = blocking(move || {
        let thread = Uuid::parse_str(&input.thread_id)
            .map_err(|_| Error::internal("invalid saved thread"))?;
        let request = calternal_content::Request {
            account: who.id,
            via: actor.via.as_ref(),
            id: &request_id,
            operation: "threads.send",
            fingerprint: &fp,
        };
        st.store.content().admit_item(
            SpaceId::parse(&space).map_err(|_| Error::internal("invalid saved space"))?,
            thread,
            request,
            |current, saved| {
                if current.kind != "threads" || current.metadata["deleted"] == true {
                    return Err(Error::not_found("item not found"));
                }
                if let Some(saved) = saved {
                    return Ok(saved);
                }
                if current.metadata["pending"] != intent {
                    return Err(Error::conflict("the pending send has been superseded"));
                }
                if current.metadata["archived"] == true {
                    // An error here would leave the saved intent unresolved.
                    return Ok(json!({"refused": ARCHIVED_SEND}));
                }
                let resume = ProviderSession::owned(&current.metadata, who.id);
                let id = st.jobs.admit(Admission {
                    space: space.clone(),
                    plugin: crate::plugins::threads_owner().into(),
                    account: who.id,
                    session,
                    operation: "agent.turn".into(),
                    input: json!({
                        "request": input,
                        "check": false,
                        "retry_settings": settings,
                        "thread_request": intent,
                        "check_target": input.machine_id,
                        (KEY): resume,
                    }),
                    parent: None,
                    generation,
                    request_key,
                })?;
                if intent["automation_run"].is_object() {
                    crate::fault::hit(crate::fault::FaultPoint::AutomationAfterJob);
                }
                Ok(json!({"job_id": id}))
            },
        )
    })
    .await?;
    if result["refused"] == ARCHIVED_SEND {
        return Err(Error::conflict(
            "This thread was archived before the turn could start. The message was saved.",
        )
        .with_details(json!({"reason": ARCHIVED_SEND})));
    }
    let id = result["job_id"]
        .as_str()
        .ok_or_else(|| Error::internal("invalid saved job"))?;
    let job = load(state, id).await.map_err(|_| {
        Error::conflict("this turn has expired; inspect the thread and use a new request_id")
    })?;
    if job.input["request"] != expected {
        return Err(Error::request_reused());
    }
    Ok(job)
}

/// Release only the exact job named by a committed thread-stop receipt. Replays
/// cannot stop a later send. Missing jobs are expired no-ops; other failures are
/// retryable. Cost: one job read and the existing bounded machine release commit,
/// entirely on blocking workers. Called after ordinary authorization/dispatch.
pub(crate) async fn finish_stop<D: Directory>(
    state: &Shared<D>,
    machines: &Arc<MachineService>,
    result: &Value,
) -> Result<(), Error> {
    let Some(id) = result["job"]["id"].as_str() else {
        return Ok(());
    };
    let job = match load(state, id).await {
        Ok(job) => job,
        Err(error) if error.code == calternal_types::ErrorCode::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if result["job"]["space_id"] != job.space {
        return Err(Error::internal("invalid stopped job"));
    }
    stop_machine(machines, &job).await
}
