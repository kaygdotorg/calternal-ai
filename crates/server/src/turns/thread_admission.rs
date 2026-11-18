//! Thread-file admission reuses manual turn validation and durable jobs.
use super::*;
use crate::{
    plugins::jobs::thread_send_key,
    threads::provider_session::{KEY, ProviderSession},
};
/// Finish a committed message intent under its content space lock, fencing Trash.
/// The job key and admission receipt survive a crash between stores. No callback
/// re-enters content; lock order is content -> jobs. Cost: settings read, current
/// head and receipt reads, one FULL commit for the job and receipt, and a job read.
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
        thread_send_key(who.id, rid),
    );
    let expected = json!(input);
    let request_id = format!("thread-admit/{rid}");
    let fp = intent["fingerprint"]
        .as_str()
        .ok_or_else(|| Error::internal("missing message fingerprint"))?
        .to_owned();
    let id = blocking(move || {
        let thread = Uuid::parse_str(&input.thread_id)
            .map_err(|_| Error::internal("invalid saved thread"))?;
        let space_id =
            SpaceId::parse(&space).map_err(|_| Error::internal("invalid saved space"))?;
        let request = calternal_content::Request {
            account: who.id,
            via: actor.via.as_ref(),
            id: &request_id,
            operation: "threads.send",
            fingerprint: &fp,
        };
        st.store
            .content()
            .admit_summary(space_id, thread, request, |current, legacy| {
                if current.kind != "threads" || current.metadata["deleted"] == true {
                    return Err(Error::not_found("item not found"));
                }
                if let Some(saved) = legacy {
                    return saved["job_id"]
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| Error::internal("invalid saved job"));
                }
                if let Some(id) = st.jobs.thread_admission(&space, &request_key)? {
                    return Ok(id);
                }
                if current.metadata["pending"] != intent {
                    return Err(Error::conflict("the pending send has been superseded"));
                }
                let resume = ProviderSession::owned(&current.metadata, who.id);
                st.jobs.admit_thread(Admission {
                    space,
                    plugin: "calternal-orchestrator".into(),
                    account: who.id,
                    session,
                    operation: "agent.turn".into(),
                    input: json!({
                        "request":input, "check":false, "retry_settings":settings,
                        "thread_request":intent, (KEY):resume,
                    }),
                    parent: None,
                    generation,
                    request_key,
                })
            })
    })
    .await?;
    let job = load(state, &id).await.map_err(|_| {
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
