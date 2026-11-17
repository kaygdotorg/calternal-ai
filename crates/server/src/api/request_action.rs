//! Dispatch a claimed once grant through a receipt-capable operation.
use super::{dispatch::admitted_operation, *};
use crate::authz::Policy;
use crate::turns::access::Authority;

/// Whether this preview can replay a claimed action after interruption.
pub(crate) fn supports_once(op: &str, mutates: bool) -> bool {
    !mutates
        || matches!(
            op,
            "notes.create"
                | "notes.update"
                | "spaces.set_role"
                | "spaces.remove_member"
                | "spaces.leave"
                | "spaces.transfer"
                | "spaces.delete"
                | "plugins.enable"
                | "threads.create"
                | "threads.send"
                | "threads.stop"
                | "threads.archive"
                | "threads.delete"
        )
}

/// Serialize concurrent attempts on a running turn through the existing live job
/// handle's async mutex. The mutex spans the action and receipt so identical
/// calls observe the first result. Durable writers keep their existing intents.
#[allow(clippy::too_many_arguments)]
pub(super) async fn once<D: Directory>(
    state: &Shared<D>,
    p: &Identity,
    session: &Session,
    a: &Authority,
    token_hash: &[u8],
    rid: &str,
    op: &str,
    input: &JsonObject,
    policy: &Policy,
    mutates: bool,
    risk: calternal_core::OperationRisk,
    job_scoped: bool,
) -> Result<Option<Value>, Error> {
    if !a.can_request_action(op, input) {
        return Ok(None);
    }
    if !supports_once(op, mutates) {
        return Ok(None);
    }
    let live = state.jobs.live(&a.job);
    let _serial = live.once_action.lock().await;
    let (st, a2, h, op2, input2, rid2) = (
        state.clone(),
        a.clone(),
        token_hash.to_vec(),
        op.to_owned(),
        input.clone(),
        rid.to_owned(),
    );
    let space = space_of(&a.space)?;
    let fp = fingerprint(op, input);
    let (fp2, account) = (fp.clone(), p.id);
    let claim = tokio::task::spawn_blocking(move || {
        if st.jobs.agent_current(&a2.space, &a2.job, &h)?.is_none() {
            return Err(Error::forbidden("thread credential has ended"));
        }
        if !st
            .jobs
            .claim_once(&a2, &op2, &input2, &rid2, risk, mutates, h)?
        {
            return Ok(Err(()));
        }
        let saved = replay_receipt(&st.store, account, space, &rid2, &op2, &fp2)?;
        Ok::<_, Error>(Ok(saved))
    })
    .await
    .map_err(|_| Error::internal("approval claim task failed"))??;
    match claim {
        Err(()) => return Ok(None),
        Ok(Some(saved)) => return Ok(Some(saved)),
        Ok(None) => (),
    }
    let result = admitted_operation(
        state.clone(),
        p.clone(),
        session.clone(),
        rid,
        op,
        input.clone(),
        Some(a.clone()),
        policy.clone(),
        job_scoped,
    )
    .await?;
    if !mutates {
        let (rid, op, saved) = (rid.to_owned(), op.to_owned(), result.clone());
        blocking(state, move |store| {
            store.content().record_receipt(
                space,
                calternal_content::Request {
                    account,
                    id: &rid,
                    operation: &op,
                    fingerprint: &fp,
                },
                saved,
            )
        })
        .await?;
    }
    Ok(Some(result))
}
