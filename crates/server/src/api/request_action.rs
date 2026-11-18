//! Consume once authority before dispatch and replay completed responses.
use super::{cursors::admitted_operation, *};
use crate::authz::Policy;
use crate::turns::access::Authority;

use crate::plugins::OnceAdmission;

/// Serialize concurrent attempts on a running turn through the existing live job
/// handle's async mutex. The mutex spans the action and receipt so identical
/// calls observe the first result. A spent approval only reopens its receipt.
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
    let account = p.id;
    let (admission, saved, receipt_key, fp) = tokio::task::spawn_blocking(move || {
        if st.jobs.agent_current(&a2.space, &a2.job, &h)?.is_none() {
            return Err(Error::forbidden("thread credential has ended"));
        }
        let fp = fingerprint(&op2, &input2);
        // Longer than a public request ID. The journal already scopes receipts
        // to the account and space, and this key adds the thread.
        let receipt_key = format!(
            "once/{}/{}",
            sha256_hex(a2.thread()?.as_bytes()),
            sha256_hex(rid2.as_bytes()),
        );
        let saved = replay_receipt(&st.store, account, space, &receipt_key, &op2, &fp)?;
        let admission = st.jobs.claim_once(
            &a2,
            &op2,
            &input2,
            &rid2,
            risk,
            mutates,
            h,
            fp.clone(),
            saved.is_some(),
        )?;
        Ok::<_, Error>((admission, saved, receipt_key, fp))
    })
    .await
    .map_err(|_| Error::internal("approval claim task failed"))??;
    match admission {
        OnceAdmission::Unapproved => return Ok(None),
        OnceAdmission::Replay => return Ok(saved),
        OnceAdmission::Fresh => (),
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
    let (op, saved) = (op.to_owned(), result.clone());
    blocking(state, move |store| {
        store.content().record_receipt(
            space,
            calternal_content::Request {
                account,
                id: &receipt_key,
                operation: &op,
                fingerprint: &fp,
            },
            saved,
        )
    })
    .await?;
    Ok(Some(result))
}
