//! Recovery of a plugin operation from its committed host effect.
use super::*;

/// Recover an acknowledgment from the first write's own atomic outbox receipt.
/// The outer digest includes the operation and its input. A changed request
/// cannot claim the effect, and recovery cannot run the worker a second time.
/// Cost: one space-local receipt lookup (plus existing outbox recovery if pending).
pub(super) fn committed_write(
    store: &Store,
    account: AccountId,
    space: SpaceId,
    request_id: &str,
    fingerprint: &str,
) -> Result<Option<Value>, Error> {
    let key = crate::plugins::broker::write_receipt_key(request_id, 0);
    let Some(receipt) = store.receipt(account, space, &key)? else {
        return Ok(None);
    };
    if receipt.operation != "plugin.write_item"
        || receipt
            .fingerprint
            .split_once(':')
            .map(|(parent, _)| parent)
            != Some(fingerprint)
    {
        return Err(Error::request_reused());
    }
    Ok(Some(serde_json::json!({"committed_write": receipt.result})))
}
