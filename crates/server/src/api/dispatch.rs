//! Operation admission (one capability lookup), receipts and the host handlers.
use super::{cursors::admitted_operation, *};
use crate::authz::Policy;

pub(super) fn decode<T: DeserializeOwned>(input: JsonObject) -> Result<T, Error> {
    serde_json::from_value(Value::Object(input.into_iter().collect()))
        .map_err(|_| Error::invalid_input("input does not match operation schema"))
}

/// Canonical operation identity digest; also used by private pending replay.
pub(crate) fn fingerprint(op: &str, input: &JsonObject) -> String {
    sha256_hex(format!("{op}\n{}", canonical_request_input(input)).as_bytes())
}

pub(super) async fn blocking<D: Directory, T: Send + 'static>(
    state: &Shared<D>,
    f: impl FnOnce(&Store) -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    let state = state.clone();
    crate::blocking::spawn(move || f(&state.store))
        .await
        .map_err(|_| Error::internal("storage task failed"))?
}

/// A `space_id` that already passed authorization. Cost: one 36-byte parse.
pub(super) fn space_of(text: &str) -> Result<SpaceId, Error> {
    SpaceId::parse(text).map_err(|_| Error::not_found("space not found"))
}

pub(super) fn to_value<T: serde::Serialize>(v: T) -> Result<Value, Error> {
    serde_json::to_value(v).map_err(|_| Error::internal("encode"))
}

/// Dispatch through the mandatory policy gate before any handler or receipt replay.
/// The caller supplies a verified identity; supplied fixture grants are discarded
/// by conversion for legacy development callers. Each attempt reads current capability sets.
/// Plugin operations carry `session` into every worker host call, which rechecks it;
/// in-process callers without a revocable sign-in pass [`Session::Static`].
/// Summarize additionally rechecks membership after the worker before delivery.
/// Stored directory checks await their authority worker; identities contain no grants.
pub async fn dispatch<D: Directory>(
    state: Shared<D>,
    p: Identity,
    session: Session,
    request_id: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let workers = state.blocking.clone();
    let catalog = state.registry.snapshot();
    let mutates = super::operation_mutates(&catalog, op);
    workers
        .scope(
            mutates,
            dispatch_scoped_inner(
                state, p, session, request_id, op, input, None, None, catalog,
            ),
        )
        .await
}

pub(super) async fn dispatch_scoped_inner<D: Directory>(
    state: Shared<D>,
    p: Identity,
    session: Session,
    request_id: &str,
    op: &str,
    input: JsonObject,
    agent: Option<crate::turns::access::Authority>,
    machines: Option<std::sync::Arc<crate::machines::MachineService>>,
    catalog: Arc<crate::plugins::Catalog>,
) -> Result<Value, Error> {
    #[cfg(all(feature = "test-plugins", debug_assertions))]
    let admission_started = std::time::Instant::now();
    if let Some(row) = calternal_core::request_operation(op) {
        return crate::requests::dispatch(state, None, p, session, agent, request_id, row, input)
            .await;
    }
    if agent.is_some()
        && matches!(
            op,
            calternal_core::ONBOARDING_STATE | calternal_core::ONBOARDING_FINISH
        )
    {
        return Err(Error::forbidden(
            "thread credentials cannot manage account onboarding",
        ));
    }
    if calternal_core::is_appearance_operation(op) {
        if agent.is_some() {
            return Err(Error::forbidden(
                "thread credentials cannot manage account appearance",
            ));
        }
        if !session.is_live(&state.directory).await {
            return Err(Error::unauthorized("session expired or revoked"));
        }
        return super::appearance::operation(state, p, op, input).await;
    }
    if calternal_core::is_account_input_operation(op) {
        if agent.is_some() {
            return Err(Error::forbidden(
                "thread credentials cannot manage private account input",
            ));
        }
        return crate::account_input::dispatch(state, p, session, op, input).await;
    }
    if catalog
        .policy(op)
        .is_some_and(|policy| policy.constraint == Constraint::Sharing)
    {
        if agent.is_some() {
            return Err(Error::forbidden("sharing requires a person session"));
        }
        return super::sharing::dispatch(state, p, request_id, op, input).await;
    }
    if catalog.policy(op).is_none()
        && let Some(space) = input.get("space_id").and_then(Value::as_str)
        && let Ok(space) = SpaceId::parse(space)
        && let Some(approved) = state.registry.space_item_head(space, op)
    {
        if agent.is_some() {
            return Err(Error::forbidden("item head requires a person session"));
        }
        return crate::plugins::space_broker::invoke(
            state,
            p,
            session,
            approved,
            decode(input)?,
            machines,
        )
        .await;
    }
    let item_grants = agent.is_none() && crate::resolve::allows_item_grants(op);
    let mut input = input;
    // Capabilities already read for this request (by the resolver), so admission
    // does not look them up again.
    let mut granted = None;
    let item_scoped = catalog
        .policy(op)
        .is_some_and(|p| p.scope == authz::Scope::Item);
    if op == "resolve" || item_scoped {
        // A plugin operation names its target in the manifest; host item operations
        // and `resolve` carry the item ID in `id`.
        let target = catalog.owner_of_operation(op).and_then(|p| p.target(op));
        let field = target.map_or("id", |t| t.field.as_str());
        if calternal_core::is_known_operation(op) {
            calternal_core::validate_operation_input(op, &input)?;
        } else {
            validate_item_input(&input, field)?;
        }
        let kind = if op == "resolve" {
            input.get("kind").and_then(Value::as_str).unwrap_or("")
        } else {
            target.map_or("notes", |t| t.kind.as_str())
        }
        .to_owned();
        let id = input
            .get(field)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let op_required = catalog
            .policy(op)
            .map(|p| p.required)
            .unwrap_or(calternal_types::Capability::SpaceRead);
        let (mut resolved, caps, item_only) =
            crate::resolve::resolve_for(&state, &p, &kind, &id, op_required, item_grants).await?;
        if op == "resolve" && kind == "notes" {
            let st = state.clone();
            resolved = crate::blocking::spawn(move || {
                let item_state = crate::notes::state(&st.store, &resolved)?;
                if item_only && item_state != calternal_core::ItemState::Active {
                    return Err(Error::not_found("item not found"));
                }
                resolved.state = Some(item_state);
                Ok::<_, Error>(resolved)
            })
            .await
            .map_err(|_| Error::internal("resolver task failed"))??;
        }
        granted = Some(caps);
        if op == "resolve" {
            return to_value(resolved);
        }
        if let Some(hint) = input
            .get("space_id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            && hint != resolved.space_id
        {
            return Err(Error::not_found("item not found"));
        }
        input.insert("space_id".into(), Value::String(resolved.space_id));
    }
    // A job's space comes from the job store, never from the caller.
    let job_scoped = catalog
        .policy(op)
        .is_some_and(|p| p.scope == authz::Scope::Job);
    if job_scoped {
        calternal_core::validate_operation_input(op, &input)?;
        let space = crate::plugins::job_api::route(&state, &input)?;
        input.insert("space_id".into(), Value::String(space));
    }
    // One authorization path: a host row or a row derived from a plugin manifest.
    // Without either, answer as a hidden target would, so a caller cannot tell an
    // unregistered plugin operation from an item or space it may not see.
    let Some(policy) = catalog.policy(op).cloned() else {
        return Err(Error::not_found(if input.contains_key("id") {
            "item not found"
        } else if input.contains_key("space_id") {
            "space not found"
        } else {
            "unknown operation"
        }));
    };
    #[cfg(feature = "dev-plugin-drafts")]
    if let Some(plugin) = catalog.owner_of_operation(op)
        && let Some(space) = input.get("space_id").and_then(Value::as_str)
        && let Ok(space) = SpaceId::parse(space)
        && !plugin.available_in(space)
    {
        return Err(Error::not_found("unknown operation"));
    }
    // Admission: one capability lookup per request (or none after the resolver);
    // handlers do not repeat it. Delivery rechecks once after the handler.
    match granted {
        Some(caps) => policy.check(Some(caps))?,
        None => {
            super::authorization::authorize(&state, &p, &policy, &input).await?;
        }
    }
    #[cfg(all(feature = "test-plugins", debug_assertions))]
    {
        let elapsed = admission_started.elapsed().as_micros();
        let request = request_id.to_owned();
        // Debug diagnostics may write stderr; keep that I/O off the async task.
        // Awaiting bounds diagnostic work by the existing admitted requests.
        crate::blocking::spawn(move || {
            calternal_store::timings::emit("admission", &request, Some(elapsed));
        })
        .await
        .map_err(|_| Error::internal("admission diagnostic failed"))?;
    }
    if let Some(a) = &agent {
        if calternal_core::PeopleOperation::from_id(op)
            == Some(calternal_core::PeopleOperation::ListInvites)
        {
            return Err(Error::forbidden("invite links require a person session"));
        }
        let declared = catalog
            .owner_of_operation(op)
            .and_then(|p| p.operation(op))
            .map(|d| (d.mutates, d.risk))
            .or_else(|| calternal_core::cached_operation(op).map(|d| (d.mutates, d.risk)));
        let Some((mutates, risk)) = declared else {
            return Err(Error::forbidden("operation has no declared risk"));
        };
        let (scope, checked, operation, retry_id) = (
            a.clone(),
            input.clone(),
            op.to_owned(),
            request_id.to_owned(),
        );
        let verdict = crate::blocking::spawn(move || {
            scope.check_operation(&operation, &checked, &retry_id, mutates, risk)
        })
        .await
        .map_err(|_| Error::internal("scope check task failed"))?;
        if let Err(e) = verdict {
            if e.code != ErrorCode::Forbidden {
                return Err(e);
            }
            if let Session::Agent { token_hash, .. } = &session
                && let Some(result) = super::request_action::once(
                    &state,
                    &p,
                    &session,
                    a,
                    token_hash,
                    request_id,
                    op,
                    &input,
                    &policy,
                    mutates,
                    risk,
                    job_scoped,
                    catalog.clone(),
                )
                .await?
            {
                return Ok(result);
            }
            let (st, a, op, input, rid) = (
                state.clone(),
                a.clone(),
                op.to_owned(),
                input.clone(),
                request_id.to_owned(),
            );
            let approval = crate::blocking::spawn(move || {
                let create = || {
                    st.jobs
                        .agent_action_request(&a, &op, &input, &rid, risk, mutates)
                };
                let id = if let Some(machines) = machines {
                    crate::requests::park(&st, &machines, &a, create)?
                } else {
                    create()?
                };
                st.inputs.invalidate(a.actor.account)?;
                Ok::<_, Error>(id)
            })
            .await
            .map_err(|_| Error::internal("approval task failed"))??;
            return Err(e.with_details(serde_json::json!({"status":"approval_required","approval_id":approval,"job_id":agent.as_ref().map(|a|a.job.as_str()),"request_id":request_id})));
        }
    }
    admitted_operation(
        state, p, session, request_id, op, input, agent, policy, job_scoped, catalog,
    )
    .await
}

/// Dispatch an admitted operation and recheck current authority before returning
/// its result. Delivery uses the current space role or, when needed, item grants.
pub(super) async fn run_admitted_operation<D: Directory>(
    state: Shared<D>,
    p: Identity,
    session: Session,
    request_id: &str,
    op: &str,
    mut input: JsonObject,
    agent: Option<crate::turns::access::Authority>,
    policy: Policy,
    job_scoped: bool,
    catalog: Arc<crate::plugins::Catalog>,
) -> Result<Value, Error> {
    if matches!(
        op,
        calternal_core::ONBOARDING_STATE | calternal_core::ONBOARDING_FINISH
    ) {
        if agent.is_some() || matches!(&session, Session::Agent { .. }) {
            return Err(Error::forbidden(
                "thread credentials cannot manage account onboarding",
            ));
        }
        let _: calternal_core::OnboardingInput = decode(input)?;
        if !session.is_live(&state.directory).await {
            return Err(Error::unauthorized("session expired or revoked"));
        }
        let finish = op == calternal_core::ONBOARDING_FINISH;
        return blocking(&state, move |store| {
            to_value(crate::onboarding::state(store, p.id, finish)?)
        })
        .await;
    }
    if job_scoped {
        let space = match input.remove("space_id") {
            Some(Value::String(space)) => space,
            _ => return Err(Error::not_found("job not found")),
        };
        return crate::plugins::job_api::dispatch(state, p, op, space, input).await;
    }
    let actor = agent
        .as_ref()
        .map(|scope| scope.actor.clone())
        .unwrap_or_else(|| calternal_types::Actor::account(p.id));
    if op.starts_with("secrets.") {
        return crate::secrets::dispatch(state, p, policy, input, actor.via).await;
    }
    if let Some(operation) = calternal_core::PeopleOperation::from_id(op)
        && !op.starts_with("spaces.")
    {
        return super::people::dispatch(state, p, operation, request_id.to_owned(), input).await;
    }
    if op.starts_with("spaces.") {
        return super::spaces::dispatch(state, p, actor, request_id, op, input).await;
    }
    let owner = catalog.owner_of_operation(op);
    if owner.is_some_and(|p| p.manifest.id == "calternal-orchestrator")
        && op.starts_with("threads.")
    {
        let result = crate::threads::dispatch(
            state.clone(),
            p.clone(),
            session,
            agent,
            actor,
            request_id,
            op,
            input.clone(),
        )
        .await?;
        super::authorization::authorize(&state, &p, &policy, &input).await?;
        return Ok(result);
    }
    if policy.constraint == Constraint::PluginInvocation {
        let plugin = owner
            .cloned()
            .ok_or_else(|| Error::not_found("unknown operation"))?;
        return invoke_plugin(
            state, p, session, actor, request_id, op, policy, input, plugin,
        )
        .await;
    }
    if let Some(operation) = calternal_core::SearchOperation::from_id(op) {
        let delivery_space = policy.scope_space(&input)?;
        let value = super::search::dispatch(&state, &p, operation, input, &catalog).await?;
        if let Some(space) = delivery_space {
            policy.check(super::authorization::capabilities(&state, p.id, space).await?)?;
        }
        return Ok(value);
    }
    if calternal_core::automation_operation(op).is_some() {
        let space = policy.scope_space(&input)?;
        let result = crate::automations::dispatch(
            state.clone(),
            p.clone(),
            session,
            actor,
            request_id,
            op,
            input,
        )
        .await?;
        if let Some(space) = space {
            policy.check(super::authorization::capabilities(&state, p.id, space).await?)?;
        }
        return Ok(result);
    }
    let delivery_space = input
        .get("space_id")
        .and_then(Value::as_str)
        .and_then(|s| SpaceId::parse(s).ok());
    let delivery_item = if agent.is_none() && crate::resolve::allows_item_grants(op) {
        input
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| uuid::Uuid::parse_str(id).ok())
    } else {
        None
    };
    let purge = if op == "trash.purge" {
        input
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| uuid::Uuid::parse_str(id).ok())
    } else {
        None
    };
    let value = dispatch_checked(state.clone(), p, actor, request_id, op, input).await?;
    if let (Some(space), Some(id), Some(directory)) =
        (delivery_space, purge, state.directory.protected_spaces())
    {
        directory
            .purge_item_grants(space, id)
            .await
            .map_err(crate::auth::auth_error)?;
    }
    if let Some(space) = delivery_space {
        super::authorization::authorize_delivery(&state, p.id, &policy, space, delivery_item)
            .await?;
    }
    Ok(value)
}

/// Private handler entry after the policy gate. Invalid input, storage and worker
/// failures propagate without bypassing authorization. Dynamic mutation inputs
/// hash once on the blocking pool; remaining cost is the selected store operation.
pub(super) async fn dispatch_checked<D: Directory>(
    state: Shared<D>,
    p: Identity,
    actor: calternal_types::Actor,
    request_id: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    match op {
        calternal_core::WALLPAPERS_LIST => super::wallpapers::list(input),
        "notes.list" | "notes.lookup" | "tags.list" | "tags.lookup" | "trash.list"
        | "notes.history" => super::note_pages::discover(&state, op, input).await,
        "notes.get" => {
            let i: NotesGetInput = decode(input)?;
            let space = space_of(&i.space_id)?;
            let note = blocking(&state, move |s| s.get(space, &i.id)).await?;
            to_value(NoteResult { note, job_id: None })
        }
        "notes.rename.status" | "notes.rename.undo" => {
            super::notes::rename_job(&state, actor, request_id, op, input).await
        }
        "trash.empty.preview" | "trash.empty" | "trash.empty.status" => {
            super::notes::empty_trash(&state, actor, request_id, op, input).await
        }
        "notes.delete" | "notes.restore" | "trash.purge" | "notes.rename" => {
            super::notes::mutate(&state, actor, request_id, op, input).await
        }
        "notes.create" | "notes.update" => mutate_note(state, actor, request_id, op, input).await,
        "plugins.list" => {
            super::plugin_slots::list(
                &state,
                p.id,
                space_of(&decode::<PluginsListInput>(input)?.space_id)?,
            )
            .await
        }
        "plugins.enable" | "plugins.disable" => {
            let (fp, input) = super::request::fingerprint(op, input).await?;
            let i: PluginInput = decode(input)?;
            let space = space_of(&i.space_id)?;
            let enable = op == "plugins.enable";
            let st = state.clone();
            let (rid, op, principal) = (request_id.to_string(), op.to_string(), p.id);
            blocking(&state, move |s| {
                if let Some(v) = replay_receipt(s, principal, space, &rid, &op, &fp)? {
                    return Ok(v);
                }
                let catalog = st.registry.snapshot();
                let plugin = catalog
                    .get(&i.plugin_id)
                    .filter(|plugin| plugin.available_in(space))
                    .ok_or_else(|| Error::not_found("plugin is not registered"))?;
                if enable {
                    if let Some(why) = &plugin.unavailable {
                        return Err(Error::unavailable(format!(
                            "plugin cannot be enabled: {why}"
                        )));
                    }
                    // Provision before activation commits, so a failure leaves it disabled.
                    let collections: Vec<_> = plugin
                        .manifest
                        .collections
                        .iter()
                        .map(|c| (c.kind.clone(), c.id.clone(), c.name.clone()))
                        .collect();
                    s.provision_plugin(
                        space,
                        actor.clone(),
                        &plugin.kind_registrations(),
                        &collections,
                    )?;
                }
                let manifest = plugin.manifest.clone();
                s.set_plugin_enabled(
                    actor,
                    &rid,
                    &op,
                    &fp,
                    space,
                    &plugin.manifest.id,
                    catalog.control_default(plugin),
                    enable,
                    |row| {
                        serde_json::to_value(PluginResult {
                            plugin: PluginState {
                                manifest,
                                enabled: row.enabled,
                                generation: row.generation,
                            },
                        })
                        .expect("encode")
                    },
                )
            })
            .await
        }
        "plugins.preview" => {
            let i: PluginInput = decode(input)?;
            let space = space_of(&i.space_id)?;
            let catalog = state.registry.snapshot();
            let plugin = catalog
                .get(&i.plugin_id)
                .filter(|plugin| plugin.available_in(space))
                .ok_or_else(|| Error::not_found("plugin is not registered"))?
                .clone();
            preview(state, p, plugin, space).await
        }
        _ => Err(Error::unavailable(
            "operation has no host binding in this preview",
        )),
    }
}

/// Retry handling shared by every mutation in an authorized space: the same
/// (account, space, request_id) with the same canonical fingerprint returns the
/// durable SSD result; userdata placement follows its bounded policy. A different fingerprint
/// is a conflict. Another account or space never sees this receipt.
pub(super) fn replay_receipt(
    s: &Store,
    principal: AccountId,
    space: SpaceId,
    rid: &str,
    op: &str,
    fp: &str,
) -> Result<Option<Value>, Error> {
    #[cfg(all(feature = "test-plugins", debug_assertions))]
    let lookup_started = std::time::Instant::now();
    let receipt = s.receipt(principal, space, rid);
    #[cfg(all(feature = "test-plugins", debug_assertions))]
    calternal_store::timings::emit(
        "receipt_lookup",
        rid,
        Some(lookup_started.elapsed().as_micros()),
    );
    let Some(r) = receipt? else {
        return Ok(None);
    };
    if r.operation != op || r.fingerprint != fp {
        return Err(Error::request_reused());
    }
    // Permanent deletion suppresses older body-bearing receipts, while ordinary
    // outside-file conflicts do not revoke an already durable acknowledgment.
    // Purge's own idempotent result contains no body and remains replayable.
    if op != "trash.purge"
        && let Some(item) = r.item
        && s.content().item_state(space, item)? == "deleted"
    {
        return Err(Error::not_found("item not found"));
    }
    Ok(Some(r.result))
}

/// Replay or commit an authorized native note edit after hashing its owned input
/// on the blocking pool. Input, revision, worker and storage failures propagate.
/// Cost: one hash task followed by the store's receipt/read/durability work.
pub(super) async fn mutate_note<D: Directory>(
    state: Shared<D>,
    actor: calternal_types::Actor,
    request_id: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let (fp, input) = super::request::fingerprint(op, input).await?;
    let (create, update) = if op == "notes.create" {
        let i: NotesCreateInput = decode(input)?;
        (Some(i), None)
    } else {
        let i: NotesUpdateInput = decode(input)?;
        (None, Some(i))
    };
    let rid = request_id.to_string();
    let op = op.to_string();
    let principal = actor.account;
    let space = space_of(
        create
            .as_ref()
            .map(|i| i.space_id.as_str())
            .or_else(|| update.as_ref().map(|i| i.space_id.as_str()))
            .expect("note scope"),
    )?;
    // An item grant authorizes this note only. Name changes use the linked
    // rename path, which may write other notes and needs space authority.
    let allow_link_rewrites = if update.is_some() {
        super::authorization::capabilities(&state, principal, space)
            .await?
            .is_some_and(|caps| caps.contains(calternal_types::Capability::ContentWrite))
    } else {
        false
    };

    let authority = if update.is_some() && !allow_link_rewrites {
        state.directory.protected_spaces().cloned()
    } else {
        None
    };
    #[cfg(feature = "perf-timings")]
    let timing = (op == "notes.update")
        .then(|| {
            super::guard::OPERATION_ENTRY_NS
                .try_with(|entry| *entry)
                .ok()
        })
        .flatten()
        .map(|entry| (entry, calternal_types::perf_monotonic_ns()));
    blocking(&state, move |s| {
        let save = || {
            if let Some(v) = replay_receipt(s, principal, space, &rid, &op, &fp)? {
                return Ok(v);
            }
            if let Some(i) = create {
                let note = s.create(actor, &rid, &op, &fp, space, &i.title, &i.markdown)?;
                to_value(NoteResult { note, job_id: None })
            } else {
                let i = update.expect("update input");
                to_value(s.update(
                    actor,
                    &rid,
                    &op,
                    &fp,
                    space,
                    &i.id,
                    &i.base_revision,
                    i.title.as_deref(),
                    &i.markdown,
                    allow_link_rewrites,
                    authority.as_ref(),
                )?)
            }
        };
        #[cfg(feature = "perf-timings")]
        if let Some((entry, queued)) = timing {
            return calternal_store::timings::observe_note_save(&rid, entry, queued, save);
        }
        save()
    })
    .await
}
