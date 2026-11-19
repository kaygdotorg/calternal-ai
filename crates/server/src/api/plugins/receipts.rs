//! Recovery of a plugin operation from its committed host effect.
use super::*;

/// Recover an acknowledgment from the first write's own atomic outbox receipt.
/// The outer digest includes the operation and its input. A changed request
/// cannot claim the effect, and recovery cannot run the worker a second time.
/// Cost: one space-local receipt lookup (plus existing outbox recovery if pending).
fn committed_write(
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

/// Replay a package-state effect using the outer fingerprint and its validated
/// host acknowledgment. Cost: one space-local receipt read.
fn committed_state(
    store: &Store,
    package: &str,
    account: AccountId,
    space: SpaceId,
    request: &str,
    fingerprint: &str,
) -> Result<Option<Value>, Error> {
    let key = crate::plugins::broker::state_receipt_key(account, request);
    Ok(store
        .content()
        .plugin_state_receipt(space, package, &key, fingerprint)?
        .map(|revision| serde_json::json!({"revision":revision})))
}

/// Recover an existing package's committed effect without re-running its worker.
/// Ordinary content-only packages retain their single receipt lookup.
pub(super) fn committed_effect(
    store: &Store,
    plugin: &Plugin,
    account: AccountId,
    space: SpaceId,
    request: &str,
    fingerprint: &str,
) -> Result<Option<Value>, Error> {
    if plugin
        .manifest
        .host_calls
        .contains(&calternal_core::HostCallKind::CasPluginStateV1)
        && let Some(ack) = committed_state(
            store,
            &plugin.manifest.id,
            account,
            space,
            request,
            fingerprint,
        )?
    {
        return Ok(Some(ack));
    }
    committed_write(store, account, space, request, fingerprint)
}
