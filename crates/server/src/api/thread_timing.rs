//! Feature-gated request phase markers for the locked append benchmark.

/// Request identifier retained through the final HTTP credential check.
#[derive(Clone)]
pub(crate) struct Reply(pub(crate) String);

/// Record a monotonic phase boundary without copying the response body.
pub(crate) fn mark(id: &str, name: &'static str) {
    at(id, name, calternal_types::perf_monotonic_ns());
}

/// Record a phase boundary captured earlier on the same server clock.
pub(crate) fn at(id: &str, name: &'static str, at_ns: u64) {
    eprintln!(
        "CALTERNAL_STAGE {}",
        serde_json::json!({"stage":name,"request_id":id,"at_ns":at_ns})
    );
}

/// Record serialization completion and the wire body length.
pub(crate) fn serialized(id: &str, bytes: Option<u64>) {
    let at_ns = calternal_types::perf_monotonic_ns();
    eprintln!(
        "CALTERNAL_STAGE {}",
        serde_json::json!({
            "stage":"http_serialized","request_id":id,"at_ns":at_ns,"bytes":bytes
        })
    );
}
