//! Per-save timing collected only on the foreground blocking callback.
use calternal_types::perf_monotonic_ns as now;
use std::cell::RefCell;

thread_local! {
    static TIMING: RefCell<Option<Durations>> = const { RefCell::new(None) };
}

/// The measured storage boundaries of a Notes update. Stages can occur more
/// than once when the callback acquires the slot for several operations.
#[derive(Clone, Copy)]
pub enum SaveStage {
    /// Wait to acquire the per-space content mutex.
    SlotWait,
    /// Validate the existing file, including any synchronous pending placement.
    FileGuard,
    /// Choose the filename, including directory cache refresh.
    Filename,
    /// Commit the content and outbox intent transaction.
    ContentCommit,
    /// Check the suffix witness and append one Markdown record.
    RecordWrite,
    /// Sync the leased file before publishing the prepared thread records.
    RecordSync,
    /// Commit the saved-turn job and its admission receipt.
    JobCommit,
    /// Journal receipt lookup, writer handoff, transaction and reply.
    Journal,
    /// Commit removal of a published outbox row.
    Retirement,
}

#[derive(Default, serde::Serialize)]
struct Durations {
    slot_wait_ns: u64,
    file_guard_ns: u64,
    filename_ns: u64,
    content_commit_ns: u64,
    record_write_ns: u64,
    record_sync_ns: u64,
    job_commit_ns: u64,
    journal_ns: u64,
    retirement_ns: u64,
    commits: u32,
}

/// Captures one active span. Outside a save only the local scope is checked.
pub struct SaveSpan(SaveStage, Option<u64>);

impl SaveStage {
    /// Capture the monotonic start only in a scoped Notes save callback.
    pub fn start(self) -> SaveSpan {
        SaveSpan(
            self,
            TIMING.with(|timing| timing.borrow().as_ref().map(|_| now())),
        )
    }
}

impl SaveSpan {
    /// Record a completed span without allocating, blocking or logging.
    pub fn finish(self) {
        let Some(started) = self.1 else { return };
        let elapsed = now() - started;
        TIMING.with(|timing| {
            let mut timing = timing.borrow_mut();
            let Some(d) = timing.as_mut() else { return };
            match self.0 {
                SaveStage::SlotWait => d.slot_wait_ns += elapsed,
                SaveStage::FileGuard => d.file_guard_ns += elapsed,
                SaveStage::Filename => d.filename_ns += elapsed,
                SaveStage::ContentCommit => {
                    d.content_commit_ns += elapsed;
                    d.commits += 1;
                }
                SaveStage::RecordWrite => d.record_write_ns += elapsed,
                SaveStage::RecordSync => d.record_sync_ns += elapsed,
                SaveStage::JobCommit => d.job_commit_ns += elapsed,
                SaveStage::Journal => d.journal_ns += elapsed,
                SaveStage::Retirement => d.retirement_ns += elapsed,
            }
        });
    }
}

struct Scope(Option<Durations>);
impl Drop for Scope {
    fn drop(&mut self) {
        TIMING.with(|t| *t.borrow_mut() = self.0.take());
    }
}

/// Measure the sync callback and emit one nanosecond record after it releases
/// content locks. Replays and failed writes before content commit have no record.
/// This fixture-only path allocates once for JSON and adds no queue or worker.
pub fn observe_note_save<T>(
    request_id: &str,
    admitted_from: u64,
    queued_from: u64,
    save: impl FnOnce() -> T,
) -> T {
    let started = now();
    let scoped = Scope(TIMING.with(|t| t.replace(Some(Durations::default()))));
    let result = save();
    let end = now();
    let durations = TIMING.with(|t| t.take());
    drop(scoped);
    if let Some(durations) = durations.filter(|d| d.commits > 0) {
        #[derive(serde::Serialize)]
        struct Record<'a> {
            stage: &'static str,
            request_id: &'a str,
            admission_wait_ns: u64,
            blocking_wait_ns: u64,
            callback_ns: u64,
            #[serde(flatten)]
            durations: Durations,
        }
        let record = Record {
            stage: "notes_save",
            request_id,
            admission_wait_ns: started - admitted_from,
            blocking_wait_ns: started - queued_from,
            callback_ns: end - started,
            durations,
        };
        if let Ok(json) = serde_json::to_string(&record) {
            eprintln!("CALTERNAL_STAGE {json}");
        }
    }
    result
}

/// Measure one foreground thread callback using the existing per-worker spans.
/// Callers emit once outside the content or jobs lock.
pub fn observe_thread_storage<T>(request_id: &str, stage: &'static str, run: impl FnOnce() -> T) -> T {
    let start_ns = now();
    let scoped = Scope(TIMING.with(|t| t.replace(Some(Durations::default()))));
    let value = run();
    let end_ns = now();
    let durations = TIMING.with(|t| t.take()).expect("installed above");
    drop(scoped);
    #[derive(serde::Serialize)]
    struct Record<'a> {
        stage: &'static str,
        request_id: &'a str,
        start_ns: u64,
        end_ns: u64,
        #[serde(flatten)]
        durations: Durations,
    }
    if let Ok(json) = serde_json::to_string(&Record{stage,request_id,start_ns,end_ns,durations}) {
        eprintln!("CALTERNAL_STAGE {json}");
    }
    value
}
