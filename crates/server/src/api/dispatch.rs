//! Operation admission (one capability lookup), receipts and the host handlers.
use super::*;
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
    tokio::task::spawn_blocking(move || f(&state.store))
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
/// Stored directory lookups run on blocking tasks; identities contain no grants.
pub async fn dispatch<D: Directory>(
    state: Shared<D>,
    p: Identity,
    session: Session,
    request_id: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    dispatch_scoped(state, p, session, request_id, op, input, None, None).await
}

pub(super) async fn dispatch_scoped<D: Directory>(
    state: Shared<D>,
    p: Identity,
    session: Session,
    request_id: &str,
    op: &str,
    input: JsonObject,
    agent: Option<crate::turns::access::Authority>,
    machines: Option<std::sync::Arc<crate::machines::MachineService>>,
) -> Result<Value, Error> {
    if let Some(row) = calternal_core::request_operation(op) {
        return crate::requests::dispatch(state, None, p, session, agent, request_id, row, input)
            .await;
    }
    if calternal_core::is_account_input_operation(op) {
        if agent.is_some() {
            return Err(Error::forbidden(
                "thread credentials cannot manage private account input",
            ));
        }
        return crate::account_input::dispatch(state, p, session, op, input).await;
    }
    let mut input = input;
    // Capabilities already read for this request (by the resolver), so admission
    // does not look them up again.
    let mut granted = None;
    let item_scoped = state
        .registry
        .policy(op)
        .is_some_and(|p| p.scope == authz::Scope::Item);
    if op == "resolve" || item_scoped {
        // A plugin operation names its target in the manifest; host item operations
        // and `resolve` carry the item ID in `id`.
        let target = state
            .registry
            .owner_of_operation(op)
            .and_then(|p| p.target(op));
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
        let op_required = state
            .registry
            .policy(op)
            .map(|p| p.required)
            .unwrap_or(calternal_types::Capability::SpaceRead);
        let st = state.clone();
        let account = p.clone();
        let resolve_address = op == "resolve";
        let (resolved, caps) = tokio::task::spawn_blocking(move || {
            let (mut resolved, caps) = crate::resolve::resolve_for(
                &st.directory,
                &st.store,
                &account,
                &kind,
                &id,
                op_required,
            )?;
            if resolve_address && kind == "notes" {
                resolved.state = Some(crate::notes::state(&st.store, &resolved)?);
            }
            Ok::<_, Error>((resolved, caps))
        })
        .await
        .map_err(|_| Error::internal("resolver task failed"))??;
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
    let job_scoped = state
        .registry
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
    let Some(policy) = state.registry.policy(op).cloned() else {
        return Err(Error::not_found(if input.contains_key("id") {
            "item not found"
        } else if input.contains_key("space_id") {
            "space not found"
        } else {
            "unknown operation"
        }));
    };
    // Admission: one capability lookup per request (or none after the resolver);
    // handlers do not repeat it. Delivery rechecks once after the handler.
    match granted {
        Some(caps) => policy.check(Some(caps))?,
        None => {
            let st = state.clone();
            let checked = input.clone();
            let admission = policy.clone();
            tokio::task::spawn_blocking(move || admission.authorize(&st.directory, &p, &checked))
                .await
                .map_err(|_| Error::internal("directory task failed"))??;
        }
    }
    if let Some(a) = &agent {
        let declared = state
            .registry
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
        let verdict = tokio::task::spawn_blocking(move || {
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
                    &state, &p, &session, a, token_hash, request_id, op, &input, &policy, mutates,
                    risk, job_scoped,
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
            let approval = tokio::task::spawn_blocking(move || {
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
        state, p, session, request_id, op, input, agent, policy, job_scoped,
    )
    .await
}

pub(super) async fn admitted_operation<D: Directory>(
    state: Shared<D>,
    p: Identity,
    session: Session,
    request_id: &str,
    op: &str,
    mut input: JsonObject,
    agent: Option<crate::turns::access::Authority>,
    policy: Policy,
    job_scoped: bool,
) -> Result<Value, Error> {
    if job_scoped {
        let space = match input.remove("space_id") {
            Some(Value::String(space)) => space,
            _ => return Err(Error::not_found("job not found")),
        };
        return crate::plugins::job_api::dispatch(state, p, op, space, input).await;
    }
    if op.starts_with("secrets.") {
        let via = agent.as_ref().and_then(|a| a.actor.via.clone());
        return crate::secrets::dispatch(state, p, policy, input, via).await;
    }
    if op.starts_with("spaces.") {
        let st = state.clone();
        let rid = request_id.to_owned();
        let operation = op.to_owned();
        return tokio::task::spawn_blocking(move || {
            // Accounts this change removes lose their private input for the space.
            let removed =
                crate::account_input::revoked_targets(&st.directory, &p, &operation, &input)?;
            let space = input
                .get("space_id")
                .and_then(Value::as_str)
                .map(space_of)
                .transpose()?;
            let change = || st.directory.space_operation(&p, &rid, &operation, &input);
            let result = match space {
                // A write in a space is admitted through that space's receipts, the
                // one admission every write there uses: one request ID cannot run
                // both a content and a directory write in a space. The directory's
                // own atomic receipt replays a change committed before a crash.
                Some(space) if !matches!(operation.as_str(), "spaces.members") => {
                    let fp = fingerprint(&operation, &input);
                    st.store.content().admit(
                        space,
                        calternal_content::Request {
                            account: p.id,
                            id: &rid,
                            operation: &operation,
                            fingerprint: &fp,
                        },
                        change,
                    )
                }
                // Reads, and account-level writes (spaces.create), whose receipt
                // scope is the acting account.
                _ => change(),
            };
            if result.is_ok()
                && let Some(space) = space
            {
                if operation == "spaces.delete" {
                    st.store
                        .secrets()
                        .erase_owner(&calternal_secrets::Owner::Space(space))
                        .map_err(|e| Error::unavailable(e.to_string()))?;
                }
                for account in removed {
                    if st
                        .directory
                        .capabilities(account, space)
                        .is_some_and(|caps| caps.contains(calternal_types::Capability::SpaceRead))
                    {
                        continue;
                    }
                    st.jobs
                        .interrupt_revoked_account(&space.to_string(), account)?;
                    st.inputs.erase_space(account, space)?;
                    st.inputs.invalidate(account)?;
                }
            }
            result
        })
        .await
        .map_err(|_| Error::internal("directory task failed"))?;
    }
    if state
        .registry
        .owner_of_operation(op)
        .is_some_and(|p| p.manifest.id == "calternal-orchestrator")
        && op.starts_with("threads.")
    {
        let result = crate::threads::dispatch(
            state.clone(),
            p.clone(),
            session,
            agent,
            request_id,
            op,
            input.clone(),
        )
        .await?;
        let (st, checked) = (state.clone(), input.clone());
        tokio::task::spawn_blocking(move || policy.authorize(&st.directory, &p, &checked))
            .await
            .map_err(|_| Error::internal("delivery check failed"))??;
        return Ok(result);
    }
    if policy.constraint == Constraint::PluginInvocation {
        return invoke_plugin(state, p, session, request_id, op, policy, input).await;
    }
    let delivery_space = input
        .get("space_id")
        .and_then(Value::as_str)
        .and_then(|s| SpaceId::parse(s).ok());
    let value = dispatch_checked(state.clone(), p.clone(), request_id, op, input).await?;
    if let Some(space) = delivery_space {
        let st = state.clone();
        tokio::task::spawn_blocking(move || policy.authorize_space(&st.directory, &p, space))
            .await
            .map_err(|_| Error::internal("directory task failed"))??;
    }
    Ok(value)
}

/// Private handler entry: called exclusively after the dispatch policy gate.
pub(super) async fn dispatch_checked<D: Directory>(
    state: Shared<D>,
    p: Identity,
    request_id: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    match op {
        "notes.list" | "notes.lookup" | "tags.list" | "tags.lookup" | "trash.list" => {
            super::notes::discover(&state, op, input).await
        }
        "notes.get" => {
            let i: NotesGetInput = decode(input)?;
            let space = space_of(&i.space_id)?;
            let note = blocking(&state, move |s| s.get(space, &i.id)).await?;
            to_value(NoteResult { note })
        }
        "notes.history" => {
            let i: NotesHistoryInput = decode(input)?;
            let limit = i.limit.unwrap_or(20);
            let space = space_of(&i.space_id)?;
            let (versions, next_cursor) = blocking(&state, move |s| {
                s.history(space, &i.id, limit, i.cursor.as_deref())
            })
            .await?;
            to_value(NotesHistoryResult {
                versions,
                next_cursor,
            })
        }
        "notes.delete" | "notes.restore" | "trash.purge" | "notes.rename" => {
            super::notes::mutate(&state, p, request_id, op, input).await
        }
        "notes.create" | "notes.update" => mutate_note(state, p, request_id, op, input).await,
        "plugins.list" => {
            let i: PluginsListInput = decode(input)?;
            let space = space_of(&i.space_id)?;
            let st = state.clone();
            let plugins = blocking(&state, move |s| {
                st.registry
                    .plugins()
                    .iter()
                    .map(|plugin| plugin_state(s, space, plugin))
                    .collect::<Result<Vec<_>, _>>()
            })
            .await?;
            to_value(PluginsListResult { plugins })
        }
        "plugins.enable" | "plugins.disable" => {
            let fp = fingerprint(op, &input);
            let i: PluginInput = decode(input)?;
            let space = space_of(&i.space_id)?;
            let enable = op == "plugins.enable";
            let st = state.clone();
            let (rid, op, principal) = (request_id.to_string(), op.to_string(), p.id);
            blocking(&state, move |s| {
                if let Some(v) = replay_receipt(s, principal, space, &rid, &op, &fp)? {
                    return Ok(v);
                }
                let plugin = st
                    .registry
                    .get(&i.plugin_id)
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
                        principal,
                        &plugin.kind_registrations(),
                        &collections,
                    )?;
                }
                let manifest = plugin.manifest.clone();
                s.set_plugin_enabled(
                    principal,
                    &rid,
                    &op,
                    &fp,
                    space,
                    &plugin.manifest.id,
                    default_row(plugin),
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
            let plugin = state
                .registry
                .get(&i.plugin_id)
                .ok_or_else(|| Error::not_found("plugin is not registered"))?
                .clone();
            preview(state, p, plugin, space_of(&i.space_id)?).await
        }
        _ => Err(Error::unavailable(
            "operation has no host binding in this preview",
        )),
    }
}

/// Retry handling shared by every mutation in an authorized space: the same
/// (account, space, request_id) with the same canonical fingerprint returns the
/// stored result (finishing any pending materialization); a different fingerprint
/// is a conflict. Another account or space never sees this receipt.
pub(super) fn replay_receipt(
    s: &Store,
    principal: AccountId,
    space: SpaceId,
    rid: &str,
    op: &str,
    fp: &str,
) -> Result<Option<Value>, Error> {
    let Some(r) = s.receipt(principal, space, rid)? else {
        return Ok(None);
    };
    if r.operation != op || r.fingerprint != fp {
        return Err(Error::request_reused());
    }
    if op != "trash.purge"
        && let Some(n) = &r.note_id
    {
        s.materialize(n).map_err(|e| {
            if matches!(e.code, ErrorCode::Conflict | ErrorCode::NotFound) {
                e
            } else {
                Error::unavailable("committed but not yet materialized; retry")
            }
        })?;
    }
    Ok(Some(r.result))
}

pub(super) async fn mutate_note<D: Directory>(
    state: Shared<D>,
    p: Identity,
    request_id: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let fp = fingerprint(op, &input);
    let (create, update) = if op == "notes.create" {
        let i: NotesCreateInput = decode(input)?;
        (Some(i), None)
    } else {
        let i: NotesUpdateInput = decode(input)?;
        (None, Some(i))
    };
    let rid = request_id.to_string();
    let op = op.to_string();
    let principal = p.id;
    let space = space_of(
        create
            .as_ref()
            .map(|i| i.space_id.as_str())
            .or_else(|| update.as_ref().map(|i| i.space_id.as_str()))
            .expect("note scope"),
    )?;
    blocking(&state, move |s| {
        if let Some(v) = replay_receipt(s, principal, space, &rid, &op, &fp)? {
            return Ok(v);
        }
        let note = if let Some(i) = create {
            s.create(principal, &rid, &op, &fp, space, &i.title, &i.markdown)?
        } else {
            let i = update.expect("update input");
            s.update(
                principal,
                &rid,
                &op,
                &fp,
                space,
                &i.id,
                &i.base_revision,
                i.title.as_deref(),
                &i.markdown,
            )?
        };
        to_value(NoteResult { note })
    })
    .await
}
