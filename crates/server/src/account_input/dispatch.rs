//! Account-derived operation admission and pending submission state machine.
use super::{Inputs, internal};
use crate::{
    api::Shared,
    auth::{Directory, Identity},
    plugins::Session,
};
use calternal_core::*;
use calternal_types::Error;
use calternal_types::{Capability, SpaceId};
use rusqlite::{TransactionBehavior, params};
use serde_json::{Value, json};
use uuid::Uuid;

/// The caller may read `space` (a validated UUID text). Returns the typed ID.
/// Cost: one capability lookup.
fn allowed(d: &impl Directory, p: &Identity, space: &str) -> Result<SpaceId, Error> {
    let hidden = || Error::not_found("input not found");
    let space = SpaceId::parse(space).map_err(|_| hidden())?;
    if d.capabilities(p.id, space)
        .is_some_and(|c| c.contains(Capability::SpaceRead))
    {
        Ok(space)
    } else {
        Err(hidden())
    }
}
pub(crate) async fn dispatch<D: Directory>(
    state: Shared<D>,
    p: Identity,
    session: Session,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    // The account-input operation's own input is parsed once, here. A submitted
    // pending envelope is then dispatched (and validated) as the ordinary
    // operation it carries, and its stored record is decoded from the database.
    let parsed = parse_account_input(op, input)?;
    if !session.is_live(&state.directory).await {
        return Err(Error::unauthorized("session expired or revoked"));
    }
    state.inputs.check_owner(p.id)?;
    let st = state.clone();
    let operation = op.to_owned();
    let prepared = crate::blocking::spawn(move || prepare(st, p, operation, parsed))
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
        p,
        session,
        &request.request_id,
        &request.operation,
        request.input,
    ))
    .await?;
    state.inputs.check_owner(p.id)?;
    crate::blocking::spawn(move || {
        let _step = state.inputs.step(p.id)?;
        let mut c = state.inputs.open(p.id)?;
        let before = c.total_changes();
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(internal)?;
        let draft: Option<String> = tx
            .query_row(
                "SELECT draft_revision FROM inputs WHERE kind='pending' AND space=?1 AND id=?2",
                [&row.space_id, &row.id],
                |r| r.get(0),
            )
            .unwrap_or(None);
        // A retry later finds the result in the space's own receipt.
        tx.execute(
            "DELETE FROM inputs WHERE kind='pending' AND space=?1 AND id=?2",
            [&row.space_id, &row.id],
        )
        .map_err(internal)?;
        if let Some(revision) = draft {
            tx.execute(
                "DELETE FROM inputs WHERE kind='draft' AND space=?1 AND id=?2 AND json_extract(body,'$.revision')=?3",
                params![row.space_id, row.item_id, revision],
            )
            .map_err(internal)?;
        }
        state.inputs.change(&tx)?;
        let changed = tx.total_changes() > before;
        tx.commit().map_err(internal)?;
        if changed { state.inputs.wakes.committed(p.id); }
        Ok(Inputs::result(None, Some(result)))
    }).await.map_err(internal)?
}

fn prepare<D: Directory>(
    st: Shared<D>,
    identity: Identity,
    operation: String,
    input: AccountInput,
) -> Result<Value, Error> {
    let service = &st.inputs;
    let _step = service.step(identity.id)?;
    let mut c = service.open(identity.id)?;
    service.clean(&mut c, &st.directory, &identity)?;
    let before = c.total_changes();
    let tx = c
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(internal)?;
    let kind = if operation.starts_with("drafts.") {
        "draft"
    } else {
        "pending"
    };
    let value = match input {
        AccountInput::List => json!({"records":Inputs::list(&tx,kind)?}),
        AccountInput::Save(i) => {
            allowed(&st.directory, &identity, &i.space_id)?;
            // A draft's space is fixed when first saved: one ownership fact.
            if Inputs::draft_space(&tx, &i.item_id)?.is_some_and(|s| s != i.space_id) {
                return Err(Error::conflict(
                    "this item already has a draft in another space; delete that draft first",
                ));
            }
            let prior = Inputs::get(&tx, kind, &i.space_id, &i.item_id)?;
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
        AccountInput::Draft(i) => {
            allowed(&st.directory, &identity, &i.space_id)?;
            let row = Inputs::get(&tx, kind, &i.space_id, &i.item_id)?
                .ok_or_else(|| Error::not_found("input not found"))?;
            if operation == "drafts.delete" {
                tx.execute(
                    "DELETE FROM inputs WHERE kind='draft' AND space=?1 AND id=?2",
                    [&row.space_id, &row.id],
                )
                .map_err(internal)?;
                service.change(&tx)?;
                Inputs::result(None, None)
            } else {
                Inputs::result(Some(row), None)
            }
        }
        AccountInput::Queue(i) => {
            let space = allowed(&st.directory, &identity, &i.space_id)?;
            if i.created_at_ms > (service.clock)() {
                return Err(Error::invalid_input("creation time is in the future"));
            }
            // A pending write may never hide a different space or item in its envelope.
            if i.request.input.get("space_id").and_then(Value::as_str) != Some(i.space_id.as_str())
                || i.request
                    .input
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|s| s != i.item_id)
            {
                return Err(Error::invalid_input(
                    "pending target does not match operation",
                ));
            }
            let policy = st
                .registry
                .policy(&i.request.operation)
                .ok_or_else(|| Error::not_found("unknown operation"))?;
            if policy.required != Capability::ContentWrite {
                return Err(Error::invalid_input("only content writes may be queued"));
            }
            policy.authorize_space(&st.directory, &identity, space)?;
            let prior = Inputs::get(&tx, kind, &i.space_id, &i.request.request_id)?;
            if prior.is_none()
                && let Some(saved) =
                    service.receipt(&st.directory, &identity, space, &i.request.request_id)?
            {
                if saved.operation != i.request.operation
                    || saved.fingerprint
                        != crate::api::fingerprint(&i.request.operation, &i.request.input)
                {
                    return Err(Error::request_reused());
                }
                return Ok(Inputs::result(None, Some(saved.result)));
            }
            if let Some(row) = &prior {
                if operation == "pending.queue" {
                    if row.request.as_ref() != Some(&i.request)
                        || row.space_id != i.space_id
                        || row.item_id != i.item_id
                    {
                        return Err(Error::conflict(
                            "pending identity reused with different input",
                        ));
                    }
                    return Ok(Inputs::result(prior, None));
                }
                if row.state == "submitting"
                    || row.space_id != i.space_id
                    || row.item_id != i.item_id
                {
                    return Err(Error::conflict(
                        "submission already attempted or target changed; retry the original request",
                    ));
                }
            } else if operation == "pending.edit" {
                return Err(Error::not_found("input not found"));
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
            let draft = Inputs::get(&tx, "draft", &row.space_id, &row.item_id)?.map(|r| r.revision);
            service.put(&tx, kind, &row, draft.as_deref())?;
            Inputs::result(Some(row), None)
        }
        AccountInput::Pending(i) => {
            let space = allowed(&st.directory, &identity, &i.space_id)?;
            let found = Inputs::get(&tx, kind, &i.space_id, &i.id)?;
            // Already sent: the reply is the space's receipt for this request ID.
            if found.is_none()
                && matches!(operation.as_str(), "pending.submit" | "pending.confirm")
                && let Some(saved) = service.receipt(&st.directory, &identity, space, &i.id)?
            {
                return Ok(Inputs::result(None, Some(saved.result)));
            }
            let mut row = found.ok_or_else(|| Error::not_found("input not found"))?;
            match operation.as_str() {
                "pending.cancel" => {
                    if row.state == "submitting" {
                        return Err(Error::conflict(
                            "send outcome may be unknown; retry submission with the original request ID",
                        ));
                    }
                    tx.execute(
                        "DELETE FROM inputs WHERE kind='pending' AND space=?1 AND id=?2",
                        [&row.space_id, &row.id],
                    )
                    .map_err(internal)?;
                    service.change(&tx)?;
                    Inputs::result(None, None)
                }
                "pending.confirm" => {
                    if row.state != "confirmation_required" {
                        return Err(Error::conflict("submission is not awaiting confirmation"));
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
                _ => return Err(Error::not_found("unknown operation")),
            }
        }
    };
    let changed = tx.total_changes() > before;
    tx.commit().map_err(internal)?;
    if changed {
        service.wakes.committed(identity.id);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::{AppState, allowed_hosts},
        auth::StoredDirectory,
        blocking::{RequestWorkers, spawn},
        plugins::{Jobs, Registry, WorkerPool},
        store::Store,
    };
    use std::{sync::Arc, time::Duration};

    #[tokio::test]
    async fn a_closed_account_is_refused_when_request_readers_are_busy() {
        let root = tempfile::tempdir().unwrap();
        let roots = calternal_roots::Roots {
            appdata: root.path().join("appdata"),
            userdata_ssd: root.path().join("ssd"),
            userdata_hdd: root.path().join("hdd"),
        };
        let store = Store::open(
            &roots,
            Uuid::from_u128(1),
            true,
            calternal_content::DEFAULT_RECEIPT_RETENTION,
        )
        .unwrap();
        let directory = StoredDirectory::load_or_create(&roots.appdata).unwrap();
        let person = Identity {
            id: calternal_types::AccountId::new(),
        };
        store.content().lifecycle().close(person.id.as_uuid());
        let state = Arc::new(AppState {
            blocking: RequestWorkers::new().unwrap(),
            inputs: store.account_inputs(),
            store,
            directory,
            registry: Registry::empty(),
            workers: WorkerPool::new(None),
            jobs: Jobs::disabled(),
            allowed_hosts: allowed_hosts(0),
        });
        let mut readers = Vec::new();
        for _ in 0..2 {
            let pool = state.blocking.clone();
            let (entered, ready) = tokio::sync::oneshot::channel();
            let (release, blocked) = std::sync::mpsc::channel();
            let reader = tokio::spawn(async move {
                pool.scope(
                    false,
                    spawn(move || {
                        entered.send(()).unwrap();
                        blocked.recv().unwrap();
                    }),
                )
                .await
                .unwrap();
            });
            ready.await.unwrap();
            readers.push((release, reader));
        }
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            crate::api::dispatch(
                state,
                person,
                Session::Static,
                "closed-account",
                "drafts.list",
                JsonObject::new(),
            ),
        )
        .await;
        for (release, reader) in readers {
            release.send(()).unwrap();
            reader.await.unwrap();
        }
        let refusal = result
            .expect("owner refusal waited behind the read pool")
            .unwrap_err();
        assert_eq!(refusal.code, calternal_types::ErrorCode::NotFound);
    }
}
