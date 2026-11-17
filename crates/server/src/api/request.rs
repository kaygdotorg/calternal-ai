//! Request identity hashing uses the existing blocking pool, never the executor.
use calternal_core::JsonObject;
use calternal_types::Error;

/// Compute the shared canonical digest and return the same parsed input, without
/// cloning its body. Join failure returns a caller-safe internal error before any
/// mutation. Cost: one operation-name copy, canonical encoding/hash proportional
/// to the bounded input, and one awaited task on the existing blocking pool.
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
