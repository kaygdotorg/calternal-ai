//! Stored space-directory operations and their shared content request receipts.
use super::dispatch::{fingerprint, space_of};
use super::*;

pub(super) async fn dispatch<D: Directory>(
    state: Shared<D>,
    p: Identity,
    request_id: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let st = state.clone();
    let rid = request_id.to_owned();
    let operation = op.to_owned();
    crate::blocking::spawn(move || {
        // Accounts this change removes lose their private input for the space.
        let removed = crate::account_input::revoked_targets(&st.directory, &p, &operation, &input)?;
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
    .map_err(|_| Error::internal("directory task failed"))?
}
