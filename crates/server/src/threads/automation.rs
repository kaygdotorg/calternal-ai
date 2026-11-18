//! Start an automation turn through the same content receipts and job admission as a send.
use super::*;

pub(crate) async fn admit<D: Directory>(
    state: &Shared<D>,
    who: Identity,
    space: SpaceId,
    automation: Uuid,
    run: Uuid,
    name: String,
    prompt: String,
    mode: AgentMode,
    machine: Option<String>,
    scheduled: Option<i64>,
    zone: String,
) -> Result<(String, String), Error> {
    let generation =
        crate::turns::enabled_generation(state, &space.to_string(), "calternal-orchestrator")
            .await?;
    let st = state.clone();
    let actor = calternal_types::Actor::plugin(who.id, "calternal-orchestrator");
    let audit = actor.clone();
    let rid = format!("automation-run/{run}");
    let send = rid.clone();
    let turn_prompt = prompt.clone();
    let target = blocking(move || {
        let create = JsonObject::from([
            ("title".into(), json!(name)),
            ("mode".into(), json!(mode)),
            ("automation_id".into(), json!(automation)),
            ("assigned_thread_id".into(), json!(run)),
        ]);
        match st.store.content().get(space, run) {
            Ok(thread)
                if thread.kind == "threads"
                    && thread.metadata["automation_id"] == json!(automation) => {}
            Ok(_) => {
                return Err(Error::conflict(
                    "the automation thread identity already exists",
                ));
            }
            Err(error) if error.code == calternal_types::ErrorCode::NotFound => {
                crate::fault::automation_content("thread", || {
                    operations::apply(
                        &st,
                        &who,
                        space,
                        "threads.create",
                        &format!("{send}/create"),
                        &create,
                        None,
                        &audit,
                    )
                })?;
            }
            Err(error) => return Err(error),
        }
        crate::fault::hit(crate::fault::FaultPoint::AutomationAfterThread);
        let input = JsonObject::from([
            ("message".into(), json!(prompt)),
            ("mode".into(), json!(mode)),
            ("automation_name".into(), json!(name)),
            (
                "automation_run".into(),
                json!({"id":run,"automation":automation,"scheduled":scheduled,"timezone":zone}),
            ),
        ]);
        crate::fault::automation_content("prompt", || {
            operations::apply(
                &st,
                &who,
                space,
                "threads.send",
                &send,
                &input,
                Some(run),
                &audit,
            )
        })
    })
    .await?;
    crate::fault::hit(crate::fault::FaultPoint::AutomationAfterPrompt);
    let thread = target["thread"]["id"]
        .as_str()
        .ok_or_else(|| Error::internal("missing saved thread"))?;
    let meta = &target["thread"]["metadata"];
    let input = AgentInput {
        space_id: space.to_string(),
        thread_id: thread.into(),
        prompt: turn_prompt,
        machine_id: machine.unwrap_or_default(),
        provider_id: meta["provider"].as_str().unwrap_or_default().into(),
        model: meta["model"].as_str().unwrap_or_default().into(),
        mode,
        source_app: "threads".into(),
        ..Default::default()
    };
    // An automation is a durable grant by an account, not a saved browser
    // token. Job execution checks that account's current space capabilities.
    let job = crate::turns::admit_thread(
        state,
        who,
        Session::Static,
        actor,
        &rid,
        input,
        generation,
        target["intent"].clone(),
    )
    .await?;
    crate::fault::hit(crate::fault::FaultPoint::AutomationAfterAdmission);
    Ok((thread.into(), job.id))
}
