//! Request identity hashing uses the existing blocking pool, never the executor.
use calternal_core::JsonObject;
use calternal_types::Error;

/// Compute the shared digest and return the original parsed input by move.
/// Handoff adds no body clone; the canonical encoder retains its own allocations.
/// Join failure returns a caller-safe error before mutation. Cost: one operation
/// copy, encoding/hash proportional to bounded input, and one blocking-pool task.
pub(super) async fn fingerprint(
    operation: &str,
    input: JsonObject,
) -> Result<(String, JsonObject), Error> {
    let operation = operation.to_owned();
    tokio::task::spawn_blocking(move || {
        let digest = super::fingerprint(&operation, &input);
        (digest, input)
    })
    .await
    .map_err(|_| Error::internal("request fingerprint task failed"))
}
