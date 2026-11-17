//! Account-derived operation admission and pending submission state machine.
use super::{Inputs, internal};
use crate::{
    api::Shared,
    auth::{Directory, Identity},
    authz::Capability,
    error::HostError,
    plugins::Session,
};
use calternal_core::*;
use rusqlite::{TransactionBehavior, params};
use serde_json::{Value, json};
use uuid::Uuid;

fn decode<T: serde::de::DeserializeOwned>(input: &JsonObject) -> Result<T, HostError> {
    serde_json::from_value(serde_json::to_value(input).map_err(internal)?)
        .map_err(|_| HostError::invalid_input("input does not match operation schema"))
}
fn allowed(d: &impl Directory, p: &Identity, space: &str) -> Result<(), HostError> {
    if d.capabilities(&p.id, space)
        .is_some_and(|c| c.contains(Capability::SpaceRead))
    {
        Ok(())
    } else {
        Err(HostError::not_found("input not found"))
    }
}
pub(crate) async fn dispatch<D: Directory>(
    state: Shared<D>,
    p: Identity,
    session: Session,
    op: &str,
    input: JsonObject,
) -> Result<Value, HostError> {
    validate_account_input(op, &input)?;
    if !session.is_live(&state.tokens).await {
        return Err(HostError::unauthorized("session expired or revoked"));
    }
    let st = state.clone();
    let identity = p.clone();
    let operation = op.to_owned();
    let prepared = tokio::task::spawn_blocking(move || prepare(st, identity, operation, input))
        .await
        .map_err(internal)??;
    if !matches!(op, "pending.submit" | "pending.confirm") {
        return Ok(prepared);
    }
    if prepared["record"].is_null() {
        return Ok(prepared);
    }
    let row: AccountInputRecord =
        serde_json::from_value(prepared["record"].clone()).map_err(internal)?;
    if row.state == "confirmation_required" {
        return Ok(prepared);
    }
    let request = row
        .request
        .clone()
        .ok_or_else(|| internal("missing request"))?;
    // The normal dispatcher resolves the real target, rechecks capabilities and
    // replays its durable receipt. Never hold an account transaction over dispatch.
    let result = Box::pin(crate::api::dispatch(
        state.clone(),
        p.clone(),
        session,
        &request.request_id,
        &request.operation,
        request.input,
    ))
    .await?;
    tokio::task::spawn_blocking(move||{
        let mut c=state.inputs.open(&p.id)?;
        let tx=c.transaction_with_behavior(TransactionBehavior::Immediate).map_err(internal)?;
        let draft:Option<String>=tx.query_row("SELECT draft_revision FROM inputs WHERE kind='pending' AND id=?1",[&row.id],|r|r.get(0)).unwrap_or(None);
        tx.execute("DELETE FROM inputs WHERE kind='pending' AND id=?1",[&row.id]).map_err(internal)?;
        if let Some(revision)=draft {tx.execute("DELETE FROM inputs WHERE kind='draft' AND item=?1 AND json_extract(body,'$.revision')=?2",params![row.item_id,revision]).map_err(internal)?;}
        state.inputs.change(&tx)?;tx.commit().map_err(internal)?;Ok(Inputs::result(None,Some(result)))
    }).await.map_err(internal)?
}

fn prepare<D: Directory>(
    st: Shared<D>,
    identity: Identity,
    operation: String,
    input: JsonObject,
) -> Result<Value, HostError> {
    let service = &st.inputs;
    let mut c = service.open(&identity.id)?;
    service.clean(&mut c, &st.tokens, &identity)?;
    let tx = c
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(internal)?;
    let kind = if operation.starts_with("drafts.") {
        "draft"
    } else {
        "pending"
    };
    let value = match operation.as_str() {
        "drafts.list" | "pending.list" => json!({"records":Inputs::list(&tx,kind)?}),
        "drafts.save" => {
            let i: DraftSaveInput = decode(&input)?;
            allowed(&st.tokens, &identity, &i.space_id)?;
            let prior = Inputs::get(&tx, kind, &i.item_id)?;
            let row = AccountInputRecord {
                id: i.item_id.clone(),
                space_id: i.space_id,
                item_id: i.item_id,
                revision: Uuid::now_v7().to_string(),
                created_at_ms: prior.map_or((service.clock)(), |r| r.created_at_ms),
                text: Some(i.text),
                request: None,
                state: "draft".into(),
            };
            service.put(&tx, kind, &row, None)?;
            Inputs::result(Some(row), None)
        }
        "drafts.get" | "drafts.delete" => {
            let i: DraftKey = decode(&input)?;
            allowed(&st.tokens, &identity, &i.space_id)?;
            let row = Inputs::get(&tx, kind, &i.item_id)?
                .filter(|r| r.space_id == i.space_id)
                .ok_or_else(|| HostError::not_found("input not found"))?;
            if operation == "drafts.delete" {
                tx.execute("DELETE FROM inputs WHERE kind='draft' AND id=?1", [&row.id])
                    .map_err(internal)?;
                service.change(&tx)?;
                Inputs::result(None, None)
            } else {
                Inputs::result(Some(row), None)
            }
        }
        "pending.queue" | "pending.edit" => {
            let i: PendingQueueInput = decode(&input)?;
            allowed(&st.tokens, &identity, &i.space_id)?;
            if i.created_at_ms > (service.clock)() {
                return Err(HostError::invalid_input("creation time is in the future"));
            }
            // A pending write may never hide a different space or item in its envelope.
            if i.request.input.get("space_id").and_then(Value::as_str) != Some(i.space_id.as_str())
                || i.request
                    .input
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|s| s != i.item_id)
            {
                return Err(HostError::invalid_input(
                    "pending target does not match operation",
                ));
            }
            let policy = st
                .registry
                .policy(&i.request.operation)
                .ok_or_else(|| HostError::not_found("unknown operation"))?;
            if policy.required != Capability::ContentWrite {
                return Err(HostError::invalid_input(
                    "only content writes may be queued",
                ));
            }
            policy.authorize_space(&st.tokens, &identity, &i.space_id)?;
            let prior = Inputs::get(&tx, kind, &i.request.request_id)?;
            if prior.is_none()
                && let Some(saved) =
                    service.receipt(&st.tokens, &identity, &i.request.request_id)?
            {
                let mut canonical = i.request.input.clone();
                if let Some(Value::String(space)) = canonical.get_mut("space_id") {
                    *space = st.tokens.content_space(space);
                }
                if saved.operation != i.request.operation
                    || saved.fingerprint
                        != crate::api::fingerprint(&i.request.operation, &canonical)
                {
                    return Err(HostError::conflict(
                        "request identity already used with different input",
                    ));
                }
                return Ok(Inputs::result(
                    None,
                    Some(crate::api::wire_spaces(&st.tokens, saved.result)),
                ));
            }
            if let Some(row) = &prior {
                if operation == "pending.queue" {
                    if row.request.as_ref() != Some(&i.request)
                        || row.space_id != i.space_id
                        || row.item_id != i.item_id
                    {
                        return Err(HostError::conflict(
                            "pending identity reused with different input",
                        ));
                    }
                    return Ok(Inputs::result(prior, None));
                }
                if row.state == "submitting"
                    || row.space_id != i.space_id
                    || row.item_id != i.item_id
                {
                    return Err(HostError::conflict(
                        "submission already attempted or target changed; retry the original request",
                    ));
                }
            } else if operation == "pending.edit" {
                return Err(HostError::not_found("input not found"));
            }
            let created = prior.as_ref().map_or(i.created_at_ms, |r| r.created_at_ms);
            let status = if (service.clock)().saturating_sub(created) > 7 * 24 * 60 * 60 * 1000 {
                "confirmation_required"
            } else {
                "queued"
            };
            let row = AccountInputRecord {
                id: i.request.request_id.clone(),
                space_id: i.space_id,
                item_id: i.item_id,
                revision: Uuid::now_v7().to_string(),
                created_at_ms: created,
                text: None,
                request: Some(i.request),
                state: status.into(),
            };
            let draft = Inputs::get(&tx, "draft", &row.item_id)?.map(|r| r.revision);
            service.put(&tx, kind, &row, draft.as_deref())?;
            Inputs::result(Some(row), None)
        }
        _ => {
            let i: PendingInput = decode(&input)?;
            let found = Inputs::get(&tx, kind, &i.id)?;
            if found.is_none()
                && matches!(operation.as_str(), "pending.submit" | "pending.confirm")
                && let Some(saved) = service.receipt(&st.tokens, &identity, &i.id)?
            {
                return Ok(Inputs::result(
                    None,
                    Some(crate::api::wire_spaces(&st.tokens, saved.result)),
                ));
            }
            let mut row = found.ok_or_else(|| HostError::not_found("input not found"))?;
            allowed(&st.tokens, &identity, &row.space_id)?;
            match operation.as_str() {
                "pending.cancel" => {
                    if row.state == "submitting" {
                        return Err(HostError::conflict(
                            "send outcome may be unknown; retry submission with the original request ID",
                        ));
                    }
                    tx.execute("DELETE FROM inputs WHERE kind='pending' AND id=?1", [i.id])
                        .map_err(internal)?;
                    service.change(&tx)?;
                    Inputs::result(None, None)
                }
                "pending.confirm" => {
                    if row.state != "confirmation_required" {
                        return Err(HostError::conflict(
                            "submission is not awaiting confirmation",
                        ));
                    }
                    // Confirm records permission to attempt this exact immutable envelope.
                    service.set_state(&tx, &mut row, "submitting")?;
                    Inputs::result(Some(row), None)
                }
                "pending.submit" => {
                    if row.state != "confirmation_required" {
                        service.set_state(&tx, &mut row, "submitting")?;
                    }
                    Inputs::result(Some(row), None)
                }
                _ => return Err(HostError::not_found("unknown operation")),
            }
        }
    };
    tx.commit().map_err(internal)?;
    Ok(value)
}
