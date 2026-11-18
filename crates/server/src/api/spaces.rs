//! Stored space-directory operations and their shared content request receipts.
use super::dispatch::{fingerprint, space_of};
use super::*;

pub(super) async fn dispatch<D: Directory>(
    state: Shared<D>,
    p: Identity,
    actor: calternal_types::Actor,
    request_id: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let lifecycle = state.store.content().lifecycle();
    let space = input
        .get("space_id")
        .and_then(Value::as_str)
        .map(space_of)
        .transpose()?;
    lifecycle.check(actor.account.as_uuid())?;
    if let Some(space) = space {
        lifecycle.check(space.as_uuid())?;
    }
    let rid = request_id.to_owned();
    let operation = op.to_owned();
    crate::blocking::spawn(move || {
        let st = state;
        // Accounts this change removes lose their private input for the space.
        let removed = crate::account_input::revoked_targets(&st.directory, &p, &operation, &input)?;
        if operation == "spaces.delete" {
            let space = space.ok_or_else(|| Error::invalid_input("space ID required"))?;
            return crate::erasure::space(&st, actor, space, &rid, &input, removed);
        }
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
                        account: actor.account,
                        via: actor.via.as_ref(),
                        id: &rid,
                        operation: &operation,
                        fingerprint: &fp,
                    },
                    change,
                )
            }
            // Reads, and account-level writes (spaces.create), whose receipt
            // scope is the acting account.
            None if operation == "spaces.create" => {
                let _step = lifecycle.step(actor.account.as_uuid())?;
                change()
            }
            _ => change(),
        };
        if result.is_ok()
            && let Some(space) = space
        {
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
    .map_err(|_| Error::internal("directory task failed"))?
}
