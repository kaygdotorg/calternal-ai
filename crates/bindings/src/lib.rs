#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
//! Async native operation boundary plus optional contract validation helpers.
//! Credentials are supplied by trusted platform sign-in/secret storage and are never
//! formatted in diagnostics. Explicit cancellation stops local waiting, not server
//! effects. Retain NativeRequest after an uncertain mutation for safe retry.
//!
//! No editor input, composition, caret, layout, animation or per-frame work crosses
//! this boundary. Invoke background operations with coalesced edits and batched results;
//! JSON helpers copy payloads and must stay off native UI hot paths.
use calternal_core::*;
use serde_json::json;
#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();

/// Generate the complete operation/worker JSON Schema as a UTF-8 JSON string. Allocates the schema tree and serialized output on every call.
///
/// # Panics
/// Panics only if serialization of the generated JSON Schema unexpectedly fails.
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn contract_schema_json() -> String {
    serde_json::to_string(&schemars::schema_for!(Contract)).expect("schema serialization")
}
/// Validate a JSON operation envelope synchronously without dispatch or authorization. Returns JSON with `valid` and either `request_id` or a safe `error`. Unknown operation schemas remain server-validated. Allocates O(input bytes); malformed input returns `valid: false`.
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn validate_request_json(request_json: String) -> String {
    let result = serde_json::from_str::<OperationRequest>(&request_json)
        .map_err(|_| ValidationError::invalid("malformed operation envelope"))
        .and_then(|r| {
            validate_client_request(&r)?;
            Ok(r)
        });
    match result {
        Ok(request) => json!({"valid":true,"request_id":request.request_id}).to_string(),
        Err(error) => {
            json!({"valid":false,"error":{"code":error.code,"message":error.message}}).to_string()
        }
    }
}
/// Validate a JSON portable view synchronously without rendering or dispatch. Returns JSON with `valid` and a safe `error` on failure. Enforces tree budgets after decoding; allocates O(input bytes).
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn validate_genui_json(view_json: String) -> String {
    let result = serde_json::from_str::<GenUiView>(&view_json)
        .map_err(|_| ValidationError::invalid("malformed GenUI tree"))
        .and_then(|view| validate_genui(&view));
    match result {
        Ok(()) => json!({"valid":true}).to_string(),
        Err(error) => {
            json!({"valid":false,"error":{"code":error.code,"message":error.message}}).to_string()
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_bad_json_without_panicking() {
        assert!(validate_request_json("[]".into()).contains("\"valid\":false"));
        assert!(validate_genui_json("null".into()).contains("\"valid\":false"));
    }
    #[test]
    fn exports_real_schema() {
        assert!(contract_schema_json().contains("WorkerRequest"));
    }
}

mod native;
pub use native::*;

mod live;
pub use live::NativeLive;
