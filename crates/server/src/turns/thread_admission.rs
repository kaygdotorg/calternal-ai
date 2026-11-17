//! Thread-file admission reuses manual turn validation and durable jobs.
use super::*;
/// Finish a committed message intent under its content space lock, fencing Trash.
/// The job key and admission receipt survive a crash between stores. No callback
/// re-enters content; lock order is content -> jobs. Cost: settings read, current
/// head and receipt reads, job FULL admission, journal FULL publication and job read.
pub(crate) async fn admit<D: Directory>(
    state: &Shared<D>,
    who: Identity,
    session: Session,
    rid: &str,
    mut input: AgentInput,
    generation: u32,
    intent: Value,
) -> Result<Job, Error> {
    validate(&mut input, "agent.run")?;
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
    let id=blocking(move || {
        let thread=Uuid::parse_str(&input.thread_id).map_err(|_|Error::internal("invalid saved thread"))?;
        st.store.content().admit_item(SpaceId::parse(&space).map_err(|_|Error::internal("invalid saved space"))?,thread,calternal_content::Request{account:who.id,id:&request_id,operation:"threads.send",fingerprint:&fp},|current,saved| {
            if current.kind!="threads" || current.metadata["deleted"]==true {return Err(Error::not_found("item not found"));}
            if let Some(saved)=saved {return Ok(saved);}
            if current.metadata["pending"]!=intent {return Err(Error::conflict("the pending send has been superseded"));}
            st.jobs.admit(Admission{space:space.clone(),plugin:"calternal-orchestrator".into(),account:who.id,session,operation:"agent.turn".into(),input:json!({"request":input,"check":false,"retry_settings":settings,"thread_request":intent}),parent:None,generation,request_key}).map(|id|json!({"job_id":id}))
        })
    }).await?;
    let id = id["job_id"]
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
