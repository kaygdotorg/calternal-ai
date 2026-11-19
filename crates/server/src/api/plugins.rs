//! Manifest-declared plugin operations on their (plugin, space) worker, and previews.
use super::*;
mod receipts;
use receipts::committed_write;

pub(super) fn plugin_state(
    s: &Store,
    space: SpaceId,
    plugin: &Plugin,
) -> Result<PluginState, Error> {
    let row = activation(s, space, plugin)?;
    Ok(PluginState {
        manifest: plugin.manifest.clone(),
        enabled: row.enabled,
        generation: row.generation,
    })
}

/// Host-checked part of an item-targeted plugin operation without a core-contract
/// entry: `field` holds a bounded string ID and the optional `space_id` hint is a
/// string. Every other property is checked by the declared input schema before the
/// worker sees it. Cost: O(1) map lookups.
pub(super) fn validate_item_input(input: &JsonObject, field: &str) -> Result<(), Error> {
    if !input
        .get(field)
        .and_then(Value::as_str)
        .is_some_and(|id| !id.is_empty() && id.len() <= 128)
    {
        return Err(Error::input_field(
            &format!("/{field}"),
            "expected a nonempty item ID string of at most 128 bytes",
        ));
    }
    if input.get("space_id").is_some_and(|v| !v.is_string()) {
        return Err(Error::input_field(
            "/space_id",
            "expected a space UUID string",
        ));
    }
    Ok(())
}

/// Map a worker outcome onto the wire, after the caller's delivery checks.
pub(super) fn outcome_value(outcome: Outcome) -> Result<Value, Error> {
    match outcome {
        Outcome::Complete(value) => Ok(value),
        Outcome::InvalidInput(error) => Err(error),
        Outcome::Refused => Err(Error::unavailable("extension request was refused")),
        Outcome::Failed(e) => Err(e),
        Outcome::Cancelled => Err(Error::unavailable("extension request was cancelled")),
    }
}

/// A generated-UI view in a plugin result must be version 2, at most 64 KiB, and
/// compile without diagnostics (`calternal-genui`: syntax, catalog components,
/// accessibility and budgets). Its actions only describe operations; dispatch checks
/// them again. Cost: one decode and one compile when the result carries `view`.
pub(super) fn check_view(plugin: &Plugin, value: &Value) -> Result<(), Error> {
    let Some(view) = value.get("view") else {
        return Ok(());
    };
    let rejected = || {
        tracing::warn!(plugin = %plugin.manifest.id, "worker view rejected");
        Error::unavailable("extension returned an invalid view")
            .with_details(serde_json::json!({"reason":"view_schema","path":"/view"}))
    };
    let view: calternal_core::GenUiView =
        serde_json::from_value(view.clone()).map_err(|_| rejected())?;
    calternal_core::validate_genui(&view).map_err(|_| rejected())?;
    let compiled = calternal_genui::compile(&view.source, calternal_genui::Budgets::default());
    if compiled.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(rejected())
    }
}

/// Invoke a manifest-declared operation on its (plugin, space) worker.
///
/// A mutating operation is receipted like every other write: a retry with the same
/// request ID and input returns the original result without invoking the worker, and
/// different input under that ID is a conflict. The worker's own `write_item` calls
/// carry per-call receipts. When the declared output cannot be delivered, the
/// first committed write supplies a host acknowledgment under the original ID.
/// Cost: one blocking canonical-hash task without a body clone, plus worker
/// interaction. Mutations read the outer receipt and, on miss, the first
/// write receipt before dispatch. Finalization checks for a competing receipt
/// before appending. Brokered writes retain their documented durability costs.
///
/// Admission (already done by the caller) checked the manifest-derived policy.
/// Capture the enabled generation once for worker routing. The broker rechecks
/// activation before each effect and before disclosing a host reply to the worker.
/// Recheck activation and grants here before returning the HTTP result. The HTTP
/// guard also rechecks the session on delivery.
pub(super) async fn invoke_plugin<D: Directory>(
    state: Shared<D>,
    p: Identity,
    session: Session,
    actor: calternal_types::Actor,
    request_id: &str,
    op: &str,
    policy: authz::Policy,
    input: JsonObject,
    plugin: Arc<Plugin>,
) -> Result<Value, Error> {
    let (fp, mut input) = super::request::fingerprint(op, input).await?;
    let mutates = plugin.operation(op).is_some_and(|o| o.mutates);
    let space = match input.remove("space_id") {
        Some(Value::String(space)) => space_of(&space)?,
        _ => {
            return Err(Error::input_field(
                "/space_id",
                "expected a space UUID string",
            ));
        }
    };
    // The resolved space travels separately; the plugin sees only its declared input.
    let plugin_input = Value::Object(input.into_iter().collect());
    let (plugin2, account) = (plugin.clone(), p.id);
    let (rid, operation, fp2) = (request_id.to_owned(), op.to_owned(), fp.clone());
    let receipt_actor = actor.clone();
    let admitted = blocking(&state, move |s| {
        let row = activation(s, space, &plugin2)?;
        if !row.enabled {
            return Err(Error::forbidden("plugin is disabled in this space"));
        }
        // A mutating operation's retry replays its receipt and runs nothing again.
        if mutates {
            if let Some(v) = replay_receipt(s, account, space, &rid, &operation, &fp2)? {
                return Ok(Err(v));
            }
            if let Some(result) = committed_write(s, account, space, &rid, &fp2)? {
                let value =
                    s.record_plugin_receipt(receipt_actor, &rid, &operation, &fp2, space, result)?;
                return Ok(Err(value));
            }
        }
        Ok(Ok(row.generation))
    })
    .await?;
    let generation = match admitted {
        Ok(generation) => generation,
        Err(replayed) => return Ok(replayed),
    };
    let host = SpaceHost::new(
        state.clone(),
        p.clone(),
        session.clone(),
        space,
        plugin.clone(),
        generation,
        request_id.to_owned(),
    );
    let host = if mutates {
        host.for_request(fp.clone())
    } else {
        host
    };
    #[cfg(feature = "dev-plugin-drafts")]
    let invocation_started = std::time::Instant::now();
    let outcome = state
        .workers
        .invoke(
            &plugin,
            space,
            Call {
                operation: op.to_owned(),
                input: plugin_input,
                account: p.id,
                host: Arc::new(host),
                timeout: if plugin.manifest.network.as_ref().is_some_and(|network| network.mail_connection.is_some()) {
                    std::time::Duration::from_secs(15)
                } else {
                    crate::plugins::WORKER_TIMEOUT
                },
                cancel: None,
                job: None,
            },
        )
        .await;
    // Delivery checks run whatever the worker answered, so a refusal caused by a
    // revocation reports the same error the revocation itself would.
    if !session.is_live_in(&state).await {
        return Err(Error::unauthorized("missing or invalid session"));
    }
    let plugin2 = plugin.clone();
    let still = blocking(&state, move |s| activation(s, space, &plugin2)).await?;
    if !state.registry.snapshot().authorizes(&plugin)
        || !still.enabled
        || still.generation != generation
    {
        return Err(
            Error::conflict("plugin state changed during invocation; result discarded")
                .with_details(serde_json::json!({ "reason": "plugin_generation_changed" })),
        );
    }
    policy.check(super::authorization::capabilities(&state, p.id, space).await?)?;
    let value = outcome_value(outcome).and_then(|value| {
        check_view(&plugin, &value)?;
        Ok(value)
    });
    if let Err(error) = &value {
        plugin.installed_diagnostics.failure(space, op, error);
    }
    #[cfg(feature = "dev-plugin-drafts")]
    if let Err(error) = &value {
        plugin.record_draft_error(space, p.id, op, error, invocation_started.elapsed());
    }
    if !mutates {
        return value;
    }
    let rid = request_id.to_owned();
    let op = op.to_owned();
    blocking(&state, move |s| {
        let result = match value {
            Ok(value) => value,
            Err(error) => committed_write(s, actor.account, space, &rid, &fp)?.ok_or(error)?,
        };
        s.record_plugin_receipt(actor, &rid, &op, &fp, space, result)
    })
    .await
}

/// Preview a plugin without activating it or touching user data: its first
/// item-targeted operation (manifest `target`) runs against the fixed fixture item, served by a
/// broker that refuses every other host call. A plugin with no worker or no
/// item-targeted operation has nothing to preview (not found).
pub(super) async fn preview<D: Directory>(
    state: Shared<D>,
    p: Identity,
    plugin: Arc<Plugin>,
    space: SpaceId,
) -> Result<Value, Error> {
    let (operation, target) = plugin
        .manifest
        .operations
        .iter()
        .find_map(|op| Some((op.id.clone(), op.target.clone()?)))
        .filter(|_| plugin.entry.worker.is_some())
        .ok_or_else(|| Error::not_found("plugin has no views"))?;
    let outcome = state
        .workers
        .invoke(
            &plugin,
            space,
            Call {
                operation,
                input: serde_json::json!({ target.field: PREVIEW_ITEM_ID }),
                account: p.id,
                host: Arc::new(PreviewHost { kind: target.kind }),
                timeout: crate::plugins::WORKER_TIMEOUT,
                cancel: None,
                job: None,
            },
        )
        .await;
    let value = outcome_value(outcome)?;
    check_view(&plugin, &value)?;
    let view = value
        .get("view")
        .cloned()
        .ok_or_else(|| Error::not_found("plugin has no views"))?;
    let view = serde_json::from_value(view)
        .map_err(|_| Error::unavailable("extension returned an invalid view"))?;
    to_value(PluginPreviewResult {
        view,
        preview: True,
    })
}
