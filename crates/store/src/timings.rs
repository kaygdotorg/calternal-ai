//! Debug-only stage observations for disposable test-plugin runs. Absent from
//! every release build, including releases compiled with the test plugin feature.

/// Emit one JSON diagnostic correlated by request ID after a stage completes.
/// `elapsed_us` measures a stage duration; `at_ms` is the shared wall clock and
/// marks the instant after durable commit for delivery correlation. Writes one
/// stderr line; the test harness owns bounded log retention. Never log payloads.
pub fn emit(stage: &str, request_id: &str, elapsed_us: Option<u128>) {
    let at_ms = calternal_types::now_ms();
    eprintln!(
        "CALTERNAL_STAGE {}",
        serde_json::json!({
            "stage":stage,"request_id":request_id,"at_ms":at_ms,"elapsed_us":elapsed_us
        })
    );
}
