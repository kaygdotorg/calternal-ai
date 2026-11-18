//! Stage observations for disposable benchmark runs. Feature-disabled builds
//! omit timestamps, formatting and logging from the writer.

#[cfg(feature = "perf-timings")]
mod note_save;
#[cfg(feature = "perf-timings")]
pub use note_save::{SaveStage, observe_note_save};

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
pub(crate) fn emit_commit(
    request_id: &str,
    at_ns: u64,
    #[cfg(feature = "perf-timings")] commit_ns: u64,
) {
    let marker = CommitMarker {
        stage: "journal_committed",
        request_id,
        release: !cfg!(debug_assertions),
        at_ns,
        #[cfg(feature = "perf-timings")]
        commit_ns,
    };
    if let Ok(json) = serde_json::to_string(&marker) {
        eprintln!("CALTERNAL_STAGE {json}");
    }
}

#[derive(serde::Serialize)]
struct CommitMarker<'a> {
    stage: &'static str,
    request_id: &'a str,
    release: bool,
    at_ns: u64,
    #[cfg(feature = "perf-timings")]
    commit_ns: u64,
}
