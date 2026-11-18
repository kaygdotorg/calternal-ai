//! Stage observations for disposable benchmark runs. Feature-disabled builds
//! omit timestamps, formatting and logging from the writer.

/// Emit one JSON diagnostic correlated by request ID after a stage completes.
/// `elapsed_us` measures a stage duration. Writes one stderr line with the
/// shared wall clock for debug diagnostics. The harness owns log retention.
#[cfg(debug_assertions)]
pub fn emit(stage: &str, request_id: &str, elapsed_us: Option<u128>) {
    let at_ms = calternal_types::now_ms();
    eprintln!(
        "CALTERNAL_STAGE {}",
        serde_json::json!({
            "stage":stage,"request_id":request_id,"at_ms":at_ms,"elapsed_us":elapsed_us
        })
    );
}

/// Emit a post-commit marker on the journal writer after broadcast. The supplied
/// timestamp was captured at successful SQLite commit; logging adds one JSON
/// line per correlated entry on the writer, with no payloads.
pub(crate) fn emit_commit(request_id: &str, at_ns: u64) {
    eprintln!(
        "CALTERNAL_STAGE {}",
        serde_json::json!({
            "stage": "journal_committed",
            "request_id": request_id,
            "at_ns": at_ns,
            "release": !cfg!(debug_assertions),
        })
    );
}
